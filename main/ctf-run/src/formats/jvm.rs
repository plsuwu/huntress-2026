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

/// A class file's declaration: access flags, names, and its methods `(name, descriptor, access)` in file order.
#[derive(Clone, Debug, Default)]
pub struct ClassInfo {
    pub access: u16,
    pub this_class: String,
    pub super_class: String,
    pub methods: Vec<(String, String, u16)>,
}

/// Parses the constant pool, header and member tables of a class file. `None` if it is not one.
pub fn class_info(c: &[u8]) -> Option<ClassInfo> {
    if !c.starts_with(&[0xCA, 0xFE, 0xBA, 0xBE]) {
        return None;
    }
    let n = u16be(c, 8)? as usize;
    // Slot -> (tag, utf8 payload or first index).
    let mut utf8: Vec<Option<&[u8]>> = vec![None; n];
    let mut class_name: Vec<u16> = vec![0; n];
    let mut o = 10;
    let mut i = 1;
    while i < n {
        let t = *c.get(o)?;
        o += 1;
        match t {
            1 => {
                let l = u16be(c, o)? as usize;
                utf8[i] = Some(c.get(o + 2..o + 2 + l)?);
                o += 2 + l;
            }
            7 => {
                class_name[i] = u16be(c, o)?;
                o += 2;
            }
            3 | 4 | 9 | 10 | 11 | 12 | 17 | 18 => o += 4,
            5 | 6 => {
                o += 8;
                i += 1;
            }
            8 | 16 | 19 | 20 => o += 2,
            15 => o += 3,
            _ => return None,
        }
        i += 1;
    }
    let s = |k: u16| utf8.get(k as usize).copied().flatten().map(|b| String::from_utf8_lossy(b).into_owned());
    let cls = |k: u16| class_name.get(k as usize).and_then(|&u| s(u)).unwrap_or_default();
    let access = u16be(c, o)?;
    let this_class = cls(u16be(c, o + 2)?);
    let super_class = cls(u16be(c, o + 4)?);
    o += 6;
    o += 2 + 2 * u16be(c, o)? as usize;
    let members = |o: &mut usize| -> Option<Vec<(String, String, u16)>> {
        let cnt = u16be(c, *o)? as usize;
        *o += 2;
        let mut out = Vec::with_capacity(cnt);
        for _ in 0..cnt {
            let (acc, name, desc, attrs) = (u16be(c, *o)?, u16be(c, *o + 2)?, u16be(c, *o + 4)?, u16be(c, *o + 6)?);
            *o += 8;
            for _ in 0..attrs {
                *o += 6 + super::bytes::u32be(c, *o + 2)? as usize;
            }
            out.push((s(name)?, s(desc)?, acc));
        }
        Some(out)
    };
    members(&mut o)?; // fields
    let methods = members(&mut o)?;
    Some(ClassInfo { access, this_class, super_class, methods })
}
