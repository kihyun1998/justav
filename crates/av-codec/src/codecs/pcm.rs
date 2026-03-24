use av_util::buffer::Buffer;
use av_util::error::{Error, Result};
use av_util::frame::Frame;

use crate::codec::{CodecId, CodecRegistry, Decoder, Encoder};
use crate::codec_par::CodecParameters;
use crate::packet::Packet;

fn is_big_endian(id: CodecId) -> bool {
    matches!(id, CodecId::PcmS16Be | CodecId::PcmF32Be)
}

fn bytes_per_sample(id: CodecId) -> usize {
    match id {
        CodecId::PcmU8 => 1,
        CodecId::PcmS16Le | CodecId::PcmS16Be => 2,
        CodecId::PcmF32Le | CodecId::PcmF32Be => 4,
        _ => 0,
    }
}

// ────────────────────────────────────────────────────────────────────────────
// PCM Decoder
// ────────────────────────────────────────────────────────────────────────────

pub struct PcmDecoder {
    codec_id: CodecId,
    channels: u16,
    sample_rate: u32,
    bps: usize,
    big_endian: bool,
    opened: bool,
    pending: Option<Vec<u8>>,
    flushed: bool,
}

impl PcmDecoder {
    pub fn new(codec_id: CodecId) -> Self {
        Self {
            codec_id,
            channels: 0,
            sample_rate: 0,
            bps: bytes_per_sample(codec_id),
            big_endian: is_big_endian(codec_id),
            opened: false,
            pending: None,
            flushed: false,
        }
    }
}

impl Decoder for PcmDecoder {
    fn codec_id(&self) -> CodecId {
        self.codec_id
    }

    fn open(&mut self, params: &CodecParameters) -> Result<()> {
        if params.channels == 0 {
            return Err(Error::InvalidArgument("channels must be > 0".into()));
        }
        if params.sample_rate == 0 {
            return Err(Error::InvalidArgument("sample_rate must be > 0".into()));
        }
        self.channels = params.channels;
        self.sample_rate = params.sample_rate;
        self.opened = true;
        Ok(())
    }

    fn send_packet(&mut self, packet: &Packet) -> Result<()> {
        if !self.opened {
            return Err(Error::InvalidState("decoder not opened".into()));
        }
        if self.pending.is_some() {
            return Err(Error::Again);
        }

        // For PCM, "decoding" is essentially byte-order conversion.
        let data = packet.data.data();
        if self.big_endian && self.bps > 1 {
            // Swap bytes to native (little-endian on most platforms).
            let mut converted = data.to_vec();
            for chunk in converted.chunks_exact_mut(self.bps) {
                chunk.reverse();
            }
            self.pending = Some(converted);
        } else {
            self.pending = Some(data.to_vec());
        }
        Ok(())
    }

    fn receive_frame(&mut self, frame: &mut Frame) -> Result<()> {
        match self.pending.take() {
            Some(data) => {
                let frame_bytes = self.bps * self.channels as usize;
                if frame_bytes == 0 {
                    return Err(Error::InvalidState("invalid codec configuration".into()));
                }
                let nb_samples = (data.len() / frame_bytes) as u32;

                *frame = Frame::new_audio(self.sample_rate, self.channels, nb_samples)?;
                frame.set_plane(0, Buffer::from_vec(data), 0)?;
                Ok(())
            }
            None => {
                if self.flushed {
                    Err(Error::Eof)
                } else {
                    Err(Error::Again)
                }
            }
        }
    }

    fn flush(&mut self) {
        self.flushed = true;
    }
}

// ────────────────────────────────────────────────────────────────────────────
// PCM Encoder
// ────────────────────────────────────────────────────────────────────────────

pub struct PcmEncoder {
    codec_id: CodecId,
    channels: u16,
    sample_rate: u32,
    bps: usize,
    big_endian: bool,
    opened: bool,
    pending: Option<Vec<u8>>,
    flushed: bool,
}

impl PcmEncoder {
    pub fn new(codec_id: CodecId) -> Self {
        Self {
            codec_id,
            channels: 0,
            sample_rate: 0,
            bps: bytes_per_sample(codec_id),
            big_endian: is_big_endian(codec_id),
            opened: false,
            pending: None,
            flushed: false,
        }
    }
}

