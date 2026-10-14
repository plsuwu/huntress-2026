//! A small backtracking regex engine with Python `re` semantics, so the reference solvers'
//! patterns can be carried over verbatim (std-only; the `regex` crate has no backreferences or
//! lookaround anyway).
//!
//! Supported: literals and escapes (`\d \w \s \D \W \S \b \B \A \Z \n \t \r \xhh \\` ...), `.`,
//! classes `[...]` / `[^...]` with ranges and class escapes, groups `( )`, `(?: )`, `(?P<name> )`,
//! alternation, quantifiers `* + ? {m} {m,} {m,n}` (greedy and lazy), backreferences `\1`..`\9`
//! and `(?P=name)`, lookahead `(?= )` `(?! )`, fixed-width lookbehind `(?<= )` `(?<! )`, inline
//! flags `(?i)` `(?s)` `(?m)` `(?x)` at the start, `^`/`$` (MULTILINE-aware; `$` also before a final
//! newline). Matching works on chars; reported offsets are byte offsets into the subject.

use std::collections::HashMap;

#[derive(Clone, Copy, Default, Debug)]
pub struct Flags {
    pub i: bool,
    pub s: bool,
    pub m: bool,
    pub x: bool,
}

#[derive(Debug, Clone)]
enum ClassItem {
    Range(char, char),
    Digit(bool),
    Word(bool),
    Space(bool),
}

#[derive(Debug, Clone)]
enum Node {
    Char(char),
    Any,
    Class(Vec<ClassItem>, bool),
    Start,
    End,
    StrStart,
    StrEnd,
    WordB(bool),
    Group(Option<usize>, Box<Node>),
    Alt(Vec<Node>),
    Seq(Vec<Node>),
    Repeat(Box<Node>, usize, usize, bool),
    Backref(usize),
    Look(Box<Node>, bool, bool, usize), // (node, ahead, negate, width for lookbehind)
}

pub struct Regex {
    root: Node,
    ngroups: usize,
    names: HashMap<String, usize>,
    flags: Flags,
    /// Single-char nodes one of which must match at a match's first position (None = unknown).
    first: Option<Vec<Node>>,
}

/// The single-char nodes that can start a match of `n`, or None when `n` can match empty / starts
/// with an assertion we do not analyse.
fn first_set(n: &Node) -> Option<Vec<Node>> {
    match n {
        Node::Char(_) | Node::Any | Node::Class(..) => Some(vec![n.clone()]),
        Node::Group(_, inner) => first_set(inner),
        Node::Alt(alts) => {
            let mut out = Vec::new();
            for a in alts {
                out.extend(first_set(a)?);
            }
            Some(out)
        }
        Node::Seq(items) => {
            // Skip zero-width word boundaries/lookarounds? Keep it simple: the first item decides.
            items.first().and_then(first_set)
        }
        Node::Repeat(inner, min, _, _) if *min >= 1 => first_set(inner),
        _ => None,
    }
}

