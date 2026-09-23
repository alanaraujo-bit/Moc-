//! Small shared helpers.

/// Serde adapter: `Vec<u8>` as standard Base64 (padded).
pub mod b64 {
    use data_encoding::BASE64;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&BASE64.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        BASE64.decode(s.as_bytes()).map_err(serde::de::Error::custom)
    }
}

/// Crockford Base32: no I, L, O, U — hard to misread when typed from paper.
pub mod crockford {
    const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

    pub fn encode(bytes: &[u8]) -> String {
        let mut out = String::with_capacity((bytes.len() * 8).div_ceil(5));
        let mut buffer: u32 = 0;
        let mut bits = 0;
        for &b in bytes {
            buffer = (buffer << 8) | b as u32;
            bits += 8;
            while bits >= 5 {
                bits -= 5;
                out.push(ALPHABET[((buffer >> bits) & 0x1f) as usize] as char);
            }
        }
        if bits > 0 {
            out.push(ALPHABET[((buffer << (5 - bits)) & 0x1f) as usize] as char);
        }
        out
    }

    fn value(c: char) -> Option<u32> {
        let c = c.to_ascii_uppercase();
        let c = match c {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        };
        ALPHABET.iter().position(|&a| a as char == c).map(|p| p as u32)
    }

    /// Decodes `symbols` into exactly `len` bytes; the leftover padding bits must be zero.
    pub fn decode(symbols: &str, len: usize) -> Option<Vec<u8>> {
        let mut out = Vec::with_capacity(len);
        let mut buffer: u32 = 0;
        let mut bits = 0;
        for c in symbols.chars() {
            buffer = (buffer << 5) | value(c)?;
            bits += 5;
            if bits >= 8 {
                bits -= 8;
                out.push((buffer >> bits) as u8);
                buffer &= (1 << bits) - 1;
            }
        }
        if out.len() != len || buffer != 0 {
            return None;
        }
        Some(out)
    }

    /// Keeps only characters that can be part of a code (drops spaces, dashes…).
    pub fn clean(input: &str) -> String {
        input.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn roundtrip() {
            for len in 1..40 {
                let data: Vec<u8> = (0..len as u8).map(|i| i.wrapping_mul(37).wrapping_add(11)).collect();
                let enc = encode(&data);
                assert_eq!(decode(&enc, len).unwrap(), data);
            }
        }

        #[test]
        fn forgiving_on_lookalikes() {
            let enc = encode(&[0x00, 0x42, 0xff]);
            let typo = enc.replace('0', "O").replace('1', "l").to_lowercase();
            assert_eq!(decode(&typo, 3).unwrap(), vec![0x00, 0x42, 0xff]);
        }
    }
}
