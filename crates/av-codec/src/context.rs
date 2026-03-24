use av_util::error::{Error, Result};
use av_util::frame::Frame;

use crate::codec::{CodecId, CodecRegistry, Decoder, Encoder};
use crate::codec_par::CodecParameters;
use crate::error_resilience::ErrorPolicy;
use crate::packet::Packet;

/// The state of a codec context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Not yet opened.
    Uninitialized,
    /// Ready for send/receive.
    Open,
    /// Flush signaled, draining remaining output.
    Draining,
    /// All output consumed after flush.
    Eof,
}

/// High-level codec context that wraps a decoder or encoder.
///
/// Provides the send/receive API. `CodecContext` is `Send` but not `Sync`
/// (requires `&mut self` for all operations).
pub struct CodecContext {
    decoder: Option<Box<dyn Decoder>>,
    encoder: Option<Box<dyn Encoder>>,
    state: State,
    /// Error handling policy.
    pub error_policy: ErrorPolicy,
    /// Parameters this context was opened with.
    params: CodecParameters,
}

impl CodecContext {
    /// Create a new decoder context from the registry.
    pub fn new_decoder(registry: &CodecRegistry, codec_id: CodecId) -> Result<Self> {
        let decoder = registry
            .find_decoder(codec_id)
            .ok_or_else(|| Error::NotFound(format!("no decoder for {}", codec_id.name())))?;
        Ok(Self {
            decoder: Some(decoder),
            encoder: None,
            state: State::Uninitialized,
            error_policy: ErrorPolicy::Fail,
            params: CodecParameters::new(),
        })
    }

    /// Create a new encoder context from the registry.
    pub fn new_encoder(registry: &CodecRegistry, codec_id: CodecId) -> Result<Self> {
        let encoder = registry
            .find_encoder(codec_id)
            .ok_or_else(|| Error::NotFound(format!("no encoder for {}", codec_id.name())))?;
        Ok(Self {
            decoder: None,
            encoder: Some(encoder),
            state: State::Uninitialized,
            error_policy: ErrorPolicy::Fail,
            params: CodecParameters::new(),
        })
    }

    /// Open the context with the given parameters.
    pub fn open(&mut self, params: &CodecParameters) -> Result<()> {
        if self.state != State::Uninitialized {
            return Err(Error::InvalidState("codec already opened".into()));
        }
        if let Some(dec) = &mut self.decoder {
            dec.open(params)?;
        } else if let Some(enc) = &mut self.encoder {
            enc.open(params)?;
        } else {
            return Err(Error::InvalidState("no decoder or encoder set".into()));
        }
        self.params = params.clone();
        self.state = State::Open;
        Ok(())
    }

    /// Returns true if the context is open and ready for send/receive.
    pub fn is_open(&self) -> bool {
        self.state == State::Open || self.state == State::Draining
    }

    /// The parameters this context was opened with.
    pub fn parameters(&self) -> &CodecParameters {
        &self.params
    }

    // ── Decoder API ──

    /// Send a packet to the decoder.
    pub fn send_packet(&mut self, packet: &Packet) -> Result<()> {
        match self.state {
            State::Uninitialized => return Err(Error::InvalidState("codec not opened".into())),
            State::Eof => return Err(Error::Eof),
            _ => {}
        }
        let dec = self.decoder.as_mut()
            .ok_or_else(|| Error::InvalidState("not a decoder context".into()))?;
        dec.send_packet(packet)
    }

    /// Receive a decoded frame from the decoder.
    pub fn receive_frame(&mut self, frame: &mut Frame) -> Result<()> {
        match self.state {
            State::Uninitialized => return Err(Error::InvalidState("codec not opened".into())),
            State::Eof => return Err(Error::Eof),
            _ => {}
        }
        let dec = self.decoder.as_mut()
            .ok_or_else(|| Error::InvalidState("not a decoder context".into()))?;
        match dec.receive_frame(frame) {
            Ok(()) => Ok(()),
            Err(Error::Eof) => {
                self.state = State::Eof;
                Err(Error::Eof)
            }
            Err(e) => Err(e),
        }
    }

    /// Signal end-of-stream to the decoder. After this, keep calling
    /// `receive_frame` until `Error::Eof`.
    pub fn flush_decoder(&mut self) -> Result<()> {
        if self.state == State::Uninitialized {
            return Err(Error::InvalidState("codec not opened".into()));
        }
        let dec = self.decoder.as_mut()
            .ok_or_else(|| Error::InvalidState("not a decoder context".into()))?;
        dec.flush();
        self.state = State::Draining;
        Ok(())
    }

    // ── Encoder API ──

