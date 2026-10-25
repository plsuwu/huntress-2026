//! Windows registry hives (`regf`): the 4096-byte base block (sequence numbers, timestamp, version, root
//! cell offset, embedded file name, XOR checksum), the `hbin` bins and their cells, and the key tree
//! (`nk` keys, `vk` values, `lf`/`lh`/`li`/`ri` subkey lists). The root key is the `nk` cell the base
//! block's root offset names; a hive whose file was renamed still carries its true root name there.

use super::bytes::{u16le, u32le, u64le, utf16le};

pub const MAGIC: &[u8; 4] = b"regf";
const BASE: usize = 0x1000;
/// `nk` flags: the hive's root key, and an ASCII (Latin-1) name.
pub const KEY_HIVE_ENTRY: u16 = 0x0004;
pub const KEY_COMP_NAME: u16 = 0x0020;

#[derive(Clone, Debug)]
pub struct BaseBlock {
    pub primary_seq: u32,
    pub secondary_seq: u32,
    pub last_written: u64,
    pub major: u32,
    pub minor: u32,
    pub file_type: u32,
    pub file_format: u32,
    pub root_cell: u32,
    pub hive_bins_size: u32,
    pub clustering: u32,
    /// UTF-16LE at 0x30 (64 bytes, often only the tail of the original path).
    pub file_name: String,
    pub checksum: u32,
    pub checksum_ok: bool,
}

