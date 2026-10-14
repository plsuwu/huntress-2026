//! Image and document headers: JPEG SOF geometry, PNG chunks, PDF header/versions.

use super::bytes::{u16be, u32be};
use crate::crypto::crc32;

/// `(width, height)` from the first JPEG SOFn marker (not DHT/JPG/DAC), scanning segments.
pub fn jpeg_sof(d: &[u8]) -> Option<(u16, u16)> {
    let mut i = 2;
    while i + 4 <= d.len() {
        if d[i] != 0xFF {
            i += 1;
            continue;
        }
        let m = d[i + 1];
        if m == 0xFF {
            i += 1;
        } else if m == 0x01 || (0xD0..=0xD8).contains(&m) {
            i += 2;
        } else {
            if (0xC0..=0xCF).contains(&m) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
                return Some((u16be(d, i + 7)?, u16be(d, i + 5)?));
            }
            if m == 0xDA {
                return None;
            }
            i += 2 + u16be(d, i + 2)? as usize;
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct PngChunk {
    pub kind: String,
    pub offset: usize,
    pub len: usize,
    pub crc_ok: bool,
}

/// IHDR `(width, height)` (first `IHDR` occurrence, as the Python reference reads it).
pub fn png_ihdr(d: &[u8]) -> Option<(u32, u32)> {
    let i = super::bytes::find(d, b"IHDR")?;
    Some((u32be(d, i + 4)?, u32be(d, i + 8)?))
}

/// Walks PNG chunks after the signature, checking CRCs; also returns bytes trailing IEND.
pub fn png_chunks(d: &[u8]) -> Option<(Vec<PngChunk>, usize)> {
    if !d.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    let mut out = Vec::new();
    let mut p = 8;
    while p + 12 <= d.len() {
        let len = u32be(d, p)? as usize;
        let kind = String::from_utf8_lossy(&d[p + 4..p + 8]).into_owned();
        let end = p + 12 + len;
        let crc_ok = d.get(p + 4..p + 8 + len).is_some_and(|body| Some(crc32::crc32(body)) == u32be(d, p + 8 + len));
        out.push(PngChunk { kind: kind.clone(), offset: p, len, crc_ok });
        p = end;
        if kind == "IEND" {
            break;
        }
    }
    Some((out, d.len().saturating_sub(p)))
}

/// `x.y` from `%PDF-x.y` in the first KiB.
pub fn pdf_header_version(d: &[u8]) -> Option<String> {
    let head = &d[..d.len().min(1024)];
    let i = super::bytes::find(head, b"%PDF-")?;
    let v: String = head[i + 5..].iter().take_while(|c| c.is_ascii_digit() || **c == b'.').map(|&c| c as char).collect();
    let mut parts = v.split('.');
    let (a, b) = (parts.next()?, parts.next()?);
    (!a.is_empty() && !b.is_empty()).then(|| format!("{a}.{b}"))
}

/// Every `/Version /x.y` (catalog overrides), and the `%%EOF` count.
pub fn pdf_facts(d: &[u8]) -> (Vec<String>, usize) {
    let mut versions = Vec::new();
    for at in super::bytes::find_all(d, b"/Version") {
        let rest = &d[at + 8..d.len().min(at + 24)];
        let v: String = rest.iter().skip_while(|c| c.is_ascii_whitespace() || **c == b'/').take_while(|c| c.is_ascii_digit() || **c == b'.').map(|&c| c as char).collect();
        if !v.is_empty() {
            versions.push(v);
        }
    }
    (versions, super::bytes::find_all(d, b"%%EOF").len())
}
