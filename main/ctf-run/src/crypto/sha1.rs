//! SHA-1, HMAC-SHA1 (with precomputed pad midstates) and PBKDF2-HMAC-SHA1.

#[derive(Clone)]
pub struct Sha1 {
    h: [u32; 5],
    buf: [u8; 64],
    buf_len: usize,
    total: u64,
}

impl Default for Sha1 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha1 {
    pub fn new() -> Sha1 {
        Sha1 { h: [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0], buf: [0; 64], buf_len: 0, total: 0 }
    }

    pub(crate) fn compress(h: &mut [u32; 5], block: &[u8]) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = *h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.total += data.len() as u64;
        if self.buf_len > 0 {
            let take = (64 - self.buf_len).min(data.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len < 64 {
                return;
            }
            let block = self.buf;
            Self::compress(&mut self.h, &block);
            self.buf_len = 0;
        }
        let mut chunks = data.chunks_exact(64);
        for block in &mut chunks {
            Self::compress(&mut self.h, block);
        }
        let rest = chunks.remainder();
        self.buf[..rest.len()].copy_from_slice(rest);
        self.buf_len = rest.len();
    }

    pub fn finish(mut self) -> [u8; 20] {
        let bits = self.total.wrapping_mul(8);
        self.update(&[0x80]);
        while self.buf_len != 56 {
            self.update(&[0]);
        }
        self.update(&bits.to_be_bytes());
        let mut out = [0u8; 20];
        for (i, v) in self.h.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
        }
        out
    }
}

pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut s = Sha1::new();
    s.update(data);
    s.finish()
}

/// HMAC-SHA1 keyed once: the inner/outer states after absorbing ipad/opad are kept, so each MAC
/// only hashes the message and one digest on top (PBKDF2 calls this thousands of times).
#[derive(Clone)]
pub struct HmacSha1 {
    inner: Sha1,
    outer: Sha1,
}

impl HmacSha1 {
    pub fn new(key: &[u8]) -> HmacSha1 {
        let mut k = [0u8; 64];
        if key.len() > 64 {
            k[..20].copy_from_slice(&sha1(key));
        } else {
            k[..key.len()].copy_from_slice(key);
        }
        let mut inner = Sha1::new();
        inner.update(&k.map(|b| b ^ 0x36));
        let mut outer = Sha1::new();
        outer.update(&k.map(|b| b ^ 0x5c));
        HmacSha1 { inner, outer }
    }

    /// The ipad/opad midstates (the 64-byte key blocks already absorbed).
    pub(crate) fn midstates(&self) -> ([u32; 5], [u32; 5]) {
        (self.inner.h, self.outer.h)
    }

    pub fn mac(&self, msg: &[u8]) -> [u8; 20] {
        let mut i = self.inner.clone();
        i.update(msg);
        let mut o = self.outer.clone();
        o.update(&i.finish());
        o.finish()
    }
}

/// One HMAC-SHA1 step over a 20-byte message, given the ipad/opad midstates: exactly two
/// compressions over pre-padded blocks (the PBKDF2 inner loop runs this 1000s of times).
#[inline]
pub(crate) fn hmac20(inner: &[u32; 5], outer: &[u32; 5], msg: &[u32; 5]) -> [u32; 5] {
    // 64-byte key block already absorbed; message (20 bytes) + 0x80 + zeros + bit length (84 * 8).
    let mut blk = [0u8; 64];
    let mut run = |state: &[u32; 5], words: &[u32; 5]| -> [u32; 5] {
        for (i, w) in words.iter().enumerate() {
            blk[i * 4..i * 4 + 4].copy_from_slice(&w.to_be_bytes());
        }
        blk[20] = 0x80;
        blk[56..64].copy_from_slice(&(84u64 * 8).to_be_bytes());
        let mut h = *state;
        Sha1::compress(&mut h, &blk);
        h
    };
    let i = run(inner, msg);
    run(outer, &i)
}

pub fn pbkdf2_hmac_sha1(password: &[u8], salt: &[u8], rounds: u32, len: usize) -> Vec<u8> {
    let prf = HmacSha1::new(password);
    let (inner, outer) = (prf.inner.h, prf.outer.h);
    let words = |b: &[u8; 20]| -> [u32; 5] { std::array::from_fn(|i| u32::from_be_bytes(b[i * 4..i * 4 + 4].try_into().unwrap())) };
    let mut out = Vec::with_capacity(len);
    let mut block = 1u32;
    while out.len() < len {
        let mut msg = salt.to_vec();
        msg.extend_from_slice(&block.to_be_bytes());
        let mut u = words(&prf.mac(&msg));
        let mut t = u;
        for _ in 1..rounds {
            u = hmac20(&inner, &outer, &u);
            for (a, b) in t.iter_mut().zip(u) {
                *a ^= b;
            }
        }
        let bytes: Vec<u8> = t.iter().flat_map(|w| w.to_be_bytes()).collect();
        out.extend_from_slice(&bytes[..(len - out.len()).min(20)]);
        block += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn vectors() {
        assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hex(&sha1(&vec![b'a'; 1_000_000])), "34aa973cd4c4daa4f61eeb2bdbad27316534016f");
        // RFC 2202 test case 2
        assert_eq!(hex(&HmacSha1::new(b"Jefe").mac(b"what do ya want for nothing?")), "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79");
        // RFC 6070
        assert_eq!(hex(&pbkdf2_hmac_sha1(b"password", b"salt", 4096, 20)), "4b007901b765489abead49d926f721d065a429c1");
    }
}
