//! DEFLATE (RFC 1951) decoder plus the gzip (RFC 1952) and zlib (RFC 1950) wrappers.
//!
//! Two-level table-driven Huffman decoding over a 64-bit bit buffer: a 10-bit primary table
//! resolves almost every literal/length code in one lookup; longer codes go through a subtable.

use crate::crypto::crc32;

const PRIMARY: u32 = 10;
const SUB_FLAG: u32 = 1 << 31;

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const CL_ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

#[derive(Debug, PartialEq)]
pub struct InflateError(pub &'static str);

impl std::fmt::Display for InflateError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "inflate: {}", self.0)
    }
}

impl std::error::Error for InflateError {}

type Result<T> = std::result::Result<T, InflateError>;

struct Bits<'a> {
    src: &'a [u8],
    pos: usize,
    buf: u64,
    count: u32,
    /// Zero bytes fed past the end of the input (tolerated up to a few bytes of lookahead).
    overrun: u32,
}

impl<'a> Bits<'a> {
    fn new(src: &'a [u8]) -> Self {
        Bits { src, pos: 0, buf: 0, count: 0, overrun: 0 }
    }

    #[inline(always)]
    fn refill(&mut self) {
        if self.count <= 56 && self.pos + 8 <= self.src.len() {
            // Fast path: read 8 bytes, keep as many whole bytes as fit.
            let w = u64::from_le_bytes(self.src[self.pos..self.pos + 8].try_into().unwrap());
            self.buf |= w << self.count;
            let take = (63 - self.count) >> 3;
            self.pos += take as usize;
            self.count += take << 3;
            return;
        }
        while self.count <= 56 {
            let b = if self.pos < self.src.len() {
                self.pos += 1;
                self.src[self.pos - 1]
            } else {
                self.overrun += 1;
                0
            };
            self.buf |= (b as u64) << self.count;
            self.count += 8;
        }
    }

    #[inline(always)]
    fn take(&mut self, n: u32) -> u32 {
        if self.count < n {
            self.refill();
        }
        let v = (self.buf & ((1u64 << n) - 1)) as u32;
        self.buf >>= n;
        self.count -= n;
        v
    }

    fn align(&mut self) {
        let r = self.count & 7;
        self.buf >>= r;
        self.count -= r;
    }

    fn check(&self) -> Result<()> {
        // Up to 8 bytes of lookahead may run past the end; real reads past it are an error.
        if self.overrun * 8 > self.count + 64 {
            Err(InflateError("input truncated"))
        } else {
            Ok(())
        }
    }

    /// Byte offset of the first unconsumed input byte (after the final block).
    fn consumed(&self) -> usize {
        (self.pos + self.overrun as usize).saturating_sub((self.count / 8) as usize)
    }
}

/// Decoding table for one Huffman code.
struct Table {
    t: Vec<u32>,
    pbits: u32,
}

