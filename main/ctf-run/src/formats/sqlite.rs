//! SQLite 3 database files: the 100-byte header (page geometry, counters, encoding, application id),
//! an embedded database's offset (the first `SQLite format 3\0` magic), and the schema table
//! (`sqlite_master`) plus row walks of any table b-tree (overflow chains followed, truncated pages skipped).

pub const MAGIC: &[u8; 16] = b"SQLite format 3\0";

#[derive(Clone, Debug)]
pub struct Header {
    /// File offset of the magic (0 unless embedded in another file).
    pub offset: usize,
    /// Page size in bytes (the stored 1 means 65536).
    pub page_size: u32,
    pub page_size_raw: u16,
    pub write_version: u8,
    pub read_version: u8,
    pub reserved: u8,
    pub max_payload_fraction: u8,
    pub min_payload_fraction: u8,
    pub leaf_payload_fraction: u8,
    pub change_counter: u32,
    /// The header's "in-header database size" in pages.
    pub header_pages: u32,
    pub freelist_trunk: u32,
    pub freelist_count: u32,
    pub schema_cookie: u32,
    pub schema_format: u32,
    pub default_cache_size: u32,
    pub largest_root: u32,
    pub text_encoding: u32,
    pub user_version: u32,
    pub incremental_vacuum: u32,
    pub application_id: u32,
    pub version_valid_for: u32,
    pub sqlite_version: u32,
    /// Bytes from the magic to the end of the file.
    pub available: usize,
}

fn be32(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

impl Header {
    /// The header at the first magic in `b` (offset 0 preferred).
    pub fn find(b: &[u8]) -> Option<Header> {
        let off = if b.starts_with(MAGIC) { 0 } else { b.windows(16).position(|w| w == MAGIC)? };
        Header::parse_at(b, off)
    }

    pub fn parse_at(b: &[u8], off: usize) -> Option<Header> {
        let h = b.get(off..off + 100)?;
        if &h[..16] != MAGIC {
            return None;
        }
        let raw = u16::from_be_bytes([h[16], h[17]]);
        Some(Header {
            offset: off,
            page_size: if raw == 1 { 65536 } else { raw as u32 },
            page_size_raw: raw,
            write_version: h[18],
            read_version: h[19],
            reserved: h[20],
            max_payload_fraction: h[21],
            min_payload_fraction: h[22],
            leaf_payload_fraction: h[23],
            change_counter: be32(h, 24),
            header_pages: be32(h, 28),
            freelist_trunk: be32(h, 32),
            freelist_count: be32(h, 36),
            schema_cookie: be32(h, 40),
            schema_format: be32(h, 44),
            default_cache_size: be32(h, 48),
            largest_root: be32(h, 52),
            text_encoding: be32(h, 56),
            user_version: be32(h, 60),
            incremental_vacuum: be32(h, 64),
            application_id: be32(h, 68),
            version_valid_for: be32(h, 92),
            sqlite_version: be32(h, 96),
            available: b.len() - off,
        })
    }

    /// A power of two in 512..=65536 (the only sizes SQLite accepts).
    pub fn page_size_valid(&self) -> bool {
        self.page_size.is_power_of_two() && (512..=65536).contains(&self.page_size)
    }

    /// Whole pages present in the file (from the magic on).
    pub fn file_pages(&self) -> u64 {
        if self.page_size == 0 { 0 } else { self.available as u64 / self.page_size as u64 }
    }

    /// The header page count is trusted by SQLite only when nonzero and `version_valid_for` matches the change counter.
    pub fn header_pages_valid(&self) -> bool {
        self.header_pages != 0 && self.version_valid_for == self.change_counter
    }

    /// Bytes the header claims (`header_pages * page_size`).
    pub fn expected_bytes(&self) -> u64 {
        self.header_pages as u64 * self.page_size as u64
    }

    pub fn truncated(&self) -> bool {
        (self.available as u64) < self.expected_bytes()
    }

    pub fn encoding_name(&self) -> Option<&'static str> {
        match self.text_encoding {
            1 => Some("UTF-8"),
            2 => Some("UTF-16le"),
            3 => Some("UTF-16be"),
            _ => None,
        }
    }

    /// `3045001` -> `3.45.1`.
    pub fn version_string(&self) -> String {
        let v = self.sqlite_version;
        format!("{}.{}.{}", v / 1_000_000, v / 1000 % 1000, v % 1000)
    }

    /// Write/read version 2 means WAL, 1 legacy rollback journal.
    pub fn journal_mode(&self) -> &'static str {
        if self.write_version == 2 || self.read_version == 2 { "wal" } else { "delete" }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl Val {
    pub fn text(&self) -> Option<&str> {
        if let Val::Text(s) = self { Some(s) } else { None }
    }
    pub fn int(&self) -> Option<i64> {
        if let Val::Int(i) = self { Some(*i) } else { None }
    }
}

#[derive(Clone, Debug)]
pub struct SchemaEntry {
    pub kind: String,
    pub name: String,
    pub tbl_name: String,
    pub rootpage: i64,
    pub sql: String,
}

pub struct Db<'a> {
    pub h: Header,
    b: &'a [u8],
}

