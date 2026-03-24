use av_util::error::{Error, Result};
use crate::codec::CodecId;
use crate::codec_par::CodecParameters;

/// Opus codec mode (RFC 6716 Section 3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpusMode {
    /// SILK-only mode (narrowband to wideband speech).
    Silk,
    /// Hybrid mode (SILK + CELT, super-wideband/fullband).
    Hybrid,
    /// CELT-only mode (fullband music/audio).
    Celt,
}

/// Opus bandwidth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpusBandwidth {
    /// 4 kHz (narrowband).
    Narrowband,
    /// 6 kHz (medium band).
    Mediumband,
    /// 8 kHz (wideband).
    Wideband,
    /// 12 kHz (super wideband).
    SuperWideband,
    /// 20 kHz (fullband).
    Fullband,
}

impl OpusBandwidth {
    pub fn sample_rate(&self) -> u32 {
        match self {
            Self::Narrowband => 8000,
            Self::Mediumband => 12000,
            Self::Wideband => 16000,
            Self::SuperWideband => 24000,
            Self::Fullband => 48000,
        }
    }
}

/// Opus frame duration in samples at 48 kHz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpusFrameDuration {
    /// Duration in samples at 48 kHz.
    pub samples_48k: u16,
}

impl OpusFrameDuration {
    /// Duration in milliseconds.
    pub fn ms(&self) -> f32 {
        self.samples_48k as f32 / 48.0
    }
}

/// Parsed Opus TOC (Table of Contents) byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpusToc {
    /// Codec mode.
    pub mode: OpusMode,
    /// Audio bandwidth.
    pub bandwidth: OpusBandwidth,
    /// Duration of each frame.
    pub frame_duration: OpusFrameDuration,
    /// Frame count code (0-3) from TOC.
    pub frame_count_code: u8,
    /// Whether this is stereo (s bit).
    pub stereo: bool,
    /// Config index (0-31).
    pub config: u8,
}

impl OpusToc {
    /// Convert TOC info to CodecParameters.
    ///
    /// Opus always operates at 48 kHz internally.
    pub fn to_codec_parameters(&self) -> Result<CodecParameters> {
        let channels = if self.stereo { 2 } else { 1 };
        let mut par = CodecParameters::new_audio(CodecId::Opus, 48000, channels)?;
        par.codec_id = CodecId::Opus;
        Ok(par)
    }
}

/// Parse the TOC byte of an Opus packet (RFC 6716 Section 3.1).
///
/// The TOC byte is the first byte of every Opus packet.
pub fn parse_toc(data: &[u8]) -> Result<OpusToc> {
    if data.is_empty() {
        return Err(Error::InvalidData("empty Opus packet".into()));
    }

    let toc = data[0];
    let config = (toc >> 3) & 0x1F; // bits 7-3
    let stereo = (toc >> 2) & 0x01 != 0; // bit 2
    let frame_count_code = toc & 0x03; // bits 1-0

    let (mode, bandwidth, duration_samples) = decode_config(config)?;

    Ok(OpusToc {
        mode,
        bandwidth,
        frame_duration: OpusFrameDuration { samples_48k: duration_samples },
        frame_count_code,
        stereo,
        config,
    })
}

/// Determine the number of frames in a packet based on TOC and packet data.
///
/// RFC 6716 Section 3.2:
/// - Code 0: 1 frame
/// - Code 1: 2 frames, equal size
/// - Code 2: 2 frames, different size
/// - Code 3: arbitrary number (count in byte 2)
pub fn frame_count(data: &[u8]) -> Result<u8> {
    if data.is_empty() {
        return Err(Error::InvalidData("empty Opus packet".into()));
    }
    let code = data[0] & 0x03;
    match code {
        0 => Ok(1),
        1 | 2 => Ok(2),
        3 => {
            if data.len() < 2 {
                return Err(Error::InvalidData("Opus code 3 packet too short".into()));
            }
            let count = data[1] & 0x3F;
            if count == 0 {
                return Err(Error::InvalidData("Opus code 3 with 0 frames".into()));
            }
            Ok(count)
        }
        _ => unreachable!(),
    }
}

