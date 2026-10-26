//! Windows Prefetch (`.pf`, `SCCA`): versions 17 (XP), 23 (Vista/7), 26 (8.1) and 30/31 (10/11). Windows 10+
//! stores them compressed as `MAM\x04` + u32 decompressed size + LZXPRESS Huffman (MS-XCA 2.2; `MAM\x84`
//! adds a CRC32 before the data). The header carries the executable name (UTF-16, 60 bytes @0x10, the
//! name Windows ran even when the `.pf` file was renamed) and the path hash @0x4C; the file information
//! block (@0x54) points at the metrics, trace chains, file-name strings and volume records.

use super::bytes::{u16le, u32le, u64le, utf16le};

/// LZXPRESS Huffman (MS-XCA 2.2.4): 64 KiB output blocks, each led by a 256-byte table of 512 4-bit code
/// lengths. Stops once `out_len` bytes are produced.
pub fn xpress_huffman(input: &[u8], out_len: usize) -> Option<Vec<u8>> {
    let mut out: Vec<u8> = Vec::with_capacity(out_len);
    let mut pos = 0usize;
    let r16 = |p: usize| -> u32 { u16le(input, p).unwrap_or(0) as u32 };
    while out.len() < out_len {
        let table = input.get(pos..pos + 256)?;
        let mut lens = [0u8; 512];
        for (i, &b) in table.iter().enumerate() {
            lens[2 * i] = b & 15;
            lens[2 * i + 1] = b >> 4;
        }
        let mut decode = vec![0u16; 1 << 15];
        let mut at = 0usize;
        for bl in 1..=15u8 {
            for sym in (0..512).filter(|&s| lens[s] == bl) {
                let n = 1usize << (15 - bl);
                decode.get_mut(at..at + n)?.fill(sym as u16);
                at += n;
            }
        }
        if at == 0 {
            return None;
        }
        pos += 256;
        let mut bits: u32 = (r16(pos) << 16) | r16(pos + 2);
        pos += 4;
        let mut extra: i32 = 16;
        let block_end = (out.len() + 65536).min(out_len);
        macro_rules! refill {
            () => {
                if extra < 0 {
                    bits |= r16(pos) << (-extra);
                    extra += 16;
                    pos += 2;
                }
            };
        }
        while out.len() < block_end {
            let sym = decode[(bits >> 17) as usize] as usize;
            let l = lens[sym] as u32;
            if l == 0 {
                return None;
            }
            bits <<= l;
            extra -= l as i32;
            refill!();
            if sym < 256 {
                out.push(sym as u8);
                continue;
            }
            let sym = sym - 256;
            let mut len = sym & 15;
            let obits = (sym >> 4) as u32;
            if len == 15 {
                len = *input.get(pos)? as usize;
                pos += 1;
                if len == 255 {
                    len = u16le(input, pos)? as usize;
                    pos += 2;
                    if len < 15 {
                        return None;
                    }
                    len -= 15;
                }
                len += 15;
            }
            len += 3;
            let off = if obits == 0 { 0 } else { (bits >> (32 - obits)) as usize } + (1usize << obits);
            if obits > 0 {
                bits = if obits == 32 { 0 } else { bits << obits };
            }
            extra -= obits as i32;
            refill!();
            let start = out.len().checked_sub(off)?;
            for i in 0..len {
                let c = out[start + i];
                out.push(c);
            }
        }
    }
    out.truncate(out_len);
    Some(out)
}

/// The SCCA body: decompressed when the file is `MAM`-wrapped, else the bytes as they are.
pub fn body(b: &[u8]) -> Option<(Vec<u8>, bool)> {
    if b.len() >= 8 && &b[..3] == b"MAM" {
        let kind = b[3];
        let size = u32le(b, 4)? as usize;
        let data = if kind & 0x80 != 0 { b.get(12..)? } else { b.get(8..)? };
        if kind & 0x7f != 4 || size > 64 << 20 {
            return None;
        }
        return Some((xpress_huffman(data, size)?, true));
    }
    (b.get(4..8) == Some(b"SCCA")).then(|| (b.to_vec(), false))
}

#[derive(Clone, Debug)]
pub struct Volume {
    pub device_path: String,
    pub created: u64,
    pub serial: u32,
}