impl Table {
    /// Entries: direct = `sym | len << 16`; subtable pointer = `SUB_FLAG | offset | bits << 16`.
    fn build(lens: &[u8]) -> Result<Table> {
        let mut count = [0u16; 16];
        for &l in lens {
            count[l as usize] += 1;
        }
        count[0] = 0;
        let mut next = [0u32; 16];
        let mut code = 0u32;
        let mut left = 1i32;
        for l in 1..16 {
            left = (left << 1) - count[l] as i32;
            if left < 0 {
                return Err(InflateError("over-subscribed code"));
            }
            code = (code + count[l - 1] as u32) << 1;
            next[l] = code;
        }
        let max_len = (1..16).rev().find(|&l| count[l] > 0).unwrap_or(0) as u32;
        let pbits = PRIMARY.min(max_len.max(1));
        let psize = 1usize << pbits;
        let mut t = vec![0u32; psize];

        // Reverse each code (deflate reads bits LSB-first) and record subtable sizes.
        let mut codes: Vec<(u32, u32, u32)> = Vec::with_capacity(lens.len()); // (rev, len, sym)
        let mut sub_bits = vec![0u32; psize];
        for (sym, &l) in lens.iter().enumerate() {
            if l == 0 {
                continue;
            }
            let l = l as u32;
            let c = next[l as usize];
            next[l as usize] += 1;
            let rev = c.reverse_bits() >> (32 - l);
            if l > pbits {
                let p = (rev & (psize as u32 - 1)) as usize;
                sub_bits[p] = sub_bits[p].max(l - pbits);
            }
            codes.push((rev, l, sym as u32));
        }
        for (p, &b) in sub_bits.iter().enumerate() {
            if b > 0 {
                let off = t.len() as u32;
                t.resize(t.len() + (1 << b), 0);
                t[p] = SUB_FLAG | off | (b << 16);
            }
        }
        for (rev, l, sym) in codes {
            if l <= pbits {
                let mut i = rev as usize;
                while i < psize {
                    t[i] = sym | (l << 16);
                    i += 1 << l;
                }
            } else {
                let p = t[(rev & (psize as u32 - 1)) as usize];
                let off = (p & 0xFFFF) as usize;
                let b = (p >> 16) & 0x1F;
                let mut i = (rev >> pbits) as usize;
                while i < (1 << b) {
                    t[off + i] = sym | (l << 16);
                    i += 1 << (l - pbits);
                }
            }
        }
        Ok(Table { t, pbits })
    }

    #[inline(always)]
    fn decode(&self, bits: &mut Bits) -> Result<u32> {
        if bits.count < 15 {
            bits.refill();
        }
        let pbits = self.pbits;
        let mut e = self.t[(bits.buf & ((1 << pbits) - 1)) as usize];
        if e & SUB_FLAG != 0 {
            let off = (e & 0xFFFF) as usize;
            let b = (e >> 16) & 0x1F;
            e = self.t[off + ((bits.buf >> pbits) & ((1 << b) - 1)) as usize];
        }
        let len = (e >> 16) & 0x1F;
        if len == 0 {
            return Err(InflateError("invalid Huffman code"));
        }
        bits.buf >>= len;
        bits.count -= len;
        Ok(e & 0xFFFF)
    }
}

fn fixed_tables() -> (Table, Table) {
    let mut l = [0u8; 288];
    l[..144].fill(8);
    l[144..256].fill(9);
    l[256..280].fill(7);
    l[280..].fill(8);
    (Table::build(&l).unwrap(), Table::build(&[5u8; 30]).unwrap())
}

fn dynamic_tables(bits: &mut Bits) -> Result<(Table, Table)> {
    let hlit = bits.take(5) as usize + 257;
    let hdist = bits.take(5) as usize + 1;
    let hclen = bits.take(4) as usize + 4;
    let mut cl = [0u8; 19];
    for &i in &CL_ORDER[..hclen] {
        cl[i] = bits.take(3) as u8;
    }
    let clt = Table::build(&cl)?;
    let mut lens = vec![0u8; hlit + hdist];
    let mut i = 0;
    while i < lens.len() {
        let sym = clt.decode(bits)?;
        let (val, rep) = match sym {
            0..=15 => (sym as u8, 1),
            16 => {
                let prev = *lens[..i].last().ok_or(InflateError("repeat with no previous length"))?;
                (prev, 3 + bits.take(2) as usize)
            }
            17 => (0, 3 + bits.take(3) as usize),
            _ => (0, 11 + bits.take(7) as usize),
        };
        if i + rep > lens.len() {
            return Err(InflateError("code lengths overflow"));
        }
        lens[i..i + rep].fill(val);
        i += rep;
    }
    if lens[256] == 0 {
        return Err(InflateError("no end-of-block code"));
    }
    Ok((Table::build(&lens[..hlit])?, Table::build(&lens[hlit..])?))
}

