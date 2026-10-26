//! ZIP reader: central directory (with ZIP64 sizes), stored/DEFLATE members, WinZip AES and
//! traditional PKWARE ("ZipCrypto") encryption. Members are decoded one at a time, on demand.

use super::inflate;
use crate::crypto::{aes::Aes, crc32, pbkdf2x8};

#[derive(Clone, Debug)]
pub struct Entry {
    /// Name as stored (CP437/UTF-8 bytes decoded lossily).
    pub name: String,
    pub raw_name: Vec<u8>,
    pub method: u16,
    pub flags: u16,
    pub crc: u32,
    pub csize: u64,
    pub usize: u64,
    pub lh_offset: u64,
    /// WinZip AES: (strength 1/2/3, actual method, vendor version).
    pub aes: Option<(u8, u16, u16)>,
    pub external_attr: u32,
}

impl Entry {
    pub fn is_dir(&self) -> bool {
        self.name.ends_with('/')
    }

    pub fn encrypted(&self) -> bool {
        self.flags & 1 != 0
    }

    /// The last path component.
    pub fn base_name(&self) -> &str {
        self.name.trim_end_matches('/').rsplit('/').next().unwrap_or(&self.name)
    }
}

pub struct Zip<'a> {
    pub data: &'a [u8],
    pub entries: Vec<Entry>,
    /// The EOCD's own "total entries" field (may disagree with the directory on purpose).
    pub eocd_total: u64,
    pub comment: Vec<u8>,
    /// The password whose WinZip-AES verifier already matched on a member of this archive. Later
    /// members with the same password then derive only the AES key (2 of 4 PBKDF2 blocks for
    /// AES-256: the MAC key and verifier are skipped) - half the cost of the script-analysis
    /// mastery's 580 key derivations.
    verified: std::sync::Mutex<Option<Vec<u8>>>,
    /// AES keys derived ahead of time by `prepare_aes`, by local-header offset.
    keys: std::sync::Mutex<std::collections::HashMap<u64, Vec<u8>>>,
}

/// WinZip AES key material: PBKDF2-HMAC-SHA1, 1000 rounds (ring's assembly SHA-1: ~1.4x ours on the jumpbox).
/// One key (the password check, and members `prepare_aes` did not batch): RustCrypto's PBKDF2 is
/// 3x ring's with SHA-NI and ~1.7x without. Batches go through the 8-lane `pbkdf2x8`, faster still.
fn winzip_kdf(password: &[u8], salt: &[u8], out: &mut [u8]) {
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password, salt, 1000, out);
}

