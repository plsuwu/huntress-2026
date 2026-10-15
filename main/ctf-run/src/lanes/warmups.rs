//! Warmups: a slug hidden in an encoded copy of a fixed sentence.
//!
//! Single/time-trial: the whole blob is wrapped in the day's encoding layer(s); peel until the
//! sentence appears. Mastery: `NN<TAB>fragment` rows, raggedly sliced from the sentence, where row
//! N uses day N's encoding (01 base64, 02 rot13, 03 hex, ... 12 reversed + dash padding, 13 atbash;
//! see utils/LANE_PATTERNS.md). Rows with a known codec are decoded directly; unknown rows try every
//! structured decoder, then every letter transform, and a search keeps only combinations consistent
//! with the known sentence. A transform pinned down for a row in one set is reused for that row in
//! the other sets (a Caesar row that falls inside the slug cannot be resolved on its own).
//!
//! Every answer is a template- or slug-validated `word-word-word-NN`; anything else is an error.

use super::{Analysed, Analysis, Lane};
use crate::codec;
use crate::formats::zip;
use crate::model::{Delivery, Question, RunCtx};
use std::collections::{BTreeMap, BTreeSet};

pub struct Warmups;

const PREFIX: &str = "Nice work decoding this warmup! To solve, submit this exact answer to the Game Master: ";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum T {
    Id,
    Rot(u8),
    Rot47,
    Atbash,
    Rev,
    B64,
    B32,
    Hex,
    Num,
    Pct,
    Ent,
    Bs,
    Bin,
}

impl T {
    fn apply(self, s: &str) -> Option<String> {
        let t = s.trim();
        match self {
            T::Id => Some(s.to_string()),
            T::Rot(n) => Some(codec::rot(s, n)),
            T::Rot47 => Some(codec::rot47(s)),
            T::Atbash => Some(codec::atbash(s)),
            T::Rev => Some(codec::reverse(s)),
            T::B64 => codec::printable(&codec::base64(t)?),
            T::B32 => codec::printable(&codec::base32(t)?),
            T::Hex => codec::printable(&codec::hex(t)?),
            T::Num => codec::numbers(t),
            T::Pct => codec::has_percent(t).then(|| codec::percent_decode_all(t)),
            T::Ent => codec::has_entity(t).then(|| codec::html_unescape_all(t)),
            T::Bs => codec::has_backslash_escape(t).then(|| codec::backslash_unescape_all(t)),
            T::Bin => {
                let c: String = t.chars().filter(|c| !c.is_whitespace()).collect();
                if c.is_empty() || c.len() % 8 != 0 || !c.bytes().all(|b| b == b'0' || b == b'1') {
                    return None;
                }
                let bytes: Vec<u8> = c.as_bytes().chunks(8).map(|ch| u8::from_str_radix(std::str::from_utf8(ch).unwrap(), 2).unwrap()).collect();
                codec::printable(&bytes)
            }
        }
    }
}

/// Row number -> codec, from the day each encoding was introduced.
fn fixed_codec(row: u32) -> Option<T> {
    Some(match row {
        1 => T::B64,
        2 => T::Rot(13),
        3 => T::Hex,
        4 | 5 | 6 => T::Num,
        7 => T::Pct,
        8 => T::Ent,
        9 => T::Bs,
        10 => T::B32,
        11 => T::B64,
        12 => T::Rev,
        13 => T::Atbash,
        14 => T::Rot(19), // Caesar +7 ("Caesar took a week off"); graded on the day-14 single-solve
        15 => T::Rot47, // "Room Forty-Seven": punctuation and digits rotate too
        _ => return None,
    })
}

/// Rows whose content carries an unambiguous structural signature.
fn structured(frag: &str) -> Option<(T, String)> {
    let v = frag.trim();
    let try_ = |t: T| t.apply(v).map(|s| (t, s));
    if codec::has_backslash_escape(v) {
        return try_(T::Bs);
    }
    if codec::has_entity(v) {
        return try_(T::Ent);
    }
    if codec::has_percent(v) {
        return try_(T::Pct);
    }
    let digits_spaces = !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit() || b == b' ');
    if digits_spaces && (v.contains(' ') || v.len() % 2 == 1) {
        // Space-separated (or odd-length) digits; even all-digit runs may be hex pairs.
        return try_(T::Num).or_else(|| try_(T::Bin));
    }
    if v.len() % 2 == 0 && v.bytes().all(|b| b.is_ascii_hexdigit()) {
        if let Some(r) = try_(T::Hex) {
            return Some(r);
        }
    }
    if v.len() % 8 == 0 && v.bytes().all(|b| b.is_ascii_uppercase() || (b'2'..=b'7').contains(&b) || b == b'=') {
        if let Some(r) = try_(T::B32) {
            return Some(r);
        }
    }
    let b64ish = v.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/_-=".contains(&b));
    if b64ish && v.bytes().any(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b"=+/_".contains(&b)) {
        return try_(T::B64);
    }
    None
}

