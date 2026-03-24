use av_util::error::Result;
use av_util::frame::{Frame, MediaType};

use crate::codec_par::CodecParameters;
use crate::packet::Packet;

/// Codec identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CodecId {
    None,

    // ── Video ──
    H264,
    Hevc,
    Vp8,
    Vp9,
    Av1,
    Png,

    // ── Audio ──
    PcmS16Le,
    PcmS16Be,
    PcmF32Le,
    PcmF32Be,
    PcmU8,
    Flac,
    Aac,
    Opus,
    Mp3,
    Vorbis,

    // ── Subtitle ──
    Srt,
    Ass,
    WebVtt,
}

impl CodecId {
    /// The media type for this codec.
    pub const fn media_type(&self) -> MediaType {
        match self {
            Self::H264 | Self::Hevc | Self::Vp8 | Self::Vp9 | Self::Av1 | Self::Png => MediaType::Video,
            Self::PcmS16Le | Self::PcmS16Be | Self::PcmF32Le | Self::PcmF32Be | Self::PcmU8
            | Self::Flac | Self::Aac | Self::Opus | Self::Mp3 | Self::Vorbis => MediaType::Audio,
            Self::Srt | Self::Ass | Self::WebVtt => MediaType::Audio, // placeholder, will be MediaType::Subtitle later
            Self::None => MediaType::Video,
        }
    }

    /// Human-readable name.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::H264 => "h264",
            Self::Hevc => "hevc",
            Self::Vp8 => "vp8",
            Self::Vp9 => "vp9",
            Self::Av1 => "av1",
            Self::Png => "png",
            Self::PcmS16Le => "pcm_s16le",
            Self::PcmS16Be => "pcm_s16be",
            Self::PcmF32Le => "pcm_f32le",
            Self::PcmF32Be => "pcm_f32be",
            Self::PcmU8 => "pcm_u8",
            Self::Flac => "flac",
            Self::Aac => "aac",
            Self::Opus => "opus",
            Self::Mp3 => "mp3",
            Self::Vorbis => "vorbis",
            Self::Srt => "srt",
            Self::Ass => "ass",
            Self::WebVtt => "webvtt",
        }
    }

    /// Parse from name.
    pub fn from_name(name: &str) -> Option<Self> {
        ALL_CODEC_IDS.iter().find(|c| c.name() == name).copied()
    }
}

impl core::fmt::Display for CodecId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// All defined codec IDs.
pub const ALL_CODEC_IDS: &[CodecId] = &[
    CodecId::None,
    CodecId::H264, CodecId::Hevc, CodecId::Vp8, CodecId::Vp9, CodecId::Av1, CodecId::Png,
    CodecId::PcmS16Le, CodecId::PcmS16Be, CodecId::PcmF32Le, CodecId::PcmF32Be, CodecId::PcmU8,
    CodecId::Flac, CodecId::Aac, CodecId::Opus, CodecId::Mp3, CodecId::Vorbis,
    CodecId::Srt, CodecId::Ass, CodecId::WebVtt,
];

/// Trait for a decoder implementation.
pub trait Decoder: Send {
    /// The codec this decoder handles.
    fn codec_id(&self) -> CodecId;

    /// Initialize the decoder with the given parameters.
    fn open(&mut self, params: &CodecParameters) -> Result<()>;

    /// Send a compressed packet for decoding.
    /// Returns `Error::Again` if the decoder needs output to be drained first.
    fn send_packet(&mut self, packet: &Packet) -> Result<()>;

    /// Receive a decoded frame.
    /// Returns `Error::Again` if more input is needed.
    /// Returns `Error::Eof` when all frames have been flushed.
    fn receive_frame(&mut self, frame: &mut Frame) -> Result<()>;

    /// Flush the decoder (signal end of stream). After flushing,
    /// keep calling `receive_frame` until `Error::Eof`.
    fn flush(&mut self);
}

/// Trait for an encoder implementation.
pub trait Encoder: Send {
    /// The codec this encoder produces.
    fn codec_id(&self) -> CodecId;

    /// Initialize the encoder with the given parameters.
    fn open(&mut self, params: &CodecParameters) -> Result<()>;

    /// Send a raw frame for encoding.
    /// Send `None` to signal end of stream and begin flushing.
    fn send_frame(&mut self, frame: Option<&Frame>) -> Result<()>;

    /// Receive an encoded packet.
    /// Returns `Error::Again` if more input is needed.
    /// Returns `Error::Eof` when all packets have been flushed.
    fn receive_packet(&mut self, packet: &mut Packet) -> Result<()>;
}

/// Codec descriptor for registration.
#[derive(Debug, Clone)]
pub struct CodecDescriptor {
    pub id: CodecId,
    pub name: &'static str,
    pub long_name: &'static str,
    pub media_type: MediaType,
    pub is_encoder: bool,
    pub is_decoder: bool,
}

