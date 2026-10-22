//! Python `re` semantics on `fancy-regex`, so the reference solvers' patterns can be carried over
//! verbatim. Patterns without backreferences or lookaround are delegated whole to the `regex`
//! crate's automata (an order of magnitude faster than the hand-rolled backtracker this replaced);
//! the rest run on fancy-regex's backtracking VM.
//!
//! `translate` bridges the syntax gaps: non-MULTILINE `$` also matches before a final newline,
//! `\Z`, `{,n}` and stray braces, `[`/`&`/`~` inside classes, and punctuation escapes such as
//! `\<` (a word boundary in Rust, a literal in Python). Offsets are byte offsets into the subject.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

/// Python's matcher gives up eventually too; this keeps a pathological pattern from hanging a run.
const BACKTRACK_LIMIT: usize = 50_000_000;

/// The pattern as searched (`Search`), as `\A(?:pattern)` for `match_`, and as `\A(?:pattern)\z`
/// for `fullmatch`.
#[derive(Clone, Copy)]
enum Kind {
    Search,
    Anchored,
    Full,
}

/// Cheap to clone: compiled patterns are shared through a process-wide cache, so patterns built
/// at run time (`format!`ed with a function name) compile once per distinct text.
#[derive(Clone)]
pub struct Regex(Arc<Inner>);

impl std::ops::Deref for Regex {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}

pub struct Inner {
    /// The translated pattern, kept to build the variants on first use.
    src: String,
    flags: (bool, bool, bool),
    /// Per `Kind`: an ASCII-classes build run on the bytes of ASCII subjects, and the Unicode build
    /// for the rest. On ASCII text the two agree (`\w`, `\s`, `\b`, `.`, case folding), and the
    /// ASCII one compiles ~10x faster: Unicode `\w` classes dominate NFA construction, which
    /// would otherwise land on a live run's critical path.
    ascii: [OnceLock<Option<fancy_regex::Regex>>; 3],
    uni: [OnceLock<Option<fancy_regex::Regex>>; 3],
    ngroups: usize,
    names: HashMap<String, usize>,
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

fn build(pattern: &str, (i, s, m): (bool, bool, bool), ascii: bool) -> Result<fancy_regex::Regex, String> {
    let mode = if ascii { fancy_regex::BytesMode::Ascii } else { fancy_regex::BytesMode::Unicode };
    fancy_regex::RegexBuilder::new(pattern)
        .bytes_mode(mode)
        .case_insensitive(i)
        .dot_matches_new_line(s)
        .multi_line(m)
        .backtrack_limit(BACKTRACK_LIMIT)
        .build()
        .map_err(|e| format!("regex {pattern:?}: {e}"))
}

/// Inline flags at the start of a pattern (`(?im)...`), which Python only allows there.
fn leading_flags(p: &str) -> &str {
    p.strip_prefix("(?").and_then(|r| r.find(')').map(|e| &r[..e])).filter(|f| !f.is_empty() && f.chars().all(|c| "aiLmsux".contains(c))).unwrap_or("")
}

/// A literal char as an escape both engines read the same way, inside or outside a class.
fn push_lit(out: &mut String, c: char) {
    out.push_str(&format!("\\x{{{:X}}}", c as u32));
}

/// Python `{m}`, `{m,}`, `{,n}`, `{m,n}` at the start of `c` as (Rust syntax, chars used); None
/// when the brace is a literal.
fn quantifier(c: &[char]) -> Option<(String, usize)> {
    let close = c.iter().position(|&x| x == '}')?;
    let body: String = c[1..close].iter().collect();
    let (lo, hi) = match body.split_once(',') {
        Some((lo, hi)) => (lo, Some(hi)),
        None => (body.as_str(), None),
    };
    let digits = |s: &str| s.chars().all(|d| d.is_ascii_digit());
    if !digits(lo) || !hi.is_none_or(digits) || (lo.is_empty() && hi.is_none()) {
        return None;
    }
    let lo = if lo.is_empty() { "0" } else { lo };
    Some((match hi {
        None => format!("{{{lo}}}"),
        Some(hi) => format!("{{{lo},{hi}}}"),
    }, close + 1))
}

/// Python `re` syntax -> fancy-regex/regex syntax (see the module docs).
fn translate(p: &str, multiline: bool) -> String {
    let c: Vec<char> = p.chars().collect();
    let mut out = String::with_capacity(p.len() + 16);
    let (mut i, mut class, mut class_start) = (0, false, false);
    while i < c.len() {
        let ch = c[i];
        if ch == '\\' && i + 1 < c.len() {
            let e = c[i + 1];
            i += 2;
            match e {
                'Z' if !class => out.push_str(r"\z"),
                'b' if class => out.push_str(r"\x08"),
                '0' => out.push_str(r"\x00"),
                e if e.is_ascii_alphanumeric() => {
                    out.push('\\');
                    out.push(e);
                }
                e => push_lit(&mut out, e),
            }
            class_start = false;
            continue;
        }
        i += 1;
        if class {
            match ch {
                ']' if !class_start => {
                    class = false;
                    out.push(']');
                }
                '[' | '&' | '~' => push_lit(&mut out, ch),
                _ => out.push(ch),
            }
            class_start = false;
            continue;
        }
        match ch {
            '[' => {
                (class, class_start) = (true, true);
                out.push('[');
                if c.get(i) == Some(&'^') {
                    out.push('^');
                    i += 1;
                }
            }
            '$' if !multiline => out.push_str(r"(?=\n?\z)"),
            '{' => match quantifier(&c[i - 1..]) {
                Some((q, used)) => {
                    out.push_str(&q);
                    i += used - 1;
                }
                None => out.push_str(r"\{"),
            },
            '}' => out.push_str(r"\}"),
            _ => out.push(ch),
        }
    }
    out
}

impl Inner {
    fn engine(&self, kind: Kind, ascii: bool) -> Option<&fancy_regex::Regex> {
        let slot = if ascii { &self.ascii[kind as usize] } else { &self.uni[kind as usize] };
        slot.get_or_init(|| {
            // Leading inline flags must stay first when the pattern is wrapped.
            let lf = leading_flags(&self.src);
            let (head, body) = if lf.is_empty() { ("", self.src.as_str()) } else { self.src.split_at(lf.len() + 3) };
            let p = match kind {
                Kind::Search => self.src.clone(),
                Kind::Anchored => format!(r"{head}\A(?:{body})"),
                Kind::Full => format!(r"{head}\A(?:{body})\z"),
            };
            build(&p, self.flags, ascii).ok()
        })
        .as_ref()
        .or_else(|| if ascii { self.engine(kind, false) } else { None })
    }
}

impl Regex {
    pub fn new(pattern: &str) -> Result<Regex, String> {
        Self::with_flags(pattern, "")
    }

