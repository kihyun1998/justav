use av_util::error::{Error, Result};
use crate::codec::CodecId;
use crate::subtitle::{SubtitleDecoder, SubtitleEntry, SubtitleFrame};

/// ASS/SSA subtitle decoder.
///
/// Parses the `[Events]` section of ASS/SSA files, extracting
/// timing and text from `Dialogue:` lines.
pub struct AssDecoder;

impl AssDecoder {
    pub fn new() -> Self { Self }
}

impl Default for AssDecoder {
    fn default() -> Self { Self::new() }
}

impl SubtitleDecoder for AssDecoder {
    fn codec_id(&self) -> CodecId { CodecId::Ass }

    fn decode(&mut self, data: &[u8]) -> Result<SubtitleFrame> {
        let text = std::str::from_utf8(data)
            .map_err(|e| Error::InvalidData(format!("invalid UTF-8: {e}")))?;

        let mut frame = SubtitleFrame::default();

        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("Dialogue:") && let Ok(entry) = parse_dialogue_line(rest.trim()) {
                frame.entries.push(entry);
            }
        }
        Ok(frame)
    }
}

/// Parse an ASS Dialogue line.
///
/// Format: `Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text`
/// Example: `0,0:01:23.45,0:01:25.67,Default,,0,0,0,,Hello {\i1}World{\i0}`
fn parse_dialogue_line(line: &str) -> Result<SubtitleEntry> {
    // Split into at most 10 fields (Text may contain commas).
    let parts: Vec<&str> = line.splitn(10, ',').collect();
    if parts.len() < 10 {
        return Err(Error::InvalidData(format!(
            "ASS dialogue needs 10 fields, got {}", parts.len()
        )));
    }

    let start_ms = parse_ass_time(parts[1].trim())?;
    let end_ms = parse_ass_time(parts[2].trim())?;
    let raw_text = parts[9];

    // Strip ASS override tags like {\b1}, {\i0}, {\an8}, etc.
    let text = strip_ass_tags(raw_text);
    // Replace \N (ASS newline) with actual newline.
    let text = text.replace("\\N", "\n").replace("\\n", "\n");

    Ok(SubtitleEntry {
        start_ms,
        end_ms,
        text,
    })
}

/// Parse ASS time format: `H:MM:SS.cc` (centiseconds, not milliseconds).
fn parse_ass_time(s: &str) -> Result<u64> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return Err(Error::InvalidData(format!("invalid ASS time: {s}")));
    }
    let h: u64 = parts[0].parse().map_err(|_| Error::InvalidData(format!("bad hours: {}", parts[0])))?;
    let m: u64 = parts[1].parse().map_err(|_| Error::InvalidData(format!("bad minutes: {}", parts[1])))?;

    let sec_parts: Vec<&str> = parts[2].split('.').collect();
    let sec: u64 = sec_parts[0].parse().map_err(|_| Error::InvalidData(format!("bad seconds: {}", sec_parts[0])))?;
    let cs: u64 = if sec_parts.len() > 1 {
        let cs_str = sec_parts[1];
        // Pad/truncate to 2 digits (centiseconds).
        let cs_str = if cs_str.len() == 1 {
            format!("{cs_str}0")
        } else {
            cs_str[..2.min(cs_str.len())].to_string()
        };
        cs_str.parse().map_err(|_| Error::InvalidData(format!("bad centiseconds: {}", sec_parts[1])))?
    } else {
        0
    };

    Ok(h * 3_600_000 + m * 60_000 + sec * 1000 + cs * 10)
}

/// Strip ASS override tags `{...}` from text.
fn strip_ass_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '{' => in_tag = true,
            '}' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASS_SAMPLE: &str = "\
[Script Info]
Title: Test

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello World
Dialogue: 0,0:00:04.00,0:00:06.00,Default,,0,0,0,,Second {\\b1}bold{\\b0} line
";

    // ── Positive ──

    #[test]
    fn parse_ass_basic() {
        let mut dec = AssDecoder::new();
        let frame = dec.decode(ASS_SAMPLE.as_bytes()).unwrap();
        assert_eq!(frame.entries.len(), 2);

        assert_eq!(frame.entries[0].start_ms, 1000);
        assert_eq!(frame.entries[0].end_ms, 3500);
        assert_eq!(frame.entries[0].text, "Hello World");

        assert_eq!(frame.entries[1].start_ms, 4000);
        assert_eq!(frame.entries[1].end_ms, 6000);
        assert_eq!(frame.entries[1].text, "Second bold line");
    }

    #[test]
    fn strip_tags() {
        assert_eq!(strip_ass_tags("{\\an8}Top text"), "Top text");
        assert_eq!(strip_ass_tags("{\\i1}italic{\\i0}"), "italic");
        assert_eq!(strip_ass_tags("no tags"), "no tags");
        assert_eq!(strip_ass_tags(""), "");
    }

    #[test]
    fn ass_newlines() {
        let line = "0,0:00:00.00,0:00:01.00,Default,,0,0,0,,Line 1\\NLine 2";
        let entry = parse_dialogue_line(line).unwrap();
        assert_eq!(entry.text, "Line 1\nLine 2");
    }

    #[test]
    fn ass_time_parsing() {
        assert_eq!(parse_ass_time("0:00:00.00").unwrap(), 0);
        assert_eq!(parse_ass_time("0:01:23.45").unwrap(), 83450);
        assert_eq!(parse_ass_time("1:00:00.00").unwrap(), 3_600_000);
    }

    #[test]
    fn ass_text_with_commas() {
        let line = "0,0:00:00.00,0:00:01.00,Default,,0,0,0,,Hello, World, Test";
        let entry = parse_dialogue_line(line).unwrap();
        assert_eq!(entry.text, "Hello, World, Test");
    }

    #[test]
    fn codec_id() {
        assert_eq!(AssDecoder::new().codec_id(), CodecId::Ass);
    }

    // ── Negative ──

    #[test]
    fn parse_ass_invalid_utf8() {
        let mut dec = AssDecoder::new();
        assert!(dec.decode(&[0xFF, 0xFE]).is_err());
    }

    #[test]
    fn parse_ass_bad_time() {
        assert!(parse_ass_time("invalid").is_err());
        assert!(parse_ass_time("0:00").is_err());
    }

    #[test]
    fn parse_dialogue_too_few_fields() {
        assert!(parse_dialogue_line("0,0:00:00.00,0:00:01.00").is_err());
    }

    // ── Edge ──

    #[test]
    fn parse_ass_empty() {
        let mut dec = AssDecoder::new();
        let frame = dec.decode(b"").unwrap();
        assert!(frame.entries.is_empty());
    }

    #[test]
    fn parse_ass_no_events() {
        let mut dec = AssDecoder::new();
        let frame = dec.decode(b"[Script Info]\nTitle: Test\n").unwrap();
        assert!(frame.entries.is_empty());
    }
}