fn u16le(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}
fn u32le(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn u64le(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

/// ZIP64 sizes and the WinZip-AES record from a local header's extra field: (uncompressed size, aes).
fn local_extra(extra: &[u8], mut usize_: u64) -> (u64, Option<(u8, u16, u16)>) {
    let mut aes = None;
    let mut x = 0;
    while x + 4 <= extra.len() {
        let id = u16le(extra, x).unwrap();
        let sz = u16le(extra, x + 2).unwrap() as usize;
        let body = extra.get(x + 4..x + 4 + sz).unwrap_or(&[]);
        match id {
            // A local ZIP64 record holds both sizes: uncompressed, then compressed.
            0x0001 if usize_ == 0xFFFF_FFFF => usize_ = u64le(body, 0).unwrap_or(usize_),
            0x9901 if body.len() >= 7 => aes = Some((body[4], u16le(body, 5).unwrap(), u16le(body, 0).unwrap())),
            _ => {}
        }
        x += 4 + sz;
    }
    (usize_, aes)
}

/// A local header's compressed size (from its ZIP64 record when the 32-bit field is saturated).
fn local_csize(head: &[u8], extra: &[u8]) -> Result<u64, String> {
    let csize = u32le(head, 18).ok_or("short header")? as u64;
    if csize != 0xFFFF_FFFF {
        return Ok(csize);
    }
    let mut x = 0;
    while x + 4 <= extra.len() {
        let (id, sz) = (u16le(extra, x).unwrap(), u16le(extra, x + 2).unwrap() as usize);
        if id == 0x0001 {
            return u64le(extra, x + 12).ok_or_else(|| "short ZIP64 extra".into());
        }
        x += 4 + sz;
    }
    Err("ZIP64 size without a ZIP64 record".into())
}

/// A local file header in a (possibly partial) archive; its data may not have arrived yet.
pub struct LocalHead<'b> {
    pub raw_name: &'b [u8],
    /// Where the member's data starts and ends.
    pub data: usize,
    pub end: usize,
    pub csize: u64,
    pub usize: u64,
}

/// The local header at `at`: `Ok(None)` while its bytes are still missing, `Err` when `at` is not a
/// local header (the central directory, or junk) or the member's size is only in a trailing data descriptor.
pub fn local_head(b: &[u8], at: usize) -> Result<Option<LocalHead<'_>>, String> {
    let Some(head) = b.get(at..at + 30) else {
        return if b.len() <= at || b[at..].iter().zip(b"PK\x03\x04").all(|(x, y)| x == y) { Ok(None) } else { Err("not a local header".into()) };
    };
    if head[..4] != *b"PK\x03\x04" {
        return Err("not a local header".into());
    }
    if u16le(head, 6).unwrap() & 8 != 0 {
        return Err("member uses a data descriptor".into());
    }
    let nlen = u16le(head, 26).unwrap() as usize;
    let xlen = u16le(head, 28).unwrap() as usize;
    let Some(extra) = b.get(at + 30 + nlen..at + 30 + nlen + xlen) else { return Ok(None) };
    let csize = local_csize(head, extra)?;
    let (usize_, _) = local_extra(extra, u32le(head, 22).unwrap() as u64);
    let data = at + 30 + nlen + xlen;
    let end = data.checked_add(usize::try_from(csize).map_err(|_| "member too large")?).ok_or("member too large")?;
    Ok(Some(LocalHead { raw_name: &b[at + 30..at + 30 + nlen], data, end, csize, usize: usize_ }))
}

/// The local member starting at `at` in a (possibly partial) archive: `Ok(Some((end, raw name,
/// compressed size)))` once its header and data are all present, otherwise as `local_head`.
pub fn local_member_end(b: &[u8], at: usize) -> Result<Option<(usize, &[u8], u64)>, String> {
    Ok(local_head(b, at)?.filter(|h| b.len() >= h.end).map(|h| (h.end, h.raw_name, h.csize)))
}

/// Decodes one member while its compressed bytes arrive: WinZip AES (CTR, block-aligned as it goes) or
/// plain, then DEFLATE or stored, into an output buffer sized from the local header. `push` the member's
/// data bytes in order (everything after the local header); `output` grows as they are decoded.
pub struct MemberStream {
    name: String,
    usize: usize,
    csize: usize,
    method: u16,
    crc: Option<u32>,
    aes: Option<(u8, u16, u16)>,
    password: Vec<u8>,
    /// Data bytes received, and those not yet decrypted (an incomplete block or the AES header).
    got: usize,
    pending: Vec<u8>,
    cipher: Option<(Aes, u128)>,
    inflater: flate2::Decompress,
    /// Zeroed up front (lazily mapped pages); `produced` bytes of it are decoded.
    out: Vec<u8>,
    produced: usize,
}

