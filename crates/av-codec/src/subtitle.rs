use av_util::error::{Error, Result};

use crate::codec::CodecId;

/// A single subtitle entry with timing and text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleEntry {
    /// Start time in milliseconds.
    pub start_ms: u64,
    /// End time in milliseconds.
    pub end_ms: u64,
    /// The text content (may contain style tags for ASS/WebVTT).
    pub text: String,
}

/// A decoded subtitle frame containing one or more entries.
#[derive(Debug, Clone, Default)]
pub struct SubtitleFrame {
    pub entries: Vec<SubtitleEntry>,
}

/// Trait for subtitle decoders.
pub trait SubtitleDecoder: Send {
    fn codec_id(&self) -> CodecId;
    fn decode(&mut self, data: &[u8]) -> Result<SubtitleFrame>;
}

/// SRT (SubRip) subtitle parser.
pub struct SrtDecoder;

impl SrtDecoder {
    pub fn new() -> Self { Self }
}

impl Default for SrtDecoder {
    fn default() -> Self { Self::new() }
}

impl SubtitleDecoder for SrtDecoder {
    fn codec_id(&self) -> CodecId { CodecId::Srt }

    fn decode(&mut self, data: &[u8]) -> Result<SubtitleFrame> {
        let text = std::str::from_utf8(data)
            .map_err(|e| Error::InvalidData(format!("invalid UTF-8: {e}")))?;

        let mut frame = SubtitleFrame::default();
        let mut lines = text.lines().peekable();

        while lines.peek().is_some() {
            // Skip sequence number.
            let seq_line = match lines.next() {
                Some(l) => l.trim(),
                None => break,
            };
            if seq_line.is_empty() {
                continue;
            }
            // Verify it's a number (sequence number).
            if seq_line.parse::<u64>().is_err() {
                continue;
            }

            // Parse timing line: "00:01:23,456 --> 00:01:25,789"
            let timing = match lines.next() {
                Some(l) => l.trim(),
                None => break,
            };
            let (start_ms, end_ms) = parse_srt_timing(timing)?;

            // Collect text lines until blank line.
            let mut text_buf = String::new();
            for line in lines.by_ref() {
                if line.trim().is_empty() {
                    break;
                }
                if !text_buf.is_empty() {
                    text_buf.push('\n');
                }
                text_buf.push_str(line);
            }

            if !text_buf.is_empty() {
                frame.entries.push(SubtitleEntry {
                    start_ms,
                    end_ms,
                    text: text_buf,
                });
            }
        }

        Ok(frame)
    }
}

/// Parse "HH:MM:SS,mmm --> HH:MM:SS,mmm" into (start_ms, end_ms).
fn parse_srt_timing(line: &str) -> Result<(u64, u64)> {
    let parts: Vec<&str> = line.split("-->").collect();
    if parts.len() != 2 {
        return Err(Error::InvalidData(format!("invalid SRT timing: {line}")));
    }
    let start = parse_srt_time(parts[0].trim())?;
    let end = parse_srt_time(parts[1].trim())?;
    Ok((start, end))
}

/// Parse "HH:MM:SS,mmm" into milliseconds.
fn parse_srt_time(s: &str) -> Result<u64> {
    // Accept both ',' and '.' as millisecond separator.
    let s = s.replace(',', ".");
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return Err(Error::InvalidData(format!("invalid SRT time: {s}")));
    }
    let h: u64 = parts[0].parse().map_err(|_| Error::InvalidData(format!("bad hours: {}", parts[0])))?;
    let m: u64 = parts[1].parse().map_err(|_| Error::InvalidData(format!("bad minutes: {}", parts[1])))?;

    let sec_parts: Vec<&str> = parts[2].split('.').collect();
    let sec: u64 = sec_parts[0].parse().map_err(|_| Error::InvalidData(format!("bad seconds: {}", sec_parts[0])))?;
    let ms: u64 = if sec_parts.len() > 1 {
        sec_parts[1].parse().map_err(|_| Error::InvalidData(format!("bad ms: {}", sec_parts[1])))?
    } else {
        0
    };

    Ok(h * 3_600_000 + m * 60_000 + sec * 1000 + ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRT_SAMPLE: &str = "\
1
00:00:01,000 --> 00:00:03,500
Hello World

2
00:00:04,000 --> 00:00:06,000
Second line
With continuation

";

    // ── Positive ──

    #[test]
    fn parse_srt_basic() {
        let mut dec = SrtDecoder::new();
        let frame = dec.decode(SRT_SAMPLE.as_bytes()).unwrap();
        assert_eq!(frame.entries.len(), 2);

        assert_eq!(frame.entries[0].start_ms, 1000);
        assert_eq!(frame.entries[0].end_ms, 3500);
        assert_eq!(frame.entries[0].text, "Hello World");

        assert_eq!(frame.entries[1].start_ms, 4000);
        assert_eq!(frame.entries[1].end_ms, 6000);
        assert_eq!(frame.entries[1].text, "Second line\nWith continuation");
    }

    #[test]
    fn parse_srt_time_values() {
        assert_eq!(parse_srt_time("01:02:03,456").unwrap(), 3_723_456);
        assert_eq!(parse_srt_time("00:00:00,000").unwrap(), 0);
        assert_eq!(parse_srt_time("00:00:01.500").unwrap(), 1500); // dot separator
    }

    #[test]
    fn parse_srt_empty_input() {
        let mut dec = SrtDecoder::new();
        let frame = dec.decode(b"").unwrap();
        assert!(frame.entries.is_empty());
    }

    #[test]
    fn srt_decoder_codec_id() {
        let dec = SrtDecoder::new();
        assert_eq!(dec.codec_id(), CodecId::Srt);
    }

    // ── Negative ──

    #[test]
    fn parse_srt_invalid_utf8() {
        let mut dec = SrtDecoder::new();
        assert!(dec.decode(&[0xFF, 0xFE]).is_err());
    }

    #[test]
    fn parse_srt_bad_timing() {
        assert!(parse_srt_timing("not a timing line").is_err());
    }

    #[test]
    fn parse_srt_bad_time_format() {
        assert!(parse_srt_time("invalid").is_err());
        assert!(parse_srt_time("aa:bb:cc,ddd").is_err());
    }

    // ── Edge ──

    #[test]
    fn parse_srt_single_entry() {
        let srt = "1\n00:00:00,000 --> 00:00:01,000\nHello\n\n";
        let mut dec = SrtDecoder::new();
        let frame = dec.decode(srt.as_bytes()).unwrap();
        assert_eq!(frame.entries.len(), 1);
    }

    #[test]
    fn parse_srt_overlapping_timing() {
        // Overlapping subtitles are valid in SRT — parser should accept them.
        let srt = "\
1
00:00:01,000 --> 00:00:05,000
First (long)

2
00:00:02,000 --> 00:00:03,000
Second (overlapping)

";
        let mut dec = SrtDecoder::new();
        let frame = dec.decode(srt.as_bytes()).unwrap();
        assert_eq!(frame.entries.len(), 2);
        // First starts at 1s, ends at 5s.
        assert_eq!(frame.entries[0].start_ms, 1000);
        assert_eq!(frame.entries[0].end_ms, 5000);
        // Second starts at 2s (inside first), ends at 3s.
        assert_eq!(frame.entries[1].start_ms, 2000);
        assert_eq!(frame.entries[1].end_ms, 3000);
    }
}
