//! Bounds-checked integer reads and small text helpers shared by the format parsers.

pub fn u8at(b: &[u8], at: usize) -> Option<u8> {
    b.get(at).copied()
}
pub fn u16le(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at.checked_add(2)?)?.try_into().ok()?))
}
pub fn u32le(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?))
}
pub fn u64le(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at.checked_add(8)?)?.try_into().ok()?))
}
pub fn u16be(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at.checked_add(2)?)?.try_into().ok()?))
}
pub fn u32be(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?))
}
pub fn u64be(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(at..at.checked_add(8)?)?.try_into().ok()?))
}

/// Endian-aware reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Endian {
    Little,
    Big,
}

impl Endian {
    pub fn u16(self, b: &[u8], at: usize) -> Option<u16> {
        if self == Endian::Little { u16le(b, at) } else { u16be(b, at) }
    }
    pub fn u32(self, b: &[u8], at: usize) -> Option<u32> {
        if self == Endian::Little { u32le(b, at) } else { u32be(b, at) }
    }
    pub fn u64(self, b: &[u8], at: usize) -> Option<u64> {
        if self == Endian::Little { u64le(b, at) } else { u64be(b, at) }
    }
}

/// NUL-terminated bytes starting at `at`.
pub fn cstr(b: &[u8], at: usize) -> Option<&[u8]> {
    let rest = b.get(at..)?;
    Some(&rest[..rest.iter().position(|&c| c == 0).unwrap_or(rest.len())])
}

/// First index of byte `b` (SIMD via the `memchr` crate).
pub fn memchr(b: u8, hay: &[u8]) -> Option<usize> {
    memchr::memchr(b, hay)
}

pub fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    memchr::memmem::find(hay, needle)
}

/// Every offset of `needle` in `hay`, overlapping matches included.
pub fn find_all(hay: &[u8], needle: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    if needle.is_empty() {
        return out;
    }
    let finder = memchr::memmem::Finder::new(needle);
    let mut i = 0;
    while let Some(p) = finder.find(&hay[i..]) {
        out.push(i + p);
        i += p + 1;
    }
    out
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// UTF-16LE (lossy).
pub fn utf16le(b: &[u8]) -> String {
    let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units)
}

const CP1252_HIGH: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜',
    '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

pub fn cp1252(b: &[u8]) -> String {
    b.iter().map(|&c| if (0x80..0xA0).contains(&c) { CP1252_HIGH[(c - 0x80) as usize] } else { c as char }).collect()
}

const CP437_HIGH: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{a0}";

pub fn cp437(b: &[u8]) -> String {
    let high: Vec<char> = CP437_HIGH.chars().collect();
    b.iter().map(|&c| if c < 0x80 { c as char } else { high[(c - 0x80) as usize] }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(find(b"abcabcd", b"cd"), Some(5));
        let hay: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        for b in [0u8, 7, 250] {
            assert_eq!(memchr(b, &hay), hay.iter().position(|&c| c == b), "{b}");
        }
        assert_eq!(memchr(255, &hay), None);
        assert_eq!(find_all(b"aXaXa", b"a"), [0, 2, 4]);
        assert_eq!(u32be(&[0, 0, 1, 0], 0), Some(256));
        assert!(u32le(&[1, 2, 3], 0).is_none());
        assert_eq!(cp437(&[0x80, b'a']), "Ça");
        assert_eq!(CP437_HIGH.chars().count(), 128);
        assert_eq!(cp1252(&[0x80, 0xe9]), "€é");
    }
}