struct Parser<'a> {
    p: Vec<char>,
    i: usize,
    ngroups: usize,
    names: HashMap<String, usize>,
    flags: Flags,
    src: &'a str,
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl Parser<'_> {
    fn err(&self, m: &str) -> String {
        format!("regex {:?}: {m} at {}", self.src, self.i)
    }
    fn peek(&self) -> Option<char> {
        self.p.get(self.i).copied()
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn skip_x(&mut self) {
        if !self.flags.x {
            return;
        }
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => self.i += 1,
                Some('#') => {
                    while let Some(c) = self.peek() {
                        self.i += 1;
                        if c == '\n' {
                            break;
                        }
                    }
                }
                _ => return,
            }
        }
    }

    fn alt(&mut self) -> Result<Node, String> {
        let mut alts = vec![self.seq()?];
        while self.eat('|') {
            alts.push(self.seq()?);
        }
        Ok(if alts.len() == 1 { alts.pop().unwrap() } else { Node::Alt(alts) })
    }

    fn seq(&mut self) -> Result<Node, String> {
        let mut items = Vec::new();
        loop {
            self.skip_x();
            match self.peek() {
                None | Some('|') | Some(')') => break,
                _ => {}
            }
            let atom = self.atom()?;
            self.skip_x();
            let atom = self.quant(atom)?;
            items.push(atom);
        }
        Ok(Node::Seq(items))
    }

    fn quant(&mut self, atom: Node) -> Result<Node, String> {
        let (min, max) = match self.peek() {
            Some('*') => {
                self.i += 1;
                (0, usize::MAX)
            }
            Some('+') => {
                self.i += 1;
                (1, usize::MAX)
            }
            Some('?') => {
                self.i += 1;
                (0, 1)
            }
            Some('{') => {
                let save = self.i;
                self.i += 1;
                let num = |s: &mut Self| -> Option<usize> {
                    let st = s.i;
                    while s.peek().is_some_and(|c| c.is_ascii_digit()) {
                        s.i += 1;
                    }
                    (s.i > st).then(|| s.p[st..s.i].iter().collect::<String>().parse().ok())?
                };
                let a = num(self);
                let (mn, mx) = if self.eat(',') {
                    let b = num(self);
                    (a.unwrap_or(0), b.unwrap_or(usize::MAX))
                } else {
                    match a {
                        Some(a) => (a, a),
                        None => {
                            self.i = save;
                            return Ok(atom);
                        }
                    }
                };
                if !self.eat('}') {
                    // Not a quantifier: a literal '{'.
                    self.i = save;
                    return Ok(atom);
                }
                (mn, mx)
            }
            _ => return Ok(atom),
        };
        let greedy = !self.eat('?');
        if matches!(atom, Node::Start | Node::End | Node::WordB(_)) {
            return Err(self.err("nothing to repeat"));
        }
        Ok(Node::Repeat(Box::new(atom), min, max, greedy))
    }

    fn escape_class(c: char) -> Option<ClassItem> {
        Some(match c {
            'd' => ClassItem::Digit(false),
            'D' => ClassItem::Digit(true),
            'w' => ClassItem::Word(false),
            'W' => ClassItem::Word(true),
            's' => ClassItem::Space(false),
            'S' => ClassItem::Space(true),
            _ => return None,
        })
    }

    fn escape_char(&mut self, c: char) -> Result<char, String> {
        Ok(match c {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            'f' => '\x0c',
            'v' => '\x0b',
            'a' => '\x07',
            '0' => '\0',
            'x' => {
                let h: String = self.p.get(self.i..self.i + 2).ok_or_else(|| self.err("bad \\x"))?.iter().collect();
                self.i += 2;
                char::from_u32(u32::from_str_radix(&h, 16).map_err(|_| self.err("bad \\x"))?).unwrap()
            }
            'u' => {
                let h: String = self.p.get(self.i..self.i + 4).ok_or_else(|| self.err("bad \\u"))?.iter().collect();
                self.i += 4;
                char::from_u32(u32::from_str_radix(&h, 16).map_err(|_| self.err("bad \\u"))?).ok_or_else(|| self.err("bad \\u"))?
            }
            c if c.is_ascii_alphanumeric() => return Err(self.err(&format!("unsupported escape \\{c}"))),
            c => c,
        })
    }

    fn atom(&mut self) -> Result<Node, String> {
        let c = self.peek().ok_or_else(|| self.err("unexpected end"))?;
        self.i += 1;
        Ok(match c {
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '(' => {
                if self.eat('?') {
                    if self.eat(':') {
                        let n = self.alt()?;
                        self.close()?;
                        Node::Group(None, Box::new(n))
                    } else if self.eat('=') || (self.peek() == Some('!')) {
                        let neg = self.eat('!');
                        let n = self.alt()?;
                        self.close()?;
                        Node::Look(Box::new(n), true, neg, 0)
                    } else if self.eat('<') {
                        let neg = if self.eat('!') {
                            true
                        } else if self.eat('=') {
                            false
                        } else {
                            return Err(self.err("bad lookbehind"));
                        };
                        let n = self.alt()?;
                        self.close()?;
                        let w = width(&n).ok_or_else(|| self.err("lookbehind needs fixed width"))?;
                        Node::Look(Box::new(n), false, neg, w)
                    } else if self.eat('P') {
                        if self.eat('<') {
                            let st = self.i;
                            while self.peek().is_some_and(|c| c != '>') {
                                self.i += 1;
                            }
                            let name: String = self.p[st..self.i].iter().collect();
                            self.i += 1;
                            self.ngroups += 1;
                            let g = self.ngroups;
                            self.names.insert(name, g);
                            let n = self.alt()?;
                            self.close()?;
                            Node::Group(Some(g), Box::new(n))
                        } else if self.eat('=') {
                            let st = self.i;
                            while self.peek().is_some_and(|c| c != ')') {
                                self.i += 1;
                            }
                            let name: String = self.p[st..self.i].iter().collect();
                            self.i += 1;
                            Node::Backref(*self.names.get(&name).ok_or_else(|| self.err("unknown group name"))?)
                        } else {
                            return Err(self.err("bad (?P"));
                        }
                    } else {
                        // Inline flags (only meaningful at the start; applied globally).
                        while let Some(f) = self.peek() {
                            match f {
                                'i' => self.flags.i = true,
                                's' => self.flags.s = true,
                                'm' => self.flags.m = true,
                                'x' => self.flags.x = true,
                                'a' | 'u' | 'L' => {}
                                ')' => break,
                                _ => return Err(self.err("unknown inline flag")),
                            }
                            self.i += 1;
                        }
                        self.close()?;
                        Node::Seq(Vec::new())
                    }
                } else {
                    self.ngroups += 1;
                    let g = self.ngroups;
                    let n = self.alt()?;
                    self.close()?;
                    Node::Group(Some(g), Box::new(n))
                }
            }
            '[' => self.class()?,
            '\\' => {
                let e = self.peek().ok_or_else(|| self.err("trailing backslash"))?;
                self.i += 1;
                match e {
                    'b' => Node::WordB(true),
                    'B' => Node::WordB(false),
                    'A' => Node::StrStart,
                    'Z' => Node::StrEnd,
                    '1'..='9' => Node::Backref(e as usize - '0' as usize),
                    e => match Self::escape_class(e) {
                        Some(ci) => Node::Class(vec![ci], false),
                        None => Node::Char(self.escape_char(e)?),
                    },
                }
            }
            c => Node::Char(c),
        })
    }

    fn close(&mut self) -> Result<(), String> {
        if self.eat(')') { Ok(()) } else { Err(self.err("missing )")) }
    }

    fn class(&mut self) -> Result<Node, String> {
        let neg = self.eat('^');
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let c = self.peek().ok_or_else(|| self.err("unterminated class"))?;
            self.i += 1;
            if c == ']' && !first {
                break;
            }
            first = false;
            let lo = if c == '\\' {
                let e = self.peek().ok_or_else(|| self.err("bad class escape"))?;
                self.i += 1;
                if let Some(ci) = Self::escape_class(e) {
                    items.push(ci);
                    continue;
                }
                if e == 'b' { '\x08' } else { self.escape_char(e)? }
            } else {
                c
            };
            if self.peek() == Some('-') && self.p.get(self.i + 1).is_some_and(|&n| n != ']') {
                self.i += 1;
                let h = self.peek().unwrap();
                self.i += 1;
                let hi = if h == '\\' {
                    let e = self.peek().ok_or_else(|| self.err("bad class escape"))?;
                    self.i += 1;
                    self.escape_char(e)?
                } else {
                    h
                };
                items.push(ClassItem::Range(lo, hi));
            } else {
                items.push(ClassItem::Range(lo, lo));
            }
        }
        Ok(Node::Class(items, neg))
    }
}

