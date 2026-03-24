use av_util::error::{Error, Result};
use crate::codec::CodecId;
use crate::subtitle::{SubtitleDecoder, SubtitleEntry, SubtitleFrame};

/// WebVTT subtitle decoder.
///
/// Parses WebVTT cues with timing, optional identifiers, and text.
pub struct WebVttDecoder;

impl WebVttDecoder {
    pub fn new() -> Self { Self }
}

impl Default for WebVttDecoder {
    fn default() -> Self { Self::new() }
}

impl SubtitleDecoder for WebVttDecoder {
    fn codec_id(&self) -> CodecId { CodecId::WebVtt }

    fn decode(&mut self, data: &[u8]) -> Result<SubtitleFrame> {
        let text = std::str::from_utf8(data)
            .map_err(|e| Error::InvalidData(format!("invalid UTF-8: {e}")))?;

        let mut frame = SubtitleFrame::default();
        let mut lines = text.lines().peekable();

        // Skip WEBVTT header line (required). Strip optional BOM.
        if let Some(header) = lines.next() {
            let header = header.trim_start_matches('\u{FEFF}');
            if !header.starts_with("WEBVTT") {
                return Err(Error::InvalidData("missing WEBVTT header".into()));
            }
        } else {
            return Err(Error::InvalidData("empty WebVTT file".into()));
        }

        // Skip any header metadata lines until blank line.
        for line in lines.by_ref() {
            if line.trim().is_empty() {
                break;
            }
        }

        // Parse cues.
        while lines.peek().is_some() {
            // Skip blank lines between cues.
            while let Some(&line) = lines.peek() {
                if line.trim().is_empty() {
                    lines.next();
                } else {
                    break;
                }
            }

            // Check if next line is a timing line or a cue identifier.
            let Some(&line) = lines.peek() else { break };

            let timing_line = if line.contains("-->") {
                // This is the timing line directly.
                lines.next().unwrap()
            } else {
                // This is a cue identifier; skip it, next line is timing.
                lines.next(); // skip identifier
                match lines.next() {
                    Some(l) if l.contains("-->") => l,
                    _ => continue, // malformed cue, skip
                }
            };

            // Parse timing: "00:00:01.000 --> 00:00:03.500"
            let (start_ms, end_ms) = match parse_webvtt_timing(timing_line) {
                Ok(t) => t,
                Err(_) => continue, // skip malformed timing
            };

            // Collect text lines until blank line or EOF.
            let mut cue_text = String::new();
            for line in lines.by_ref() {
                if line.trim().is_empty() {
                    break;
                }
                if !cue_text.is_empty() {
                    cue_text.push('\n');
                }
                cue_text.push_str(line);
            }

            if !cue_text.is_empty() {
                // Strip basic WebVTT tags like <b>, <i>, <u>, <c.class>, etc.
                let clean_text = strip_webvtt_tags(&cue_text);
                frame.entries.push(SubtitleEntry {
                    start_ms,
                    end_ms,
                    text: clean_text,
                });
            }
        }

        Ok(frame)
    }
}

/// Parse WebVTT timing: "HH:MM:SS.mmm --> HH:MM:SS.mmm" or "MM:SS.mmm --> MM:SS.mmm"
fn parse_webvtt_timing(line: &str) -> Result<(u64, u64)> {
    // Split on " --> " (with spaces).
    let parts: Vec<&str> = line.split("-->").collect();
    if parts.len() != 2 {
        return Err(Error::InvalidData(format!("invalid WebVTT timing: {line}")));
    }
    // Trim any positioning info after the end time (e.g. "position:10%").
    let start_str = parts[0].trim();
    let end_part = parts[1].trim();
    let end_str = end_part.split_whitespace().next().unwrap_or(end_part);

    let start = parse_webvtt_time(start_str)?;
    let end = parse_webvtt_time(end_str)?;
    Ok((start, end))
}

/// Parse "HH:MM:SS.mmm" or "MM:SS.mmm" to milliseconds.
fn parse_webvtt_time(s: &str) -> Result<u64> {
    let parts: Vec<&str> = s.split(':').collect();
    let (h, m, sec_ms) = match parts.len() {
        2 => (0u64, parts[0], parts[1]),
        3 => {
            let h: u64 = parts[0].parse().map_err(|_| Error::InvalidData(format!("bad hours: {}", parts[0])))?;
            (h, parts[1], parts[2])
        }
        _ => return Err(Error::InvalidData(format!("invalid WebVTT time: {s}"))),
    };

    let m: u64 = m.parse().map_err(|_| Error::InvalidData(format!("bad minutes: {m}")))?;

    let sec_parts: Vec<&str> = sec_ms.split('.').collect();
    let sec: u64 = sec_parts[0].parse().map_err(|_| Error::InvalidData(format!("bad seconds: {}", sec_parts[0])))?;
    let ms: u64 = if sec_parts.len() > 1 {
        let ms_str = sec_parts[1];
        // Pad to 3 digits.
        let padded = format!("{ms_str:0<3}");
        padded[..3].parse().map_err(|_| Error::InvalidData(format!("bad ms: {ms_str}")))?
    } else {
        0
    };

    Ok(h * 3_600_000 + m * 60_000 + sec * 1000 + ms)
}

