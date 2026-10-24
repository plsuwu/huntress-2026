//! Windows XML event logs (`.evtx`): the file header (`ElfFile\0`, 4096 bytes), 64 KiB chunks (`ElfChnk\0`,
//! 512-byte header with CRC32s) and the event records they hold (`**\0\0`, size, record id, FILETIME, BinXML,
//! trailing size copy). Records are walked, not trusted from headers: a truncated file's headers still claim
//! the records it lost, so a record counts only when it is complete and its size copy matches.

use crate::crypto::crc32::crc32;

pub const FILE_MAGIC: &[u8; 8] = b"ElfFile\0";
pub const CHUNK_MAGIC: &[u8; 8] = b"ElfChnk\0";
pub const CHUNK_SIZE: usize = 0x10000;
const CHUNK_HEADER: usize = 0x200;

fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn u64le(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

#[derive(Clone, Debug)]
pub struct FileHeader {
    pub first_chunk: u64,
    pub last_chunk: u64,
    pub next_record_id: u64,
    pub header_size: u32,
    pub minor: u16,
    pub major: u16,
    pub header_block_size: u16,
    pub chunk_count: u16,
    pub flags: u32,
    pub checksum: u32,
    pub checksum_ok: bool,
}

#[derive(Clone, Debug)]
pub struct Chunk {
    pub index: usize,
    pub offset: usize,
    pub first_record_number: u64,
    pub last_record_number: u64,
    pub first_record_id: u64,
    pub last_record_id: u64,
    pub header_size: u32,
    pub last_record_offset: u32,
    pub free_space_offset: u32,
    pub records_checksum: u32,
    pub header_checksum: u32,
    pub header_checksum_ok: bool,
    /// `None` when the chunk's record area is cut off by the end of the file.
    pub records_checksum_ok: Option<bool>,
    /// Bytes of this chunk present in the file (< 65536 for a truncated tail chunk).
    pub present: usize,
}

#[derive(Clone, Debug)]
pub struct Record {
    pub chunk: usize,
    pub offset: usize,
    pub size: u32,
    pub id: u64,
    pub filetime: u64,
}

#[derive(Clone, Debug)]
pub struct Evtx {
    pub header: Option<FileHeader>,
    pub chunks: Vec<Chunk>,
    pub records: Vec<Record>,
    /// Bytes after the last complete record of the last chunk that hold a record magic but no complete record.
    pub partial_records: usize,
    pub len: usize,
}

impl Evtx {
    pub fn parse(b: &[u8]) -> Option<Evtx> {
        if !b.starts_with(FILE_MAGIC) && !b.starts_with(CHUNK_MAGIC) {
            return None;
        }
        let header = (b.len() >= 0x80 && b.starts_with(FILE_MAGIC)).then(|| FileHeader {
            first_chunk: u64le(b, 8),
            last_chunk: u64le(b, 16),
            next_record_id: u64le(b, 24),
            header_size: u32le(b, 32),
            minor: u16le(b, 36),
            major: u16le(b, 38),
            header_block_size: u16le(b, 40),
            chunk_count: u16le(b, 42),
            flags: u32le(b, 0x78),
            checksum: u32le(b, 0x7c),
            checksum_ok: crc32(&b[..0x78]) == u32le(b, 0x7c),
        });
        let base = match &header {
            Some(h) if h.header_block_size as usize >= 0x80 => h.header_block_size as usize,
            Some(_) => 0x1000,
            None => 0,
        };
        let (mut chunks, mut records, mut partial_records) = (Vec::new(), Vec::new(), 0);
        let mut o = base;
        while o + CHUNK_HEADER <= b.len() {
            if &b[o..o + 8] != CHUNK_MAGIC {
                o += CHUNK_SIZE;
                continue;
            }
            let end = (o + CHUNK_SIZE).min(b.len());
            let c = &b[o..end];
            let free = u32le(c, 48);
            let mut hc = crc32fast::Hasher::new();
            hc.update(&c[..0x78]);
            hc.update(&c[0x80..CHUNK_HEADER]);
            let records_checksum_ok = ((free as usize) <= c.len() && free as usize >= CHUNK_HEADER).then(|| crc32(&c[CHUNK_HEADER..free as usize]) == u32le(c, 52));
            let index = chunks.len();
            chunks.push(Chunk {
                index,
                offset: o,
                first_record_number: u64le(c, 8),
                last_record_number: u64le(c, 16),
                first_record_id: u64le(c, 24),
                last_record_id: u64le(c, 32),
                header_size: u32le(c, 40),
                last_record_offset: u32le(c, 44),
                free_space_offset: free,
                records_checksum: u32le(c, 52),
                header_checksum: u32le(c, 0x7c),
                header_checksum_ok: hc.finalize() == u32le(c, 0x7c),
                records_checksum_ok,
                present: c.len(),
            });
            let mut p = CHUNK_HEADER;
            while p + 4 <= c.len() && &c[p..p + 4] == b"**\0\0" {
                if p + 24 > c.len() {
                    partial_records += 1;
                    break;
                }
                let size = u32le(c, p + 4) as usize;
                if size < 28 || p + size > c.len() || u32le(c, p + size - 4) as usize != size {
                    partial_records += 1;
                    break;
                }
                records.push(Record { chunk: index, offset: o + p, size: size as u32, id: u64le(c, p + 8), filetime: u64le(c, p + 16) });
                p += size;
            }
            o += CHUNK_SIZE;
        }
        Some(Evtx { header, chunks, records, partial_records, len: b.len() })
    }

    /// Distinct record ids (a duplicated chunk repeats its records).
    pub fn distinct_ids(&self) -> Vec<u64> {
        let mut v: Vec<u64> = self.records.iter().map(|r| r.id).collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Records the chunk headers claim (`last - first + 1` per chunk).
    pub fn claimed_records(&self) -> u64 {
        self.chunks.iter().map(|c| (c.last_record_id + 1).saturating_sub(c.first_record_id)).sum()
    }

    /// The file is shorter than its header's chunk count says, or a chunk / record is cut off.
    pub fn truncated(&self) -> bool {
        let want = self.header.as_ref().map_or(0, |h| h.header_block_size.max(0x80) as usize + h.chunk_count as usize * CHUNK_SIZE);
        self.len < want || self.chunks.iter().any(|c| c.present < CHUNK_SIZE) || self.partial_records > 0
    }
}

/// FILETIME (100 ns since 1601) -> `YYYY-MM-DDTHH:MM:SS.fffffffZ` (all seven fraction digits).
pub fn filetime_iso(ft: u64) -> String {
    let secs = (ft / 10_000_000) as i64 - 11_644_473_600;
    let frac = ft % 10_000_000;
    let s = crate::time::iso(secs * 1000);
    format!("{}.{frac:07}Z", &s[..19])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetime() {
        assert_eq!(filetime_iso(116_444_736_000_000_000), "1970-01-01T00:00:00.0000000Z");
        assert_eq!(filetime_iso(116_444_736_000_000_000 + 12_345_678), "1970-01-01T00:00:01.2345678Z");
    }

    #[test]
    fn walks_and_truncates() {
        let mut f = vec![0u8; 0x1000];
        f[..8].copy_from_slice(FILE_MAGIC);
        f[40..42].copy_from_slice(&0x1000u16.to_le_bytes());
        f[42..44].copy_from_slice(&1u16.to_le_bytes());
        let mut c = vec![0u8; CHUNK_SIZE];
        c[..8].copy_from_slice(CHUNK_MAGIC);
        let mut p = CHUNK_HEADER;
        for id in 1..=3u64 {
            c[p..p + 4].copy_from_slice(b"**\0\0");
            c[p + 4..p + 8].copy_from_slice(&40u32.to_le_bytes());
            c[p + 8..p + 16].copy_from_slice(&id.to_le_bytes());
            c[p + 36..p + 40].copy_from_slice(&40u32.to_le_bytes());
            p += 40;
        }
        c[24..32].copy_from_slice(&1u64.to_le_bytes());
        c[32..40].copy_from_slice(&3u64.to_le_bytes());
        f.extend_from_slice(&c);
        let e = Evtx::parse(&f).unwrap();
        assert_eq!((e.records.len(), e.claimed_records(), e.truncated()), (3, 3, false));
        f.truncate(0x1000 + CHUNK_HEADER + 100);
        let e = Evtx::parse(&f).unwrap();
        assert_eq!((e.records.len(), e.claimed_records(), e.partial_records, e.truncated()), (2, 3, 1, true));
    }
}