/// Fixed width of a node in chars, if it has one.
fn width(n: &Node) -> Option<usize> {
    match n {
        Node::Char(_) | Node::Any | Node::Class(..) => Some(1),
        Node::Start | Node::End | Node::StrStart | Node::StrEnd | Node::WordB(_) | Node::Look(..) => Some(0),
        Node::Group(_, n) => width(n),
        Node::Seq(v) => v.iter().map(width).sum(),
        Node::Alt(v) => {
            let w: Vec<usize> = v.iter().map(width).collect::<Option<_>>()?;
            w.windows(2).all(|p| p[0] == p[1]).then(|| w.first().copied().unwrap_or(0))
        }
        Node::Repeat(n, a, b, _) if a == b => Some(width(n)? * a),
        _ => None,
    }
}

pub struct Match<'t> {
    text: &'t str,
    /// Byte spans per group (0 = whole match).
    spans: Vec<Option<(usize, usize)>>,
}

impl<'t> Match<'t> {
    pub fn group(&self, i: usize) -> Option<&'t str> {
        self.spans.get(i).copied().flatten().map(|(a, b)| &self.text[a..b])
    }
    /// Group text, or "" when the group did not participate (Python's `m.group(i) or ""`).
    pub fn g(&self, i: usize) -> &'t str {
        self.group(i).unwrap_or("")
    }
    pub fn start(&self) -> usize {
        self.spans[0].unwrap().0
    }
    pub fn end(&self) -> usize {
        self.spans[0].unwrap().1
    }
    pub fn as_str(&self) -> &'t str {
        self.g(0)
    }
    pub fn span(&self, i: usize) -> Option<(usize, usize)> {
        self.spans.get(i).copied().flatten()
    }
    pub fn len(&self) -> usize {
        self.spans.len()
    }
}