fn letter_candidates(frag: &str) -> Vec<(T, String)> {
    let mut out: Vec<(T, String)> = Vec::new();
    let mut push = |t: T, s: String| {
        if !out.iter().any(|(_, x)| *x == s) {
            out.push((t, s));
        }
    };
    push(T::Id, frag.to_string());
    push(T::Rot(13), codec::rot(frag, 13));
    push(T::Atbash, codec::atbash(frag));
    push(T::Rot47, codec::rot47(frag));
    for s in rev_variants(frag) {
        push(T::Rev, s);
    }
    for n in 1..26u8 {
        push(T::Rot(n), codec::rot(frag, n));
    }
    out
}

/// A reversed row may be right-padded with dashes (`89-enud-t----`): the padding becomes a leading
/// run after reversal; a genuine boundary hyphen is at most one dash.
fn rev_variants(frag: &str) -> Vec<String> {
    let trimmed = frag.trim_end_matches('-');
    let pad = frag.len() - trimmed.len();
    let mut v = vec![codec::reverse(frag)];
    if pad >= 2 {
        v = vec![codec::reverse(trimmed), format!("-{}", codec::reverse(trimmed))];
    }
    v
}

/// The slug, if `text` is exactly the sentence (trailing whitespace allowed). The last row may be
/// padded to the common row width with `-<digits>` (seen 2026-10-15: `v-35-3783` -> `...whale-35-3783`);
/// the slug format is exact (three words, two digits), so such a tail is unambiguous.
pub fn match_template(text: &str) -> Option<String> {
    let i = text.find(PREFIX)?;
    let rest = text[i + PREFIX.len()..].trim_end();
    if codec::is_slug(rest) {
        return Some(rest.to_string());
    }
    let parts: Vec<&str> = rest.splitn(5, '-').collect();
    if parts.len() == 5 && parts[4].bytes().all(|b| b.is_ascii_digit()) && !parts[4].is_empty() {
        let slug = parts[..4].join("-");
        if codec::is_slug(&slug) {
            return Some(slug);
        }
    }
    None
}

/// Could `s` still grow into the sentence?
fn consistent(s: &str) -> bool {
    if s.len() <= PREFIX.len() {
        PREFIX.starts_with(s)
    } else {
        s.starts_with(PREFIX) && s[PREFIX.len()..].bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b.is_ascii_whitespace())
    }
}

#[derive(Clone, Debug)]
pub enum Outcome {
    Solved { slug: String, learned: Vec<(u32, T)> },
    Ambiguous { slugs: Vec<String> },
    Failed(String),
}

struct Row {
    num: u32,
    frag: String,
}

fn parse_rows(text: &str) -> Option<Vec<Row>> {
    let rows: Vec<Row> = text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .filter(|l| l.contains('\t'))
        .map(|l| {
            let (n, f) = l.split_once('\t').unwrap();
            Some(Row { num: n.trim().parse().ok()?, frag: f.to_string() })
        })
        .collect::<Option<_>>()?;
    (!rows.is_empty()).then_some(rows)
}

fn is_rows(text: &str) -> bool {
    let first = text.trim_start_matches('\u{feff}');
    let digits = first.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && first.as_bytes().get(digits) == Some(&b'\t')
}

