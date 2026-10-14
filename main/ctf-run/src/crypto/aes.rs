//! AES-128/192/256 encryption and CTR mode (WinZip AES uses a little-endian 128-bit counter
//! starting at 1). AES-NI on x86-64 when the CPU has it, else a portable T-table implementation.

const SBOX: [u8; 256] = {
    // Generated from the GF(2^8) inverse + affine transform.
    let mut s = [0u8; 256];
    let mut p: u8 = 1;
    let mut q: u8 = 1;
    loop {
        // p *= 3
        p = p ^ (p << 1) ^ if p & 0x80 != 0 { 0x1B } else { 0 };
        // q /= 3
        q ^= q << 1;
        q ^= q << 2;
        q ^= q << 4;
        if q & 0x80 != 0 {
            q ^= 0x09;
        }
        let x = q ^ q.rotate_left(1) ^ q.rotate_left(2) ^ q.rotate_left(3) ^ q.rotate_left(4);
        s[p as usize] = x ^ 0x63;
        if p == 1 {
            break;
        }
    }
    s[0] = 0x63;
    s
};

const fn xtime(x: u8) -> u8 {
    (x << 1) ^ if x & 0x80 != 0 { 0x1B } else { 0 }
}

/// Te0[x] = (2s, s, s, 3s) as a big-endian word; Te1..3 are byte rotations of it.
const TE: [[u32; 256]; 4] = {
    let mut t = [[0u32; 256]; 4];
    let mut i = 0;
    while i < 256 {
        let s = SBOX[i];
        let s2 = xtime(s);
        let s3 = s2 ^ s;
        let w = ((s2 as u32) << 24) | ((s as u32) << 16) | ((s as u32) << 8) | s3 as u32;
        t[0][i] = w;
        t[1][i] = w.rotate_right(8);
        t[2][i] = w.rotate_right(16);
        t[3][i] = w.rotate_right(24);
        i += 1;
    }
    t
};

pub struct Aes {
    /// Round keys as big-endian words, 4 per round.
    rk: Vec<u32>,
    rounds: usize,
    /// The same round keys as 16-byte blocks (for AES-NI).
    rk_bytes: Vec<[u8; 16]>,
}

impl Aes {
    pub fn new(key: &[u8]) -> Option<Aes> {
        let nk = match key.len() {
            16 => 4,
            24 => 6,
            32 => 8,
            _ => return None,
        };
        let rounds = nk + 6;
        let total = 4 * (rounds + 1);
        let mut w = vec![0u32; total];
        for i in 0..nk {
            w[i] = u32::from_be_bytes(key[i * 4..i * 4 + 4].try_into().unwrap());
        }
        let mut rcon: u8 = 1;
        for i in nk..total {
            let mut t = w[i - 1];
            if i % nk == 0 {
                t = sub_word(t.rotate_left(8)) ^ ((rcon as u32) << 24);
                rcon = xtime(rcon);
            } else if nk > 6 && i % nk == 4 {
                t = sub_word(t);
            }
            w[i] = w[i - nk] ^ t;
        }
        let rk_bytes = w
            .chunks(4)
            .map(|c| {
                let mut b = [0u8; 16];
                for (j, x) in c.iter().enumerate() {
                    b[j * 4..j * 4 + 4].copy_from_slice(&x.to_be_bytes());
                }
                b
            })
            .collect();
        Some(Aes { rk: w, rounds, rk_bytes })
    }

    /// Encrypts one block (portable path).
    pub fn encrypt_block(&self, block: &mut [u8; 16]) {
        let rk = &self.rk;
        let mut s = [0u32; 4];
        for i in 0..4 {
            s[i] = u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().unwrap()) ^ rk[i];
        }
        for r in 1..self.rounds {
            let mut t = [0u32; 4];
            for i in 0..4 {
                t[i] = TE[0][(s[i] >> 24) as usize]
                    ^ TE[1][((s[(i + 1) % 4] >> 16) & 0xFF) as usize]
                    ^ TE[2][((s[(i + 2) % 4] >> 8) & 0xFF) as usize]
                    ^ TE[3][(s[(i + 3) % 4] & 0xFF) as usize]
                    ^ rk[r * 4 + i];
            }
            s = t;
        }
        let r = self.rounds;
        for i in 0..4 {
            let v = ((SBOX[(s[i] >> 24) as usize] as u32) << 24)
                | ((SBOX[((s[(i + 1) % 4] >> 16) & 0xFF) as usize] as u32) << 16)
                | ((SBOX[((s[(i + 2) % 4] >> 8) & 0xFF) as usize] as u32) << 8)
                | SBOX[(s[(i + 3) % 4] & 0xFF) as usize] as u32;
            block[i * 4..i * 4 + 4].copy_from_slice(&(v ^ rk[r * 4 + i]).to_be_bytes());
        }
    }

    /// XORs `data` with the keystream of CTR mode where block `n` uses the little-endian counter
    /// `start + n` (WinZip AES: `start = 1`).
    pub fn ctr_le_xor(&self, start: u128, data: &mut [u8]) {
        #[cfg(target_arch = "x86_64")]
        if std::arch::is_x86_feature_detected!("aes") && std::arch::is_x86_feature_detected!("sse2") {
            // SAFETY: the CPU supports the features `ctr_ni` is compiled for.
            unsafe { ni::ctr_le_xor(&self.rk_bytes, self.rounds, start, data) };
            return;
        }
        self.ctr_le_xor_portable(start, data);
    }

    pub fn ctr_le_xor_portable(&self, start: u128, data: &mut [u8]) {
        let mut ctr = start;
        for chunk in data.chunks_mut(16) {
            let mut ks = ctr.to_le_bytes();
            self.encrypt_block(&mut ks);
            for (d, k) in chunk.iter_mut().zip(ks) {
                *d ^= k;
            }
            ctr = ctr.wrapping_add(1);
        }
    }
}