impl MemberStream {
    /// `head`: the local header through the end of its extra field.
    pub fn new(head: &[u8], password: &[u8]) -> Result<MemberStream, String> {
        if head.get(..4) != Some(b"PK\x03\x04") {
            return Err("not a local header".into());
        }
        let flags = u16le(head, 6).ok_or("short header")?;
        let nlen = u16le(head, 26).ok_or("short header")? as usize;
        let xlen = u16le(head, 28).ok_or("short header")? as usize;
        let extra = head.get(30 + nlen..30 + nlen + xlen).ok_or("short header")?;
        let csize = local_csize(head, extra)?;
        let (usize_, aes) = local_extra(extra, u32le(head, 22).unwrap() as u64);
        if flags & 8 != 0 {
            return Err("member uses a data descriptor".into());
        }
        if flags & 1 != 0 && aes.is_none() {
            return Err("ZipCrypto member".into());
        }
        let method = match aes {
            Some((_, m, _)) => m,
            None => u16le(head, 8).unwrap(),
        };
        if method != 0 && method != 8 {
            return Err(format!("unsupported compression method {method}"));
        }
        let usize_ = usize::try_from(usize_).map_err(|_| "member too large")?;
        if usize_ > 1 << 30 {
            return Err("member too large".into());
        }
        let out = vec![0u8; usize_]; // calloc: pages are mapped as the inflater reaches them
        Ok(MemberStream {
            name: String::from_utf8_lossy(&head[30..30 + nlen]).into_owned(),
            usize: usize_,
            csize: usize::try_from(csize).map_err(|_| "member too large")?,
            method,
            crc: match aes {
                Some((_, _, 2)) => None, // AE-2 stores no CRC
                _ => Some(u32le(head, 14).unwrap()),
            },
            aes,
            password: password.to_vec(),
            got: 0,
            pending: Vec::new(),
            cipher: None,
            inflater: flate2::Decompress::new(false),
            out,
            produced: 0,
        })
    }

    pub fn output(&self) -> &[u8] {
        &self.out[..self.produced]
    }

    pub fn complete(&self) -> bool {
        self.got == self.csize
    }

    pub fn push(&mut self, data: &[u8]) -> Result<(), String> {
        let data = &data[..data.len().min(self.csize - self.got)];
        self.got += data.len();
        let Some((strength, _, _)) = self.aes else { return self.decode(data.to_vec()) };
        let salt_len: usize = match strength {
            1 => 8,
            2 => 12,
            3 => 16,
            _ => return Err(format!("{}: bad AES strength {strength}", self.name)),
        };
        // Encrypted body: salt, 2-byte verifier, ciphertext, 10-byte MAC (not checked, as in `read`).
        let body_end = self.csize.checked_sub(10).ok_or("AES member too short")?;
        let before = self.got - data.len();
        let lo = (salt_len + 2).saturating_sub(before).min(data.len());
        if self.cipher.is_none() {
            self.pending.extend_from_slice(&data[..lo]);
            if self.pending.len() < salt_len + 2 {
                return Ok(());
            }
            let key_len = salt_len * 2;
            let mut k = vec![0u8; key_len * 2 + 2];
            winzip_kdf(&self.password, &self.pending[..salt_len], &mut k);
            if k[key_len * 2..] != self.pending[salt_len..salt_len + 2] {
                return Err(format!("{}: wrong password", self.name));
            }
            self.cipher = Some((Aes::new(&k[..key_len]).unwrap(), 1));
            self.pending.clear();
        }
        // Ciphertext bytes in this push: data offsets before..before+len, clipped to salt+2..body_end.
        let hi = body_end.saturating_sub(before).min(data.len());
        if hi > lo {
            self.pending.extend_from_slice(&data[lo..hi]);
        }
        let last = self.got.min(body_end) == body_end;
        let n = if last { self.pending.len() } else { self.pending.len() / 16 * 16 };
        if n == 0 {
            return Ok(());
        }
        let mut chunk: Vec<u8> = self.pending.drain(..n).collect();
        let (aes, ctr) = self.cipher.as_mut().unwrap();
        aes.ctr_le_xor(*ctr, &mut chunk);
        *ctr += (n / 16) as u128;
        self.decode(chunk)
    }