fn varint(b: &[u8], mut i: usize) -> Option<(u64, usize)> {
    let mut v: u64 = 0;
    for n in 0..9 {
        let c = *b.get(i)?;
        i += 1;
        if n == 8 {
            return Some(((v << 8) | c as u64, i));
        }
        v = (v << 7) | (c & 0x7f) as u64;
        if c & 0x80 == 0 {
            return Some((v, i));
        }
    }
    None
}

impl<'a> Db<'a> {
    pub fn parse(b: &'a [u8]) -> Option<Db<'a>> {
        let h = Header::find(b)?;
        let b = &b[h.offset..];
        Some(Db { h, b })
    }

    fn page(&self, n: u32) -> Option<&'a [u8]> {
        if n == 0 || !self.h.page_size_valid() {
            return None;
        }
        let ps = self.h.page_size as usize;
        self.b.get((n as usize - 1) * ps..n as usize * ps)
    }

    fn usable(&self) -> usize {
        self.h.page_size as usize - self.h.reserved as usize
    }

    /// The full payload of a table-leaf cell starting at `i` in page `p` (overflow chain followed).
    fn leaf_payload(&self, p: &[u8], i: usize) -> Option<(i64, Vec<u8>)> {
        let (len, i) = varint(p, i)?;
        let (rowid, i) = varint(p, i)?;
        let len = len as usize;
        let u = self.usable();
        let x = u - 35;
        let local = if len <= x {
            len
        } else {
            let m = (u - 12) * 32 / 255 - 23;
            let k = m + (len - m) % (u - 4);
            if k <= x { k } else { m }
        };
        let mut out = p.get(i..i + local)?.to_vec();
        if local < len {
            let mut next = p.get(i + local..i + local + 4).map(|s| be32(s, 0))?;
            let mut guard = 0;
            while out.len() < len && next != 0 && guard < 100_000 {
                let op = self.page(next)?;
                next = be32(op, 0);
                let take = (len - out.len()).min(u - 4);
                out.extend_from_slice(op.get(4..4 + take)?);
                guard += 1;
            }
        }
        Some((rowid as i64, out))
    }