fn sub_word(w: u32) -> u32 {
    u32::from_be_bytes(w.to_be_bytes().map(|b| SBOX[b as usize]))
}

#[cfg(target_arch = "x86_64")]
mod ni {
    use std::arch::x86_64::*;

    #[target_feature(enable = "aes,sse2")]
    pub unsafe fn ctr_le_xor(rk: &[[u8; 16]], rounds: usize, start: u128, data: &mut [u8]) {
        unsafe {
            let keys: Vec<__m128i> = rk.iter().map(|k| _mm_loadu_si128(k.as_ptr() as *const __m128i)).collect();
            let mut ctr = start;
            let mut chunks = data.chunks_exact_mut(128);
            for chunk in &mut chunks {
                let mut b = [_mm_setzero_si128(); 8];
                for (j, x) in b.iter_mut().enumerate() {
                    let c = ctr.wrapping_add(j as u128).to_le_bytes();
                    *x = _mm_xor_si128(_mm_loadu_si128(c.as_ptr() as *const __m128i), keys[0]);
                }
                for k in &keys[1..rounds] {
                    for x in b.iter_mut() {
                        *x = _mm_aesenc_si128(*x, *k);
                    }
                }
                for (j, x) in b.iter().enumerate() {
                    let ks = _mm_aesenclast_si128(*x, keys[rounds]);
                    let p = chunk.as_mut_ptr().add(j * 16) as *mut __m128i;
                    _mm_storeu_si128(p, _mm_xor_si128(_mm_loadu_si128(p), ks));
                }
                ctr = ctr.wrapping_add(8);
            }
            for chunk in chunks.into_remainder().chunks_mut(16) {
                let c = ctr.to_le_bytes();
                let mut x = _mm_xor_si128(_mm_loadu_si128(c.as_ptr() as *const __m128i), keys[0]);
                for k in &keys[1..rounds] {
                    x = _mm_aesenc_si128(x, *k);
                }
                let mut ks = [0u8; 16];
                _mm_storeu_si128(ks.as_mut_ptr() as *mut __m128i, _mm_aesenclast_si128(x, keys[rounds]));
                for (d, k) in chunk.iter_mut().zip(ks) {
                    *d ^= k;
                }
                ctr = ctr.wrapping_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn fips197_vectors() {
        let pt: [u8; 16] = unhex("00112233445566778899aabbccddeeff").try_into().unwrap();
        for (key, ct) in [
            ("000102030405060708090a0b0c0d0e0f", "69c4e0d86a7b0430d8cdb78070b4c55a"),
            ("000102030405060708090a0b0c0d0e0f1011121314151617", "dda97ca4864cdfe06eaf70a0ec0d7191"),
            ("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f", "8ea2b7ca516745bfeafc49904b496089"),
        ] {
            let aes = Aes::new(&unhex(key)).unwrap();
            let mut b = pt;
            aes.encrypt_block(&mut b);
            assert_eq!(b.to_vec(), unhex(ct), "key {key}");
        }
    }

    #[test]
    fn ctr_paths_agree() {
        let aes = Aes::new(&[7u8; 32]).unwrap();
        let mut a: Vec<u8> = (0..1000u32).map(|i| (i * 31) as u8).collect();
        let mut b = a.clone();
        aes.ctr_le_xor(1, &mut a);
        aes.ctr_le_xor_portable(1, &mut b);
        assert_eq!(a, b);
        aes.ctr_le_xor(1, &mut a);
        assert_eq!(a, (0..1000u32).map(|i| (i * 31) as u8).collect::<Vec<_>>());
    }
}