    fn decode(&mut self, plain: Vec<u8>) -> Result<(), String> {
        if self.method == 0 {
            let end = self.produced + plain.len();
            if end > self.usize {
                return Err(format!("{}: stored data longer than its size", self.name));
            }
            self.out[self.produced..end].copy_from_slice(&plain);
            self.produced = end;
            return Ok(());
        }
        // Bounded output windows: offered the whole remaining buffer, each call cost time in proportion
        // to it (16 KB pushes into a 67 MB member: 1.2 s locally, ~17 s on the jumpbox).
        const WINDOW: usize = 1 << 20;
        let mut input = &plain[..];
        while !input.is_empty() {
            let (in0, out0) = (self.inflater.total_in(), self.inflater.total_out());
            let hi = self.usize.min(self.produced + WINDOW);
            let status = self
                .inflater
                .decompress(input, &mut self.out[self.produced..hi], flate2::FlushDecompress::None)
                .map_err(|e| format!("{}: inflate: {e}", self.name))?;
            let used = (self.inflater.total_in() - in0) as usize;
            let made = (self.inflater.total_out() - out0) as usize;
            input = &input[used..];
            self.produced += made;
            match status {
                flate2::Status::StreamEnd => break,
                _ if used == 0 && made == 0 => {
                    return Err(format!("{}: inflate made no progress (output {} of {})", self.name, self.produced, self.usize));
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// The decoded member, once every data byte has been pushed.
    pub fn finish(self) -> Result<Vec<u8>, String> {
        if self.got != self.csize {
            return Err(format!("{}: {} of {} data bytes", self.name, self.got, self.csize));
        }
        if self.produced != self.usize {
            return Err(format!("{}: decoded {} of {} bytes", self.name, self.produced, self.usize));
        }
        if self.crc.is_some_and(|c| crc32::crc32(&self.out) != c) {
            return Err(format!("{}: CRC mismatch", self.name));
        }
        Ok(self.out)
    }
}

pub fn is_zip(data: &[u8]) -> bool {
    data.starts_with(b"PK\x03\x04") || data.starts_with(b"PK\x05\x06")
}

/// Offset of the last end-of-central-directory record.
pub fn find_eocd(data: &[u8]) -> Option<usize> {
    let lo = data.len().saturating_sub(22 + 65535);
    (lo..=data.len().checked_sub(22)?).rev().find(|&i| data[i..i + 4] == *b"PK\x05\x06")
}

impl<'a> Zip<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Zip<'a>, String> {
        let e = find_eocd(data).ok_or("no end-of-central-directory record")?;
        let mut total = u16le(data, e + 10).unwrap() as u64;
        let mut cd_size = u32le(data, e + 12).unwrap() as u64;
        let mut cd_off = u32le(data, e + 16).unwrap() as u64;
        let clen = u16le(data, e + 20).unwrap() as usize;
        let comment = data.get(e + 22..e + 22 + clen).unwrap_or(&[]).to_vec();
        // ZIP64 end-of-central-directory locator just before the EOCD.
        if e >= 20 && data[e - 20..e - 16] == *b"PK\x06\x07" {
            if let Some(z) = u64le(data, e - 12).map(|o| o as usize) {
                if data.get(z..z + 4) == Some(b"PK\x06\x06") {
                    total = u64le(data, z + 32).unwrap_or(total);
                    cd_size = u64le(data, z + 40).unwrap_or(cd_size);
                    cd_off = u64le(data, z + 48).unwrap_or(cd_off);
                }
            }
        }
        let _ = cd_size;
        let mut entries = Vec::new();
        let mut p = cd_off as usize;
        while data.get(p..p + 4) == Some(b"PK\x01\x02") {
            let flags = u16le(data, p + 8).ok_or("truncated directory")?;
            let method = u16le(data, p + 10).ok_or("truncated directory")?;
            let crc = u32le(data, p + 16).ok_or("truncated directory")?;
            let mut csize = u32le(data, p + 20).ok_or("truncated directory")? as u64;
            let mut usize_ = u32le(data, p + 24).ok_or("truncated directory")? as u64;
            let nlen = u16le(data, p + 28).ok_or("truncated directory")? as usize;
            let xlen = u16le(data, p + 30).ok_or("truncated directory")? as usize;
            let klen = u16le(data, p + 32).ok_or("truncated directory")? as usize;
            let external_attr = u32le(data, p + 46).ok_or("truncated directory")?;
            let mut lh = u32le(data, p + 42).ok_or("truncated directory")? as u64;
            let raw_name = data.get(p + 46..p + 46 + nlen).ok_or("truncated name")?.to_vec();
            let extra = data.get(p + 46 + nlen..p + 46 + nlen + xlen).unwrap_or(&[]);
            let mut aes = None;
            let mut x = 0;
            while x + 4 <= extra.len() {
                let id = u16le(extra, x).unwrap();
                let sz = u16le(extra, x + 2).unwrap() as usize;
                let body = extra.get(x + 4..x + 4 + sz).unwrap_or(&[]);
                match id {
                    0x0001 => {
                        // ZIP64: present only for fields that are 0xFFFFFFFF, in this order.
                        let mut k = 0;
                        if usize_ == 0xFFFF_FFFF {
                            usize_ = u64le(body, k).unwrap_or(usize_);
                            k += 8;
                        }
                        if csize == 0xFFFF_FFFF {
                            csize = u64le(body, k).unwrap_or(csize);
                            k += 8;
                        }
                        if lh == 0xFFFF_FFFF {
                            lh = u64le(body, k).unwrap_or(lh);
                        }
                    }
                    0x9901 if body.len() >= 7 => {
                        aes = Some((body[4], u16le(body, 5).unwrap(), u16le(body, 0).unwrap()));
                    }
                    _ => {}
                }
                x += 4 + sz;
            }
            entries.push(Entry {
                name: String::from_utf8_lossy(&raw_name).into_owned(),
                raw_name,
                method,
                flags,
                crc,
                csize,
                usize: usize_,
                lh_offset: lh,
                aes,
                external_attr,
            });
            p += 46 + nlen + xlen + klen;
        }
        Ok(Zip { data, entries, eocd_total: total, comment, verified: Default::default(), keys: Default::default() })
    }

    /// A one-member archive over `data` = a local file header and its data (no central directory), for
    /// members decoded while the rest of the archive is still downloading. Sizes, CRC and the WinZip-AES
    /// extra come from the local header, so members written with a data descriptor are refused.
    pub fn local(data: &'a [u8]) -> Result<(Zip<'a>, Entry), String> {
        let (end, _, _) = local_member_end(data, 0)?.ok_or("truncated local member")?;
        let nlen = u16le(data, 26).unwrap() as usize;
        let xlen = u16le(data, 28).unwrap() as usize;
        let raw_name = data[30..30 + nlen].to_vec();
        let (usize_, aes) = local_extra(&data[30 + nlen..30 + nlen + xlen], u32le(data, 22).unwrap() as u64);
        let e = Entry {
            name: String::from_utf8_lossy(&raw_name).into_owned(),
            raw_name,
            method: u16le(data, 8).unwrap(),
            flags: u16le(data, 6).unwrap(),
            crc: u32le(data, 14).unwrap(),
            csize: (end - 30 - nlen - xlen) as u64,
            usize: usize_,
            lh_offset: 0,
            aes,
            external_attr: 0,
        };
        let z = Zip { data, entries: vec![e.clone()], eocd_total: 1, comment: Vec::new(), verified: Default::default(), keys: Default::default() };
        Ok((z, e))
    }

    pub fn find(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name == name).or_else(|| self.entries.iter().find(|e| e.base_name() == name && !e.is_dir()))
    }

    /// Files (not directories), in directory order.
    pub fn files(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|e| !e.is_dir())
    }

    /// The member's compressed (and possibly encrypted) bytes.
    fn raw(&self, e: &Entry) -> Result<&'a [u8], String> {
        let lh = e.lh_offset as usize;
        if self.data.get(lh..lh + 4) != Some(b"PK\x03\x04") {
            return Err(format!("{}: bad local header", e.name));
        }
        let nlen = u16le(self.data, lh + 26).unwrap() as usize;
        let xlen = u16le(self.data, lh + 28).unwrap() as usize;
        let start = lh + 30 + nlen + xlen;
        self.data.get(start..start + e.csize as usize).ok_or_else(|| format!("{}: data runs past the end", e.name))
    }