/// Inflates a raw DEFLATE stream. `size_hint` pre-sizes the output (e.g. a zip's uncompressed size).
pub fn inflate(src: &[u8], size_hint: usize) -> Result<Vec<u8>> {
    inflate_with_end(src, size_hint).map(|(out, _)| out)
}

/// As [`inflate`], also returning how many input bytes the stream used.
pub fn inflate_with_end(src: &[u8], size_hint: usize) -> Result<(Vec<u8>, usize)> {
    let mut out = Vec::with_capacity(size_hint);
    let mut bits = Bits::new(src);
    let mut fixed: Option<(Table, Table)> = None;
    loop {
        let last = bits.take(1) == 1;
        match bits.take(2) {
            0 => {
                bits.align();
                let len = bits.take(16) as usize;
                let nlen = bits.take(16) as usize;
                if len != !nlen & 0xFFFF {
                    return Err(InflateError("stored block length mismatch"));
                }
                // Drain whole bytes still in the bit buffer, then copy straight from the input.
                let mut need = len;
                while need > 0 && bits.count >= 8 {
                    out.push(bits.take(8) as u8);
                    need -= 1;
                }
                bits.check()?;
                // The fast refill leaves a copy of the next input byte above the valid bits; the
                // stored bytes are copied straight from the input, so drop that stale lookahead.
                if bits.count == 0 {
                    bits.buf = 0;
                }
                let end = bits.pos + need;
                if end > src.len() {
                    return Err(InflateError("input truncated"));
                }
                out.extend_from_slice(&src[bits.pos..end]);
                bits.pos = end;
            }
            1 => {
                let (lt, dt) = fixed.get_or_insert_with(fixed_tables);
                block(&mut bits, &mut out, lt, dt)?;
            }
            2 => {
                let (lt, dt) = dynamic_tables(&mut bits)?;
                block(&mut bits, &mut out, &lt, &dt)?;
            }
            _ => return Err(InflateError("invalid block type")),
        }
        bits.check()?;
        if last {
            return Ok((out, bits.consumed()));
        }
    }
}

#[inline(always)]
fn block(bits: &mut Bits, out: &mut Vec<u8>, lt: &Table, dt: &Table) -> Result<()> {
    loop {
        let sym = lt.decode(bits)?;
        if sym < 256 {
            out.push(sym as u8);
            continue;
        }
        if sym == 256 {
            return Ok(());
        }
        let li = (sym - 257) as usize;
        if li >= 29 {
            return Err(InflateError("invalid length symbol"));
        }
        let len = LEN_BASE[li] as usize + bits.take(LEN_EXTRA[li] as u32) as usize;
        let ds = dt.decode(bits)? as usize;
        if ds >= 30 {
            return Err(InflateError("invalid distance symbol"));
        }
        let dist = DIST_BASE[ds] as usize + bits.take(DIST_EXTRA[ds] as u32) as usize;
        if dist > out.len() {
            return Err(InflateError("distance too far back"));
        }
        let start = out.len() - dist;
        if dist >= 16 || dist >= len {
            // Inline 16-byte chunk copy instead of a memcpy call per match (musl's memcpy made the
            // static build's inflate ~25 % slower). Chunks may overshoot `len` into spare capacity;
            // with dist >= 16 every source byte a chunk reads is already final.
            let n = out.len();
            out.reserve(len + 16);
            // SAFETY: capacity covers n + len + 16; reads/writes are MaybeUninit (no typed reads of
            // spare capacity); set_len only covers the `len` bytes just written from initialised data.
            unsafe {
                let p = out.as_mut_ptr();
                let mut k = 0;
                while k < len {
                    let v = std::ptr::read_unaligned(p.add(start + k) as *const std::mem::MaybeUninit<[u8; 16]>);
                    std::ptr::write_unaligned(p.add(n + k) as *mut std::mem::MaybeUninit<[u8; 16]>, v);
                    k += 16;
                }
                out.set_len(n + len);
            }
        } else {
            out.reserve(len);
            for k in 0..len {
                let b = out[start + k];
                out.push(b);
            }
        }
        if bits.overrun > 16 {
            return Err(InflateError("input truncated"));
        }
    }
}

