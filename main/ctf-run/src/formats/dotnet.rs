//! .NET (ECMA-335) metadata: the `BSJB` root, its streams, and the Module table's MVID.

use super::bytes::{cstr, find, u16le, u32le, u64le};

pub struct Metadata<'a> {
    pub root: usize,
    /// e.g. `v4.0.30319`.
    pub version: String,
    /// (name, absolute offset, size).
    pub streams: Vec<(String, usize, usize)>,
    d: &'a [u8],
}

impl<'a> Metadata<'a> {
    /// Locates the metadata root by its `BSJB` signature.
    pub fn find(d: &'a [u8]) -> Option<Metadata<'a>> {
        let r = find(d, b"BSJB")?;
        let vl = u32le(d, r + 12)? as usize;
        let version = String::from_utf8_lossy(cstr(d.get(r + 16..r + 16 + vl)?, 0)?).into_owned();
        let mut p = r + 16 + vl + 2;
        let ns = u16le(d, p)? as usize;
        p += 2;
        let mut streams = Vec::new();
        for _ in 0..ns {
            let off = u32le(d, p)? as usize;
            let size = u32le(d, p + 4)? as usize;
            p += 8;
            let name = cstr(d, p)?;
            let n = String::from_utf8_lossy(name).into_owned();
            p = (p + name.len() + 1 + 3) & !3;
            streams.push((n, r + off, size));
        }
        Some(Metadata { root: r, version, streams, d })
    }

    pub fn stream(&self, name: &str) -> Option<(usize, usize)> {
        self.streams.iter().find(|(n, _, _)| n == name).map(|(_, o, s)| (*o, *s))
    }

    /// The Module table's Mvid, as 16 GUID bytes (little-endian field layout, as stored).
    pub fn mvid_bytes(&self) -> Option<[u8; 16]> {
        let d = self.d;
        let (t, _) = self.stream("#~").or_else(|| self.stream("#-"))?;
        let heaps = d[t + 6];
        let valid = u64le(d, t + 8)?;
        // Row counts for every present table follow; the Module table's rows come first.
        let mut q = t + 24 + 4 * valid.count_ones() as usize;
        let sidx = if heaps & 1 != 0 { 4 } else { 2 };
        let gidx = if heaps & 2 != 0 { 4 } else { 2 };
        q += 2 + sidx; // Generation, Name
        let gi = if gidx == 4 { u32le(d, q)? as usize } else { u16le(d, q)? as usize };
        let (g, _) = self.stream("#GUID")?;
        d.get(g + (gi.checked_sub(1)?) * 16..g + gi * 16)?.try_into().ok()
    }

    /// Mvid as lowercase `8-4-4-4-12` (Python `uuid.UUID(bytes_le=...)`).
    pub fn mvid(&self) -> Option<String> {
        Some(guid_le(&self.mvid_bytes()?, false))
    }
}

/// A GUID stored with its first three fields little-endian, formatted 8-4-4-4-12.
pub fn guid_le(b: &[u8; 16], upper: bool) -> String {
    let s = format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{}",
        u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        u16::from_le_bytes([b[4], b[5]]),
        u16::from_le_bytes([b[6], b[7]]),
        b[8],
        b[9],
        b[10..].iter().map(|x| format!("{x:02x}")).collect::<String>()
    );
    if upper { s.to_uppercase() } else { s }
}