    /// Derives every WinZip-AES member's key in one multi-buffer batch (8 SHA-1 chains per AVX2
    /// stream) once the password's verifier has matched on the first such member; `read` then
    /// skips its own derivation. A no-op for archives without AES members.
    pub fn prepare_aes(&self, password: &[u8]) {
        let aes: Vec<(&Entry, &[u8], usize)> = self
            .entries
            .iter()
            .filter(|e| e.encrypted())
            .filter_map(|e| {
                let salt_len = match e.aes?.0 {
                    1 => 8,
                    2 => 12,
                    3 => 16,
                    _ => return None,
                };
                let raw = self.raw(e).ok().filter(|r| r.len() >= salt_len + 12)?;
                Some((e, &raw[..salt_len], salt_len * 2))
            })
            .collect();
        let Some(&(first, salt, key_len)) = aes.first() else { return };
        // Verify the password once (full derivation incl. the 2-byte verifier).
        let mut full = vec![0u8; key_len * 2 + 2];
        winzip_kdf(password, salt, &mut full);
        let raw = self.raw(first).unwrap();
        if full[key_len * 2..] != raw[salt.len()..salt.len() + 2] {
            return; // read() reports the wrong password
        }
        let mut jobs: Vec<(&[u8], u32)> = Vec::new();
        for &(_, salt, key_len) in &aes[1..] {
            for b in 1..=key_len.div_ceil(20) as u32 {
                jobs.push((salt, b));
            }
        }
        let blocks = pbkdf2x8::pbkdf2_blocks(password, 1000, &jobs);
        let mut keys = self.keys.lock().unwrap();
        keys.insert(first.lh_offset, full[..key_len].to_vec());
        let mut it = blocks.iter();
        for &(e, _, key_len) in &aes[1..] {
            let mut k: Vec<u8> = Vec::with_capacity(key_len.div_ceil(20) * 20);
            for _ in 0..key_len.div_ceil(20) {
                k.extend_from_slice(it.next().unwrap());
            }
            k.truncate(key_len);
            keys.insert(e.lh_offset, k);
        }
        *self.verified.lock().unwrap() = Some(password.to_vec());
    }

