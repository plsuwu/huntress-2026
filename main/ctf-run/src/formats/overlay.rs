//! Overlays appended to a binary, both validated by SHA-256 (a magic hit that does not validate is a decoy).
//!
//! - **Packed** (binary-RE d19 "Jack in the Box", magic `PK19`): a decoder stub unpacks
//!   `magic[4] | ver u8 | k1 u8 @+5 | k2 u8 @+6 | pad u8 | packed len u32le @+8 | sha256(unpacked)[32] @+12 | packed @+44`,
//!   each packed byte decoded as `ror8(b ^ k2, 3) ^ k1`. The rotation is searched (0..8), so a sibling scheme with
//!   another rotation still unpacks.
//! - **Resource** (d20 "Resources and Overlays", magic `RS20`):
//!   `magic[4] | ... | resource len u32le @+8 | scratch len u16le @+12 | .. | sha256(scratch)[32] @+16 | resource @+48 | scratch`,
//!   ending exactly at end of file; the resource is a plain HCFG record at its offset 0.

use super::bytes::{find_all, u16le, u32le};
use crate::crypto::sha256::sha256;

#[derive(Clone, Debug)]
pub struct Overlay {
    pub magic: String,
    /// Zero-based offset of the magic in the file.
    pub offset: usize,
    pub k1: u8,
    pub k2: u8,
    pub rotation: u32,
    /// The decoded payload (packed: unpacked bytes; resource: the resource bytes).
    pub unpacked: Vec<u8>,
    /// Resource overlays: the resource length field (`hcfg_resource_size`).
    pub resource_size: Option<usize>,
}

/// Magic prefixes followed by two ASCII digits (`PK19`, `RS20`).
const PREFIXES: [&[u8]; 2] = [b"PK", b"RS"];

fn is_magic(d: &[u8], i: usize) -> bool {
    d.get(i..i + 4).is_some_and(|m| PREFIXES.contains(&&m[..2]) && m[2].is_ascii_digit() && m[3].is_ascii_digit())
}

fn packed_at(d: &[u8], i: usize) -> Option<Overlay> {
    let (k1, k2) = (*d.get(i + 5)?, *d.get(i + 6)?);
    let n = u32le(d, i + 8)? as usize;
    let want = d.get(i + 12..i + 44)?;
    let packed = d.get(i + 44..(i + 44).checked_add(n)?)?;
    (0..8).find_map(|r| {
        let unpacked: Vec<u8> = packed.iter().map(|b| (b ^ k2).rotate_right(r) ^ k1).collect();
        (sha256(&unpacked) == want).then(|| Overlay {
            magic: String::from_utf8_lossy(&d[i..i + 4]).into_owned(),
            offset: i,
            k1,
            k2,
            rotation: r,
            unpacked,
            resource_size: None,
        })
    })
}

fn resource_at(d: &[u8], i: usize) -> Option<Overlay> {
    let n = u32le(d, i + 8)? as usize;
    let s = u16le(d, i + 12)? as usize;
    let want = d.get(i + 16..i + 48)?;
    let end = (i + 48).checked_add(n)?.checked_add(s)?;
    if end != d.len() {
        return None;
    }
    (sha256(&d[i + 48 + n..end]) == want).then(|| Overlay {
        magic: String::from_utf8_lossy(&d[i..i + 4]).into_owned(),
        offset: i,
        k1: 0,
        k2: 0,
        rotation: 0,
        unpacked: d[i + 48..i + 48 + n].to_vec(),
        resource_size: Some(n),
    })
}

/// Validates an overlay whose magic starts at `i` (either layout; the SHA-256 decides).
pub fn at(d: &[u8], i: usize) -> Option<Overlay> {
    if !is_magic(d, i) {
        return None;
    }
    packed_at(d, i).or_else(|| resource_at(d, i))
}

/// The final (highest-offset) overlay that validates.
pub fn last(d: &[u8]) -> Option<Overlay> {
    let mut hits: Vec<usize> = PREFIXES.iter().flat_map(|p| find_all(d, p)).collect();
    hits.sort_unstable();
    hits.into_iter().rev().find_map(|i| at(d, i))
}
