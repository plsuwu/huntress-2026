//! Text codecs shared by every lane: strict decoders (return `None` unless the whole input is valid),
//! repeat-until-stable unescapers, letter ciphers, and an "is this English" score used to decide
//! whether a self-inverse transform (reverse, atbash, rot-n) actually helped.

/// Printable text (UTF-8, no control characters other than whitespace).
pub fn printable(b: &[u8]) -> Option<String> {
    let t = std::str::from_utf8(b).ok()?;
    (!t.is_empty() && t.chars().all(|c| !c.is_control() || matches!(c, '\r' | '\n' | '\t'))).then(|| t.to_string())
}

/// UTF-16 text (day 19 "Little Things": hex of UTF-16LE with a BOM). A BOM picks the byte order;
/// without one, every high (or low) byte must be zero, i.e. Latin-1 text in either order. The
/// result must pass `printable`.
pub fn utf16_text(b: &[u8]) -> Option<String> {
    if b.len() < 2 || b.len() % 2 != 0 {
        return None;
    }
    let (body, le) = match (b[0], b[1]) {
        (0xff, 0xfe) => (&b[2..], true),
        (0xfe, 0xff) => (&b[2..], false),
        _ if b.chunks(2).all(|p| p[1] == 0 && p[0] != 0) => (b, true),
        _ if b.chunks(2).all(|p| p[0] == 0 && p[1] != 0) => (b, false),
        _ => return None,
    };
    let units: Vec<u16> = body.chunks(2).map(|p| if le { u16::from_le_bytes([p[0], p[1]]) } else { u16::from_be_bytes([p[0], p[1]]) }).collect();
    printable(String::from_utf16(&units).ok()?.as_bytes())
}

/// Printable text as UTF-8, else as UTF-16 (`utf16_text`).
pub fn text_any(b: &[u8]) -> Option<String> {
    printable(b).or_else(|| utf16_text(b))
}

