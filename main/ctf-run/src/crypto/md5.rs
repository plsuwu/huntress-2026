//! MD5 (for metadata hash questions only): the `md-5` crate, 1.2-1.5x the hand-rolled one.

pub fn md5(data: &[u8]) -> [u8; 16] {
    use md5::Digest;
    md5::Md5::digest(data).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::sha256::hex;

    #[test]
    fn vectors() {
        assert_eq!(hex(&md5(b"")), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(hex(&md5(b"The quick brown fox jumps over the lazy dog")), "9e107d9d372bb6826bd81d3542a419d6");
        // Multi-block: Python hashlib.md5(b"a" * 200).
        assert_eq!(hex(&md5(&[b'a'; 200])), "887f30b43b2867f4a9accceee7d16e6c");
    }
}
