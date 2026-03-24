/// Result of probing a format.
#[derive(Debug, Clone)]
pub struct ProbeResult {
    /// Name of the matched format.
    pub format_name: String,
    /// Confidence score (0-100). Higher = more confident.
    pub score: u32,
}

/// Maximum bytes to read for probing.
pub const PROBE_BUF_SIZE: usize = 4096;

/// Probe function signature: given the first bytes, return a score 0-100.
pub type ProbeFn = fn(data: &[u8]) -> u32;

/// A registered format probe entry.
pub struct ProbeEntry {
    pub format_name: &'static str,
    pub probe_fn: ProbeFn,
}

/// Probe a buffer against all registered formats, returning the best match.
pub fn probe_buffer(data: &[u8], entries: &[ProbeEntry]) -> Option<ProbeResult> {
    let mut best: Option<ProbeResult> = None;
    for entry in entries {
        let score = (entry.probe_fn)(data);
        if score > 0 && best.as_ref().is_none_or(|b| score > b.score) {
            best = Some(ProbeResult {
                format_name: entry.format_name.to_string(),
                score,
            });
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_probe(data: &[u8]) -> u32 {
        if data.starts_with(b"RIFF") { 80 } else { 0 }
    }

    fn dummy_probe2(data: &[u8]) -> u32 {
        if data.starts_with(b"\x1aE") { 90 } else { 0 }
    }

    #[test]
    fn probe_matches() {
        let entries = [
            ProbeEntry { format_name: "wav", probe_fn: dummy_probe },
        ];
        let result = probe_buffer(b"RIFF\x00\x00\x00\x00WAVEfmt ", &entries).unwrap();
        assert_eq!(result.format_name, "wav");
        assert_eq!(result.score, 80);
    }

    #[test]
    fn probe_no_match() {
        let entries = [
            ProbeEntry { format_name: "wav", probe_fn: dummy_probe },
        ];
        assert!(probe_buffer(b"NOT_RIFF_DATA", &entries).is_none());
    }

    #[test]
    fn probe_best_score_wins() {
        let entries = [
            ProbeEntry { format_name: "wav", probe_fn: dummy_probe },
            ProbeEntry { format_name: "mkv", probe_fn: dummy_probe2 },
        ];
        let result = probe_buffer(b"\x1aE\xdf\xa3", &entries).unwrap();
        assert_eq!(result.format_name, "mkv");
        assert_eq!(result.score, 90);
    }
}