fn strip_ws(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Base64, standard or URL-safe alphabet, padded or not, whitespace ignored.
pub fn base64(s: &str) -> Option<Vec<u8>> {
    let c = strip_ws(s);
    let c = c.trim_end_matches('=');
    if c.is_empty() || c.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(c.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for b in c.bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// Base32 (RFC 4648), case-insensitive, padded or not, whitespace ignored.
pub fn base32(s: &str) -> Option<Vec<u8>> {
    let c = strip_ws(s);
    let c = c.trim_end_matches('=');
    if c.is_empty() || matches!(c.len() % 8, 1 | 3 | 6) {
        return None;
    }
    let mut out = Vec::with_capacity(c.len() * 5 / 8);
    let (mut acc, mut bits) = (0u64, 0u32);
    for b in c.bytes() {
        let v = match b.to_ascii_uppercase() {
            u @ b'A'..=b'Z' => u - b'A',
            d @ b'2'..=b'7' => d - b'2' + 26,
            _ => return None,
        };
        acc = (acc << 5) | v as u64;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// Hex pairs, whitespace ignored.
pub fn hex(s: &str) -> Option<Vec<u8>> {
    let c = strip_ws(s);
    if c.is_empty() || c.len() % 2 != 0 || !c.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..c.len()).step_by(2).map(|i| u8::from_str_radix(&c[i..i + 2], 16).ok()).collect()
}

/// Fraction of "texty" characters.
pub fn text_score(t: &str) -> f64 {
    let n = t.chars().count().max(1);
    let ok = t.chars().filter(|c| c.is_ascii() && (c.is_ascii_alphanumeric() || " \t\r\n.,;:!?-_'\"()".contains(*c))).count();
    ok as f64 / n as f64
}

/// Whitespace-separated numeric tokens (char codes) in an unknown base (8/10/16/2): every base that
/// parses is tried and the most text-like decode wins (3-digit octal also parses as decimal).
pub fn numbers(text: &str) -> Option<String> {
    let toks: Vec<&str> = text.split_whitespace().collect();
    if toks.is_empty() {
        return None;
    }
    let mut best: Option<(f64, String)> = None;
    for (base, ok) in [
        (8u32, (|c: char| ('0'..='7').contains(&c)) as fn(char) -> bool),
        (10, |c: char| c.is_ascii_digit()),
        (16, |c: char| c.is_ascii_hexdigit()),
        (2, |c: char| c == '0' || c == '1'),
    ] {
        if !toks.iter().all(|t| t.chars().all(ok)) {
            continue;
        }
        let Some(vals) = toks.iter().map(|t| u32::from_str_radix(t, base).ok()).collect::<Option<Vec<u32>>>() else { continue };
        if !vals.iter().all(|&v| v > 0 && v < 128) {
            continue;
        }
        let t: String = vals.iter().map(|&v| v as u8 as char).collect();
        if !t.chars().all(|c| !c.is_control() || matches!(c, '\r' | '\n' | '\t')) {
            continue;
        }
        let s = text_score(&t);
        if best.as_ref().is_none_or(|(b, _)| s > *b) {
            best = Some((s, t));
        }
    }
    best.filter(|(s, _)| *s >= 0.9).map(|(_, t)| t)
}

pub fn has_percent(t: &str) -> bool {
    let b = t.as_bytes();
    (0..b.len().saturating_sub(2)).any(|i| b[i] == b'%' && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit())
}

/// `%XX` decoding, repeated while escapes remain (double-encoding). Invalid UTF-8 is replaced.
pub fn percent_decode_all(t: &str) -> String {
    let mut s = t.to_string();
    while has_percent(&s) {
        let b = s.as_bytes();
        let mut out = Vec::with_capacity(b.len());
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                i += 3;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        let n = String::from_utf8_lossy(&out).into_owned();
        if n == s {
            break;
        }
        s = n;
    }
    s
}

/// Finds HTML character references: `&#123;`, `&#x7B;`, `&name;`.
fn next_entity(t: &str, from: usize) -> Option<(usize, usize, Option<char>)> {
    let b = t.as_bytes();
    let mut i = from;
    while let Some(off) = t[i..].find('&') {
        let s = i + off;
        if let Some(len) = t[s + 1..].find(';').filter(|&l| l > 0 && l <= 32) {
            let body = &t[s + 1..s + 1 + len];
            let c = if let Some(h) = body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
                u32::from_str_radix(h, 16).ok().and_then(char::from_u32)
            } else if let Some(d) = body.strip_prefix('#') {
                d.parse::<u32>().ok().and_then(char::from_u32)
            } else if body.bytes().all(|c| c.is_ascii_alphanumeric()) && b[s + 1].is_ascii_alphabetic() {
                named_entity(body)
            } else {
                None
            };
            if c.is_some() || body.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'#') {
                return Some((s, s + 2 + len, c));
            }
        }
        i = s + 1;
    }
    None
}

fn named_entity(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        "excl" => '!',
        "colon" => ':',
        "comma" => ',',
        "period" => '.',
        "hyphen" | "dash" => '-',
        "sol" => '/',
        "num" => '#',
        "percnt" => '%',
        "lpar" => '(',
        "rpar" => ')',
        "semi" => ';',
        "equals" => '=',
        "quest" => '?',
        "commat" => '@',
        "lowbar" => '_',
        "Tab" => '\t',
        "NewLine" => '\n',
        _ => return None,
    })
}

pub fn has_entity(t: &str) -> bool {
    let mut from = 0;
    while let Some((_, end, c)) = next_entity(t, from) {
        if c.is_some() {
            return true;
        }
        from = end;
    }
    false
}

/// HTML character references decoded, repeated while any remain.
pub fn html_unescape_all(t: &str) -> String {
    let mut s = t.to_string();
    loop {
        let mut out = String::with_capacity(s.len());
        let mut last = 0;
        let mut from = 0;
        let mut changed = false;
        while let Some((a, b, c)) = next_entity(&s, from) {
            if let Some(c) = c {
                out.push_str(&s[last..a]);
                out.push(c);
                last = b;
                changed = true;
            }
            from = b;
        }
        if !changed {
            return s;
        }
        out.push_str(&s[last..]);
        s = out;
    }
}