    /// `flags`: any of `i s m` (Python re.I / re.S / re.M).
    pub fn with_flags(pattern: &str, flags: &str) -> Result<Regex, String> {
        static CACHE: LazyLock<Mutex<HashMap<(String, String), Regex>>> = LazyLock::new(Default::default);
        let key = (pattern.to_string(), flags.to_string());
        if let Some(r) = CACHE.lock().unwrap().get(&key) {
            return Ok(r.clone());
        }
        let r = Self::compile(pattern, flags)?;
        CACHE.lock().unwrap().insert(key, r.clone());
        Ok(r)
    }

    fn compile(pattern: &str, flags: &str) -> Result<Regex, String> {
        let mut f = (false, false, false);
        for c in flags.chars() {
            match c {
                'i' => f.0 = true,
                's' => f.1 = true,
                'm' => f.2 = true,
                _ => return Err(format!("unknown flag {c}")),
            }
        }
        let src = translate(pattern, f.2 || leading_flags(pattern).contains('m'));
        let r = Inner { src, flags: f, ascii: Default::default(), uni: Default::default(), ngroups: 0, names: HashMap::new() };
        // Validate now (patterns ASCII mode rejects, such as `\p{..}`, use the Unicode build).
        let re = match r.engine(Kind::Search, true) {
            Some(re) => re,
            None => r.engine(Kind::Search, false).ok_or_else(|| build(&r.src, f, false).unwrap_err())?,
        };
        let names = re.capture_names().enumerate().filter_map(|(i, n)| n.map(|n| (n.to_string(), i))).collect();
        let ngroups = re.captures_len() - 1;
        Ok(Regex(Arc::new(Inner { names, ngroups, ..r })))
    }

