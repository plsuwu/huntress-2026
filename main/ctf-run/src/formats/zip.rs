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
fn winzip_kdf(password: &[u8], salt: &[u8], out: &mut [u8]) {
    ring::pbkdf2::derive(ring::pbkdf2::PBKDF2_HMAC_SHA1, std::num::NonZeroU32::new(1000).unwrap(), salt, password, out);
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
    // The PKWARE key schedule uses the raw CRC-32 table step, (c >> 8) ^ T[(c ^ b) & 0xff], which is
    // `update` without its pre/post inversion.
    let step = |c: u32, b: u8| !crc32::update(!c, &[b]);
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
}
