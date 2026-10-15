//! Multi-buffer PBKDF2-HMAC-SHA1: many independent (salt, block index) jobs under one password.
//!
//! A WinZip-AES archive derives a key per member (1000 rounds, two 20-byte blocks for AES-256), and
//! the script-analysis mastery has ~580 members: ~1160 independent SHA-1 chains that share the HMAC
//! midstates. With AVX2 they run 8 chains per instruction stream - the jumpbox's two vCPUs are
//! hyperthreads of one core, so data parallelism is the only real speedup there. Scalar fallback
//! when AVX2 is missing; `sha1::pbkdf2_hmac_sha1` stays the reference (see the tests).

use super::sha1::{HmacSha1, hmac20};

/// T_block = PBKDF2-HMAC-SHA1(password, salt, rounds) block `block` (1-based) for every job.
pub fn pbkdf2_blocks(password: &[u8], rounds: u32, jobs: &[(&[u8], u32)]) -> Vec<[u8; 20]> {
    let prf = HmacSha1::new(password);
    let (inner, outer) = prf.midstates();
    // U1 needs the salt (variable length): scalar, 2-3 compressions per job.
    let u1: Vec<[u32; 5]> = jobs
        .iter()
        .map(|(salt, block)| {
            let mut msg = salt.to_vec();
            msg.extend_from_slice(&block.to_be_bytes());
            words(&prf.mac(&msg))
        })
        .collect();
    let mut out = Vec::with_capacity(jobs.len());
    for chunk in u1.chunks(8) {
        let t = run8(&inner, &outer, rounds, chunk);
        out.extend(t[..chunk.len()].iter().map(|w| bytes(w)));
    }
    out
}

fn words(b: &[u8; 20]) -> [u32; 5] {
    std::array::from_fn(|i| u32::from_be_bytes(b[i * 4..i * 4 + 4].try_into().unwrap()))
}

fn bytes(w: &[u32; 5]) -> [u8; 20] {
    let mut o = [0u8; 20];
    for (i, x) in w.iter().enumerate() {
        o[i * 4..i * 4 + 4].copy_from_slice(&x.to_be_bytes());
    }
    o
}

/// Rounds 2..=rounds for up to 8 chains starting at U1; returns T (8 lanes, extra lanes garbage).
fn run8(inner: &[u32; 5], outer: &[u32; 5], rounds: u32, u1: &[[u32; 5]]) -> [[u32; 5]; 8] {
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") {
        // SAFETY: AVX2 availability checked at runtime.
        return unsafe { avx2::run8(inner, outer, rounds, u1) };
    }
    let mut t = [[0u32; 5]; 8];
    for (lane, u0) in u1.iter().enumerate() {
        let (mut u, mut acc) = (*u0, *u0);
        for _ in 1..rounds {
            u = hmac20(inner, outer, &u);
            for (a, b) in acc.iter_mut().zip(u) {
                *a ^= b;
            }
        }
        t[lane] = acc;
    }
    t
}

#[cfg(target_arch = "x86_64")]
mod avx2 {
    use std::arch::x86_64::*;

    macro_rules! rotl {
        ($x:expr, $n:literal) => {
            _mm256_or_si256(_mm256_slli_epi32::<$n>($x), _mm256_srli_epi32::<{ 32 - $n }>($x))
        };
    }