    /// Send a frame to the encoder. Pass `None` to signal end-of-stream.
    pub fn send_frame(&mut self, frame: Option<&Frame>) -> Result<()> {
        match self.state {
            State::Uninitialized => return Err(Error::InvalidState("codec not opened".into())),
            State::Eof => return Err(Error::Eof),
            _ => {}
        }
        let enc = self.encoder.as_mut()
            .ok_or_else(|| Error::InvalidState("not an encoder context".into()))?;
        if frame.is_none() {
            self.state = State::Draining;
        }
        enc.send_frame(frame)
    }

    /// Receive an encoded packet from the encoder.
    pub fn receive_packet(&mut self, packet: &mut Packet) -> Result<()> {
        match self.state {
            State::Uninitialized => return Err(Error::InvalidState("codec not opened".into())),
            State::Eof => return Err(Error::Eof),
            _ => {}
        }
        let enc = self.encoder.as_mut()
            .ok_or_else(|| Error::InvalidState("not an encoder context".into()))?;
        match enc.receive_packet(packet) {
            Ok(()) => Ok(()),
            Err(Error::Eof) => {
                self.state = State::Eof;
                Err(Error::Eof)
            }
            Err(e) => Err(e),
        }
    }
}

// CodecContext cannot be Sync since it holds &mut-requiring state.
// Send is fine because all inner types are Send.

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive tests happen in PCM codec integration (codecs/pcm.rs) ──

    // ── Negative ──

    #[test]
    fn decoder_not_found() {
        let reg = CodecRegistry::new();
        assert!(CodecContext::new_decoder(&reg, CodecId::H264).is_err());
    }

    #[test]
    fn encoder_not_found() {
        let reg = CodecRegistry::new();
        assert!(CodecContext::new_encoder(&reg, CodecId::H264).is_err());
    }

    #[test]
    fn send_before_open() {
        let mut reg = CodecRegistry::new();
        // Register a dummy so we can create a context.
        use crate::codec::Decoder as _;
        struct Dummy;
        impl crate::codec::Decoder for Dummy {
            fn codec_id(&self) -> CodecId { CodecId::PcmU8 }
            fn open(&mut self, _: &CodecParameters) -> Result<()> { Ok(()) }
            fn send_packet(&mut self, _: &Packet) -> Result<()> { Ok(()) }
            fn receive_frame(&mut self, _: &mut Frame) -> Result<()> { Err(Error::Again) }
            fn flush(&mut self) {}
        }
        reg.register_decoder(CodecId::PcmU8, || Box::new(Dummy));

        let mut ctx = CodecContext::new_decoder(&reg, CodecId::PcmU8).unwrap();
        let pkt = Packet::empty();
        assert!(ctx.send_packet(&pkt).is_err()); // not opened
    }

    #[test]
    fn double_open() {
        let mut reg = CodecRegistry::new();
        struct Dummy;
        impl crate::codec::Decoder for Dummy {
            fn codec_id(&self) -> CodecId { CodecId::PcmU8 }
            fn open(&mut self, _: &CodecParameters) -> Result<()> { Ok(()) }
            fn send_packet(&mut self, _: &Packet) -> Result<()> { Ok(()) }
            fn receive_frame(&mut self, _: &mut Frame) -> Result<()> { Err(Error::Again) }
            fn flush(&mut self) {}
        }
        reg.register_decoder(CodecId::PcmU8, || Box::new(Dummy));

        let mut ctx = CodecContext::new_decoder(&reg, CodecId::PcmU8).unwrap();
        let params = CodecParameters::new_audio(CodecId::PcmU8, 44100, 1).unwrap();
        ctx.open(&params).unwrap();
        assert!(ctx.open(&params).is_err()); // already open
    }

    #[test]
    fn send_frame_on_decoder() {
        let mut reg = CodecRegistry::new();
        struct Dummy;
        impl crate::codec::Decoder for Dummy {
            fn codec_id(&self) -> CodecId { CodecId::PcmU8 }
            fn open(&mut self, _: &CodecParameters) -> Result<()> { Ok(()) }
            fn send_packet(&mut self, _: &Packet) -> Result<()> { Ok(()) }
            fn receive_frame(&mut self, _: &mut Frame) -> Result<()> { Err(Error::Again) }
            fn flush(&mut self) {}
        }
        reg.register_decoder(CodecId::PcmU8, || Box::new(Dummy));

        let mut ctx = CodecContext::new_decoder(&reg, CodecId::PcmU8).unwrap();
        let params = CodecParameters::new_audio(CodecId::PcmU8, 44100, 1).unwrap();
        ctx.open(&params).unwrap();
        // send_frame on a decoder context should fail.
        assert!(ctx.send_frame(None).is_err());
    }

    #[test]
    fn send_trait() {
        fn assert_send<T: Send>() {}
        assert_send::<CodecContext>();
    }
}
