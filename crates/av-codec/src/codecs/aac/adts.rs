use av_util::error::{Error, Result};
use crate::codec::CodecId;
use crate::codec_par::CodecParameters;

/// AAC profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AacProfile {
    /// AAC Main.
    Main = 0,
    /// AAC-LC (Low Complexity) — most common.
    Lc = 1,
    /// AAC SSR (Scalable Sample Rate).
    Ssr = 2,
    /// AAC LTP (Long Term Prediction).
    Ltp = 3,
}

impl AacProfile {
    pub fn from_index(idx: u8) -> Result<Self> {
        match idx {
            0 => Ok(Self::Main),
            1 => Ok(Self::Lc),
            2 => Ok(Self::Ssr),
            3 => Ok(Self::Ltp),
            _ => Err(Error::InvalidData(format!("unknown AAC profile index: {idx}"))),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Main => "AAC Main",
            Self::Lc => "AAC-LC",
            Self::Ssr => "AAC SSR",
            Self::Ltp => "AAC LTP",
        }
    }
}

/// Standard AAC sample rates indexed by `sampling_frequency_index`.
const SAMPLE_RATES: [u32; 13] = [
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050,
    16000, 12000, 11025, 8000, 7350,
];

/// Parsed ADTS frame header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdtsHeader {
    /// AAC profile (0-3).
    pub profile: AacProfile,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Sampling frequency index (0-12).
    pub sample_rate_index: u8,
    /// Channel configuration (1-7).
    pub channel_config: u8,
    /// Number of audio channels.
    pub channels: u16,
    /// Total frame length in bytes (including header).
    pub frame_length: u16,
    /// True if CRC is present (protection_absent = 0).
    pub has_crc: bool,
    /// ADTS header size (7 or 9 bytes).
    pub header_size: u8,
}

impl AdtsHeader {
    /// Payload size (frame_length minus header).
    pub fn payload_size(&self) -> u16 {
        self.frame_length.saturating_sub(self.header_size as u16)
    }

    /// Convert ADTS header info to CodecParameters.
    pub fn to_codec_parameters(&self) -> Result<CodecParameters> {
        let channels = if self.channels > 0 { self.channels } else { 2 };
        let mut par = CodecParameters::new_audio(CodecId::Aac, self.sample_rate, channels)?;
        par.codec_id = CodecId::Aac;
        Ok(par)
    }
}

/// Parse an ADTS header from the given bytes.
///
/// Requires at least 7 bytes. Returns the parsed header.
pub fn parse_adts_header(data: &[u8]) -> Result<AdtsHeader> {
    if data.len() < 7 {
        return Err(Error::InvalidData("ADTS header requires at least 7 bytes".into()));
    }

    // Sync word: 12 bits (0xFFF).
    let sync = ((data[0] as u16) << 4) | ((data[1] as u16) >> 4);
    if sync != 0xFFF {
        return Err(Error::InvalidData(format!(
            "ADTS sync word mismatch: expected 0xFFF, got {sync:#05X}"
        )));
    }

    // Bit layout (after sync):
    // [1] ID (0=MPEG-4, 1=MPEG-2)
    // [2] Layer (always 00)
    // [1] protection_absent (1=no CRC, 0=CRC present)
    let protection_absent = data[1] & 0x01;
    let has_crc = protection_absent == 0;
    let header_size = if has_crc { 9 } else { 7 };

    // [2] profile (object_type - 1)
    let profile_idx = (data[2] >> 6) & 0x03;
    let profile = AacProfile::from_index(profile_idx)?;

    // [4] sampling_frequency_index
    let sf_index = (data[2] >> 2) & 0x0F;
    if sf_index as usize >= SAMPLE_RATES.len() {
        return Err(Error::InvalidData(format!(
            "invalid sampling_frequency_index: {sf_index}"
        )));
    }
    let sample_rate = SAMPLE_RATES[sf_index as usize];

    // [1] private_bit (skip)
    // [3] channel_configuration
    let channel_config = ((data[2] & 0x01) << 2) | ((data[3] >> 6) & 0x03);
    let channels = match channel_config {
        1 => 1, 2 => 2, 3 => 3, 4 => 4, 5 => 5, 6 => 6, 7 => 8,
        0 => 0, // defined in stream
        _ => return Err(Error::InvalidData(format!("invalid channel_config: {channel_config}"))),
    };

    // [13] frame_length (includes header)
    let frame_length = (((data[3] & 0x03) as u16) << 11)
        | ((data[4] as u16) << 3)
        | ((data[5] >> 5) as u16);

    if (frame_length as u8) < header_size {
        return Err(Error::InvalidData(format!(
            "ADTS frame_length {frame_length} < header_size {header_size}"
        )));
    }

    Ok(AdtsHeader {
        profile,
        sample_rate,
        sample_rate_index: sf_index,
        channel_config,
        channels,
        frame_length,
        has_crc,
        header_size,
    })
}