#[derive(Clone, Debug)]
pub struct Prefetch {
    pub compressed: bool,
    pub version: u32,
    pub file_size: u32,
    pub executable_name: String,
    pub hash: u32,
    pub metrics_count: u32,
    pub trace_count: u32,
    pub filenames: Vec<String>,
    pub volumes: Vec<Volume>,
    /// Non-zero last-run FILETIMEs, most recent first (1 before v26, up to 8 after).
    pub last_runs: Vec<u64>,
    pub run_count: u32,
    pub len: usize,
}

impl Prefetch {
    pub fn parse(b: &[u8]) -> Option<Prefetch> {
        let (d, compressed) = body(b)?;
        if d.get(4..8) != Some(b"SCCA") {
            return None;
        }
        let version = u32le(&d, 0)?;
        let name_raw = d.get(0x10..0x4C)?;
        let end = name_raw.chunks(2).position(|c| c == [0, 0]).map(|i| i * 2).unwrap_or(name_raw.len());
        let executable_name = utf16le(&name_raw[..end]);
        let metrics_off = u32le(&d, 0x54)?;
        let (fn_off, fn_size) = (u32le(&d, 0x64)? as usize, u32le(&d, 0x68)? as usize);
        let (vol_off, vol_count) = (u32le(&d, 0x6C)? as usize, u32le(&d, 0x70)? as usize);
        let filenames = d
            .get(fn_off..fn_off + fn_size)
            .map(|s| {
                let mut v = Vec::new();
                let mut st = 0;
                for i in (0..s.len() & !1).step_by(2) {
                    if s[i] == 0 && s[i + 1] == 0 {
                        if i > st {
                            v.push(utf16le(&s[st..i]));
                        }
                        st = i + 2;
                    }
                }
                v
            })
            .unwrap_or_default();
        let (runs_at, nruns, count_at) = match version {
            17 => (0x78, 1, 0x90),
            23 => (0x80, 1, 0x98),
            _ => (0x80, 8, if metrics_off == 0x128 { 0xC8 } else { 0xD0 }),
        };
        let last_runs = (0..nruns).filter_map(|i| u64le(&d, runs_at + 8 * i)).filter(|&t| t != 0).collect();
        let entry = match version {
            17 => 40,
            23 | 26 => 104,
            _ => 96,
        };
        let volumes = (0..vol_count.min(64))
            .filter_map(|i| {
                let e = vol_off + i * entry;
                let (po, pn) = (u32le(&d, e)? as usize, u32le(&d, e + 4)? as usize);
                let raw = d.get(vol_off + po..vol_off + po + 2 * pn)?;
                Some(Volume { device_path: utf16le(raw), created: u64le(&d, e + 8)?, serial: u32le(&d, e + 16)? })
            })
            .collect();
        Some(Prefetch {
            compressed,
            version,
            file_size: u32le(&d, 0xC)?,
            executable_name,
            hash: u32le(&d, 0x4C)?,
            metrics_count: u32le(&d, 0x58)?,
            trace_count: u32le(&d, 0x60)?,
            filenames,
            volumes,
            last_runs,
            run_count: u32le(&d, count_at)?,
            len: d.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressed_win10() {
        let b = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/d26-prefetch-001.pf")).unwrap();
        let p = Prefetch::parse(&b).unwrap();
        assert!(p.compressed);
        assert_eq!((p.version, p.len, p.file_size as usize), (30, 15412, 15412));
        assert_eq!(p.executable_name, "CMD.EXE");
        assert_eq!(p.hash, 0xCD245F9E);
        assert_eq!(p.run_count, 20);
        assert_eq!(p.volumes[0].serial, 0x5A3A0DD2);
        assert!(p.filenames.iter().any(|f| f.ends_with("\\SYSTEM32\\CMD.EXE")));
    }

    #[test]
    fn plain_scca() {
        let mut d = vec![0u8; 0x130];
        d[..4].copy_from_slice(&30u32.to_le_bytes());
        d[4..8].copy_from_slice(b"SCCA");
        for (i, c) in "BROKER.EXE".encode_utf16().enumerate() {
            d[0x10 + 2 * i..0x12 + 2 * i].copy_from_slice(&c.to_le_bytes());
        }
        let p = Prefetch::parse(&d).unwrap();
        assert!(!p.compressed);
        assert_eq!(p.executable_name, "BROKER.EXE");
        assert!(Prefetch::parse(b"MAM\x04\x10\0\0\0").is_none());
    }
}
