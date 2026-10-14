//! Java class files: the constant pool's `CONSTANT_Utf8` entries (where string constants live).

use super::bytes::u16be;

/// Every `CONSTANT_Utf8` payload of a class file, in pool order. `None` if it is not a class file.
pub fn utf8_constants(c: &[u8]) -> Option<Vec<&[u8]>> {
    if !c.starts_with(&[0xCA, 0xFE, 0xBA, 0xBE]) {
        return None;
    }
    let n = u16be(c, 8)? as usize;
    let mut o = 10;
    let mut i = 1;
    let mut out = Vec::new();
    while i < n {
        let t = *c.get(o)?;
        o += 1;
        match t {
            1 => {
                let l = u16be(c, o)? as usize;
                out.push(c.get(o + 2..o + 2 + l)?);
                o += 2 + l;
            }
            3 | 4 | 9 | 10 | 11 | 12 | 17 | 18 => o += 4,
            5 | 6 => {
                o += 8;
                i += 1; // long/double take two slots
            }
            7 | 8 | 16 | 19 | 20 => o += 2,
            15 => o += 3,
            _ => return None,
        }
        i += 1;
    }
    Some(out)
}

/// Class-file major version (52 = Java 8, 65 = Java 21).
pub fn major_version(c: &[u8]) -> Option<u16> {
    c.starts_with(&[0xCA, 0xFE, 0xBA, 0xBE]).then(|| u16be(c, 6))?
}
