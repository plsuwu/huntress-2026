//! Raw disk images: MBR (four primary entries + the extended/EBR chain) and GPT (primary header at LBA 1,
//! entry array, backup header at the header's alternate LBA). The sector size is 512 unless a GPT header
//! is found at 4096 instead. Partitions are kept in table order; empty entries are skipped.

use super::bytes::{u32le, u64le, utf16le};
use super::dotnet::guid_le;
use crate::crypto::crc32::crc32;

#[derive(Clone, Debug)]
pub struct Partition {
    /// 1-based slot in its table (GPT entry index, MBR slot; logical partitions continue from 5).
    pub index: usize,
    pub start_lba: u64,
    /// Inclusive last LBA.
    pub end_lba: u64,
    /// GPT type GUID (lowercase) or MBR type byte (`0x07`).
    pub kind: String,
    pub guid: Option<String>,
    pub name: String,
    pub attributes: u64,
    pub bootable: bool,
}

impl Partition {
    pub fn sectors(&self) -> u64 {
        self.end_lba + 1 - self.start_lba
    }
}

#[derive(Clone, Debug)]
pub struct GptHeader {
    pub lba: u64,
    pub revision: u32,
    pub header_size: u32,
    pub header_crc_ok: bool,
    pub current_lba: u64,
    pub backup_lba: u64,
    pub first_usable: u64,
    pub last_usable: u64,
    pub disk_guid: String,
    pub entries_lba: u64,
    pub entry_count: u32,
    pub entry_size: u32,
    pub entries_crc: u32,
    pub entries_crc_ok: bool,
}

#[derive(Debug)]
pub struct Disk {
    pub sector: usize,
    pub len: usize,
    pub mbr_signature: bool,
    pub mbr_disk_signature: u32,
    /// Raw MBR entries (type byte, start, sectors) including the protective 0xEE one.
    pub mbr: Vec<Partition>,
    pub gpt: Option<GptHeader>,
    pub backup: Option<GptHeader>,
    /// GPT partitions from the primary array (else the backup's), else MBR partitions.
    pub partitions: Vec<Partition>,
    pub backup_partitions: Vec<Partition>,
}

fn header(b: &[u8], sector: usize, lba: u64) -> Option<GptHeader> {
    let o = (lba as usize).checked_mul(sector)?;
    if b.get(o..o + 8)? != b"EFI PART" {
        return None;
    }
    let size = u32le(b, o + 12)?;
    let mut h = b.get(o..o + (size as usize).clamp(92, sector))?.to_vec();
    let crc = u32le(&h, 16)?;
    h[16..20].fill(0);
    let entries_lba = u64le(b, o + 72)?;
    let (count, esize) = (u32le(b, o + 80)?, u32le(b, o + 84)?);
    let ecrc = u32le(b, o + 88)?;
    let eo = (entries_lba as usize).saturating_mul(sector);
    let elen = (count as usize).saturating_mul(esize as usize);
    let entries_crc_ok = b.get(eo..eo.saturating_add(elen)).is_some_and(|e| crc32(e) == ecrc);
    let g: [u8; 16] = b.get(o + 56..o + 72)?.try_into().ok()?;
    Some(GptHeader {
        lba,
        revision: u32le(b, o + 8)?,
        header_size: size,
        header_crc_ok: crc32(&h) == crc,
        current_lba: u64le(b, o + 24)?,
        backup_lba: u64le(b, o + 32)?,
        first_usable: u64le(b, o + 40)?,
        last_usable: u64le(b, o + 48)?,
        disk_guid: guid_le(&g, false),
        entries_lba,
        entry_count: count,
        entry_size: esize,
        entries_crc: ecrc,
        entries_crc_ok,
    })
}

