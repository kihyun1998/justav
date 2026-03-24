use av_util::buffer::Buffer;
use av_util::channel_layout::ChannelLayout;
use av_util::error::{Error, Result};
use av_util::frame::MediaType;
use av_util::pixfmt::PixelFormat;
use av_util::rational::Rational;
use av_util::samplefmt::SampleFormat;

use crate::codec::CodecId;

/// Parameters describing a codec configuration.
///
/// This is the "lightweight" description of a stream's codec that can be
/// passed around without carrying the full codec state. It's the bridge
/// between demuxers/muxers and decoders/encoders.
#[derive(Debug, Clone)]
pub struct CodecParameters {
    /// The codec identifier.
    pub codec_id: CodecId,
    /// Media type (video, audio).
    pub media_type: MediaType,
    /// Codec-specific extra data (e.g. SPS/PPS for H.264, ESDS for AAC).
    pub extradata: Option<Buffer>,
    /// Average bitrate in bits/second (0 if unknown).
    pub bit_rate: u64,

    // ── Video ──
    /// Video width in pixels.
    pub width: u32,
    /// Video height in pixels.
    pub height: u32,
    /// Pixel format.
    pub pixel_format: Option<PixelFormat>,
    /// Frame rate (frames per second).
    pub frame_rate: Rational,
    /// Sample aspect ratio.
    pub sample_aspect_ratio: Rational,

    // ── Audio ──
    /// Sample format.
    pub sample_format: Option<SampleFormat>,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Channel layout.
    pub channel_layout: Option<ChannelLayout>,
    /// Number of audio channels (fallback if layout is None).
    pub channels: u16,
    /// Samples per audio frame (0 if variable).
    pub frame_size: u32,
}

impl CodecParameters {
    /// Create default (empty) parameters.
    pub fn new() -> Self {
        Self {
            codec_id: CodecId::None,
            media_type: MediaType::Video,
            extradata: None,
            bit_rate: 0,
            width: 0,
            height: 0,
            pixel_format: None,
            frame_rate: Rational::UNKNOWN,
            sample_aspect_ratio: Rational::new(1, 1),
            sample_format: None,
            sample_rate: 0,
            channel_layout: None,
            channels: 0,
            frame_size: 0,
        }
    }

    /// Create parameters for a video stream.
    pub fn new_video(codec_id: CodecId, width: u32, height: u32) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(Error::InvalidArgument("video dimensions must be > 0".into()));
        }
        Ok(Self {
            codec_id,
            media_type: MediaType::Video,
            width,
            height,
            ..Self::new()
        })
    }

    /// Create parameters for an audio stream.
    pub fn new_audio(codec_id: CodecId, sample_rate: u32, channels: u16) -> Result<Self> {
        if sample_rate == 0 {
            return Err(Error::InvalidArgument("sample_rate must be > 0".into()));
        }
        if channels == 0 {
            return Err(Error::InvalidArgument("channels must be > 0".into()));
        }
        Ok(Self {
            codec_id,
            media_type: MediaType::Audio,
            sample_rate,
            channels,
            ..Self::new()
        })
    }

    /// Copy all fields from another `CodecParameters`.
    pub fn copy_from(&mut self, other: &CodecParameters) {
        *self = other.clone();
    }

    /// Returns the number of channels, preferring `channel_layout` if set.
    pub fn nb_channels(&self) -> u16 {
        self.channel_layout.as_ref().map_or(self.channels, |l| l.nb_channels())
    }
}

impl Default for CodecParameters {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn new_default() {
        let par = CodecParameters::new();
        assert_eq!(par.codec_id, CodecId::None);
        assert_eq!(par.bit_rate, 0);
    }

    #[test]
    fn new_video() {
        let par = CodecParameters::new_video(CodecId::H264, 1920, 1080).unwrap();
        assert_eq!(par.media_type, MediaType::Video);
        assert_eq!(par.codec_id, CodecId::H264);
        assert_eq!(par.width, 1920);
        assert_eq!(par.height, 1080);
    }

    #[test]
    fn new_audio() {
        let par = CodecParameters::new_audio(CodecId::Aac, 48000, 2).unwrap();
        assert_eq!(par.media_type, MediaType::Audio);
        assert_eq!(par.sample_rate, 48000);
        assert_eq!(par.channels, 2);
    }

    #[test]
    fn copy_from() {
        let src = CodecParameters::new_video(CodecId::H264, 1280, 720).unwrap();
        let mut dst = CodecParameters::new();
        dst.copy_from(&src);
        assert_eq!(dst.codec_id, CodecId::H264);
        assert_eq!(dst.width, 1280);
    }

    #[test]
    fn extradata() {
        let mut par = CodecParameters::new_video(CodecId::H264, 1920, 1080).unwrap();
        par.extradata = Some(Buffer::from_vec(vec![0, 0, 0, 1, 0x67]));
        assert_eq!(par.extradata.as_ref().unwrap().len(), 5);
    }

    #[test]
    fn nb_channels_from_layout() {
        let mut par = CodecParameters::new_audio(CodecId::Aac, 48000, 2).unwrap();
        par.channel_layout = Some(av_util::channel_layout::ChannelLayout::layout_5point1());
        // Layout overrides raw channels count.
        assert_eq!(par.nb_channels(), 6);
    }

    #[test]
    fn nb_channels_fallback() {
        let par = CodecParameters::new_audio(CodecId::Aac, 48000, 2).unwrap();
        assert_eq!(par.nb_channels(), 2);
    }

    #[test]
    fn clone_is_independent() {
        let par = CodecParameters::new_video(CodecId::Hevc, 3840, 2160).unwrap();
        let par2 = par.clone();
        assert_eq!(par2.codec_id, CodecId::Hevc);
        assert_eq!(par2.width, 3840);
    }

    #[test]
    fn send_trait() {
        fn assert_send<T: Send>() {}
        assert_send::<CodecParameters>();
    }

    // ── Negative ──

    #[test]
    fn new_video_zero_width() {
        assert!(CodecParameters::new_video(CodecId::H264, 0, 1080).is_err());
    }

    #[test]
    fn new_video_zero_height() {
        assert!(CodecParameters::new_video(CodecId::H264, 1920, 0).is_err());
    }

    #[test]
    fn new_audio_zero_sample_rate() {
        assert!(CodecParameters::new_audio(CodecId::Aac, 0, 2).is_err());
    }

    #[test]
    fn new_audio_zero_channels() {
        assert!(CodecParameters::new_audio(CodecId::Aac, 48000, 0).is_err());
    }
}