    #[inline(always)]
    unsafe fn add(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_add_epi32(a, b) }
    }

    /// One SHA-1 compression of 8 independent blocks from a broadcast midstate.
    #[target_feature(enable = "avx2")]
    unsafe fn compress8(h: &[__m256i; 5], blk: &[__m256i; 16]) -> [__m256i; 5] {
        unsafe {
            let mut w = *blk;
            let [mut a, mut b, mut c, mut d, mut e] = *h;
            macro_rules! step {
                ($i:expr, $f:expr, $k:expr) => {{
                    let i: usize = $i;
                    let wi = if i < 16 {
                        w[i]
                    } else {
                        let x = _mm256_xor_si256(_mm256_xor_si256(w[(i + 13) & 15], w[(i + 8) & 15]), _mm256_xor_si256(w[(i + 2) & 15], w[i & 15]));
                        let x = rotl!(x, 1);
                        w[i & 15] = x;
                        x
                    };
                    let t = add(add(rotl!(a, 5), $f), add(add(e, _mm256_set1_epi32($k as i32)), wi));
                    e = d;
                    d = c;
                    c = rotl!(b, 30);
                    b = a;
                    a = t;
                }};
            }
            for i in 0..20 {
                step!(i, _mm256_or_si256(_mm256_and_si256(b, c), _mm256_andnot_si256(b, d)), 0x5A827999u32);
            }
            for i in 20..40 {
                step!(i, _mm256_xor_si256(_mm256_xor_si256(b, c), d), 0x6ED9EBA1u32);
            }
            for i in 40..60 {
                step!(i, _mm256_or_si256(_mm256_and_si256(b, c), _mm256_and_si256(d, _mm256_or_si256(b, c))), 0x8F1BBCDCu32);
            }
            for i in 60..80 {
                step!(i, _mm256_xor_si256(_mm256_xor_si256(b, c), d), 0xCA62C1D6u32);
            }
            [add(a, h[0]), add(b, h[1]), add(c, h[2]), add(d, h[3]), add(e, h[4])]
        }
    }

    #[target_feature(enable = "avx2")]
    pub unsafe fn run8(inner: &[u32; 5], outer: &[u32; 5], rounds: u32, u1: &[[u32; 5]]) -> [[u32; 5]; 8] {
        unsafe {
            let bcast = |s: &[u32; 5]| -> [__m256i; 5] { std::array::from_fn(|k| _mm256_set1_epi32(s[k] as i32)) };
            let (hi, ho) = (bcast(inner), bcast(outer));
            let mut u = [_mm256_setzero_si256(); 5];
            for (k, slot) in u.iter_mut().enumerate() {
                let mut lanes = [0u32; 8];
                for (l, x) in u1.iter().enumerate() {
                    lanes[l] = x[k];
                }
                *slot = _mm256_loadu_si256(lanes.as_ptr() as *const __m256i);
            }
            let mut t = u;
            // Each HMAC step hashes a 20-byte message: u || 0x80 || zeros || bit length (64 + 20) * 8.
            let mut blk = [_mm256_setzero_si256(); 16];
            blk[5] = _mm256_set1_epi32(0x8000_0000u32 as i32);
            blk[15] = _mm256_set1_epi32(((64 + 20) * 8) as i32);
            for _ in 1..rounds {
                blk[..5].copy_from_slice(&u);
                let x = compress8(&hi, &blk);
                blk[..5].copy_from_slice(&x);
                u = compress8(&ho, &blk);
                for k in 0..5 {
                    t[k] = _mm256_xor_si256(t[k], u[k]);
                }
            }
            let mut out = [[0u32; 5]; 8];
            for k in 0..5 {
                let mut lanes = [0u32; 8];
                _mm256_storeu_si256(lanes.as_mut_ptr() as *mut __m256i, t[k]);
                for l in 0..8 {
                    out[l][k] = lanes[l];
                }
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::sha1::pbkdf2_hmac_sha1;

    #[test]
    fn matches_reference() {
        let salts: Vec<Vec<u8>> = (0..19u8).map(|i| (0..8 + (i as usize % 9)).map(|j| i.wrapping_mul(31) ^ j as u8).collect()).collect();
        let jobs: Vec<(&[u8], u32)> = salts.iter().enumerate().map(|(i, s)| (s.as_slice(), 1 + (i as u32 % 4))).collect();
        for rounds in [1, 2, 1000] {
            let got = pbkdf2_blocks(b"infected", rounds, &jobs);
            for ((salt, block), g) in jobs.iter().zip(&got) {
                let full = pbkdf2_hmac_sha1(b"infected", salt, rounds, 20 * *block as usize);
                assert_eq!(&full[20 * (*block as usize - 1)..], g, "rounds {rounds} block {block}");
            }
        }
    }
}
