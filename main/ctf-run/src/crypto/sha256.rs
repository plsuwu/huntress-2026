//! SHA-256.

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01,
    0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
    0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08,
    0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

#[allow(unused_assignments)] // the last rounds write schedule words nobody reads
fn compress(h: &mut [u32; 8], block: &[u8]) {
    // Fully unrolled rounds with a rolling 16-word schedule; the eight working variables rotate by
    // renaming (macro arguments) instead of moves. ~1.5x the naive loop on the scalar-only jumpbox.
    let block: &[u8; 64] = block.try_into().unwrap();
    let mut w = [0u32; 16];
    for (i, c) in block.chunks_exact(4).enumerate() {
        w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = *h;
    macro_rules! round {
        ($a:ident, $b:ident, $c:ident, $d:ident, $e:ident, $f:ident, $g:ident, $h:ident, $i:expr) => {{
            let wi = if $i < 16 {
                w[$i & 15]
            } else {
                let x = w[($i + 1) & 15];
                let y = w[($i + 14) & 15];
                let s0 = x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
                let s1 = y.rotate_right(17) ^ y.rotate_right(19) ^ (y >> 10);
                let v = w[$i & 15].wrapping_add(s0).wrapping_add(w[($i + 9) & 15]).wrapping_add(s1);
                w[$i & 15] = v;
                v
            };
            let t1 = $h
                .wrapping_add($e.rotate_right(6) ^ $e.rotate_right(11) ^ $e.rotate_right(25))
                .wrapping_add($g ^ ($e & ($f ^ $g)))
                .wrapping_add(K[$i])
                .wrapping_add(wi);
            let t2 = ($a.rotate_right(2) ^ $a.rotate_right(13) ^ $a.rotate_right(22)).wrapping_add(($a & $b) | ($c & ($a | $b)));
            $d = $d.wrapping_add(t1);
            $h = t1.wrapping_add(t2);
        }};
    }
    macro_rules! eight {
        ($i:expr) => {
            round!(a, b, c, d, e, f, g, hh, $i);
            round!(hh, a, b, c, d, e, f, g, $i + 1);
            round!(g, hh, a, b, c, d, e, f, $i + 2);
            round!(f, g, hh, a, b, c, d, e, $i + 3);
            round!(e, f, g, hh, a, b, c, d, $i + 4);
            round!(d, e, f, g, hh, a, b, c, $i + 5);
            round!(c, d, e, f, g, hh, a, b, $i + 6);
            round!(b, c, d, e, f, g, hh, a, $i + 7);
        };
    }
    eight!(0);
    eight!(8);
    eight!(16);
    eight!(24);
    eight!(32);
    eight!(40);
    eight!(48);
    eight!(56);
    for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
        *x = x.wrapping_add(y);
    }
}

/// SHA-256 via ring (assembly with AVX2/SHA-NI dispatch); `sha256_portable` is the reference.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    ring::digest::digest(&ring::digest::SHA256, data).as_ref().try_into().unwrap()
}

/// Portable SHA-256 (kept as the cross-check in tests).
pub fn sha256_portable(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut chunks = data.chunks_exact(64);
    for b in &mut chunks {
        compress(&mut h, b);
    }
    let rest = chunks.remainder();
    let mut tail = rest.to_vec();
    tail.push(0x80);
    while tail.len() % 64 != 56 {
        tail.push(0);
    }
    tail.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for b in tail.chunks(64) {
        compress(&mut h, b);
    }
    let mut out = [0u8; 32];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vectors() {
        assert_eq!(hex(&sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let m = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        assert_eq!(hex(&sha256(m)), "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");
        // The portable implementation agrees with ring on every padding boundary.
        let data: Vec<u8> = (0..300u32).map(|i| (i * 31 + 7) as u8).collect();
        for n in 0..data.len() {
            assert_eq!(sha256_portable(&data[..n]), sha256(&data[..n]), "len {n}");
        }
    }
}
