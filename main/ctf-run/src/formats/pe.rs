//! PE/COFF: headers, sections, data directories, and the VS_VERSION_INFO string table.

use super::bytes::{cstr, find, u16le, u32le, u64le, utf16le};

#[derive(Clone, Debug)]
pub struct Section {
    pub name: String,
    pub vaddr: u32,
    pub vsize: u32,
    pub raw_off: u32,
    pub raw_size: u32,
}

#[derive(Clone, Debug)]
pub struct Pe {
    pub pe_off: usize,
    pub machine: u16,
    pub characteristics: u16,
    pub timestamp: u32,
    /// 0x10b (PE32) or 0x20b (PE32+).
    pub opt_magic: u16,
    pub entry_rva: u32,
    pub image_base: u64,
    pub subsystem: u16,
    pub sections: Vec<Section>,
    /// (rva, size) per data directory.
    pub dirs: Vec<(u32, u32)>,
}

impl Pe {
    pub fn parse(d: &[u8]) -> Option<Pe> {
        if !d.starts_with(b"MZ") {
            return None;
        }
        let pe_off = u32le(d, 0x3C)? as usize;
        if d.get(pe_off..pe_off + 4)? != b"PE\0\0" {
            return None;
        }
        let coff = pe_off + 4;
        let machine = u16le(d, coff)?;
        let nsec = u16le(d, coff + 2)? as usize;
        let timestamp = u32le(d, coff + 4)?;
        let opt_size = u16le(d, coff + 16)? as usize;
        let characteristics = u16le(d, coff + 18)?;
        let opt = coff + 20;
        let opt_magic = u16le(d, opt).unwrap_or(0);
        let entry_rva = u32le(d, opt + 16).unwrap_or(0);
        let (image_base, subsystem, ndirs_off) = match opt_magic {
            0x20b => (u64le(d, opt + 24).unwrap_or(0), u16le(d, opt + 68).unwrap_or(0), opt + 108),
            _ => (u32le(d, opt + 28).unwrap_or(0) as u64, u16le(d, opt + 68).unwrap_or(0), opt + 92),
        };
        let ndirs = u32le(d, ndirs_off).unwrap_or(0).min(16) as usize;
        let dirs = (0..ndirs).filter_map(|i| Some((u32le(d, ndirs_off + 4 + i * 8)?, u32le(d, ndirs_off + 8 + i * 8)?))).collect();
        let st = opt + opt_size;
        let sections = (0..nsec)
            .filter_map(|i| {
                let s = st + i * 40;
                Some(Section {
                    name: String::from_utf8_lossy(cstr(d.get(s..s + 8)?, 0)?).into_owned(),
                    vsize: u32le(d, s + 8)?,
                    vaddr: u32le(d, s + 12)?,
                    raw_size: u32le(d, s + 16)?,
                    raw_off: u32le(d, s + 20)?,
                })
            })
            .collect();
        Some(Pe { pe_off, machine, characteristics, timestamp, opt_magic, entry_rva, image_base, subsystem, sections, dirs })
    }

    pub fn rva_to_offset(&self, rva: u32) -> Option<usize> {
        self.sections
            .iter()
            .find(|s| rva >= s.vaddr && rva < s.vaddr + s.vsize.max(s.raw_size))
            .map(|s| (rva - s.vaddr + s.raw_off) as usize)
    }

    pub fn is_dll(&self) -> bool {
        self.characteristics & 0x2000 != 0
    }

    /// The CLR (COM descriptor) data directory is present: a .NET assembly.
    pub fn is_dotnet(&self) -> bool {
        self.dirs.get(14).is_some_and(|&(rva, size)| rva != 0 && size != 0)
    }
}

/// Machine field names as `file`/objdump print them.
pub fn machine_name(m: u16) -> &'static str {
    match m {
        0x014c => "x86",
        0x8664 => "x86-64",
        0xaa64 => "arm64",
        0x01c0 | 0x01c4 => "arm",
        0x0200 => "ia64",
        _ => "unknown",
    }
}

/// A StringFileInfo value (`OriginalFilename`, `ProductName`, ...) found by its UTF-16LE key.
pub fn version_string(d: &[u8], key: &str) -> Option<String> {
    let mut k: Vec<u8> = key.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    k.extend_from_slice(&[0, 0]);
    let i = find(d, &k)?;
    let mut p = i + k.len();
    while d.get(p..p + 2) == Some(&[0, 0]) {
        p += 2;
    }
    let mut e = p;
    while e + 2 <= d.len() && d[e..e + 2] != [0, 0] {
        e += 2;
    }
    Some(utf16le(&d[p..e]))
}
