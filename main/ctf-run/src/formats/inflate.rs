//! DEFLATE (RFC 1951) decoder plus the gzip (RFC 1952) and zlib (RFC 1950) wrappers.
//!
//! Decoding is libdeflate (`libdeflate-sys`; built by `cc` like ring's C): 1.7x the hand-rolled
//! table decoder it replaced and 1.3x zlib-rs on a binary-RE evidence set.

use crate::crypto::crc32;

#[derive(Debug, PartialEq)]
pub struct InflateError(pub &'static str);

impl std::fmt::Display for InflateError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "inflate: {}", self.0)
    }
}

impl std::error::Error for InflateError {}

type Result<T> = std::result::Result<T, InflateError>;

pub fn inflate(src: &[u8], size_hint: usize) -> Result<Vec<u8>> {
    inflate_with_end(src, size_hint).map(|(out, _)| out)
}

/// libdeflate decompressors are ~10 KB and not thread-safe: one per thread, reused.
struct Decompressor(*mut libdeflate_sys::libdeflate_decompressor);

impl Drop for Decompressor {
    fn drop(&mut self) {
        unsafe { libdeflate_sys::libdeflate_free_decompressor(self.0) }
    }
}

thread_local! {
    static DECOMPRESSOR: Decompressor = Decompressor(unsafe { libdeflate_sys::libdeflate_alloc_decompressor() });
}

/// Raw DEFLATE: the decoded bytes and how many input bytes the stream used. libdeflate decodes
/// whole buffers only, so an output larger than `size_hint` (exact for zip members) is retried
/// with a doubled buffer.
pub fn inflate_with_end(src: &[u8], size_hint: usize) -> Result<(Vec<u8>, usize)> {
    use libdeflate_sys::*;
    let mut cap = if size_hint > 0 { size_hint } else { src.len().saturating_mul(4).max(256) };
    DECOMPRESSOR.with(|d| {
        if d.0.is_null() {
            return Err(InflateError("out of memory"));
        }
        loop {
            let mut out: Vec<u8> = Vec::with_capacity(cap);
            let (mut used, mut written) = (0usize, 0usize);
            // SAFETY: libdeflate reads `src` and writes at most `cap` bytes into `out`'s spare
            // capacity, reporting how many it wrote.
            let r = unsafe {
                libdeflate_deflate_decompress_ex(d.0, src.as_ptr().cast(), src.len(), out.as_mut_ptr().cast(), cap, &mut used, &mut written)
            };
            #[allow(non_upper_case_globals)]
            match r {
                libdeflate_result_LIBDEFLATE_SUCCESS => {
                    unsafe { out.set_len(written) };
                    return Ok((out, used));
                }
                libdeflate_result_LIBDEFLATE_INSUFFICIENT_SPACE => cap = cap.checked_mul(2).ok_or(InflateError("output too large"))?,
                _ => return Err(InflateError("corrupt or truncated deflate stream")),
            }
        }
    })
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
        // ISIZE (the last 4 bytes) is the exact size of a single-member stream.
        let isize = u32::from_le_bytes(s[s.len() - 4..].try_into().unwrap()) as usize;
        let (data, used) = inflate_with_end(body, isize.min(body.len().saturating_mul(1032)))?;
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