struct Ctx<'a> {
    chars: &'a [char],
    flags: Flags,
    caps: Vec<Option<(usize, usize)>>,
    steps: usize,
}

const STEP_LIMIT: usize = 50_000_000;

impl Regex {
    pub fn new(pattern: &str) -> Result<Regex, String> {
        Self::with_flags(pattern, "")
    }

    /// `flags`: any of `i s m x` (Python re.I / re.S / re.M / re.X).
    pub fn with_flags(pattern: &str, flags: &str) -> Result<Regex, String> {
        let mut f = Flags::default();
        for c in flags.chars() {
            match c {
                'i' => f.i = true,
                's' => f.s = true,
                'm' => f.m = true,
                'x' => f.x = true,
                _ => return Err(format!("unknown flag {c}")),
            }
        }
        let mut p = Parser { p: pattern.chars().collect(), i: 0, ngroups: 0, names: HashMap::new(), flags: f, src: pattern };
        let root = p.alt()?;
        if p.i != p.p.len() {
            return Err(p.err("unbalanced )"));
        }
        let first = first_set(&root);
        Ok(Regex { root, ngroups: p.ngroups, names: p.names, flags: p.flags, first })
    }

    pub fn group_index(&self, name: &str) -> Option<usize> {
        self.names.get(name).copied()
    }

    fn can_start(&self, chars: &[char], ci: usize) -> bool {
        match &self.first {
            None => true,
            Some(set) => ci < chars.len() && set.iter().any(|n| one(n, chars[ci], self.flags) == Some(true)),
        }
    }

    /// Runs the matcher anchored at char index `ci`; returns the end char index and spans.
    fn run_at(&self, chars: &[char], ci: usize, full: bool) -> Option<(usize, Vec<Option<(usize, usize)>>)> {
        if !self.can_start(chars, ci) {
            return None;
        }
        let mut ctx = Ctx { chars, flags: self.flags, caps: vec![None; self.ngroups + 1], steps: 0 };
        let mut end = None;
        let ok = m(&self.root, ci, &mut ctx, &mut |j, c| {
            if full && j != c.chars.len() {
                return false;
            }
            end = Some((j, c.caps.clone()));
            true
        });
        if ok { end } else { None }
    }