fn gpt_entries(b: &[u8], sector: usize, h: &GptHeader) -> Vec<Partition> {
    let mut out = Vec::new();
    let base = (h.entries_lba as usize).saturating_mul(sector);
    for i in 0..h.entry_count.min(4096) as usize {
        let o = base + i * h.entry_size as usize;
        let Some(e) = b.get(o..o + 128) else { break };
        if e[..16].iter().all(|&x| x == 0) {
            continue;
        }
        let t: [u8; 16] = e[..16].try_into().unwrap();
        let u: [u8; 16] = e[16..32].try_into().unwrap();
        let name = utf16le(&e[56..128]);
        let name = name.split('\0').next().unwrap_or("").to_string();
        let attributes = u64le(e, 48).unwrap_or(0);
        out.push(Partition {
            index: i + 1,
            start_lba: u64le(e, 32).unwrap_or(0),
            end_lba: u64le(e, 40).unwrap_or(0),
            kind: guid_le(&t, false),
            guid: Some(guid_le(&u, false)),
            name,
            attributes,
            bootable: attributes & 4 != 0,
        });
    }
    out
}

fn mbr_entry(b: &[u8], o: usize, index: usize, base: u64) -> Option<Partition> {
    let kind = *b.get(o + 4)?;
    let start = u32le(b, o + 8)? as u64;
    let n = u32le(b, o + 12)? as u64;
    if kind == 0 || n == 0 {
        return None;
    }
    Some(Partition {
        index,
        start_lba: base + start,
        end_lba: base + start + n - 1,
        kind: format!("0x{kind:02x}"),
        guid: None,
        name: String::new(),
        attributes: 0,
        bootable: b[o] == 0x80,
    })
}

fn is_extended(k: &str) -> bool {
    matches!(k, "0x05" | "0x0f" | "0x85")
}

impl Disk {
    pub fn parse(b: &[u8]) -> Option<Disk> {
        if b.len() < 1024 {
            return None;
        }
        let mbr_signature = b[510..512] == [0x55, 0xAA];
        let sector = if header(b, 512, 1).is_some() { 512 } else if header(b, 4096, 1).is_some() { 4096 } else { 512 };
        let gpt = header(b, sector, 1);
        if !mbr_signature && gpt.is_none() {
            return None;
        }
        let mut mbr = Vec::new();
        for s in 0..4 {
            if let Some(p) = mbr_entry(b, 446 + 16 * s, s + 1, 0) {
                mbr.push(p);
            }
        }
        // Logical partitions: EBR chain, offsets relative to the extended partition's start.
        let mut logical = Vec::new();
        if let Some(ext) = mbr.iter().find(|p| is_extended(&p.kind)) {
            let mut ebr = ext.start_lba;
            for n in 0..128 {
                let o = ebr as usize * sector;
                if b.get(o + 510..o + 512) != Some(&[0x55, 0xAA]) {
                    break;
                }
                if let Some(p) = mbr_entry(b, o + 446, 5 + n, ebr) {
                    logical.push(p);
                }
                match mbr_entry(b, o + 462, 0, ext.start_lba) {
                    Some(next) if is_extended(&next.kind) && next.start_lba != ebr => ebr = next.start_lba,
                    _ => break,
                }
            }
        }
        let last = (b.len() / sector).saturating_sub(1) as u64;
        let backup = gpt.as_ref().map(|h| h.backup_lba).and_then(|l| header(b, sector, l)).or_else(|| header(b, sector, last));
        let primary_parts = gpt.as_ref().map(|h| gpt_entries(b, sector, h)).unwrap_or_default();
        let backup_partitions = backup.as_ref().map(|h| gpt_entries(b, sector, h)).unwrap_or_default();
        let partitions = if gpt.is_some() {
            primary_parts
        } else if backup.is_some() {
            backup_partitions.clone()
        } else {
            mbr.iter().filter(|p| !is_extended(&p.kind)).cloned().chain(logical).collect()
        };
        Some(Disk {
            sector,
            len: b.len(),
            mbr_signature,
            mbr_disk_signature: u32le(b, 440).unwrap_or(0),
            mbr,
            gpt,
            backup,
            partitions,
            backup_partitions,
        })
    }

    pub fn scheme(&self) -> &'static str {
        if self.gpt.is_some() || self.backup.is_some() {
            "gpt"
        } else {
            "mbr"
        }
    }

    pub fn data<'a>(&self, b: &'a [u8], p: &Partition) -> &'a [u8] {
        let s = (p.start_lba as usize).saturating_mul(self.sector).min(b.len());
        let e = ((p.end_lba as usize).saturating_add(1)).saturating_mul(self.sector).min(b.len());
        &b[s..e.max(s)]
    }
}