    pub fn group_index(&self, name: &str) -> Option<usize> {
        self.names.get(name).copied()
    }

    /// Leftmost match of `kind` at or after byte `pos` (a char boundary).
    fn find<'t>(&self, kind: Kind, text: &'t str, pos: usize) -> Option<Match<'t>> {
        let ascii = text.is_ascii();
        let re = self.engine(kind, ascii)?;
        let spans = if ascii {
            let c = re.captures_from_pos(text.as_bytes(), pos).ok()??;
            (0..c.len()).map(|i| c.get(i).map(|m| (m.start(), m.end()))).collect()
        } else {
            let c = re.captures_from_pos(text, pos).ok()??;
            (0..c.len()).map(|i| c.get(i).map(|m| (m.start(), m.end()))).collect()
        };
        Some(Match { text, spans })
    }

    /// `re.search` from byte offset `pos`.
    pub fn search_from<'t>(&self, text: &'t str, mut pos: usize) -> Option<Match<'t>> {
        if pos > text.len() {
            return None;
        }
        while !text.is_char_boundary(pos) {
            pos += 1;
        }
        self.find(Kind::Search, text, pos)
    }

    pub fn search<'t>(&self, text: &'t str) -> Option<Match<'t>> {
        self.search_from(text, 0)
    }

    /// `re.match` (anchored at the start).
    pub fn match_<'t>(&self, text: &'t str) -> Option<Match<'t>> {
        self.find(Kind::Anchored, text, 0)
    }

    /// `pattern.match(text, pos)`: the leftmost match from `pos`, if it starts there.
    pub fn match_at<'t>(&self, text: &'t str, pos: usize) -> Option<Match<'t>> {
        if pos == 0 {
            return self.match_(text);
        }
        if !text.is_char_boundary(pos) {
            return None;
        }
        self.search_from(text, pos).filter(|m| m.start() == pos)
    }

    pub fn fullmatch<'t>(&self, text: &'t str) -> Option<Match<'t>> {
        self.find(Kind::Full, text, 0)
    }

    pub fn is_match(&self, text: &str) -> bool {
        let ascii = text.is_ascii();
        match self.engine(Kind::Search, ascii) {
            Some(re) if ascii => re.is_match(text.as_bytes()).unwrap_or(false),
            Some(re) => re.is_match(text).unwrap_or(false),
            None => false,
        }
    }

    /// `re.finditer`: non-overlapping matches left to right (an empty match advances one char).
    pub fn find_iter<'t>(&self, text: &'t str) -> Vec<Match<'t>> {
        let mut out = Vec::new();
        let mut pos = 0;
        while let Some(m) = self.search_from(text, pos) {
            pos = if m.end() == m.start() { m.end() + 1 } else { m.end() };
            out.push(m);
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
        // Python-only syntax the translator rewrites.
        assert_eq!(s(r"\<b\>", "a<b>").as_deref(), Some("<b>"));
        assert_eq!(s(r"[[\]]+", "x[]y").as_deref(), Some("[]"));
        assert_eq!(s(r"[a&~]+", "x&~ay").as_deref(), Some("&~a"));
        assert_eq!(s(r"a\Z", "a\n").as_deref(), None);
        assert_eq!(s(r"(?i)A$", "xa\n").as_deref(), Some("a"));
        assert_eq!(Regex::with_flags(r"^b$", "m").unwrap().findall("a\nb\nc"), ["b"]);
        let t = Regex::new(r"\s*(\d+|[a-z]+)").unwrap();
        assert_eq!(t.match_at("ab 12", 2).map(|m| m.g(1)), Some("12"));
        assert!(t.match_at("ab -12", 2).is_none());
        assert_eq!(Regex::new(r"x*").unwrap().find_iter("éxé").len(), 4); // Python: ['', 'x', '', '']
        // Long single-char repeats do not recurse per char.
        let long = "a".repeat(200_000);
        assert!(Regex::new(r#"^a*$"#).unwrap().fullmatch(&long).is_some());
    }
}
