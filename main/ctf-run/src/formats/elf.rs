//! ELF: header, section headers (with names), and symbol tables.

use super::bytes::{Endian, cstr};

#[derive(Clone, Debug)]
pub struct Section {
    pub name: String,
    pub kind: u32,
    pub flags: u64,
    pub addr: u64,
    pub offset: u64,
    pub size: u64,
    pub link: u32,
    pub entsize: u64,
}

#[derive(Clone, Debug)]
pub struct Symbol {
    pub name: String,
    pub value: u64,
    pub size: u64,
    pub info: u8,
    pub shndx: u16,
}

#[derive(Clone, Debug)]
pub struct Elf {
    /// 1 = 32-bit, 2 = 64-bit.
    pub class: u8,
    pub endian: Endian,
    pub osabi: u8,
    /// e_type: 1 REL, 2 EXEC, 3 DYN, 4 CORE.
    pub kind: u16,
    pub machine: u16,
    pub entry: u64,
    pub sections: Vec<Section>,
}

impl Elf {
    pub fn parse(d: &[u8]) -> Option<Elf> {
        if !d.starts_with(b"\x7fELF") {
            return None;
        }
        let class = d[4];
        let endian = if d[5] == 2 { Endian::Big } else { Endian::Little };
        let e = endian;
        let wide = class == 2;
        let kind = e.u16(d, 16)?;
        let machine = e.u16(d, 18)?;
        let (entry, shoff, shentsize, shnum, shstrndx) = if wide {
            (e.u64(d, 24)?, e.u64(d, 40)?, e.u16(d, 58)?, e.u16(d, 60)?, e.u16(d, 62)?)
        } else {
            (e.u32(d, 24)? as u64, e.u32(d, 32)? as u64, e.u16(d, 46)?, e.u16(d, 48)?, e.u16(d, 50)?)
        };
        let mut sections = Vec::new();
        let mut names = Vec::new();
        for i in 0..shnum as usize {
            let s = shoff as usize + i * shentsize as usize;
            let sec = if wide {
                Section {
                    name: String::new(),
                    kind: e.u32(d, s + 4)?,
                    flags: e.u64(d, s + 8)?,
                    addr: e.u64(d, s + 16)?,
                    offset: e.u64(d, s + 24)?,
                    size: e.u64(d, s + 32)?,
                    link: e.u32(d, s + 40)?,
                    entsize: e.u64(d, s + 56)?,
                }
            } else {
                Section {
                    name: String::new(),
                    kind: e.u32(d, s + 4)?,
                    flags: e.u32(d, s + 8)? as u64,
                    addr: e.u32(d, s + 12)? as u64,
                    offset: e.u32(d, s + 16)? as u64,
                    size: e.u32(d, s + 20)? as u64,
                    link: e.u32(d, s + 24)?,
                    entsize: e.u32(d, s + 36)? as u64,
                }
            };
            names.push(e.u32(d, s)?);
            sections.push(sec);
        }
        if let Some(strtab) = sections.get(shstrndx as usize).map(|s| s.offset as usize) {
            for (s, n) in sections.iter_mut().zip(names) {
                s.name = cstr(d, strtab + n as usize).map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
            }
        }
        Some(Elf { class, endian, osabi: d[7], kind, machine, entry, sections })
    }

    pub fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.name == name)
    }

    pub fn data<'a>(&self, d: &'a [u8], s: &Section) -> Option<&'a [u8]> {
        if s.kind == 8 {
            return Some(&[]); // SHT_NOBITS
        }
        d.get(s.offset as usize..(s.offset + s.size) as usize)
    }

    /// Symbols of the first section of type `kind` (2 = SHT_SYMTAB, 11 = SHT_DYNSYM), in table order.
    pub fn symbols(&self, d: &[u8], kind: u32) -> Option<Vec<Symbol>> {
        let st = self.sections.iter().find(|s| s.kind == kind)?;
        let strtab = self.sections.get(st.link as usize)?;
        let data = self.data(d, st)?;
        let e = self.endian;
        let ent = if st.entsize > 0 { st.entsize as usize } else if self.class == 2 { 24 } else { 16 };
        let mut out = Vec::new();
        for i in 0..data.len() / ent {
            let p = i * ent;
            let (name, info, shndx, value, size) = if self.class == 2 {
                (e.u32(data, p)?, data[p + 4], e.u16(data, p + 6)?, e.u64(data, p + 8)?, e.u64(data, p + 16)?)
            } else {
                (e.u32(data, p)?, data[p + 12], e.u16(data, p + 14)?, e.u32(data, p + 4)? as u64, e.u32(data, p + 8)? as u64)
            };
            let name = cstr(d, strtab.offset as usize + name as usize).map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
            out.push(Symbol { name, value, size, info, shndx });
        }
        Some(out)
    }

    /// The virtual address -> file offset mapping through the section table.
    pub fn vaddr_to_offset(&self, va: u64) -> Option<usize> {
        self.sections
            .iter()
            .find(|s| s.addr != 0 && s.kind != 8 && va >= s.addr && va < s.addr + s.size)
            .map(|s| (va - s.addr + s.offset) as usize)
    }
}

/// e_machine names as `readelf`/`file` print them.
pub fn machine_name(m: u16) -> &'static str {
    match m {
        3 => "x86",
        62 => "x86-64",
        40 => "arm",
        183 => "aarch64",
        243 => "riscv",
        8 => "mips",
        20 => "ppc",
        21 => "ppc64",
        _ => "unknown",
    }
}
