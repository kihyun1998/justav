use av_util::error::{Error, Result};
use av_util::frame::Frame;

use crate::codec::{CodecId, CodecRegistry, Decoder};
use crate::codec_par::CodecParameters;
use crate::packet::Packet;

/// Stub decoder that returns `Unsupported` for all operations.
///
/// Used as a placeholder for codecs whose parser is implemented but
/// the full decoding algorithm is not yet available (Phase 3b).
struct StubDecoder {
    codec_id: CodecId,
}

impl StubDecoder {
    fn new(codec_id: CodecId) -> Self {
        Self { codec_id }
    }

    fn unsupported(&self) -> Error {
        Error::Unsupported(format!(
            "{} decoder not yet implemented (parser only)", self.codec_id.name()
        ))
    }
}

impl Decoder for StubDecoder {
    fn codec_id(&self) -> CodecId { self.codec_id }

    fn open(&mut self, _params: &CodecParameters) -> Result<()> {
        Err(self.unsupported())
    }

    fn send_packet(&mut self, _packet: &Packet) -> Result<()> {
        Err(self.unsupported())
    }

    fn receive_frame(&mut self, _frame: &mut Frame) -> Result<()> {
        Err(self.unsupported())
    }

    fn flush(&mut self) {}
}

/// Register stub decoders for codecs with parsers but no decoding implementation.
pub fn register(registry: &mut CodecRegistry) {
    registry.register_decoder(CodecId::H264, || Box::new(StubDecoder::new(CodecId::H264)));
    registry.register_decoder(CodecId::Hevc, || Box::new(StubDecoder::new(CodecId::Hevc)));
    registry.register_decoder(CodecId::Vp8, || Box::new(StubDecoder::new(CodecId::Vp8)));
    registry.register_decoder(CodecId::Vp9, || Box::new(StubDecoder::new(CodecId::Vp9)));
    registry.register_decoder(CodecId::Av1, || Box::new(StubDecoder::new(CodecId::Av1)));
    registry.register_decoder(CodecId::Aac, || Box::new(StubDecoder::new(CodecId::Aac)));
    registry.register_decoder(CodecId::Opus, || Box::new(StubDecoder::new(CodecId::Opus)));
    registry.register_decoder(CodecId::Mp3, || Box::new(StubDecoder::new(CodecId::Mp3)));
    registry.register_decoder(CodecId::Vorbis, || Box::new(StubDecoder::new(CodecId::Vorbis)));
    registry.register_decoder(CodecId::Flac, || Box::new(StubDecoder::new(CodecId::Flac)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registry() -> CodecRegistry {
        let mut reg = CodecRegistry::new();
        register(&mut reg);
        reg
    }

    #[test]
    fn stub_decoders_registered() {
        let reg = make_registry();
        for id in [CodecId::H264, CodecId::Aac, CodecId::Opus, CodecId::Vp9, CodecId::Av1] {
            assert!(reg.find_decoder(id).is_some(), "stub decoder missing for {}", id.name());
        }
    }

    #[test]
    fn stub_decoder_returns_unsupported() {
        let reg = make_registry();
        let mut dec = reg.find_decoder(CodecId::H264).unwrap();
        let params = CodecParameters::new_video(CodecId::H264, 1920, 1080).unwrap();
        let err = dec.open(&params);
        assert!(err.is_err());
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("not yet implemented"), "unexpected error: {msg}");
    }

    #[test]
    fn stub_send_returns_unsupported() {
        let reg = make_registry();
        let mut dec = reg.find_decoder(CodecId::Aac).unwrap();
        let pkt = Packet::empty();
        assert!(dec.send_packet(&pkt).is_err());
    }

    #[test]
    fn stub_receive_returns_unsupported() {
        let reg = make_registry();
        let mut dec = reg.find_decoder(CodecId::Opus).unwrap();
        let mut frame = Frame::new_audio(48000, 2, 0).unwrap();
        assert!(dec.receive_frame(&mut frame).is_err());
    }

    #[test]
    fn pcm_not_overwritten_by_stubs() {
        let mut reg = CodecRegistry::new();
        crate::codecs::pcm::register(&mut reg);
        register(&mut reg); // stubs registered after PCM

        // PCM should still have its real decoder (not stub).
        let mut dec = reg.find_decoder(CodecId::PcmS16Le).unwrap();
        let params = CodecParameters::new_audio(CodecId::PcmS16Le, 44100, 1).unwrap();
        // Real PCM decoder opens successfully.
        assert!(dec.open(&params).is_ok());
    }
}