    fn to_match<'t>(&self, text: &'t str, offs: &[usize], start: usize, end: usize, caps: Vec<Option<(usize, usize)>>) -> Match<'t> {
        let mut spans: Vec<Option<(usize, usize)>> = caps.into_iter().map(|c| c.map(|(a, b)| (offs[a], offs[b]))).collect();
        spans[0] = Some((offs[start], offs[end]));
        Match { text, spans }
    }

    /// Chars and byte offsets of `text`, cached per thread for the last subject (analysers run
    /// dozens of patterns over the same file). The cache is validated by content, not address.
    fn prepare(text: &str) -> std::rc::Rc<(Vec<char>, Vec<usize>)> {
        thread_local! {
            static LAST: std::cell::RefCell<Option<(Vec<u8>, std::rc::Rc<(Vec<char>, Vec<usize>)>)>> = const { std::cell::RefCell::new(None) };
        }
        LAST.with(|l| {
            if let Some((bytes, prep)) = l.borrow().as_ref() {
                if bytes.as_slice() == text.as_bytes() {
                    return prep.clone();
                }
            }
            let mut chars = Vec::with_capacity(text.len());
            let mut offs = Vec::with_capacity(text.len() + 1);
            for (o, c) in text.char_indices() {
                chars.push(c);
                offs.push(o);
            }
            offs.push(text.len());
            let prep = std::rc::Rc::new((chars, offs));
            if text.len() >= 64 {
                *l.borrow_mut() = Some((text.as_bytes().to_vec(), prep.clone()));
            }
            prep
        })
    }

    /// `re.search` from byte offset `pos`.
    pub fn search_from<'t>(&self, text: &'t str, pos: usize) -> Option<Match<'t>> {
        let prep = Self::prepare(text);
        let (chars, offs) = (&prep.0, &prep.1);
        let start_ci = offs.iter().position(|&o| o >= pos)?;
        for ci in start_ci..=chars.len() {
            if let Some((end, caps)) = self.run_at(chars, ci, false) {
                return Some(self.to_match(text, offs, ci, end, caps));
            }
        }
        None
    }

    pub fn search<'t>(&self, text: &'t str) -> Option<Match<'t>> {
        self.search_from(text, 0)
    }

    /// `re.match` (anchored at the start).
    pub fn match_<'t>(&self, text: &'t str) -> Option<Match<'t>> {
        self.match_at(text, 0)
    }

    /// `pattern.match(text, pos)`.
    pub fn match_at<'t>(&self, text: &'t str, pos: usize) -> Option<Match<'t>> {
        let prep = Self::prepare(text);
        let (chars, offs) = (&prep.0, &prep.1);
        let ci = offs.iter().position(|&o| o == pos)?;
        self.run_at(chars, ci, false).map(|(end, caps)| self.to_match(text, offs, ci, end, caps))
    }

    pub fn fullmatch<'t>(&self, text: &'t str) -> Option<Match<'t>> {
        let prep = Self::prepare(text);
        let (chars, offs) = (&prep.0, &prep.1);
        self.run_at(chars, 0, true).map(|(end, caps)| self.to_match(text, offs, 0, end, caps))
    }

    pub fn is_match(&self, text: &str) -> bool {
        self.search(text).is_some()
    }

    /// `re.finditer`: non-overlapping matches left to right (an empty match advances one char).
    pub fn find_iter<'t>(&self, text: &'t str) -> Vec<Match<'t>> {
        let prep = Self::prepare(text);
        let (chars, offs) = (&prep.0, &prep.1);
        let mut out = Vec::new();
        let mut ci = 0;
        while ci <= chars.len() {
            match self.run_at(chars, ci, false) {
                Some((end, caps)) => {
                    out.push(self.to_match(text, offs, ci, end, caps));
                    ci = if end == ci { ci + 1 } else { end };
                }
                None => ci += 1,
            }
        }
        out
    }

    /// `re.findall` with exactly one group (or none): the group (or whole match) texts.
    pub fn findall<'t>(&self, text: &'t str) -> Vec<&'t str> {
        let g = if self.ngroups >= 1 { 1 } else { 0 };
        self.find_iter(text).into_iter().map(|m| m.g(g)).collect()
    }

    /// `re.sub` with a replacement function.
    pub fn replace_all(&self, text: &str, mut rep: impl FnMut(&Match) -> String) -> String {
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        for m in self.find_iter(text) {
            out.push_str(&text[last..m.start()]);
            out.push_str(&rep(&m));
            last = m.end();
        }
        out.push_str(&text[last..]);
        out
    }

    /// `re.sub` with a template: `\1`..`\9` and `\g<name|n>` are expanded.
    pub fn sub(&self, text: &str, template: &str) -> String {
        self.replace_all(text, |m| expand(template, m, self))
    }

    /// `re.split` (without capture groups in the pattern).
    pub fn split<'t>(&self, text: &'t str) -> Vec<&'t str> {
        let mut out = Vec::new();
        let mut last = 0;
        for m in self.find_iter(text) {
            if m.end() == m.start() && m.start() == last && last != 0 {
                continue;
            }
            out.push(&text[last..m.start()]);
            last = m.end();
        }
        out.push(&text[last..]);
        out
    }
}

