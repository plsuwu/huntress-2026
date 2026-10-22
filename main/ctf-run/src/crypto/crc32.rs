//! CRC-32 (IEEE 802.3, as used by zip, gzip and png): `crc32fast` (PCLMULQDQ folding, ~10x the old
//! slice-by-8 table) for buffers, plus the raw table step the ZipCrypto key schedule needs.

const fn table() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}

static T: [u32; 256] = table();

/// Continues a running CRC: `update(0, a)` then `update(crc, b)` equals `crc32(a ++ b)`.
pub fn update(crc: u32, data: &[u8]) -> u32 {
    let mut h = crc32fast::Hasher::new_with_initial(crc);
    h.update(data);
    h.finalize()
}

pub fn crc32(data: &[u8]) -> u32 {
    crc32fast::hash(data)
}

/// One raw table step, `(c >> 8) ^ T[(c ^ b) & 0xff]` (no pre/post inversion), as the PKWARE
/// (ZipCrypto) key schedule uses it.
#[inline(always)]
pub fn step(c: u32, b: u8) -> u32 {
    (c >> 8) ^ T[((c ^ b as u32) & 0xFF) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        let data: Vec<u8> = (0..1000u32).map(|i| (i * 7 + 3) as u8).collect();
        assert_eq!(update(update(0, &data[..333]), &data[333..]), crc32(&data));
        // The raw step is `update` without its inversions.
        assert_eq!(step(0x1234_5678, 0xAB), !update(!0x1234_5678, &[0xAB]));
    }
}
