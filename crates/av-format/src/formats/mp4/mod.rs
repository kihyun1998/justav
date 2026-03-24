pub mod boxes;
pub mod demux;
pub mod mux;

/// MP4 format probe function.
pub fn probe(data: &[u8]) -> u32 {
    if data.len() < 8 {
        return 0;
    }
    // Check for ftyp box at offset 4.
    if &data[4..8] == b"ftyp" {
        return 95;
    }
    // Some files start with moov or mdat.
    if &data[4..8] == b"moov" || &data[4..8] == b"free" || &data[4..8] == b"mdat" {
        return 50;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_ftyp() {
        let mut data = vec![0u8; 20];
        data[0..4].copy_from_slice(&20u32.to_be_bytes());
        data[4..8].copy_from_slice(b"ftyp");
        assert_eq!(probe(&data), 95);
    }

    #[test]
    fn probe_no_match() {
        assert_eq!(probe(b"RIFFWAVE"), 0);
    }

    #[test]
    fn probe_too_short() {
        assert_eq!(probe(b"short"), 0);
    }
}
