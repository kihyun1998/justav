/// Supported hash algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Crc32,
}

impl HashAlgorithm {
    /// Name of this algorithm.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Crc32 => "crc32",
        }
    }

    /// Output size in bytes.
    pub const fn output_size(&self) -> usize {
        match self {
            Self::Crc32 => 4,
        }
    }

    /// Parse from name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "crc32" => Some(Self::Crc32),
            _ => None,
        }
    }
}

/// A streaming hash context.
pub struct HashContext {
    algorithm: HashAlgorithm,
    state: HashState,
}

enum HashState {
    Crc32(u32),
}

impl HashContext {
    /// Create a new hash context for the given algorithm.
    pub fn new(algorithm: HashAlgorithm) -> Self {
        let state = match algorithm {
            HashAlgorithm::Crc32 => HashState::Crc32(0xFFFF_FFFF),
        };
        Self { algorithm, state }
    }

    /// Feed data into the hash.
    pub fn update(&mut self, data: &[u8]) {
        match &mut self.state {
            HashState::Crc32(crc) => {
                for &byte in data {
                    let mut c = *crc ^ byte as u32;
                    for _ in 0..8 {
                        if c & 1 != 0 {
                            c = (c >> 1) ^ 0xEDB8_8320;
                        } else {
                            c >>= 1;
                        }
                    }
                    *crc = c;
                }
            }
        }
    }

    /// Finalize and return the hash digest as bytes.
    pub fn finalize(&self) -> Vec<u8> {
        match &self.state {
            HashState::Crc32(crc) => {
                let result = !crc;
                result.to_be_bytes().to_vec()
            }
        }
    }

    /// Finalize and return the hash as a hex string.
    pub fn finalize_hex(&self) -> String {
        self.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Reset to initial state for reuse.
    pub fn reset(&mut self) {
        self.state = match self.algorithm {
            HashAlgorithm::Crc32 => HashState::Crc32(0xFFFF_FFFF),
        };
    }

    /// The algorithm used by this context.
    pub fn algorithm(&self) -> HashAlgorithm {
        self.algorithm
    }
}

/// Convenience: compute hash of a byte slice in one call.
pub fn hash(algorithm: HashAlgorithm, data: &[u8]) -> Vec<u8> {
    let mut ctx = HashContext::new(algorithm);
    ctx.update(data);
    ctx.finalize()
}

/// Convenience: compute hash as hex string.
pub fn hash_hex(algorithm: HashAlgorithm, data: &[u8]) -> String {
    let mut ctx = HashContext::new(algorithm);
    ctx.update(data);
    ctx.finalize_hex()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn crc32_empty() {
        let h = hash_hex(HashAlgorithm::Crc32, b"");
        assert_eq!(h, "00000000");
    }

    #[test]
    fn crc32_hello() {
        // Known CRC32 of "Hello" = 0xF7D18982
        let h = hash_hex(HashAlgorithm::Crc32, b"Hello");
        assert_eq!(h, "f7d18982");
    }

    #[test]
    fn crc32_check_value() {
        // CRC32 of "123456789" = 0xCBF43926
        let h = hash_hex(HashAlgorithm::Crc32, b"123456789");
        assert_eq!(h, "cbf43926");
    }

    #[test]
    fn streaming_matches_oneshot() {
        let data = b"Hello, World!";
        let oneshot = hash(HashAlgorithm::Crc32, data);

        let mut ctx = HashContext::new(HashAlgorithm::Crc32);
        ctx.update(b"Hello, ");
        ctx.update(b"World!");
        assert_eq!(ctx.finalize(), oneshot);
    }

    #[test]
    fn reset_and_reuse() {
        let mut ctx = HashContext::new(HashAlgorithm::Crc32);
        ctx.update(b"first");
        ctx.reset();
        ctx.update(b"123456789");
        assert_eq!(ctx.finalize_hex(), "cbf43926");
    }

    #[test]
    fn output_size() {
        assert_eq!(HashAlgorithm::Crc32.output_size(), 4);
    }

    #[test]
    fn algorithm_name() {
        assert_eq!(HashAlgorithm::Crc32.name(), "crc32");
    }

    #[test]
    fn from_name() {
        assert_eq!(HashAlgorithm::from_name("crc32"), Some(HashAlgorithm::Crc32));
    }

    // ── Negative ──

    #[test]
    fn from_name_invalid() {
        assert_eq!(HashAlgorithm::from_name("sha256"), None);
        assert_eq!(HashAlgorithm::from_name(""), None);
    }
}