/// Solves a mastery row blob. `hints` pins transforms for rows learned from other sets.
pub fn solve_rows(text: &str, hints: &BTreeMap<u32, T>) -> Outcome {
    let Some(rows) = parse_rows(text) else { return Outcome::Failed("unparsable rows".into()) };
    // Candidates per row: (transform, decoded text, is_free_letter_row).
    let mut cands: Vec<Vec<(T, String)>> = Vec::with_capacity(rows.len());
    let mut free: Vec<bool> = Vec::with_capacity(rows.len());
    for r in &rows {
        let pinned = fixed_codec(r.num).or_else(|| hints.get(&r.num).copied());
        if let Some(t) = pinned {
            let outs: Vec<String> = if t == T::Rev { rev_variants(&r.frag) } else { t.apply(&r.frag).into_iter().collect() };
            if !outs.is_empty() {
                cands.push(outs.into_iter().map(|s| (t, s)).collect());
                free.push(false);
                continue;
            }
        }
        if let Some((t, s)) = structured(&r.frag) {
            cands.push(vec![(t, s)]);
            free.push(false);
        } else {
            cands.push(letter_candidates(&r.frag));
            free.push(true);
        }
    }
    // Depth-first search over candidates, pruned by the sentence prefix.
    let mut results: BTreeMap<String, Vec<Vec<T>>> = BTreeMap::new();
    let mut path: Vec<T> = Vec::with_capacity(rows.len());
    let mut budget = 2_000_000usize;
    fn dfs(
        i: usize,
        acc: &mut String,
        cands: &[Vec<(T, String)>],
        path: &mut Vec<T>,
        results: &mut BTreeMap<String, Vec<Vec<T>>>,
        budget: &mut usize,
    ) {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        if i == cands.len() {
            if let Some(slug) = match_template(acc) {
                results.entry(slug).or_default().push(path.clone());
            }
            return;
        }
        for (t, s) in &cands[i] {
            let len = acc.len();
            acc.push_str(s);
            if consistent(acc) {
                path.push(*t);
                dfs(i + 1, acc, cands, path, results, budget);
                path.pop();
            }
            acc.truncate(len);
        }
    }
    dfs(0, &mut String::new(), &cands, &mut path, &mut results, &mut budget);
    match results.len() {
        1 => {
            let (slug, paths) = results.into_iter().next().unwrap();
            // A free row's transform is learned when every successful path agrees on it and the
            // fragment has letters (otherwise all letter transforms look alike).
            let learned = rows
                .iter()
                .enumerate()
                .filter(|(i, r)| free[*i] && r.frag.bytes().any(|b| b.is_ascii_alphabetic()))
                .filter_map(|(i, r)| {
                    let t = paths[0][i];
                    paths.iter().all(|p| p[i] == t).then_some((r.num, t))
                })
                .collect();
            Outcome::Solved { slug, learned }
        }
        0 if budget == 0 => Outcome::Failed("search budget exhausted".into()),
        0 => {
            let preview: String = cands.iter().map(|c| c[0].1.as_str()).collect::<String>().chars().take(160).collect();
            Outcome::Failed(format!("no row combination forms the sentence; first-choice decode: {preview:?}"))
        }
        _ => Outcome::Ambiguous { slugs: results.into_keys().collect() },
    }
}

/// Peels whole-blob layers until the sentence (or a last-line slug) appears.
pub fn solve_blob(text: &str) -> Outcome {
    let mut t = text.trim_start_matches('\u{feff}').to_string();
    for _ in 0..32 {
        if let Some(slug) = match_template(&t) {
            return Outcome::Solved { slug, learned: Vec::new() };
        }
        // A letter transform that yields the sentence wins over structural layers: ROT47 output can
        // look like escapes (`-c` -> `\4`, which a backslash peel would eat as octal).
        let letters = [T::Rot47, T::Atbash, T::Rev].into_iter().chain((1..26u8).map(T::Rot));
        if let Some(slug) = letters.filter_map(|k| k.apply(&t)).find_map(|c| match_template(&c)) {
            return Outcome::Solved { slug, learned: Vec::new() };
        }
        let next = [T::Bs, T::Ent, T::Pct, T::Num, T::Bin, T::Hex, T::B64, T::B32].iter().find_map(|k| k.apply(&t).filter(|n| *n != t));
        if let Some(n) = next {
            t = n;
            continue;
        }
        // Letter transforms are self-inverse or cyclic: apply the best one only if it reads as more English.
        let now = codec::englishness(&t);
        let best = std::iter::once(codec::atbash(&t))
            .chain(std::iter::once(codec::rot47(&t)))
            .chain(std::iter::once(codec::reverse(t.trim())))
            .chain((1..26u8).map(|n| codec::rot(&t, n)))
            .map(|c| (codec::englishness(&c), c))
            .max_by_key(|(s, _)| *s);
        match best {
            Some((s, c)) if s > now => t = c,
            _ => break,
        }
    }
    if let Some(slug) = match_template(&t) {
        return Outcome::Solved { slug, learned: Vec::new() };
    }
    let last = t.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("");
    match codec::slugs(last).last() {
        Some(s) => Outcome::Solved { slug: s.to_string(), learned: Vec::new() },
        None => Outcome::Failed(format!("no slug after peeling; text starts {:?}", t.chars().take(120).collect::<String>())),
    }
}