fn expand(t: &str, m: &Match, re: &Regex) -> String {
    let mut out = String::new();
    let mut it = t.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some(d @ '1'..='9') => out.push_str(m.g(d as usize - '0' as usize)),
            Some('g') if it.peek() == Some(&'<') => {
                it.next();
                let name: String = it.by_ref().take_while(|&c| c != '>').collect();
                let i = name.parse().ok().or_else(|| re.group_index(&name)).unwrap_or(usize::MAX);
                out.push_str(m.g(i));
            }
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(o) => {
                out.push('\\');
                out.push(o);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn fold(c: char) -> char {
    if c.is_ascii() { c.to_ascii_lowercase() } else { c.to_lowercase().next().unwrap_or(c) }
}

fn class_has(items: &[ClassItem], c: char, icase: bool) -> bool {
    let test = |c: char| {
        items.iter().any(|it| match *it {
            ClassItem::Range(a, b) => a <= c && c <= b,
            ClassItem::Digit(n) => c.is_ascii_digit() != n,
            ClassItem::Word(n) => is_word(c) != n,
            ClassItem::Space(n) => c.is_whitespace() != n,
        })
    };
    test(c) || (icase && (test(fold(c)) || c.to_uppercase().next().is_some_and(test)))
}

/// Does single-char node `n` accept `c`?
fn one(n: &Node, c: char, f: Flags) -> Option<bool> {
    Some(match n {
        Node::Char(x) => *x == c || (f.i && fold(*x) == fold(c)),
        Node::Any => f.s || c != '\n',
        Node::Class(items, neg) => class_has(items, c, f.i) != *neg,
        _ => return None,
    })
}

fn m(n: &Node, i: usize, cx: &mut Ctx, k: &mut dyn FnMut(usize, &mut Ctx) -> bool) -> bool {
    cx.steps += 1;
    if cx.steps > STEP_LIMIT {
        return false;
    }
    let len = cx.chars.len();
    match n {
        Node::Char(_) | Node::Any | Node::Class(..) => i < len && one(n, cx.chars[i], cx.flags).unwrap() && k(i + 1, cx),
        Node::Start => (i == 0 || (cx.flags.m && cx.chars[i - 1] == '\n')) && k(i, cx),
        Node::End => {
            let ok = i == len || (cx.chars[i] == '\n' && (i + 1 == len || cx.flags.m));
            ok && k(i, cx)
        }
        Node::StrStart => i == 0 && k(i, cx),
        Node::StrEnd => i == len && k(i, cx),
        Node::WordB(want) => {
            let a = i > 0 && is_word(cx.chars[i - 1]);
            let b = i < len && is_word(cx.chars[i]);
            ((a != b) == *want) && k(i, cx)
        }
        Node::Group(g, inner) => match g {
            None => m(inner, i, cx, k),
            Some(g) => {
                let g = *g;
                m(inner, i, cx, &mut |j, cx| {
                    let prev = cx.caps[g];
                    cx.caps[g] = Some((i, j));
                    if k(j, cx) {
                        return true;
                    }
                    cx.caps[g] = prev;
                    false
                })
            }
        },
        Node::Alt(alts) => alts.iter().any(|a| m(a, i, cx, k)),
        Node::Seq(items) => seq(items, i, cx, k),
        Node::Repeat(inner, min, max, greedy) => {
            if one(inner, 'a', cx.flags).is_some() {
                // Single-char body: count iteratively (no recursion per repetition).
                let mut n = 0;
                while n < *max && i + n < len && one(inner, cx.chars[i + n], cx.flags).unwrap() {
                    n += 1;
                }
                if n < *min {
                    return false;
                }
                if *greedy {
                    (*min..=n).rev().any(|c| k(i + c, cx))
                } else {
                    (*min..=n).any(|c| k(i + c, cx))
                }
            } else {
                rep(inner, *min, *max, *greedy, 0, i, cx, k)
            }
        }
        Node::Backref(g) => match cx.caps.get(*g).copied().flatten() {
            None => false,
            Some((a, b)) => {
                let w = b - a;
                if i + w > len {
                    return false;
                }
                let same = (0..w).all(|t| {
                    let (x, y) = (cx.chars[a + t], cx.chars[i + t]);
                    x == y || (cx.flags.i && fold(x) == fold(y))
                });
                same && k(i + w, cx)
            }
        },
        Node::Look(inner, ahead, neg, w) => {
            let at = if *ahead {
                i
            } else if i >= *w {
                i - w
            } else {
                return *neg && k(i, cx);
            };
            let saved = cx.caps.clone();
            let found = m(inner, at, cx, &mut |j, _| *ahead || j == i);
            if *neg {
                cx.caps = saved;
                !found && k(i, cx)
            } else if found {
                k(i, cx) || {
                    cx.caps = saved;
                    false
                }
            } else {
                cx.caps = saved;
                false
            }
        }
    }
}

fn seq(items: &[Node], i: usize, cx: &mut Ctx, k: &mut dyn FnMut(usize, &mut Ctx) -> bool) -> bool {
    match items.split_first() {
        None => k(i, cx),
        Some((first, rest)) => m(first, i, cx, &mut |j, cx| seq(rest, j, cx, k)),
    }
}

#[allow(clippy::too_many_arguments)]
fn rep(n: &Node, min: usize, max: usize, greedy: bool, count: usize, i: usize, cx: &mut Ctx, k: &mut dyn FnMut(usize, &mut Ctx) -> bool) -> bool {
    let try_more = |cx: &mut Ctx, k: &mut dyn FnMut(usize, &mut Ctx) -> bool| {
        count < max
            && m(n, i, cx, &mut |j, cx| {
                // A zero-width iteration past the minimum would loop forever.
                if j == i && count >= min {
                    return false;
                }
                rep(n, min, max, greedy, count + 1, j, cx, k)
            })
    };
    if greedy {
        try_more(cx, k) || (count >= min && k(i, cx))
    } else {
        (count >= min && k(i, cx)) || try_more(cx, k)
    }
}

/// Compiles a pattern once per call site (patterns are literals, so failures are bugs).
#[macro_export]
macro_rules! re {
    ($p:expr) => {{
        static R: std::sync::OnceLock<$crate::rx::Regex> = std::sync::OnceLock::new();
        R.get_or_init(|| $crate::rx::Regex::new($p).expect("valid regex"))
    }};
    ($p:expr, $f:expr) => {{
        static R: std::sync::OnceLock<$crate::rx::Regex> = std::sync::OnceLock::new();
        R.get_or_init(|| $crate::rx::Regex::with_flags($p, $f).expect("valid regex"))
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s<'a>(p: &str, t: &'a str) -> Option<String> {
        Regex::new(p).unwrap().search(t).map(|m| m.as_str().to_string())
    }

    #[test]
    fn basics() {
        assert_eq!(s(r"a+b", "xxaaab").as_deref(), Some("aaab"));
        assert_eq!(s(r"a+?", "aaa").as_deref(), Some("a"));
        assert_eq!(s(r"\d{2,3}", "a1234").as_deref(), Some("123"));
        assert_eq!(s(r"[^a-c]+", "abcdef").as_deref(), Some("def"));
        assert_eq!(s(r"(?i)hello", "say HeLLo").as_deref(), Some("HeLLo"));
        assert_eq!(s(r"\bcat\b", "concat cat").as_deref(), Some("cat"));
        assert_eq!(s(r"(a|ab)c", "abc").as_deref(), Some("abc"));
        assert_eq!(s(r"x{,2}y", "xxxy").as_deref(), Some("xxy"));
        assert_eq!(s(r"a{b", "a{b").as_deref(), Some("a{b"));
        let r = Regex::new(r#"(\w+)=("[^"]*"|\S+)"#).unwrap();
        let m = r.search(r#"k="v w" z"#).unwrap();
        assert_eq!((m.g(1), m.g(2)), ("k", "\"v w\""));
    }

    #[test]
    fn backrefs_and_lookaround() {
        assert_eq!(s(r"for\s*\(\s*var\s+(\w+)\s*=\s*0\s*;\s*\1\s*<", "for (var i = 0; i < n").as_deref(), Some("for (var i = 0; i <"));
        assert!(s(r"for\s*\(\s*var\s+(\w+)\s*=\s*0\s*;\s*\1\s*<", "for (var i = 0; j < n").is_none());
        assert_eq!(s(r"set\s+(?!/)(\w+)", "set /a x set y").as_deref(), Some("set y"));
        assert_eq!(s(r"(?<=\$)\d+", "cost $42").as_deref(), Some("42"));
        assert_eq!(s(r"\[([\d,]*)\](?!\s*\.\s*slice)", "[1,2].slice(0) [3]").as_deref(), Some("[3]"));
        assert_eq!(s(r"(?P<n>ab)c(?P=n)", "abcab").as_deref(), Some("abcab"));
    }

    #[test]
    fn anchors_flags_and_helpers() {
        let r = Regex::with_flags(r"^\s*def\s+(\w+)", "m").unwrap();
        assert_eq!(r.findall("x\n def a\ndef b"), ["a", "b"]);
        assert_eq!(s(r"a$", "a\n").as_deref(), Some("a"));
        assert!(Regex::new(r"a.b").unwrap().search("a\nb").is_none());
        assert!(Regex::with_flags(r"a.b", "s").unwrap().search("a\nb").is_some());
        assert_eq!(Regex::new(r"\s*,\s*").unwrap().split("1, 2 ,3"), ["1", "2", "3"]);
        assert_eq!(Regex::new(r"([{,]\s*)([A-Za-z_]\w*)\s*:").unwrap().sub("{a: 1, b:2}", r#"\1"\2":"#), r#"{"a": 1, "b":2}"#);
        assert!(Regex::new(r"(?:[0-9a-f]{2})+").unwrap().fullmatch("0a1b").is_some());
        assert!(Regex::new(r"(?:[0-9a-f]{2})+").unwrap().fullmatch("0a1").is_none());
        let m = Regex::new(r"é(\w)").unwrap().search("aéb").unwrap();
        assert_eq!((m.start(), m.g(1)), (1, "b"));
        // Long single-char repeats do not recurse per char.
        let long = "a".repeat(200_000);
        assert!(Regex::new(r#"^a*$"#).unwrap().fullmatch(&long).is_some());
    }
}