/// Decode the config index (0-31) into (mode, bandwidth, frame_duration_samples_48k).
fn decode_config(config: u8) -> Result<(OpusMode, OpusBandwidth, u16)> {
    // RFC 6716 Table 2.
    let (mode, bw, dur) = match config {
        // SILK-only (configs 0-11)
        0 => (OpusMode::Silk, OpusBandwidth::Narrowband, 480),   // 10ms
        1 => (OpusMode::Silk, OpusBandwidth::Narrowband, 960),   // 20ms
        2 => (OpusMode::Silk, OpusBandwidth::Narrowband, 1920),  // 40ms
        3 => (OpusMode::Silk, OpusBandwidth::Narrowband, 2880),  // 60ms
        4 => (OpusMode::Silk, OpusBandwidth::Mediumband, 480),
        5 => (OpusMode::Silk, OpusBandwidth::Mediumband, 960),
        6 => (OpusMode::Silk, OpusBandwidth::Mediumband, 1920),
        7 => (OpusMode::Silk, OpusBandwidth::Mediumband, 2880),
        8 => (OpusMode::Silk, OpusBandwidth::Wideband, 480),
        9 => (OpusMode::Silk, OpusBandwidth::Wideband, 960),
        10 => (OpusMode::Silk, OpusBandwidth::Wideband, 1920),
        11 => (OpusMode::Silk, OpusBandwidth::Wideband, 2880),
        // Hybrid (configs 12-15)
        12 => (OpusMode::Hybrid, OpusBandwidth::SuperWideband, 480),
        13 => (OpusMode::Hybrid, OpusBandwidth::SuperWideband, 960),
        14 => (OpusMode::Hybrid, OpusBandwidth::Fullband, 480),
        15 => (OpusMode::Hybrid, OpusBandwidth::Fullband, 960),
        // CELT-only (configs 16-31)
        16 => (OpusMode::Celt, OpusBandwidth::Narrowband, 120),  // 2.5ms
        17 => (OpusMode::Celt, OpusBandwidth::Narrowband, 240),  // 5ms
        18 => (OpusMode::Celt, OpusBandwidth::Narrowband, 480),
        19 => (OpusMode::Celt, OpusBandwidth::Narrowband, 960),
        20 => (OpusMode::Celt, OpusBandwidth::Wideband, 120),
        21 => (OpusMode::Celt, OpusBandwidth::Wideband, 240),
        22 => (OpusMode::Celt, OpusBandwidth::Wideband, 480),
        23 => (OpusMode::Celt, OpusBandwidth::Wideband, 960),
        24 => (OpusMode::Celt, OpusBandwidth::SuperWideband, 120),
        25 => (OpusMode::Celt, OpusBandwidth::SuperWideband, 240),
        26 => (OpusMode::Celt, OpusBandwidth::SuperWideband, 480),
        27 => (OpusMode::Celt, OpusBandwidth::SuperWideband, 960),
        28 => (OpusMode::Celt, OpusBandwidth::Fullband, 120),
        29 => (OpusMode::Celt, OpusBandwidth::Fullband, 240),
        30 => (OpusMode::Celt, OpusBandwidth::Fullband, 480),
        31 => (OpusMode::Celt, OpusBandwidth::Fullband, 960),
        _ => return Err(Error::InvalidData(format!("invalid Opus config: {config}"))),
    };
    Ok((mode, bw, dur))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn parse_toc_silk_narrowband_20ms_mono() {
        // config=1 (SILK NB 20ms), stereo=0, code=0 (1 frame)
        // toc = (1 << 3) | (0 << 2) | 0 = 0x08
        let data = [0x08, 0xAA, 0xBB]; // TOC + payload
        let toc = parse_toc(&data).unwrap();
        assert_eq!(toc.mode, OpusMode::Silk);
        assert_eq!(toc.bandwidth, OpusBandwidth::Narrowband);
        assert_eq!(toc.frame_duration.samples_48k, 960);
        assert!((toc.frame_duration.ms() - 20.0).abs() < 0.1);
        assert!(!toc.stereo);
        assert_eq!(toc.frame_count_code, 0);
        assert_eq!(toc.config, 1);
    }

    #[test]
    fn parse_toc_celt_fullband_20ms_stereo() {
        // config=31 (CELT FB 20ms), stereo=1, code=0
        // toc = (31 << 3) | (1 << 2) | 0 = 0xFC
        let data = [0xFC];
        let toc = parse_toc(&data).unwrap();
        assert_eq!(toc.mode, OpusMode::Celt);
        assert_eq!(toc.bandwidth, OpusBandwidth::Fullband);
        assert_eq!(toc.frame_duration.samples_48k, 960);
        assert!(toc.stereo);
        assert_eq!(toc.config, 31);
    }

    #[test]
    fn parse_toc_hybrid_swb_10ms() {
        // config=12 (Hybrid SWB 10ms), stereo=0, code=1 (2 frames equal)
        // toc = (12 << 3) | (0 << 2) | 1 = 0x61
        let data = [0x61];
        let toc = parse_toc(&data).unwrap();
        assert_eq!(toc.mode, OpusMode::Hybrid);
        assert_eq!(toc.bandwidth, OpusBandwidth::SuperWideband);
        assert_eq!(toc.frame_duration.samples_48k, 480);
        assert_eq!(toc.frame_count_code, 1);
    }

    #[test]
    fn frame_count_code_0() {
        assert_eq!(frame_count(&[0x08]).unwrap(), 1);
    }

    #[test]
    fn frame_count_code_1() {
        assert_eq!(frame_count(&[0x09]).unwrap(), 2); // code=1
    }

    #[test]
    fn frame_count_code_2() {
        assert_eq!(frame_count(&[0x0A]).unwrap(), 2); // code=2
    }

    #[test]
    fn frame_count_code_3() {
        // code=3, second byte has frame count in lower 6 bits
        assert_eq!(frame_count(&[0x0B, 0x05]).unwrap(), 5);
    }

    #[test]
    fn all_configs_valid() {
        for config in 0..32u8 {
            let toc_byte = config << 3; // stereo=0, code=0
            let toc = parse_toc(&[toc_byte]).unwrap();
            assert_eq!(toc.config, config);
            assert!(toc.frame_duration.samples_48k > 0);
        }
    }

    #[test]
    fn bandwidth_sample_rates() {
        assert_eq!(OpusBandwidth::Narrowband.sample_rate(), 8000);
        assert_eq!(OpusBandwidth::Fullband.sample_rate(), 48000);
    }

    // ── Negative ──

    #[test]
    fn parse_toc_empty() {
        assert!(parse_toc(&[]).is_err());
    }

    #[test]
    fn frame_count_empty() {
        assert!(frame_count(&[]).is_err());
    }

    #[test]
    fn frame_count_code3_too_short() {
        assert!(frame_count(&[0x0B]).is_err()); // code=3 but no second byte
    }

    #[test]
    fn frame_count_code3_zero_frames() {
        assert!(frame_count(&[0x0B, 0x00]).is_err()); // 0 frames invalid
    }
}
