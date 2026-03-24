use crate::error::{Error, Result};

const ENCODE_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode bytes to Base64 string.
pub fn encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(ENCODE_TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ENCODE_TABLE[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            out.push(ENCODE_TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ENCODE_TABLE[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Decode a Base64 string to bytes.
pub fn decode(input: &str) -> Result<Vec<u8>> {
    let input = input.trim_end_matches('=');
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;

    for ch in input.bytes() {
        let val = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b' ' | b'\n' | b'\r' | b'\t' => continue, // skip whitespace
            _ => return Err(Error::InvalidData(format!("invalid base64 char: {}", ch as char))),
        };
        buf = (buf << 6) | val as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn encode_empty() {
        assert_eq!(encode(b""), "");
    }

    #[test]
    fn encode_hello() {
        assert_eq!(encode(b"Hello"), "SGVsbG8=");
    }

    #[test]
    fn encode_padding_1() {
        assert_eq!(encode(b"a"), "YQ==");
    }

    #[test]
    fn encode_padding_2() {
        assert_eq!(encode(b"ab"), "YWI=");
    }

    #[test]
    fn encode_no_padding() {
        assert_eq!(encode(b"abc"), "YWJj");
    }

    #[test]
    fn decode_hello() {
        assert_eq!(decode("SGVsbG8=").unwrap(), b"Hello");
    }

    #[test]
    fn roundtrip() {
        let data = b"The quick brown fox jumps over the lazy dog";
        assert_eq!(decode(&encode(data)).unwrap(), data);
    }

    #[test]
    fn roundtrip_binary() {
        let data: Vec<u8> = (0..=255).collect();
        assert_eq!(decode(&encode(&data)).unwrap(), data);
    }

    #[test]
    fn decode_ignores_whitespace() {
        assert_eq!(decode("SGVs\nbG8=").unwrap(), b"Hello");
    }

    // ── Negative ──

    #[test]
    fn decode_invalid_char() {
        assert!(decode("SGVs!G8=").is_err());
    }

    // ── Edge ──

    #[test]
    fn decode_empty() {
        assert_eq!(decode("").unwrap(), b"");
    }

    #[test]
    fn roundtrip_single_byte() {
        for b in 0..=255u8 {
            let encoded = encode(&[b]);
            let decoded = decode(&encoded).unwrap();
            assert_eq!(decoded, vec![b]);
        }
    }
}