pub fn solve_text(text: &str, hints: &BTreeMap<u32, T>) -> Outcome {
    if is_rows(text) { solve_rows(text, hints) } else { solve_blob(text) }
}

/// One delivery: every blob it holds (a bundle zip holds many), by file name.
pub struct Blobs {
    pub items: Vec<(String, String, Outcome)>,
}

impl Blobs {
    fn find(&self, name: &str) -> Option<&(String, String, Outcome)> {
        self.items.iter().find(|(n, _, _)| n == name)
    }
}

fn learned_hints(done: &Analysed) -> BTreeMap<u32, T> {
    let mut votes: BTreeMap<u32, BTreeSet<T>> = BTreeMap::new();
    for (_, b) in done.all::<Blobs>() {
        for (_, _, o) in &b.items {
            if let Outcome::Solved { learned, .. } = o {
                for (row, t) in learned {
                    votes.entry(*row).or_default().insert(*t);
                }
            }
        }
    }
    votes.into_iter().filter(|(_, ts)| ts.len() == 1).map(|(r, ts)| (r, *ts.iter().next().unwrap())).collect()
}

fn finish(text: &str, outcome: &Outcome, done: &Analysed) -> Result<String, String> {
    match outcome {
        Outcome::Solved { slug, .. } => Ok(slug.clone()),
        Outcome::Failed(e) => Err(e.clone()),
        Outcome::Ambiguous { slugs } => {
            let hints = learned_hints(done);
            match solve_rows(text, &hints) {
                Outcome::Solved { slug, .. } => Ok(slug),
                _ => Err(format!("ambiguous: {slugs:?} (row transforms learned from other sets: {hints:?})")),
            }
        }
    }
}