/// One backslash escape at `i` (`\uXXXX`, `\u{X}`, `\UXXXXXXXX`, `\xNN`, octal `\NNN`, `\n`...):
/// returns (code point, length).
fn bs_escape(b: &[u8], i: usize) -> Option<(u32, usize)> {
    let hexn = |from: usize, n: usize| -> Option<u32> {
        let s = std::str::from_utf8(b.get(from..from + n)?).ok()?;
        s.bytes().all(|c| c.is_ascii_hexdigit()).then(|| u32::from_str_radix(s, 16).ok())?
    };
    match *b.get(i + 1)? {
        b'u' if b.get(i + 2) == Some(&b'{') => {
            let end = b[i + 3..].iter().position(|&c| c == b'}')?;
            Some((hexn(i + 3, end)?, end + 4))
        }
        b'u' => Some((hexn(i + 2, 4)?, 6)),
        b'U' => Some((hexn(i + 2, 8)?, 10)),
        b'x' => Some((hexn(i + 2, 2)?, 4)),
        b'0'..=b'7' => {
            let n = b[i + 1..].iter().take(3).take_while(|c| (b'0'..=b'7').contains(c)).count();
            let s = std::str::from_utf8(&b[i + 1..i + 1 + n]).ok()?;
            Some((u32::from_str_radix(s, 8).ok()?, 1 + n))
        }
        c @ (b'a' | b'b' | b'f' | b'n' | b'r' | b't' | b'v' | b'\\' | b'\'' | b'"' | b'/') => Some((
            match c {
                b'a' => 7,
                b'b' => 8,
                b'f' => 12,
                b'n' => 10,
                b'r' => 13,
                b't' => 9,
                b'v' => 11,
                c => c as u32,
            },
            2,
        )),
        _ => None,
    }
}

pub fn has_backslash_escape(t: &str) -> bool {
    let b = t.as_bytes();
    (0..b.len()).any(|i| b[i] == b'\\' && bs_escape(b, i).is_some())
}

/// Backslash escapes decoded (surrogate pairs recombined), surrounding quotes stripped, repeated
/// while escapes remain.
pub fn backslash_unescape_all(t: &str) -> String {
    let mut s = t.trim().to_string();
    if s.len() >= 2 && (s.starts_with('"') && s.ends_with('"') || s.starts_with('\'') && s.ends_with('\'')) {
        s = s[1..s.len() - 1].to_string();
    }
    while has_backslash_escape(&s) {
        let b = s.as_bytes();
        let mut units: Vec<u32> = Vec::with_capacity(b.len());
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'\\' {
                if let Some((cp, n)) = bs_escape(b, i) {
                    units.push(cp);
                    i += n;
                    continue;
                }
            }
            let c = s[i..].chars().next().unwrap();
            units.push(c as u32);
            i += c.len_utf8();
        }
        let mut out = String::with_capacity(units.len());
        let mut k = 0;
        while k < units.len() {
            let u = units[k];
            if (0xD800..0xDC00).contains(&u) && k + 1 < units.len() && (0xDC00..0xE000).contains(&units[k + 1]) {
                out.push(char::from_u32(0x10000 + ((u - 0xD800) << 10) + (units[k + 1] - 0xDC00)).unwrap_or('\u{FFFD}'));
                k += 2;
                continue;
            }
            out.push(char::from_u32(u).unwrap_or('\u{FFFD}'));
            k += 1;
        }
        if out == s {
            break;
        }
        s = out;
    }
    s
}

/// Rotates ASCII letters by `n` (rot13 = `rot(t, 13)`; decode a Caesar +k with `rot(t, 26 - k)`).
pub fn rot(t: &str, n: u8) -> String {
    let n = n % 26;
    t.chars()
        .map(|c| match c {
            'a'..='z' => (b'a' + (c as u8 - b'a' + n) % 26) as char,
            'A'..='Z' => (b'A' + (c as u8 - b'A' + n) % 26) as char,
            _ => c,
        })
        .collect()
}

/// ROT47: rotates printable ASCII `!`..`~` by 47 (self-inverse).
pub fn rot47(t: &str) -> String {
    t.chars().map(|c| if ('!'..='~').contains(&c) { (b'!' + (c as u8 - b'!' + 47) % 94) as char } else { c }).collect()
}