    /// Rows of the table b-tree rooted at `root`, in rowid order (unreadable pages are skipped).
    pub fn rows(&self, root: u32) -> Vec<(i64, Vec<Val>)> {
        let mut out = Vec::new();
        let mut stack = vec![root];
        let mut seen = std::collections::HashSet::new();
        // Depth-first, left to right: push children in reverse.
        while let Some(n) = stack.pop() {
            if !seen.insert(n) {
                continue;
            }
            let Some(p) = self.page(n) else { continue };
            let hdr = if n == 1 { 100 } else { 0 };
            let Some(&kind) = p.get(hdr) else { continue };
            let cells = u16::from_be_bytes([p[hdr + 3], p[hdr + 4]]) as usize;
            let ptrs = hdr + if kind == 0x05 { 12 } else { 8 };
            let ptr = |c: usize| p.get(ptrs + 2 * c..ptrs + 2 * c + 2).map(|s| u16::from_be_bytes([s[0], s[1]]) as usize);
            match kind {
                0x0d => {
                    for c in 0..cells {
                        if let Some((rowid, pl)) = ptr(c).and_then(|o| self.leaf_payload(p, o)) {
                            out.push((rowid, record(&pl, self.h.text_encoding)));
                        }
                    }
                }
                0x05 => {
                    let mut kids: Vec<u32> = (0..cells).filter_map(|c| ptr(c).and_then(|o| p.get(o..o + 4)).map(|s| be32(s, 0))).collect();
                    kids.push(be32(p, hdr + 8));
                    stack.extend(kids.into_iter().rev());
                }
                _ => {}
            }
        }
        out
    }

    pub fn schema(&self) -> Vec<SchemaEntry> {
        self.rows(1)
            .into_iter()
            .map(|(_, v)| {
                let s = |i: usize| v.get(i).and_then(|x| x.text()).unwrap_or("").to_string();
                SchemaEntry { kind: s(0), name: s(1), tbl_name: s(2), rootpage: v.get(3).and_then(Val::int).unwrap_or(0), sql: s(4) }
            })
            .collect()
    }

    pub fn tables(&self) -> Vec<SchemaEntry> {
        self.schema().into_iter().filter(|e| e.kind == "table").collect()
    }
}

fn decode_text(b: &[u8], enc: u32) -> String {
    match enc {
        2 | 3 => {
            let u: Vec<u16> = b.chunks_exact(2).map(|c| if enc == 2 { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
            String::from_utf16_lossy(&u)
        }
        _ => String::from_utf8_lossy(b).into_owned(),
    }
}

/// A record (header of serial types, then values).
pub fn record(b: &[u8], enc: u32) -> Vec<Val> {
    let Some((hlen, mut i)) = varint(b, 0) else { return Vec::new() };
    let mut types = Vec::new();
    while i < hlen as usize {
        let Some((t, j)) = varint(b, i) else { break };
        types.push(t);
        i = j;
    }
    let mut d = hlen as usize;
    let mut out = Vec::new();
    for t in types {
        let int = |n: usize, d: usize| -> Option<i64> {
            let s = b.get(d..d + n)?;
            let mut v: i64 = if s[0] & 0x80 != 0 { -1 } else { 0 };
            for &c in s {
                v = (v << 8) | c as i64;
            }
            Some(v)
        };
        let (v, n) = match t {
            0 => (Val::Null, 0),
            1..=4 => (int(t as usize, d).map_or(Val::Null, Val::Int), t as usize),
            5 => (int(6, d).map_or(Val::Null, Val::Int), 6),
            6 => (int(8, d).map_or(Val::Null, Val::Int), 8),
            7 => (b.get(d..d + 8).map_or(Val::Null, |s| Val::Real(f64::from_be_bytes(s.try_into().unwrap()))), 8),
            8 => (Val::Int(0), 0),
            9 => (Val::Int(1), 0),
            t if t >= 12 && t % 2 == 0 => {
                let n = (t as usize - 12) / 2;
                (Val::Blob(b.get(d..(d + n).min(b.len())).unwrap_or(&[]).to_vec()), n)
            }
            t if t >= 13 => {
                let n = (t as usize - 13) / 2;
                (Val::Text(decode_text(b.get(d..(d + n).min(b.len())).unwrap_or(&[]), enc)), n)
            }
            _ => (Val::Null, 0),
        };
        out.push(v);
        d += n;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_and_schema() {
        let b = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/d23-database-001.sqlite")).unwrap();
        let db = Db::parse(&b).unwrap();
        assert_eq!(db.h.page_size, 4096);
        assert_eq!(db.h.offset, 0);
        assert_eq!(db.h.header_pages, 3);
        assert!(!db.h.truncated());
        assert!(!db.schema().is_empty());
    }
}
