//! Mach-O headers (thin and fat), enough to identify architecture and segments/sections.

use super::bytes::{Endian, cstr};

#[derive(Clone, Debug)]
pub struct MachO {
    pub bits: u8,
    pub cputype: u32,
    pub filetype: u32,
    /// (segment, section) names in load-command order.
    pub sections: Vec<(String, String)>,
}

const MH_MAGIC: u32 = 0xFEED_FACE;
const MH_MAGIC_64: u32 = 0xFEED_FACF;

pub fn parse(d: &[u8]) -> Option<MachO> {
    let le = u32::from_le_bytes(d.get(..4)?.try_into().ok()?);
    let (e, bits) = match le {
        MH_MAGIC => (Endian::Little, 32),
        MH_MAGIC_64 => (Endian::Little, 64),
        x if x.swap_bytes() == MH_MAGIC => (Endian::Big, 32),
        x if x.swap_bytes() == MH_MAGIC_64 => (Endian::Big, 64),
        _ => return fat_first(d),
    };
    let cputype = e.u32(d, 4)?;
    let filetype = e.u32(d, 12)?;
    let ncmds = e.u32(d, 16)? as usize;
    let mut p = if bits == 64 { 32 } else { 28 };
    let mut sections = Vec::new();
    for _ in 0..ncmds.min(4096) {
        let cmd = e.u32(d, p)?;
        let size = e.u32(d, p + 4)? as usize;
        if cmd == 0x19 || cmd == 0x1 {
            let (nsects_off, hdr, sz) = if cmd == 0x19 { (64, 72, 80) } else { (48, 56, 68) };
            let n = e.u32(d, p + nsects_off)? as usize;
            for s in 0..n.min(1024) {
                let q = p + hdr + s * sz;
                let sect = String::from_utf8_lossy(cstr(d.get(q..q + 16)?, 0)?).into_owned();
                let seg = String::from_utf8_lossy(cstr(d.get(q + 16..q + 32)?, 0)?).into_owned();
                sections.push((seg, sect));
            }
        }
        if size == 0 {
            break;
        }
        p += size;
    }
    Some(MachO { bits, cputype, filetype, sections })
}

/// A fat (universal) binary: the first architecture's slice. Java class files share the magic, so
/// require a small architecture count.
fn fat_first(d: &[u8]) -> Option<MachO> {
    let e = Endian::Big;
    if e.u32(d, 0)? != 0xCAFE_BABE {
        return None;
    }
    let n = e.u32(d, 4)?;
    if n == 0 || n > 16 {
        return None;
    }
    let off = e.u32(d, 16)? as usize;
    let size = e.u32(d, 20)? as usize;
    parse(d.get(off..off + size)?)
}

pub fn is_macho(d: &[u8]) -> bool {
    parse(d).is_some()
}

pub fn cpu_name(cputype: u32) -> &'static str {
    match cputype {
        0x0100_0007 => "x86-64",
        7 => "x86",
        0x0100_000C => "arm64",
        12 => "arm",
        _ => "unknown",
    }
}