/// Split an ADTS byte stream into individual frames.
///
/// Returns a list of (header, payload_data) pairs.
pub fn split_adts_frames(data: &[u8]) -> Result<Vec<(AdtsHeader, Vec<u8>)>> {
    let mut frames = Vec::new();
    let mut pos = 0;

    while pos + 7 <= data.len() {
        // Find sync word.
        if data[pos] != 0xFF || (data[pos + 1] & 0xF0) != 0xF0 {
            pos += 1;
            continue;
        }

        let header = parse_adts_header(&data[pos..])?;
        let frame_end = pos + header.frame_length as usize;
        if frame_end > data.len() {
            break; // Incomplete frame at end — don't error, just stop.
        }

        let payload_start = pos + header.header_size as usize;
        let payload = data[payload_start..frame_end].to_vec();
        frames.push((header, payload));
        pos = frame_end;
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid ADTS header (7 bytes, no CRC).
    fn build_adts_header(profile: u8, sf_index: u8, channels: u8, payload_size: u16) -> Vec<u8> {
        let frame_length = payload_size + 7;
        let mut h = vec![0u8; 7];
        // Sync word (12 bits) + ID(1) + Layer(2) + protection_absent(1)
        h[0] = 0xFF;
        h[1] = 0xF1; // sync=0xFFF, ID=0, Layer=00, protection_absent=1
        // profile(2) + sf_index(4) + private(1) + channel_config_high(1)
        h[2] = (profile << 6) | (sf_index << 2) | ((channels >> 2) & 0x01);
        // channel_config_low(2) + originality(1) + home(1) + copyright_id(1) + copyright_start(1) + frame_length_high(2)
        h[3] = ((channels & 0x03) << 6) | ((frame_length >> 11) as u8 & 0x03);
        // frame_length_mid(8)
        h[4] = (frame_length >> 3) as u8;
        // frame_length_low(3) + buffer_fullness_high(5)
        h[5] = ((frame_length & 0x07) as u8) << 5 | 0x1F;
        // buffer_fullness_low(6) + num_raw_data_blocks(2)
        h[6] = 0xFC;
        h
    }

    // ── Positive ──

    #[test]
    fn parse_adts_aac_lc_44100_stereo() {
        let header_bytes = build_adts_header(1, 4, 2, 100); // LC, 44100, stereo, 100 payload
        let h = parse_adts_header(&header_bytes).unwrap();
        assert_eq!(h.profile, AacProfile::Lc);
        assert_eq!(h.sample_rate, 44100);
        assert_eq!(h.channels, 2);
        assert_eq!(h.frame_length, 107);
        assert_eq!(h.payload_size(), 100);
        assert!(!h.has_crc);
        assert_eq!(h.header_size, 7);
    }

    #[test]
    fn parse_adts_48000_mono() {
        let header_bytes = build_adts_header(1, 3, 1, 50); // LC, 48000, mono
        let h = parse_adts_header(&header_bytes).unwrap();
        assert_eq!(h.sample_rate, 48000);
        assert_eq!(h.channels, 1);
    }

    #[test]
    fn parse_adts_all_profiles() {
        for (idx, expected) in [(0, AacProfile::Main), (1, AacProfile::Lc), (2, AacProfile::Ssr), (3, AacProfile::Ltp)] {
            let h = build_adts_header(idx, 4, 2, 10);
            let parsed = parse_adts_header(&h).unwrap();
            assert_eq!(parsed.profile, expected);
        }
    }

    #[test]
    fn split_adts_two_frames() {
        let mut data = build_adts_header(1, 4, 2, 5);
        data.extend_from_slice(&[0xAA; 5]); // payload 1
        let mut frame2 = build_adts_header(1, 4, 2, 3);
        frame2.extend_from_slice(&[0xBB; 3]); // payload 2
        data.extend_from_slice(&frame2);

        let frames = split_adts_frames(&data).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].1, vec![0xAA; 5]);
        assert_eq!(frames[1].1, vec![0xBB; 3]);
    }

    #[test]
    fn all_sample_rates() {
        for (idx, &expected) in SAMPLE_RATES.iter().enumerate() {
            let h = build_adts_header(1, idx as u8, 2, 10);
            let parsed = parse_adts_header(&h).unwrap();
            assert_eq!(parsed.sample_rate, expected);
        }
    }

    #[test]
    fn profile_names() {
        assert_eq!(AacProfile::Lc.name(), "AAC-LC");
        assert_eq!(AacProfile::Main.name(), "AAC Main");
    }

    // ── Negative ──

    #[test]
    fn parse_adts_too_short() {
        assert!(parse_adts_header(&[0xFF, 0xF1, 0x00]).is_err());
    }

    #[test]
    fn parse_adts_bad_sync() {
        let mut h = build_adts_header(1, 4, 2, 10);
        h[0] = 0x00; // break sync
        assert!(parse_adts_header(&h).is_err());
    }

    #[test]
    fn parse_adts_invalid_sf_index() {
        let mut h = build_adts_header(1, 4, 2, 10);
        h[2] = (h[2] & 0xC3) | (0x0F << 2); // sf_index = 15 (invalid)
        assert!(parse_adts_header(&h).is_err());
    }

    #[test]
    fn split_adts_truncated_frame() {
        let mut data = build_adts_header(1, 4, 2, 100);
        data.extend_from_slice(&[0x00; 10]); // only 10 of 100 payload bytes
        let frames = split_adts_frames(&data).unwrap();
        assert!(frames.is_empty()); // incomplete frame skipped
    }

    #[test]
    fn split_adts_empty() {
        let frames = split_adts_frames(&[]).unwrap();
        assert!(frames.is_empty());
    }
}