impl Encoder for PcmEncoder {
    fn codec_id(&self) -> CodecId {
        self.codec_id
    }

    fn open(&mut self, params: &CodecParameters) -> Result<()> {
        if params.channels == 0 {
            return Err(Error::InvalidArgument("channels must be > 0".into()));
        }
        if params.sample_rate == 0 {
            return Err(Error::InvalidArgument("sample_rate must be > 0".into()));
        }
        self.channels = params.channels;
        self.sample_rate = params.sample_rate;
        self.opened = true;
        Ok(())
    }

    fn send_frame(&mut self, frame: Option<&Frame>) -> Result<()> {
        if !self.opened {
            return Err(Error::InvalidState("encoder not opened".into()));
        }
        if self.pending.is_some() {
            return Err(Error::Again);
        }

        match frame {
            Some(f) => {
                let plane = f.plane(0)
                    .ok_or_else(|| Error::InvalidArgument("frame has no plane 0".into()))?;

                if self.big_endian && self.bps > 1 {
                    let mut converted = plane.to_vec();
                    for chunk in converted.chunks_exact_mut(self.bps) {
                        chunk.reverse();
                    }
                    self.pending = Some(converted);
                } else {
                    self.pending = Some(plane.to_vec());
                }
                Ok(())
            }
            None => {
                self.flushed = true;
                Ok(())
            }
        }
    }

    fn receive_packet(&mut self, packet: &mut Packet) -> Result<()> {
        match self.pending.take() {
            Some(data) => {
                *packet = Packet::new(Buffer::from_vec(data));
                Ok(())
            }
            None => {
                if self.flushed {
                    Err(Error::Eof)
                } else {
                    Err(Error::Again)
                }
            }
        }
    }
}

