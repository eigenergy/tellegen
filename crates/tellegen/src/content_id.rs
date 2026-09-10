//! Stable SHA-256 identities shared by model preparation and saved Studies.

use sha2::{Digest, Sha256};

pub(crate) fn content_id(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut id = String::with_capacity(7 + 64);
    id.push_str("sha256:");
    for byte in Sha256::digest(bytes) {
        id.push(char::from(HEX[usize::from(byte >> 4)]));
        id.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    id
}

#[cfg(test)]
mod tests {
    #[test]
    fn hashes_match_standard_sha256_vectors() {
        assert_eq!(
            super::content_id(b""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            super::content_id(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