/// Strip WebVTT tags like `<b>`, `</b>`, `<i>`, `<c.classname>`, etc.
fn strip_webvtt_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const WEBVTT_SAMPLE: &str = "\
WEBVTT

1
00:00:01.000 --> 00:00:03.500
Hello World

2
00:00:04.000 --> 00:00:06.000
Second <b>bold</b> line
With continuation
";

    // ── Positive ──

    #[test]
    fn parse_webvtt_basic() {
        let mut dec = WebVttDecoder::new();
        let frame = dec.decode(WEBVTT_SAMPLE.as_bytes()).unwrap();
        assert_eq!(frame.entries.len(), 2);

        assert_eq!(frame.entries[0].start_ms, 1000);
        assert_eq!(frame.entries[0].end_ms, 3500);
        assert_eq!(frame.entries[0].text, "Hello World");

        assert_eq!(frame.entries[1].start_ms, 4000);
        assert_eq!(frame.entries[1].end_ms, 6000);
        assert_eq!(frame.entries[1].text, "Second bold line\nWith continuation");
    }

    #[test]
    fn parse_webvtt_no_cue_id() {
        let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nHello\n";
        let mut dec = WebVttDecoder::new();
        let frame = dec.decode(vtt.as_bytes()).unwrap();
        assert_eq!(frame.entries.len(), 1);
        assert_eq!(frame.entries[0].text, "Hello");
    }

    #[test]
    fn parse_webvtt_mm_ss_format() {
        let vtt = "WEBVTT\n\n01:23.456 --> 02:34.567\nShort format\n";
        let mut dec = WebVttDecoder::new();
        let frame = dec.decode(vtt.as_bytes()).unwrap();
        assert_eq!(frame.entries[0].start_ms, 83456);
        assert_eq!(frame.entries[0].end_ms, 154567);
    }

    #[test]
    fn strip_tags() {
        assert_eq!(strip_webvtt_tags("<b>bold</b>"), "bold");
        assert_eq!(strip_webvtt_tags("<i>italic</i>"), "italic");
        assert_eq!(strip_webvtt_tags("<c.yellow>colored</c>"), "colored");
        assert_eq!(strip_webvtt_tags("no tags"), "no tags");
    }

    #[test]
    fn webvtt_time_parsing() {
        assert_eq!(parse_webvtt_time("00:00:00.000").unwrap(), 0);
        assert_eq!(parse_webvtt_time("01:02:03.456").unwrap(), 3_723_456);
        assert_eq!(parse_webvtt_time("00:01.500").unwrap(), 1500);
    }

    #[test]
    fn webvtt_with_positioning() {
        // Positioning info after end time should be ignored.
        let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000 position:10% line:0\nPositioned\n";
        let mut dec = WebVttDecoder::new();
        let frame = dec.decode(vtt.as_bytes()).unwrap();
        assert_eq!(frame.entries[0].end_ms, 2000);
        assert_eq!(frame.entries[0].text, "Positioned");
    }

    #[test]
    fn webvtt_with_header_metadata() {
        let vtt = "WEBVTT - Title\nKind: captions\nLanguage: en\n\n00:00:01.000 --> 00:00:02.000\nHello\n";
        let mut dec = WebVttDecoder::new();
        let frame = dec.decode(vtt.as_bytes()).unwrap();
        assert_eq!(frame.entries.len(), 1);
    }

    #[test]
    fn codec_id() {
        assert_eq!(WebVttDecoder::new().codec_id(), CodecId::WebVtt);
    }

    // ── Negative ──

    #[test]
    fn missing_webvtt_header() {
        let mut dec = WebVttDecoder::new();
        assert!(dec.decode(b"00:00:01.000 --> 00:00:02.000\nHello\n").is_err());
    }

    #[test]
    fn empty_webvtt() {
        let mut dec = WebVttDecoder::new();
        assert!(dec.decode(b"").is_err());
    }

    #[test]
    fn invalid_utf8() {
        let mut dec = WebVttDecoder::new();
        assert!(dec.decode(&[0xFF, 0xFE]).is_err());
    }

    #[test]
    fn bad_time_format() {
        assert!(parse_webvtt_time("invalid").is_err());
        assert!(parse_webvtt_time("aa:bb:cc.ddd").is_err());
    }

    // ── Edge ──

    #[test]
    fn webvtt_only_header() {
        let mut dec = WebVttDecoder::new();
        let frame = dec.decode(b"WEBVTT\n\n").unwrap();
        assert!(frame.entries.is_empty());
    }

    #[test]
    fn webvtt_bom() {
        let vtt = "\u{FEFF}WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nBOM test\n";
        let mut dec = WebVttDecoder::new();
        let frame = dec.decode(vtt.as_bytes()).unwrap();
        assert_eq!(frame.entries.len(), 1);
    }
}