/// A simple registry of available codec factories.
///
/// Codecs register themselves here. The registry is used to find
/// the right decoder/encoder for a given `CodecId`.
type DecoderFactory = fn() -> Box<dyn Decoder>;
type EncoderFactory = fn() -> Box<dyn Encoder>;

pub struct CodecRegistry {
    decoders: Vec<(CodecId, DecoderFactory)>,
    encoders: Vec<(CodecId, EncoderFactory)>,
}

impl CodecRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            decoders: Vec::new(),
            encoders: Vec::new(),
        }
    }

    /// Register a decoder factory.
    pub fn register_decoder(&mut self, id: CodecId, factory: fn() -> Box<dyn Decoder>) {
        if !self.decoders.iter().any(|(cid, _)| *cid == id) {
            self.decoders.push((id, factory));
        }
    }

    /// Register an encoder factory.
    pub fn register_encoder(&mut self, id: CodecId, factory: fn() -> Box<dyn Encoder>) {
        if !self.encoders.iter().any(|(cid, _)| *cid == id) {
            self.encoders.push((id, factory));
        }
    }

    /// Find and create a decoder for the given codec ID.
    pub fn find_decoder(&self, id: CodecId) -> Option<Box<dyn Decoder>> {
        self.decoders.iter().find(|(cid, _)| *cid == id).map(|(_, f)| f())
    }

    /// Find and create an encoder for the given codec ID.
    pub fn find_encoder(&self, id: CodecId) -> Option<Box<dyn Encoder>> {
        self.encoders.iter().find(|(cid, _)| *cid == id).map(|(_, f)| f())
    }

    /// List all registered decoder codec IDs.
    pub fn decoder_ids(&self) -> Vec<CodecId> {
        self.decoders.iter().map(|(id, _)| *id).collect()
    }

    /// List all registered encoder codec IDs.
    pub fn encoder_ids(&self) -> Vec<CodecId> {
        self.encoders.iter().map(|(id, _)| *id).collect()
    }
}

impl Default for CodecRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn codec_id_name() {
        assert_eq!(CodecId::H264.name(), "h264");
        assert_eq!(CodecId::PcmS16Le.name(), "pcm_s16le");
        assert_eq!(CodecId::Aac.name(), "aac");
    }

    #[test]
    fn codec_id_from_name() {
        assert_eq!(CodecId::from_name("h264"), Some(CodecId::H264));
        assert_eq!(CodecId::from_name("flac"), Some(CodecId::Flac));
    }

    #[test]
    fn codec_id_media_type() {
        assert_eq!(CodecId::H264.media_type(), MediaType::Video);
        assert_eq!(CodecId::Aac.media_type(), MediaType::Audio);
    }

    #[test]
    fn codec_id_display() {
        assert_eq!(format!("{}", CodecId::Opus), "opus");
    }

    #[test]
    fn all_codec_ids_unique_names() {
        let mut names: Vec<&str> = ALL_CODEC_IDS.iter().map(|c| c.name()).collect();
        let len = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), len);
    }

    #[test]
    fn registry_empty() {
        let reg = CodecRegistry::new();
        assert!(reg.find_decoder(CodecId::H264).is_none());
        assert!(reg.find_encoder(CodecId::H264).is_none());
        assert!(reg.decoder_ids().is_empty());
    }

    #[test]
    fn registry_no_duplicate() {
        let mut reg = CodecRegistry::new();
        // Registering same ID twice should only keep the first.
        reg.register_decoder(CodecId::PcmS16Le, || Box::new(DummyDecoder));
        reg.register_decoder(CodecId::PcmS16Le, || Box::new(DummyDecoder));
        assert_eq!(reg.decoder_ids().len(), 1);
    }

    // ── Negative ──

    #[test]
    fn codec_id_from_name_invalid() {
        assert_eq!(CodecId::from_name("not_a_codec"), None);
        assert_eq!(CodecId::from_name(""), None);
    }

    #[test]
    fn registry_find_unregistered() {
        let reg = CodecRegistry::new();
        assert!(reg.find_decoder(CodecId::Aac).is_none());
    }

    // ── Helpers ──

    struct DummyDecoder;
    impl Decoder for DummyDecoder {
        fn codec_id(&self) -> CodecId { CodecId::PcmS16Le }
        fn open(&mut self, _: &CodecParameters) -> Result<()> { Ok(()) }
        fn send_packet(&mut self, _: &Packet) -> Result<()> { Ok(()) }
        fn receive_frame(&mut self, _: &mut Frame) -> Result<()> { Err(av_util::error::Error::Again) }
        fn flush(&mut self) {}
    }
}
