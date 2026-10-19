//! Packed overlays appended to a binary (binary-RE d19 "Jack in the Box": a decoder stub unpacks a `PK19`
//! overlay). Layout as the d19 question spells it out:
//! `magic[4] | ver u8 | k1 u8 @+5 | k2 u8 @+6 | pad u8 | packed len u32le @+8 | sha256(unpacked)[32] @+12 | packed @+44`,
//! each packed byte decoded as `ror8(b ^ k2, 3) ^ k1`. The rotation is searched (0..8) and the SHA-256 is the
//! validator, so a sibling scheme with another rotation still unpacks; a magic hit that does not validate is a decoy.

use super::bytes::{find_all, u32le};
use crate::crypto::sha256::sha256;

#[derive(Clone, Debug)]
pub struct Overlay {
    pub magic: String,
    /// Zero-based offset of the magic in the file.
    pub offset: usize,
    pub k1: u8,
    pub k2: u8,
    pub rotation: u32,
    pub unpacked: Vec<u8>,
}

/// `PK` + two ASCII digits (`PK19`), at `i`.
fn is_magic(d: &[u8], i: usize) -> bool {
    d.get(i..i + 4).is_some_and(|m| m[2].is_ascii_digit() && m[3].is_ascii_digit())
}

/// Validates an overlay whose magic starts at `i`.
pub fn at(d: &[u8], i: usize) -> Option<Overlay> {
    if !is_magic(d, i) {
        return None;
    }
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
        })
    })
}

/// The final (highest-offset) overlay that validates.
pub fn last(d: &[u8]) -> Option<Overlay> {
    find_all(d, b"PK").into_iter().rev().find_map(|i| at(d, i))
}