/// Decodes a gzip member (or several concatenated members), verifying each CRC-32.
pub fn gunzip(src: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(src.len() * 4);
    let mut s = src;
    loop {
        if s.len() < 18 || s[0] != 0x1F || s[1] != 0x8B || s[2] != 8 {
            return Err(InflateError("not a gzip stream"));
        }
        let flags = s[3];
        let mut p = 10;
        if flags & 4 != 0 {
            let xlen = u16::from_le_bytes([s[p], s[p + 1]]) as usize;
            p += 2 + xlen;
        }
        for bit in [8u8, 16] {
            if flags & bit != 0 {
                p += s.get(p..).and_then(|r| r.iter().position(|&b| b == 0)).ok_or(InflateError("bad gzip header"))? + 1;
            }
        }
        if flags & 2 != 0 {
            p += 2;
        }
        let body = s.get(p..).ok_or(InflateError("bad gzip header"))?;
        let (data, used) = inflate_with_end(body, body.len() * 4)?;
        let trailer = body.get(used..used + 8).ok_or(InflateError("gzip trailer missing"))?;
        let crc = u32::from_le_bytes(trailer[..4].try_into().unwrap());
        if crc != crc32::crc32(&data) {
            return Err(InflateError("gzip CRC mismatch"));
        }
        out.extend_from_slice(&data);
        s = &body[used + 8..];
        if s.is_empty() || s.iter().all(|&b| b == 0) {
            return Ok(out);
        }
    }
}

/// Decodes a zlib stream (header + deflate + adler32; the checksum is not verified).
pub fn unzlib(src: &[u8]) -> Result<Vec<u8>> {
    if src.len() < 6 || src[0] & 0x0F != 8 || (u16::from_be_bytes([src[0], src[1]]) % 31) != 0 {
        return Err(InflateError("not a zlib stream"));
    }
    if src[1] & 0x20 != 0 {
        return Err(InflateError("zlib preset dictionary"));
    }
    inflate(&src[2..], src.len() * 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Python: `gzip.compress(b"hello hello hello hello\n", mtime=0)`
    const HELLO_GZ: &[u8] = &[
        31, 139, 8, 0, 0, 0, 0, 0, 2, 255, 203, 72, 205, 201, 201, 87, 200, 64, 39, 185, 0, 0, 136,
        89, 11, 24, 0, 0, 0,
    ];

    #[test]
    fn gzip_small() {
        assert_eq!(gunzip(HELLO_GZ).unwrap(), b"hello hello hello hello\n");
    }

    /// testdata/mixed.bin: 400 KB of text + random bytes (dynamic and stored blocks), from Python gzip/zlib.
    #[test]
    fn large_streams() {
        let want = std::fs::read("testdata/mixed.bin").unwrap();
        assert_eq!(gunzip(&std::fs::read("testdata/mixed.bin.gz").unwrap()).unwrap(), want);
        assert_eq!(unzlib(&std::fs::read("testdata/mixed.bin.zz").unwrap()).unwrap(), want);
        assert_eq!(crc32::crc32(&want), 3540185562);
    }

    #[test]
    fn stored_block() {
        // BFINAL=1, BTYPE=00, LEN=5, NLEN=!5, "abcde"
        let src = [0x01, 0x05, 0x00, 0xfa, 0xff, b'a', b'b', b'c', b'd', b'e'];
        assert_eq!(inflate(&src, 0).unwrap(), b"abcde");
        assert!(inflate(&src[..8], 0).is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(inflate(&[0xff; 64], 0).is_err());
        assert!(gunzip(b"not gzip at all, really").is_err());
    }
}

