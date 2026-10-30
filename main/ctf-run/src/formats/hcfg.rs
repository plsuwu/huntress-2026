//! The binary-RE lane's config blob ("HCFG"):
//! `"HCFG" | ver u8 | xor key u8 | len u16le | crc32(plaintext) u32le | payload ^ key` with a JSON payload.
//! It appears raw (possibly in a `.hcfg` section), as a base64 string constant (`SENGR...` = base64 of
//! "HCFG"), inside a Java class constant pool, or only as a single-byte-XOR'd JSON object.
//! Raw `HCFG` byte runs in code/rodata are common decoys: a hit only counts when its CRC and JSON validate.

use super::bytes::{find, find_all, u16le, u32le};
use crate::codec;
use crate::crypto::crc32::crc32;
use crate::json::Value;

#[derive(Clone, Debug)]
pub struct Hcfg {
    pub config: Value,
    pub version: u8,
    pub key: u8,
    pub crc: u32,
    /// File offset of the blob (raw magic) or of its base64 text.
    pub offset: usize,
    pub source: &'static str,
}

/// Validates a blob whose magic starts at `i`.
pub fn at(d: &[u8], i: usize) -> Option<Hcfg> {
    record_at(d, i, b"HCFG")
}

/// The last CRC-valid record with the HCFG layout under another 4-byte magic (`HGRF` campaign graph, Day 30),
/// found by scanning backward from the end.
pub fn last_record(d: &[u8], magic: &[u8; 4]) -> Option<Hcfg> {
    find_all(d, magic).into_iter().rev().find_map(|i| record_at(d, i, magic))
}

/// A record with the HCFG layout whose 4-byte `magic` starts at `i`.
pub fn record_at(d: &[u8], i: usize, magic: &[u8; 4]) -> Option<Hcfg> {
    if d.get(i..i + 4)? != magic || *d.get(i + 4)? != 1 {
        return None;
    }
    let key = d[i + 5];
    let n = u16le(d, i + 6)? as usize;
    let crc = u32le(d, i + 8)?;
    let plain: Vec<u8> = d.get(i + 12..i + 12 + n)?.iter().map(|b| b ^ key).collect();
    if crc32(&plain) != crc {
        return None;
    }
    let config = Value::parse_bytes(&plain).ok()?;
    config.as_object()?;
    Some(Hcfg { config, version: d[i + 4], key, crc, offset: i, source: "raw" })
}

/// The first raw blob that validates.
pub fn raw(d: &[u8]) -> Option<Hcfg> {
    find_all(d, b"HCFG").into_iter().find_map(|i| at(d, i))
}

fn is_b64(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'+' || c == b'/'
}

/// A base64 run starting at `i` (at least 40 chars, optional `=` padding), decoded.
fn b64_run(d: &[u8], i: usize) -> Option<Vec<u8>> {
    let mut e = i;
    while e < d.len() && is_b64(d[e]) {
        e += 1;
    }
    if e - i < 40 {
        return None;
    }
    let mut pad = 0;
    while pad < 2 && d.get(e) == Some(&b'=') {
        e += 1;
        pad += 1;
    }
    codec::base64(std::str::from_utf8(&d[i..e]).ok()?)
}

/// A base64 string constant whose decode holds a valid blob (`SENGR` = base64 of "HCFG").
pub fn base64_constant(d: &[u8]) -> Option<Hcfg> {
    for i in find_all(d, b"SENGR") {
        if let Some(blob) = b64_run(d, i) {
            if let Some(mut h) = raw(&blob) {
                h.offset = i;
                h.source = "base64";
                return Some(h);
            }
        }
    }
    None
}

/// Any base64 run (>= 40 chars) in `d` whose decode holds a blob (for resources/split constants).
pub fn any_base64(d: &[u8]) -> Option<Hcfg> {
    let mut i = 0;
    while i < d.len() {
        if is_b64(d[i]) && (i == 0 || !is_b64(d[i - 1])) {
            if let Some(blob) = b64_run(d, i) {
                if let Some(mut h) = raw(&blob) {
                    h.offset = i;
                    h.source = "base64";
                    return Some(h);
                }
            }
        }
        i += 1;
    }
    None
}

/// A JSON config XOR'd with one byte and stored without a header: brute-force the key against
/// `{"campaign"` and parse the object that follows. Returns (config, key, offset).
pub fn xor_json(d: &[u8]) -> Option<(Value, u8, usize)> {
    let probe = br#"{"campaign""#;
    for k in 0..=255u8 {
        let needle: Vec<u8> = probe.iter().map(|b| b ^ k).collect();
        if let Some(j) = find(d, &needle) {
            let tail: Vec<u8> = d[j..d.len().min(j + 4096)].iter().map(|b| b ^ k).collect();
            // The object ends at its matching brace.
            let mut depth = 0i32;
            let mut in_str = false;
            let mut esc = false;
            for (n, &c) in tail.iter().enumerate() {
                if in_str {
                    match (esc, c) {
                        (true, _) => esc = false,
                        (false, b'\\') => esc = true,
                        (false, b'"') => in_str = false,
                        _ => {}
                    }
                    continue;
                }
                match c {
                    b'"' => in_str = true,
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Value::parse_bytes(&tail[..=n]).ok().map(|v| (v, k, j));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob(json: &[u8], key: u8) -> Vec<u8> {
        let mut b = b"HCFG\x01".to_vec();
        b.push(key);
        b.extend_from_slice(&(json.len() as u16).to_le_bytes());
        b.extend_from_slice(&crc32(json).to_le_bytes());
        b.extend(json.iter().map(|c| c ^ key));
        b
    }

    #[test]
    fn finds_and_rejects_decoys() {
        let j = br#"{"campaign":"BR05-000","url":"https://x"}"#;
        let mut d = b"xxHCFG\x01\x00garbageHCFGzz".to_vec();
        let real_at = d.len();
        d.extend(blob(j, 0x61));
        let h = raw(&d).unwrap();
        assert_eq!((h.offset, h.key, h.config.str_of("campaign")), (real_at, 0x61, Some("BR05-000")));
        // As a base64 constant.
        let b = blob(j, 0x31);
        let t = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = Vec::new();
        for c in b.chunks(3) {
            let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
            for k in 0..4 {
                s.push(if k <= c.len() { t[(n >> (18 - 6 * k) & 63) as usize] } else { b'=' });
            }
        }
        let mut d2 = b"\0\0\x08\0".to_vec();
        d2.extend_from_slice(&s);
        d2.push(0);
        let h = base64_constant(&d2).unwrap();
        assert_eq!((h.offset, h.key), (4, 0x31));
        // Headerless XOR'd JSON.
        let mut d3 = vec![0u8; 10];
        d3.extend(j.iter().map(|c| c ^ 0x41));
        let (v, k, at) = xor_json(&d3).unwrap();
        assert_eq!((v.str_of("url"), k, at), (Some("https://x"), 0x41, 10));
    }
}