    /// Decodes a member. `password` is used only for encrypted members.
    pub fn read(&self, e: &Entry, password: &[u8]) -> Result<Vec<u8>, String> {
        let raw = self.raw(e)?;
        let (body, method, check_crc): (std::borrow::Cow<[u8]>, u16, bool) = match (e.encrypted(), e.aes) {
            (false, _) => (raw.into(), e.method, true),
            (true, Some((strength, method, version))) => {
                let salt_len = match strength {
                    1 => 8,
                    2 => 12,
                    3 => 16,
                    _ => return Err(format!("{}: bad AES strength {strength}", e.name)),
                };
                let key_len = salt_len * 2;
                if raw.len() < salt_len + 2 + 10 {
                    return Err(format!("{}: AES member too short", e.name));
                }
                let salt = &raw[..salt_len];
                let trusted = self.verified.lock().unwrap().as_deref() == Some(password);
                let cached = if trusted { self.keys.lock().unwrap().get(&e.lh_offset).cloned() } else { None };
                let keys = cached.unwrap_or_else(|| {
                    let mut k = vec![0u8; if trusted { key_len } else { key_len * 2 + 2 }];
                    winzip_kdf(password, salt, &mut k);
                    k
                });
                if !trusted {
                    if keys[key_len * 2..] != raw[salt_len..salt_len + 2] {
                        return Err(format!("{}: wrong password", e.name));
                    }
                    *self.verified.lock().unwrap() = Some(password.to_vec());
                }
                let mut data = raw[salt_len + 2..raw.len() - 10].to_vec();
                Aes::new(&keys[..key_len]).unwrap().ctr_le_xor(1, &mut data);
                // AE-2 stores no CRC; AE-1 does.
                (data.into(), method, version == 1)
            }
            (true, None) => {
                let mut data = raw.to_vec();
                zipcrypto_decrypt(password, &mut data);
                if data.len() < 12 {
                    return Err(format!("{}: ZipCrypto member too short", e.name));
                }
                let check = if e.flags & 8 != 0 { (u16le(self.data, e.lh_offset as usize + 10).unwrap() >> 8) as u8 } else { (e.crc >> 24) as u8 };
                if data[11] != check {
                    return Err(format!("{}: wrong password", e.name));
                }
                (data[12..].to_vec().into(), e.method, true)
            }
        };
        let out = match method {
            0 => body.into_owned(),
            8 => inflate::inflate(&body, e.usize as usize).map_err(|err| format!("{}: {err}", e.name))?,
            m => return Err(format!("{}: unsupported compression method {m}", e.name)),
        };
        if check_crc && crc32::crc32(&out) != e.crc {
            return Err(format!("{}: CRC mismatch", e.name));
        }
        Ok(out)
    }