impl Lane for Warmups {
    fn name(&self) -> &'static str {
        "warmups"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["warmups-day-"]
    }

    fn analyse(&self, d: &Delivery, _: &RunCtx) -> Result<Analysis, String> {
        let none = BTreeMap::new();
        let mut items = Vec::new();
        if zip::is_zip(&d.bytes) {
            let z = zip::Zip::parse(&d.bytes)?;
            for e in z.files() {
                let bytes = z.read(e, b"infected")?;
                let text = String::from_utf8_lossy(&bytes).into_owned();
                let o = solve_text(&text, &none);
                items.push((e.base_name().to_string(), text, o));
            }
        } else {
            let text = String::from_utf8_lossy(&d.bytes).into_owned();
            let o = solve_text(&text, &none);
            items.push((d.name.clone(), text, o));
        }
        Ok(Box::new(Blobs { items }))
    }

    fn answer(&self, q: &Question, done: &Analysed, _: &RunCtx) -> Result<String, String> {
        let name = q.artifact_path().last().map(|s| s.to_string());
        // `delivery N: name` -> that delivery (its only blob, or the named one).
        if let Some((n, _)) = q.delivery_label() {
            let b = done.get::<Blobs>(n.checked_sub(1).ok_or("delivery 0")?)?;
            let item = match (b.items.len(), &name) {
                (1, _) => &b.items[0],
                (_, Some(nm)) => b.find(nm).ok_or_else(|| format!("delivery {n} has no {nm}"))?,
                _ => return Err("cannot pick a blob".into()),
            };
            return finish(&item.1, &item.2, done);
        }
        // Otherwise by file name across every delivery (bundles), else the only blob there is.
        let mut found = Vec::new();
        for (_, b) in done.all::<Blobs>() {
            for it in &b.items {
                if name.as_deref() == Some(it.0.as_str()) {
                    found.push(it);
                }
            }
        }
        if found.is_empty() {
            let all: Vec<_> = done.all::<Blobs>().flat_map(|(_, b)| b.items.iter()).collect();
            if all.len() == 1 {
                found = all;
            }
        }
        match found.as_slice() {
            [it] => finish(&it.1, &it.2, done),
            [] => Err(format!("no blob for artifact {:?}", q.artifact)),
            _ => Err(format!("{} blobs named {:?}", found.len(), name)),
        }
    }

    fn delivery_of(&self, q: &Question, done: &Analysed) -> Option<usize> {
        if let Some(i) = super::default_delivery_of(q, done.len()) {
            return Some(i);
        }
        let name = q.artifact_path().last()?.to_string();
        done.all::<Blobs>().find(|(_, b)| b.find(&name).is_some()).map(|(i, _)| i)
    }

    fn self_validating(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SENT: &str = "Nice work decoding this warmup! To solve, submit this exact answer to the Game Master: zesty-sunset-quartz-65";

    #[test]
    fn whole_blob_layers() {
        let b64 = "TmljZSB3b3JrIGRlY29kaW5nIHRoaXMgd2FybXVwISBUbyBzb2x2ZSwgc3VibWl0IHRoaXMgZXhhY3QgYW5zd2VyIHRvIHRoZSBHYW1lIE1hc3RlcjogemVzdHktc3Vuc2V0LXF1YXJ0ei02NQ==";
        let slug = |o: Outcome| match o {
            Outcome::Solved { slug, .. } => slug,
            o => panic!("{o:?}"),
        };
        assert_eq!(slug(solve_blob(b64)), "zesty-sunset-quartz-65");
        assert_eq!(slug(solve_blob(&codec::reverse(SENT))), "zesty-sunset-quartz-65");
        assert_eq!(slug(solve_blob(&codec::atbash(SENT))), "zesty-sunset-quartz-65");
        // A Caesar shift of 7 ("Seven Steps"), not hard-coded.
        assert_eq!(slug(solve_blob(&codec::rot(SENT, 7))), "zesty-sunset-quartz-65");
        assert!(matches!(solve_blob("nothing to see"), Outcome::Failed(_)));
    }

    /// 2026-10-15 timed variant: the final (atbash) row is padded with `-<digits>`.
    #[test]
    fn digit_padded_final_row() {
        let blob = "01\tTmljZSB3b3I=\n02\tx qrpbqva\n03\t672074686973207761\n04\t01110010 01101101 01110101 01110000 00100001 00100000 01010100 01101111 00100000\n05\t115 111 108 118 101 44 32 115\n06\t165 142 155 151 164 040 164 150 151\n07\t%73%20%65%78%61%63%74%20%61\n08\t&#110;&#115;&#119;&#101;&#114;&#32;&#116;&#111;&#32;\n09\t\"\\u0074\\u0068\\u0065\\u0020\\u0047\\u0061\\u006d\\u0065\"\n10\tEBGWC43UMVZDUIA=\n11\tcGVwcHktc2Nv\n12\tlahw-reto\n13\tv-35-3783";
        let Outcome::Solved { slug, .. } = solve_rows(blob, &BTreeMap::new()) else { panic!("{:?}", solve_rows(blob, &BTreeMap::new())) };
        assert_eq!(slug, "peppy-scooter-whale-35");
        assert!(match_template(&format!("{PREFIX}a-b-c-12-x")).is_none());
    }

    #[test]
    fn rows_with_unknown_caesar_row_learn_across_sets() {
        // An unpinned row (16 here, caesar +7). Set A: inside the known prefix -> learnable. Set B: inside the slug.
        let a = format!("01\t{}\n02\t{}\n16\t{}\n13\t{}\n", "TmljZSB3b3Jr", codec::rot(" decoding this warmup! To solve,", 13), codec::rot(" submit this exact answer", 7), codec::atbash(" to the Game Master: zesty-sunset-quartz-65"));
        let none = BTreeMap::new();
        let Outcome::Solved { slug, learned } = solve_rows(&a, &none) else { panic!() };
        assert_eq!(slug, "zesty-sunset-quartz-65");
        assert_eq!(learned, [(16, T::Rot(19))]);
        let b = format!("01\t{}\n16\t{}\n", "TmljZSB3b3JrIGRlY29kaW5nIHRoaXMgd2FybXVwISBUbyBzb2x2ZSwgc3VibWl0IHRoaXMgZXhhY3QgYW5zd2VyIHRvIHRoZSBHYW1lIE1hc3RlcjogemVzdHkt", codec::rot("sunset-quartz-65", 7));
        assert!(matches!(solve_rows(&b, &none), Outcome::Ambiguous { .. }));
        let hints = BTreeMap::from([(16, T::Rot(19))]);
        let Outcome::Solved { slug, .. } = solve_rows(&b, &hints) else { panic!() };
        assert_eq!(slug, "zesty-sunset-quartz-65");
        // Row 14 is pinned (day 14 = caesar +7), so the same slug-only set solves without hints.
        let b14 = b.replace("\n16\t", "\n14\t");
        let Outcome::Solved { slug, .. } = solve_rows(&b14, &none) else { panic!("{:?}", solve_rows(&b14, &none)) };
        assert_eq!(slug, "zesty-sunset-quartz-65");
    }
}