/// Register all PCM codecs into the given registry.
pub fn register(registry: &mut CodecRegistry) {
    registry.register_decoder(CodecId::PcmS16Le, || Box::new(PcmDecoder::new(CodecId::PcmS16Le)));
    registry.register_decoder(CodecId::PcmS16Be, || Box::new(PcmDecoder::new(CodecId::PcmS16Be)));
    registry.register_decoder(CodecId::PcmF32Le, || Box::new(PcmDecoder::new(CodecId::PcmF32Le)));
    registry.register_decoder(CodecId::PcmF32Be, || Box::new(PcmDecoder::new(CodecId::PcmF32Be)));
    registry.register_decoder(CodecId::PcmU8, || Box::new(PcmDecoder::new(CodecId::PcmU8)));

    registry.register_encoder(CodecId::PcmS16Le, || Box::new(PcmEncoder::new(CodecId::PcmS16Le)));
    registry.register_encoder(CodecId::PcmS16Be, || Box::new(PcmEncoder::new(CodecId::PcmS16Be)));
    registry.register_encoder(CodecId::PcmF32Le, || Box::new(PcmEncoder::new(CodecId::PcmF32Le)));
    registry.register_encoder(CodecId::PcmF32Be, || Box::new(PcmEncoder::new(CodecId::PcmF32Be)));
    registry.register_encoder(CodecId::PcmU8, || Box::new(PcmEncoder::new(CodecId::PcmU8)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CodecContext;

    fn make_registry() -> CodecRegistry {
        let mut reg = CodecRegistry::new();
        register(&mut reg);
        reg
    }

    // ── Positive ──

    #[test]
    fn pcm_s16le_encode_decode_roundtrip() {
        let reg = make_registry();
        let params = CodecParameters::new_audio(CodecId::PcmS16Le, 44100, 2).unwrap();

        // Encode.
        let mut enc_ctx = CodecContext::new_encoder(&reg, CodecId::PcmS16Le).unwrap();
        enc_ctx.open(&params).unwrap();

        let samples: Vec<u8> = (0..200).map(|i| (i % 256) as u8).collect();
        let mut frame = Frame::new_audio(44100, 2, 50).unwrap(); // 50 samples * 2ch * 2bytes = 200
        frame.set_plane(0, Buffer::from_vec(samples.clone()), 0).unwrap();

        enc_ctx.send_frame(Some(&frame)).unwrap();
        let mut pkt = Packet::empty();
        enc_ctx.receive_packet(&mut pkt).unwrap();

        // Decode.
        let mut dec_ctx = CodecContext::new_decoder(&reg, CodecId::PcmS16Le).unwrap();
        dec_ctx.open(&params).unwrap();

        dec_ctx.send_packet(&pkt).unwrap();
        let mut out_frame = Frame::new_audio(44100, 2, 0).unwrap();
        dec_ctx.receive_frame(&mut out_frame).unwrap();

        // Roundtrip should be byte-exact.
        assert_eq!(out_frame.plane(0).unwrap(), &samples);
        assert_eq!(out_frame.nb_samples, 50);
    }

    #[test]
    fn pcm_u8_roundtrip() {
        let reg = make_registry();
        let params = CodecParameters::new_audio(CodecId::PcmU8, 8000, 1).unwrap();

        let mut enc = CodecContext::new_encoder(&reg, CodecId::PcmU8).unwrap();
        enc.open(&params).unwrap();

        let samples = vec![128u8; 100]; // 100 samples, mono, 1 byte each
        let mut frame = Frame::new_audio(8000, 1, 100).unwrap();
        frame.set_plane(0, Buffer::from_vec(samples.clone()), 0).unwrap();

        enc.send_frame(Some(&frame)).unwrap();
        let mut pkt = Packet::empty();
        enc.receive_packet(&mut pkt).unwrap();

        let mut dec = CodecContext::new_decoder(&reg, CodecId::PcmU8).unwrap();
        dec.open(&params).unwrap();
        dec.send_packet(&pkt).unwrap();
        let mut out = Frame::new_audio(8000, 1, 0).unwrap();
        dec.receive_frame(&mut out).unwrap();

        assert_eq!(out.plane(0).unwrap(), &samples);
    }

    #[test]
    fn pcm_s16be_roundtrip() {
        let reg = make_registry();
        let params = CodecParameters::new_audio(CodecId::PcmS16Be, 48000, 1).unwrap();

        // Original native bytes: [0x01, 0x02, 0x03, 0x04] (2 samples, s16)
        let native_samples = vec![0x01u8, 0x02, 0x03, 0x04];

        let mut enc = CodecContext::new_encoder(&reg, CodecId::PcmS16Be).unwrap();
        enc.open(&params).unwrap();
        let mut frame = Frame::new_audio(48000, 1, 2).unwrap();
        frame.set_plane(0, Buffer::from_vec(native_samples.clone()), 0).unwrap();
        enc.send_frame(Some(&frame)).unwrap();
        let mut pkt = Packet::empty();
        enc.receive_packet(&mut pkt).unwrap();

        // Encoded packet should be byte-swapped (big-endian).
        assert_eq!(pkt.data.data(), &[0x02, 0x01, 0x04, 0x03]);

        // Decode back to native.
        let mut dec = CodecContext::new_decoder(&reg, CodecId::PcmS16Be).unwrap();
        dec.open(&params).unwrap();
        dec.send_packet(&pkt).unwrap();
        let mut out = Frame::new_audio(48000, 1, 0).unwrap();
        dec.receive_frame(&mut out).unwrap();

        assert_eq!(out.plane(0).unwrap(), &native_samples);
    }

    #[test]
    fn pcm_f32le_roundtrip() {
        let reg = make_registry();
        let params = CodecParameters::new_audio(CodecId::PcmF32Le, 48000, 2).unwrap();

        let samples: Vec<u8> = vec![0; 48000 * 2 * 4]; // 1 second of silence
        let mut frame = Frame::new_audio(48000, 2, 48000).unwrap();
        frame.set_plane(0, Buffer::from_vec(samples.clone()), 0).unwrap();

        let mut enc = CodecContext::new_encoder(&reg, CodecId::PcmF32Le).unwrap();
        enc.open(&params).unwrap();
        enc.send_frame(Some(&frame)).unwrap();
        let mut pkt = Packet::empty();
        enc.receive_packet(&mut pkt).unwrap();

        let mut dec = CodecContext::new_decoder(&reg, CodecId::PcmF32Le).unwrap();
        dec.open(&params).unwrap();
        dec.send_packet(&pkt).unwrap();
        let mut out = Frame::new_audio(48000, 2, 0).unwrap();
        dec.receive_frame(&mut out).unwrap();

        assert_eq!(out.plane(0).unwrap(), &samples);
    }

    #[test]
    fn pcm_f32be_roundtrip() {
        let reg = make_registry();
        let params = CodecParameters::new_audio(CodecId::PcmF32Be, 44100, 1).unwrap();

        // Native LE bytes for 2 f32 samples.
        let native_samples = vec![0x00u8, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x00, 0x40]; // 1.0f, 2.0f in LE

        let mut enc = CodecContext::new_encoder(&reg, CodecId::PcmF32Be).unwrap();
        enc.open(&params).unwrap();
        let mut frame = Frame::new_audio(44100, 1, 2).unwrap();
        frame.set_plane(0, Buffer::from_vec(native_samples.clone()), 0).unwrap();
        enc.send_frame(Some(&frame)).unwrap();
        let mut pkt = Packet::empty();
        enc.receive_packet(&mut pkt).unwrap();

        // Encoded should be byte-swapped (big-endian).
        assert_eq!(pkt.data.data(), &[0x3F, 0x80, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00]);

        // Decode back.
        let mut dec = CodecContext::new_decoder(&reg, CodecId::PcmF32Be).unwrap();
        dec.open(&params).unwrap();
        dec.send_packet(&pkt).unwrap();
        let mut out = Frame::new_audio(44100, 1, 0).unwrap();
        dec.receive_frame(&mut out).unwrap();

        assert_eq!(out.plane(0).unwrap(), &native_samples);
    }

    #[test]
    fn flush_signals_eof() {
        let reg = make_registry();
        let params = CodecParameters::new_audio(CodecId::PcmS16Le, 44100, 1).unwrap();

        let mut dec = CodecContext::new_decoder(&reg, CodecId::PcmS16Le).unwrap();
        dec.open(&params).unwrap();
        dec.flush_decoder().unwrap();

        let mut out = Frame::new_audio(44100, 1, 0).unwrap();
        let err = dec.receive_frame(&mut out);
        assert!(matches!(err, Err(Error::Eof)));
    }

    #[test]
    fn all_pcm_codecs_registered() {
        let reg = make_registry();
        for id in [CodecId::PcmS16Le, CodecId::PcmS16Be, CodecId::PcmF32Le, CodecId::PcmF32Be, CodecId::PcmU8] {
            assert!(reg.find_decoder(id).is_some(), "decoder missing for {}", id.name());
            assert!(reg.find_encoder(id).is_some(), "encoder missing for {}", id.name());
        }
    }

    // ── Negative ──

    #[test]
    fn decode_before_open() {
        let mut dec = PcmDecoder::new(CodecId::PcmS16Le);
        let pkt = Packet::new(Buffer::from_vec(vec![0; 100]));
        assert!(dec.send_packet(&pkt).is_err());
    }

    #[test]
    fn open_zero_channels() {
        let mut dec = PcmDecoder::new(CodecId::PcmS16Le);
        let mut params = CodecParameters::new_audio(CodecId::PcmS16Le, 44100, 1).unwrap();
        params.channels = 0;
        assert!(dec.open(&params).is_err());
    }

    #[test]
    fn encode_no_plane() {
        let reg = make_registry();
        let params = CodecParameters::new_audio(CodecId::PcmS16Le, 44100, 1).unwrap();
        let mut enc = CodecContext::new_encoder(&reg, CodecId::PcmS16Le).unwrap();
        enc.open(&params).unwrap();

        let frame = Frame::new_audio(44100, 1, 100).unwrap(); // no plane set
        assert!(enc.send_frame(Some(&frame)).is_err());
    }

    #[test]
    fn receive_without_send() {
        let reg = make_registry();
        let params = CodecParameters::new_audio(CodecId::PcmS16Le, 44100, 1).unwrap();
        let mut dec = CodecContext::new_decoder(&reg, CodecId::PcmS16Le).unwrap();
        dec.open(&params).unwrap();

        let mut out = Frame::new_audio(44100, 1, 0).unwrap();
        assert!(matches!(dec.receive_frame(&mut out), Err(Error::Again)));
    }
}