/// Rail fence (zigzag) decryption with `rails` rails, starting on the top rail going down.
pub fn rail_fence_decode(t: &str, rails: usize) -> String {
    let cs: Vec<char> = t.chars().collect();
    if rails < 2 || cs.len() <= 1 {
        return t.to_string();
    }
    let cycle = 2 * (rails - 1);
    let rail = |i: usize| {
        let m = i % cycle;
        if m < rails { m } else { cycle - m }
    };
    let mut order: Vec<usize> = (0..cs.len()).collect();
    order.sort_by_key(|&i| (rail(i), i));
    let mut out = vec![' '; cs.len()];
    for (k, &i) in order.iter().enumerate() {
        out[i] = cs[k];
    }
    out.into_iter().collect()
}

pub fn atbash(t: &str) -> String {
    t.chars()
        .map(|c| match c {
            'a'..='z' => (b'z' - (c as u8 - b'a')) as char,
            'A'..='Z' => (b'Z' - (c as u8 - b'A')) as char,
            _ => c,
        })
        .collect()
}

pub fn reverse(t: &str) -> String {
    t.chars().rev().collect()
}

const MORSE: &[(&str, char)] = &[
    (".-", 'A'), ("-...", 'B'), ("-.-.", 'C'), ("-..", 'D'), (".", 'E'), ("..-.", 'F'), ("--.", 'G'), ("....", 'H'),
    ("..", 'I'), (".---", 'J'), ("-.-", 'K'), (".-..", 'L'), ("--", 'M'), ("-.", 'N'), ("---", 'O'), (".--.", 'P'),
    ("--.-", 'Q'), (".-.", 'R'), ("...", 'S'), ("-", 'T'), ("..-", 'U'), ("...-", 'V'), (".--", 'W'), ("-..-", 'X'),
    ("-.--", 'Y'), ("--..", 'Z'), ("-----", '0'), (".----", '1'), ("..---", '2'), ("...--", '3'), ("....-", '4'),
    (".....", '5'), ("-....", '6'), ("--...", '7'), ("---..", '8'), ("----.", '9'), (".-.-.-", '.'), ("--..--", ','),
    ("..--..", '?'), (".----.", '\''), ("-.-.--", '!'), ("-..-.", '/'), ("-.--.", '('), ("-.--.-", ')'), (".-...", '&'),
    ("---...", ':'), ("-.-.-.", ';'), ("-...-", '='), (".-.-.", '+'), ("-....-", '-'), ("..--.-", '_'), (".-..-.", '"'),
    ("...-..-", '$'), (".--.-.", '@'),
];

pub fn is_morse(t: &str) -> bool {
    let t = t.trim();
    !t.is_empty() && t.bytes().any(|b| b == b'.' || b == b'-') && t.bytes().all(|b| matches!(b, b'.' | b'-' | b'/' | b'|') || b.is_ascii_whitespace())
}

/// International Morse (uppercase letters, digits, punctuation): codes separated by whitespace,
/// words by `/`, `|` or a run of 3+ spaces. `None` on any unknown code.
pub fn morse(t: &str) -> Option<String> {
    if !is_morse(t) {
        return None;
    }
    let mut out = String::new();
    let t = t.trim();
    let mut words: Vec<String> = Vec::new();
    for w in t.split(['/', '|']) {
        words.extend(w.split("   ").map(str::to_string));
    }
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        for code in w.split_whitespace() {
            out.push(MORSE.iter().find(|(m, _)| *m == code)?.1);
        }
    }
    Some(out)
}

/// Bacon-style 5-bit groups of `a`/`b` (either case), whitespace-separated or run together.
pub fn is_bacon(t: &str) -> bool {
    let n = t.bytes().filter(|b| !b.is_ascii_whitespace()).count();
    n > 0 && n % 5 == 0 && t.bytes().all(|b| matches!(b, b'a' | b'b' | b'A' | b'B') || b.is_ascii_whitespace())
        && t.split_whitespace().all(|g| g.len() % 5 == 0)
}

/// The 5-bit values (`a` = 0, `b` = 1, MSB first) of a Bacon string; `None` unless `is_bacon`.
pub fn bacon_values(t: &str) -> Option<Vec<u8>> {
    if !is_bacon(t) {
        return None;
    }
    let bits: Vec<u8> = t.bytes().filter(|b| !b.is_ascii_whitespace()).map(|b| (b | 0x20 == b'b') as u8).collect();
    Some(bits.chunks(5).map(|c| c.iter().fold(0, |v, b| v << 1 | b)).collect())
}

