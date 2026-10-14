//! Label-driven decoding: most specimens declare `Material_<id>` (the encoded descriptor) and
//! `Codec_<id> = "<representation>|<transform>|<key>"`. The labels have been truthful in every
//! fixture (the reference solvers only cross-checked them), so the decode is driven by the label and
//! *validated* by the result: a candidate counts only if it parses as a pipe descriptor. Ambiguous
//! readings (pass count, add vs subtract, rolling-xor feedback) are tried and must agree.
//!
//! Representations: base64 (`base64_*`, `urlsafe_base64`, `reversed_base64`), hex-ish (`hex_*`,
//! `reversed_hex`, `percent_hex`, `escaped_octets`, `unicode_escapes`, `xor_hex`) and decimal-ish
//! (`decimal_*`, `integer_*`, `json_numbers`, `reversed_*`). Transforms: identity, reverse_bytes,
//! single_byte_xor, repeating_key_xor, rolling_xor, reverse_then_xor, add/subtract_modulo_256.

use super::descriptor;
use crate::codec;
use crate::re;

#[derive(Clone, Debug)]
pub struct Labelled {
    pub material_src: String,
    pub representation: String,
    pub transform: String,
    pub key: String,
    pub descriptor: String,
}

/// The source text of `Material_<id>`'s right-hand side (string-aware; spans continuation lines,
/// bracketed literals and adjacent quoted chunks).
pub fn material_rhs(text: &str) -> Option<&str> {
    let m = re!(r"(?:^|\n)[ \t]*(?:readonly\s+|declare\s+-r\s+|const\s+|Const\s+|CONST\s+|var\s+|let\s+|local\s+)?(?:MATERIAL|Material)_\w+\s*(?::\s*\w+\s*)?=\s*").search(text)?;
    let start = m.end();
    let b = text.as_bytes();
    let mut i = start;
    let mut depth = 0i32;
    let mut quote: Option<u8> = None;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            if c == b'\\' && q != b'\'' {
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' | b'\'' => quote = Some(c),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b';' if depth <= 0 => break,
            b'\n' if depth <= 0 => {
                // Continuation: a trailing + & _ \ or a next line starting with + & or a quote chunk.
                let line = text[start..i].trim_end();
                let next = text[i + 1..].trim_start();
                let cont = line.ends_with('+') || line.ends_with('&') || line.ends_with(" _") || line.ends_with('\\') || next.starts_with('+') || next.starts_with('&');
                if !cont {
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }
    Some(&text[start..i])
}

pub fn codec_label(text: &str) -> Option<String> {
    re!(r#"(?:CODEC|Codec)_\w+\s*(?::\s*\w+\s*)?=\s*['"]([^'"]*)['"]"#).search(text).map(|m| m.g(1).to_string())
}

/// Contents of every string literal in `rhs`, raw (escapes kept), concatenated; `None` when there
/// is no string literal (a bare number list).
fn string_contents(rhs: &str) -> Option<String> {
    let lits = re!(r#""((?:[^"\\]|\\.)*)"|'([^']*)'"#).find_iter(rhs);
    if lits.is_empty() {
        return None;
    }
    Some(lits.iter().map(|m| m.group(1).or(m.group(2)).unwrap_or("")).collect())
}

fn hex_tokens(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    for m in re!(r"\\+u([0-9a-fA-F]{4})|\\+x([0-9a-fA-F]{2})|%([0-9a-fA-F]{2})|0x([0-9a-fA-F]{1,2})\b|([0-9a-fA-F]+)").find_iter(s) {
        if let Some(t) = m.group(1).or(m.group(2)).or(m.group(3)).or(m.group(4)) {
            out.push(u32::from_str_radix(t, 16).ok().filter(|&v| v < 256)? as u8);
        } else {
            let t = m.g(5);
            if t.len() <= 2 {
                out.push(u8::from_str_radix(t, 16).ok()?);
            } else if t.len() % 2 == 0 {
                out.extend(codec::hex(t)?);
            } else {
                return None;
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

fn dec_tokens(s: &str) -> Option<Vec<u8>> {
    let v: Option<Vec<u8>> = re!(r"\d+").findall(s).iter().map(|t| t.parse::<u32>().ok().filter(|&x| x < 256).map(|x| x as u8)).collect();
    v.filter(|v| !v.is_empty())
}

/// Bytes of the material per its representation (`None` when the shape does not fit).
pub fn load(rhs: &str, rep: &str) -> Option<Vec<u8>> {
    let strs = string_contents(rhs);
    let text = strs.clone().unwrap_or_else(|| rhs.to_string());
    let rep_l = rep.to_ascii_lowercase();
    let reversed = rep_l.starts_with("reversed");
    if rep_l.contains("base64") {
        let mut s: String = text.chars().filter(|c| !c.is_whitespace() && *c != '\\').collect();
        if reversed {
            s = s.chars().rev().collect();
        }
        return codec::base64(&s);
    }
    let mut bytes = if rep_l.contains("hex") || rep_l.contains("percent") || rep_l.contains("octet") || rep_l.contains("unicode") {
        hex_tokens(&text)?
    } else if rep_l.contains("decimal") || rep_l.contains("integer") || rep_l.contains("json") || rep_l.contains("number") || rep_l.contains("csv") {
        dec_tokens(&text)?
    } else {
        return None;
    };
    if reversed {
        bytes.reverse();
    }
    Some(bytes)
}

fn parse_key(k: &str, hex: bool) -> Option<Vec<u8>> {
    if k.trim().is_empty() {
        return Some(Vec::new());
    }
    k.split(',').map(|x| if hex { u8::from_str_radix(x.trim(), 16).ok() } else { x.trim().parse().ok() }).collect()
}

/// All plausible decodes of `data` under the transform label (one pass).
fn transform_variants(data: &[u8], transform: &str, key: &[u8]) -> Vec<Vec<u8>> {
    let k0 = key.first().copied().unwrap_or(0);
    let rev = |v: &[u8]| v.iter().rev().copied().collect::<Vec<u8>>();
    let xor1 = |v: &[u8], k: u8| v.iter().map(|b| b ^ k).collect::<Vec<u8>>();
    let rolling = |v: &[u8], cipher_feedback: bool| {
        let mut prev = k0;
        v.iter()
            .map(|&b| {
                let o = b ^ prev;
                prev = if cipher_feedback { b } else { o };
                o
            })
            .collect::<Vec<u8>>()
    };
    match transform {
        "identity" | "" => vec![data.to_vec()],
        "reverse_bytes" | "reverse" => vec![rev(data)],
        "single_byte_xor" => vec![xor1(data, k0)],
        "repeating_key_xor" if !key.is_empty() => vec![data.iter().enumerate().map(|(i, b)| b ^ key[i % key.len()]).collect()],
        "rolling_xor" => vec![rolling(data, true), rolling(data, false), rev(&rolling(&rev(data), true))],
        "reverse_then_xor" => vec![rev(&xor1(data, k0)), xor1(data, k0)],
        "add_modulo_256" => vec![data.iter().map(|b| b.wrapping_sub(k0)).collect(), data.iter().map(|b| b.wrapping_add(k0)).collect()],
        "subtract_modulo_256" => vec![data.iter().map(|b| b.wrapping_add(k0)).collect(), data.iter().map(|b| b.wrapping_sub(k0)).collect()],
        _ => Vec::new(),
    }
}

/// Decodes a specimen from its labels; the result is a validated pipe descriptor.
pub fn decode(text: &str) -> Result<Labelled, String> {
    decode_with(text, descriptor::looks_valid)
}

/// As [`decode`], with the caller's notion of a valid result.
pub fn decode_with(text: &str, valid: impl Fn(&str) -> bool) -> Result<Labelled, String> {
    let label = codec_label(text).ok_or("no Codec_ label")?;
    let rhs = material_rhs(text).ok_or("no Material_ assignment")?;
    let mut parts = label.split('|');
    let (rep, mut transform, mut key) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
    // `xor_hex|<key>`: the key sits in the transform slot.
    if rep == "xor_hex" {
        key = transform.clone();
        transform = "single_byte_xor".into();
    }
    let data = load(rhs, &rep).ok_or_else(|| format!("material does not fit representation {rep:?}"))?;
    let mut found: Vec<String> = Vec::new();
    for hex in [true, false] {
        let Some(k) = parse_key(&key, hex) else { continue };
        let mut layer = vec![data.clone()];
        for _pass in 0..3 {
            let mut next = Vec::new();
            for d in &layer {
                for v in transform_variants(d, &transform, &k) {
                    if let Ok(s) = String::from_utf8(v.clone()) {
                        if valid(&s) && !found.contains(&s) {
                            found.push(s);
                        }
                    }
                    next.push(v);
                }
            }
            if !found.is_empty() {
                break;
            }
            next.truncate(64);
            layer = next;
        }
        if !found.is_empty() {
            break;
        }
    }
    match found.len() {
        1 => Ok(Labelled { material_src: rhs.to_string(), representation: rep, transform, key, descriptor: found.pop().unwrap() }),
        0 => Err(format!("label {label:?}: no valid descriptor ({} material bytes)", data.len())),
        _ => Err(format!("label {label:?}: ambiguous decodes {found:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_by_label() {
        let desc = "v=1|steps=acquire_remote,dispatch_memory|adapter=curl|campaign_id=c";
        let enc: Vec<String> = desc.bytes().map(|b| format!("{:02x}", b ^ 0xf3)).collect();
        let py = format!("    MATERIAL_0867 = \"{}\"\n    CODEC_0867 = \"hex_colon|single_byte_xor|f3\"\n", enc.join(":"));
        assert_eq!(decode(&py).unwrap().descriptor, desc);
        let rev: Vec<String> = desc.bytes().rev().map(|b| b.to_string()).collect();
        let tup = format!("MATERIAL_1 = (\n {},\n)\nCODEC_1 = \"integer_tuple|reverse_bytes|\"\n", rev.join(", "));
        assert_eq!(decode(&tup).unwrap().descriptor, desc);
        let node = format!("const Material_x = \"{}\";\nconst Codec_x = \"escaped_octets|identity|\";\n", desc.bytes().map(|b| format!("\\\\x{b:02x}")).collect::<String>());
        assert_eq!(decode(&node).unwrap().descriptor, desc);
        let added: Vec<String> = desc.bytes().map(|b| b.wrapping_add(13).to_string()).collect();
        let js = format!("const Material_e = \"[{}]\";\nconst Codec_e = \"json_numbers|add_modulo_256|0d\";\n", added.join(","));
        assert_eq!(decode(&js).unwrap().descriptor, desc);
        let sh = format!("readonly Material_ab='{}''{}'\nreadonly Codec_ab='hex_compact|identity|'\n", &crate::formats::bytes::hex(&desc.as_bytes()[..10]), crate::formats::bytes::hex(&desc.as_bytes()[10..]));
        assert_eq!(decode(&sh).unwrap().descriptor, desc);
    }
}