#[derive(Clone, Debug)]
pub struct Key {
    /// Absolute file offset of the cell (size field).
    pub offset: usize,
    pub flags: u16,
    pub last_written: u64,
    pub parent: u32,
    pub subkey_count: u32,
    pub subkey_list: u32,
    pub value_count: u32,
    pub value_list: u32,
    pub security: u32,
    pub class_name: u32,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Val {
    pub name: String,
    pub kind: u32,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Hive {
    pub base: BaseBlock,
    pub bins: Vec<(usize, u32)>,
    pub len: usize,
    data: Vec<u8>,
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

impl Hive {
    pub fn parse(b: &[u8]) -> Option<Hive> {
        if !b.starts_with(MAGIC) || b.len() < 0x200 {
            return None;
        }
        let x = (0..508).step_by(4).fold(0u32, |a, o| a ^ u32le(b, o).unwrap_or(0));
        let want = match x {
            0 => 1,
            0xffff_ffff => 0xffff_fffe,
            v => v,
        };
        let name_raw = &b[0x30..0x70];
        let n = (0..name_raw.len() / 2).find(|&i| name_raw[2 * i] == 0 && name_raw[2 * i + 1] == 0).unwrap_or(name_raw.len() / 2);
        let base = BaseBlock {
            primary_seq: u32le(b, 4)?,
            secondary_seq: u32le(b, 8)?,
            last_written: u64le(b, 12)?,
            major: u32le(b, 20)?,
            minor: u32le(b, 24)?,
            file_type: u32le(b, 28)?,
            file_format: u32le(b, 32)?,
            root_cell: u32le(b, 36)?,
            hive_bins_size: u32le(b, 40)?,
            clustering: u32le(b, 44)?,
            file_name: utf16le(&name_raw[..2 * n]),
            checksum: u32le(b, 508)?,
            checksum_ok: u32le(b, 508)? == want,
        };
        let mut bins = Vec::new();
        let mut o = BASE;
        while o + 32 <= b.len() && &b[o..o + 4] == b"hbin" {
            let size = u32le(b, o + 8)?;
            bins.push((o, size));
            if size < 0x1000 {
                break;
            }
            o += size as usize;
        }
        Some(Hive { base, bins, len: b.len(), data: b.to_vec() })
    }

    /// Cell body (after the 4-byte size) at a hive-bins offset; allocated cells have a negative size.
    pub fn cell(&self, off: u32) -> Option<&[u8]> {
        if off == u32::MAX {
            return None;
        }
        let at = BASE + off as usize;
        let size = u32le(&self.data, at)? as i32;
        let len = size.unsigned_abs() as usize;
        if len < 8 {
            return None;
        }
        self.data.get(at + 4..at + len)
    }

    pub fn key(&self, off: u32) -> Option<Key> {
        let c = self.cell(off)?;
        if c.get(..2)? != b"nk" {
            return None;
        }
        let flags = u16le(c, 2)?;
        let n = u16le(c, 0x48)? as usize;
        let raw = c.get(0x4c..0x4c + n)?;
        let name = if flags & KEY_COMP_NAME != 0 { latin1(raw) } else { utf16le(raw) };
        Some(Key {
            offset: BASE + off as usize,
            flags,
            last_written: u64le(c, 4)?,
            parent: u32le(c, 0x10)?,
            subkey_count: u32le(c, 0x14)?,
            subkey_list: u32le(c, 0x1c)?,
            value_count: u32le(c, 0x24)?,
            value_list: u32le(c, 0x28)?,
            security: u32le(c, 0x2c)?,
            class_name: u32le(c, 0x30)?,
            name,
        })
    }

    /// The key at the base block's root offset.
    pub fn root(&self) -> Option<Key> {
        self.key(self.base.root_cell)
    }

    /// Every allocated `nk` cell in the bins flagged as a hive root (for cross-checks).
    pub fn hive_entry_keys(&self) -> Vec<Key> {
        let mut out = Vec::new();
        for &(o, size) in &self.bins {
            let end = (o + size as usize).min(self.data.len());
            let mut p = o + 32;
            while p + 4 <= end {
                let s = u32le(&self.data, p).unwrap_or(0) as i32;
                let len = s.unsigned_abs() as usize;
                if len < 8 {
                    break;
                }
                if s < 0 {
                    if let Some(k) = self.key((p - BASE) as u32).filter(|k| k.flags & KEY_HIVE_ENTRY != 0) {
                        out.push(k);
                    }
                }
                p += len;
            }
        }
        out
    }

    pub fn subkeys(&self, k: &Key) -> Vec<Key> {
        let mut out = Vec::new();
        self.list(k.subkey_list, &mut out, 0);
        out
    }

    fn list(&self, off: u32, out: &mut Vec<Key>, depth: usize) {
        let Some(c) = self.cell(off) else { return };
        if depth > 4 || c.len() < 4 {
            return;
        }
        let n = u16le(c, 2).unwrap_or(0) as usize;
        let step = match &c[..2] {
            b"lf" | b"lh" => 8,
            b"li" | b"ri" => 4,
            _ => return,
        };
        for i in 0..n {
            let Some(e) = u32le(c, 4 + i * step) else { break };
            if &c[..2] == b"ri" {
                self.list(e, out, depth + 1);
            } else if let Some(k) = self.key(e) {
                out.push(k);
            }
        }
    }

    pub fn values(&self, k: &Key) -> Vec<Val> {
        let Some(list) = self.cell(k.value_list).filter(|_| k.value_count > 0) else { return Vec::new() };
        (0..k.value_count as usize).filter_map(|i| u32le(list, i * 4)).filter_map(|o| self.value(o)).collect()
    }

    fn value(&self, off: u32) -> Option<Val> {
        let c = self.cell(off)?;
        if c.get(..2)? != b"vk" {
            return None;
        }
        let n = u16le(c, 2)? as usize;
        let size = u32le(c, 4)?;
        let data_off = u32le(c, 8)?;
        let kind = u32le(c, 12)?;
        let flags = u16le(c, 16)?;
        let raw = c.get(20..20 + n)?;
        let name = if flags & 1 != 0 { latin1(raw) } else { utf16le(raw) };
        let len = (size & 0x7fff_ffff) as usize;
        let data = if size & 0x8000_0000 != 0 {
            data_off.to_le_bytes()[..len.min(4)].to_vec()
        } else {
            self.cell(data_off).map(|d| d[..len.min(d.len())].to_vec()).unwrap_or_default()
        };
        Some(Val { name, kind, data })
    }

    /// Every key below (and including) `k`, depth-first, as (path, key).
    pub fn walk(&self, k: &Key) -> Vec<(String, Key)> {
        let mut out = vec![(k.name.clone(), k.clone())];
        let mut i = 0;
        while i < out.len() && out.len() < 100_000 {
            let (path, key) = out[i].clone();
            for s in self.subkeys(&key) {
                if !out.iter().any(|(_, o)| o.offset == s.offset) {
                    out.push((format!("{path}\\{}", s.name), s));
                }
            }
            i += 1;
        }
        out
    }
}

/// Value data as text: strings (REG_SZ / EXPAND_SZ / MULTI_SZ joined by `\n`), DWORD/QWORD in decimal,
/// anything else as lowercase hex.
pub fn value_text(v: &Val) -> String {
    match v.kind {
        1 | 2 => utf16le(&v.data).trim_end_matches('\0').to_string(),
        7 => utf16le(&v.data).split('\0').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n"),
        4 if v.data.len() >= 4 => u32le(&v.data, 0).unwrap().to_string(),
        11 if v.data.len() >= 8 => u64le(&v.data, 0).unwrap().to_string(),
        _ => super::bytes::hex(&v.data),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A base block + one bin holding a root `nk` with one inline DWORD value.
    fn hive() -> Vec<u8> {
        let mut f = vec![0u8; 0x2000];
        f[..4].copy_from_slice(MAGIC);
        f[36..40].copy_from_slice(&0x20u32.to_le_bytes());
        f[0x30..0x36].copy_from_slice(&[b'a', 0, b'.', 0, b'h', 0]);
        let x = (0..508).step_by(4).fold(0u32, |a, o| a ^ u32le(&f, o).unwrap());
        f[508..512].copy_from_slice(&x.to_le_bytes());
        f[0x1000..0x1004].copy_from_slice(b"hbin");
        f[0x1008..0x100c].copy_from_slice(&0x1000u32.to_le_bytes());
        let nk = 0x1020;
        f[nk..nk + 4].copy_from_slice(&(-0x58i32).to_le_bytes());
        f[nk + 4..nk + 6].copy_from_slice(b"nk");
        f[nk + 6..nk + 8].copy_from_slice(&0x2cu16.to_le_bytes());
        f[nk + 4 + 0x1c..nk + 4 + 0x20].copy_from_slice(&u32::MAX.to_le_bytes());
        f[nk + 4 + 0x24..nk + 4 + 0x28].copy_from_slice(&1u32.to_le_bytes());
        f[nk + 4 + 0x28..nk + 4 + 0x2c].copy_from_slice(&0x80u32.to_le_bytes());
        f[nk + 4 + 0x48..nk + 4 + 0x4a].copy_from_slice(&4u16.to_le_bytes());
        f[nk + 4 + 0x4c..nk + 4 + 0x50].copy_from_slice(b"Root");
        let vl = 0x1080;
        f[vl..vl + 4].copy_from_slice(&(-8i32).to_le_bytes());
        f[vl + 4..vl + 8].copy_from_slice(&0x88u32.to_le_bytes());
        let vk = 0x1088;
        f[vk..vk + 4].copy_from_slice(&(-0x20i32).to_le_bytes());
        f[vk + 4..vk + 6].copy_from_slice(b"vk");
        f[vk + 6..vk + 8].copy_from_slice(&1u16.to_le_bytes());
        f[vk + 8..vk + 12].copy_from_slice(&0x8000_0004u32.to_le_bytes());
        f[vk + 12..vk + 16].copy_from_slice(&7u32.to_le_bytes());
        f[vk + 16..vk + 20].copy_from_slice(&4u32.to_le_bytes());
        f[vk + 20..vk + 22].copy_from_slice(&1u16.to_le_bytes());
        f[vk + 24] = b'N';
        f
    }

    #[test]
    fn root_and_values() {
        let h = Hive::parse(&hive()).unwrap();
        assert!(h.base.checksum_ok);
        assert_eq!(h.base.file_name, "a.h");
        let r = h.root().unwrap();
        assert_eq!((r.name.as_str(), r.flags & KEY_HIVE_ENTRY != 0), ("Root", true));
        let v = h.values(&r);
        assert_eq!((v.len(), v[0].name.as_str(), value_text(&v[0])), (1, "N", "7".to_string()));
        assert_eq!(h.hive_entry_keys().len(), 1);
    }
}