/// Classic Bacon letters: the 26-letter alphabet, or the 24-letter one (I=J, U=V).
pub fn bacon_letters(vals: &[u8], alphabet26: bool) -> Option<String> {
    const A24: &[u8] = b"ABCDEFGHIKLMNOPQRSTUWXYZ";
    vals.iter()
        .map(|&v| if alphabet26 { (v < 26).then(|| (b'A' + v) as char) } else { A24.get(v as usize).map(|&c| c as char) })
        .collect()
}

const WORDS: &[&str] = &[
    "the", "this", "to", "and", "answer", "submit", "exact", "game", "master", "warmup", "solve", "work", "nice", "of", "is",
];

/// A `word-word-word-NN` slug: three lowercase words and two digits.
pub fn is_slug(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 4
        && parts[..3].iter().all(|w| !w.is_empty() && w.bytes().all(|c| c.is_ascii_lowercase()))
        && parts[3].len() == 2
        && parts[3].bytes().all(|c| c.is_ascii_digit())
}

/// Slugs appearing as whole tokens in `t`.
pub fn slugs(t: &str) -> Vec<&str> {
    t.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).filter(|w| is_slug(w)).collect()
}

/// Rough "how English is this" count: known words (case-insensitive, whole) + 3 per slug.
pub fn englishness(t: &str) -> usize {
    let words = t
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| {
            let l = w.to_ascii_lowercase();
            WORDS.contains(&l.as_str()) || l.starts_with("decod")
        })
        .count();
    words + 3 * slugs(t).len()
}

/// English letter-frequency log-likelihood per letter (higher = more English-like); used to rank
/// rot-n / atbash candidates for short letter-only fragments.
pub fn letter_score(t: &str) -> f64 {
    const F: [f64; 26] = [
        8.2, 1.5, 2.8, 4.3, 12.7, 2.2, 2.0, 6.1, 7.0, 0.15, 0.77, 4.0, 2.4, 6.7, 7.5, 1.9, 0.095, 6.0, 6.3, 9.1, 2.8, 0.98, 2.4,
        0.15, 2.0, 0.074,
    ];
    let mut n = 0;
    let mut s = 0.0;
    for c in t.chars().filter(char::is_ascii_alphabetic) {
        s += (F[(c.to_ascii_lowercase() as u8 - b'a') as usize] / 100.0).ln();
        n += 1;
    }
    if n == 0 { f64::NEG_INFINITY } else { s / n as f64 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoders() {
        assert_eq!(base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64("aGVsbG8").unwrap(), b"hello");
        assert_eq!(base64("-_8").unwrap(), [0xfb, 0xff]);
        assert!(base64("a$b=").is_none());
        assert_eq!(base32("NBSWY3DP").unwrap(), b"hello");
        assert_eq!(base32("nbswy3dp").unwrap(), b"hello");
        assert_eq!(hex("68 65 6c").unwrap(), b"hel");
        assert!(hex("6").is_none());
        assert_eq!(numbers("116 151 143 145").as_deref(), Some("Nice"));
        assert_eq!(numbers("78 105 99 101").as_deref(), Some("Nice"));
        assert_eq!(numbers("01001110 01101001").as_deref(), Some("Ni"));
        assert_eq!(percent_decode_all("%254e%69"), "Ni");
        assert_eq!(html_unescape_all("&#78;&#x69;c&amp;e"), "Nic&e");
        assert_eq!(html_unescape_all("a & b; c"), "a & b; c");
        assert_eq!(backslash_unescape_all(r#""N\x69\143\n😀""#), "Nic\n😀");
        assert_eq!(rot("Nice", 13), "Avpr");
        assert_eq!(atbash("Nice"), "Mrxv");
        assert_eq!(rot47("Nice-65!"), "}:46\\edP");
        assert!(is_slug("zesty-sunset-quartz-65") && !is_slug("zesty-sunset-65"));
        assert!(englishness("Nice work decoding this warmup!") > englishness("Avpr jbex"));
        assert!(letter_score("sunset") > letter_score(&rot("sunset", 7)));
    }
}