    pub fn read_name(&self, name: &str, password: &[u8]) -> Result<Vec<u8>, String> {
        let e = self.find(name).ok_or_else(|| format!("no member {name:?}"))?;
        self.read(e, password)
    }
}

fn zipcrypto_decrypt(password: &[u8], data: &mut [u8]) {
    let step = crc32::step;
    let mut k = [0x1234_5678u32, 0x2345_6789, 0x3456_7890];
    let update = |k: &mut [u32; 3], b: u8| {
        k[0] = step(k[0], b);
        k[1] = k[1].wrapping_add(k[0] & 0xFF).wrapping_mul(134_775_813).wrapping_add(1);
        k[2] = step(k[2], (k[1] >> 24) as u8);
    };
    for &b in password {
        update(&mut k, b);
    }
    for d in data.iter_mut() {
        let t = (k[2] | 2) as u16;
        let c = *d ^ ((t.wrapping_mul(t ^ 1)) >> 8) as u8;
        *d = c;
        update(&mut k, c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_made_archives() {
        // testdata/plain.zip: stored + deflated members, a directory, made by Python zipfile.
        let data = std::fs::read("testdata/plain.zip").unwrap();
        let z = Zip::parse(&data).unwrap();
        let names: Vec<&str> = z.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["dir/", "dir/a.txt", "b.bin"]);
        assert_eq!(z.read_name("a.txt", b"").unwrap(), b"hello zip\n".repeat(50));
        assert_eq!(z.read_name("b.bin", b"").unwrap(), (0..=255u8).collect::<Vec<_>>());
        assert_eq!(z.eocd_total, 3);
    }

    #[test]
    fn encrypted_archives() {
        let want = b"secret evidence\n".repeat(20);
        // testdata/zipcrypto.zip: `zip -P infected`; testdata/aes.zip: pyzipper AES-256 (AE-2).
        for f in ["testdata/zipcrypto.zip", "testdata/aes.zip"] {
            let data = std::fs::read(f).unwrap();
            let z = Zip::parse(&data).unwrap();
            assert_eq!(z.read_name("s.txt", b"infected").unwrap(), want, "{f}");
            assert!(z.read_name("s.txt", b"wrong").is_err(), "{f}");
        }
    }

    #[test]
    fn member_stream_matches_read() {
        // Every member pushed in awkward step sizes (inside an AES block, across the salt/verifier and the MAC).
        for f in ["testdata/plain.zip", "testdata/aes.zip"] {
            let data = std::fs::read(f).unwrap();
            let z = Zip::parse(&data).unwrap();
            for e in z.files() {
                let want = z.read(e, b"infected").unwrap();
                let at = e.lh_offset as usize;
                let (end, _, _) = local_member_end(&data, at).unwrap().unwrap();
                let head = (at + 30 + u16le(&data, at + 26).unwrap() as usize + u16le(&data, at + 28).unwrap() as usize).min(end);
                for step in [1, 7, 16, 17, 4096] {
                    let mut m = MemberStream::new(&data[at..head], b"infected").unwrap();
                    for c in data[head..end].chunks(step) {
                        m.push(c).unwrap();
                    }
                    assert!(m.complete());
                    assert_eq!(m.finish().unwrap(), want, "{f} {} step {step}", e.name);
                }
            }
            if f.ends_with("aes.zip") {
                let e = z.files().next().unwrap();
                let at = e.lh_offset as usize;
                let head = at + 30 + u16le(&data, at + 26).unwrap() as usize + u16le(&data, at + 28).unwrap() as usize;
                let mut m = MemberStream::new(&data[at..head], b"wrong").unwrap();
                assert!(m.push(&data[head..head + e.csize as usize]).is_err());
            }
        }
    }
}
