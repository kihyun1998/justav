use av_codec::codec::{CodecRegistry, CodecId};
use av_codec::context::CodecContext;
use av_codec::packet::Packet;
use av_format::context::{InputContext, OutputContext};
use av_util::error::{Error, Result};
use av_util::frame::Frame;

/// A simple demux → decode → encode → mux pipeline.
///
/// This is the core transcoding pipeline. Filters can be inserted
/// between decode and encode in future phases.
pub struct Pipeline {
    registry: CodecRegistry,
}

impl Pipeline {
    /// Create a pipeline with the given codec registry.
    pub fn new(registry: CodecRegistry) -> Self {
        Self { registry }
    }

    /// Create a pipeline with all built-in codecs registered.
    pub fn with_defaults() -> Self {
        let mut reg = CodecRegistry::new();
        av_codec::codecs::pcm::register(&mut reg);
        av_codec::codecs::stubs::register(&mut reg);
        Self::new(reg)
    }

    /// Run a transcode: read from input, decode, re-encode, write to output.
    ///
    /// `output_codec` specifies the codec to encode each stream with.
    /// If `None` for a stream, the input codec is used (copy mode not yet
    /// supported — decode+re-encode is always performed).
    pub fn transcode(
        &self,
        input: &mut InputContext,
        output: &mut OutputContext,
        output_codecs: &[CodecId],
    ) -> Result<TranscodeResult> {
        // Open input.
        input.open()?;

        // Set up decoder/encoder pairs.
        let mut codec_pairs: Vec<(CodecContext, CodecContext)> = Vec::new();

        for (i, stream) in input.streams.iter().enumerate() {
            let out_codec = output_codecs.get(i).copied().unwrap_or(stream.params.codec_id);

            let mut decoder = CodecContext::new_decoder(&self.registry, stream.params.codec_id)?;
            decoder.open(&stream.params)?;

            let mut enc_params = stream.params.clone();
            enc_params.codec_id = out_codec;
            let mut encoder = CodecContext::new_encoder(&self.registry, out_codec)?;
            encoder.open(&enc_params)?;

            // Add output stream.
            let mut out_stream = av_format::stream::Stream::new(i as u32, enc_params);
            out_stream.time_base = stream.time_base;
            output.add_stream(out_stream);

            codec_pairs.push((decoder, encoder));
        }

        // Write output header.
        output.write_header()?;

        let mut packets_read = 0u64;
        let mut packets_written = 0u64;

        // Main loop: read → decode → encode → write.
        loop {
            let pkt = match input.read_packet() {
                Ok(p) => p,
                Err(Error::Eof) => break,
                Err(e) => return Err(e),
            };
            packets_read += 1;

            let stream_idx = pkt.stream_index as usize;
            if stream_idx >= codec_pairs.len() {
                continue;
            }

            let (decoder, encoder) = &mut codec_pairs[stream_idx];

            // Decode.
            decoder.send_packet(&pkt)?;
            let mut frame = Frame::new_audio(1, 1, 0) // placeholder, will be overwritten
                .unwrap_or_else(|_| Frame::new_video(1, 1).unwrap());
            loop {
                match decoder.receive_frame(&mut frame) {
                    Ok(()) => {
                        // Encode.
                        encoder.send_frame(Some(&frame))?;
                        let mut out_pkt = Packet::empty();
                        loop {
                            match encoder.receive_packet(&mut out_pkt) {
                                Ok(()) => {
                                    out_pkt.stream_index = stream_idx as u32;
                                    output.write_packet(&out_pkt)?;
                                    packets_written += 1;
                                }
                                Err(Error::Again) => break,
                                Err(e) => return Err(e),
                            }
                        }
                    }
                    Err(Error::Again) => break,
                    Err(e) => return Err(e),
                }
            }
        }

        // Flush decoders and encoders.
        for (i, (decoder, encoder)) in codec_pairs.iter_mut().enumerate() {
            decoder.flush_decoder()?;
            loop {
                let mut frame = Frame::new_audio(1, 1, 0)
                    .unwrap_or_else(|_| Frame::new_video(1, 1).unwrap());
                match decoder.receive_frame(&mut frame) {
                    Ok(()) => {
                        encoder.send_frame(Some(&frame))?;
                        let mut out_pkt = Packet::empty();
                        loop {
                            match encoder.receive_packet(&mut out_pkt) {
                                Ok(()) => {
                                    out_pkt.stream_index = i as u32;
                                    output.write_packet(&out_pkt)?;
                                    packets_written += 1;
                                }
                                Err(Error::Again) => break,
                                Err(e) => return Err(e),
                            }
                        }
                    }
                    Err(Error::Eof) => break,
                    Err(Error::Again) => break,
                    Err(e) => return Err(e),
                }
            }

            // Flush encoder.
            encoder.send_frame(None)?;
            let mut out_pkt = Packet::empty();
            loop {
                match encoder.receive_packet(&mut out_pkt) {
                    Ok(()) => {
                        out_pkt.stream_index = i as u32;
                        output.write_packet(&out_pkt)?;
                        packets_written += 1;
                    }
                    Err(Error::Eof) | Err(Error::Again) => break,
                    Err(e) => return Err(e),
                }
            }
        }

        // Write output trailer.
        output.write_trailer()?;

        Ok(TranscodeResult {
            packets_read,
            packets_written,
        })
    }
}

/// Result of a transcode operation.
#[derive(Debug, Clone)]
pub struct TranscodeResult {
    pub packets_read: u64,
    pub packets_written: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use av_codec::codec_par::CodecParameters;
    use av_format::formats::wav::{WavDemuxer, WavMuxer};
    use av_format::io::IOContext;

    /// Build a WAV file in memory with PCM s16le data.
    fn build_wav(sample_rate: u32, channels: u16, samples: &[u8]) -> Vec<u8> {
        let params = CodecParameters::new_audio(CodecId::PcmS16Le, sample_rate, channels).unwrap();
        let stream = av_format::stream::Stream::new(0, params);
        let mut out = OutputContext::new(IOContext::memory_writer(), Box::new(WavMuxer::new()));
        out.add_stream(stream);
        out.write_header().unwrap();
        out.write_packet(&Packet::new(av_util::buffer::Buffer::from_vec(samples.to_vec()))).unwrap();
        out.write_trailer().unwrap();
        out.into_io().into_vec().unwrap()
    }

    // ── Positive ──

    #[test]
    fn e2e_wav_pcm_roundtrip() {
        let pipeline = Pipeline::with_defaults();

        // Create input WAV.
        let pcm_data: Vec<u8> = (0..400).map(|i| (i % 256) as u8).collect();
        let wav_in = build_wav(44100, 2, &pcm_data);

        // Set up input.
        let mut input = InputContext::new(
            IOContext::from_memory(wav_in),
            Box::new(WavDemuxer::new()),
        );

        // Set up output.
        let mut output = OutputContext::new(
            IOContext::memory_writer(),
            Box::new(WavMuxer::new()),
        );

        // Transcode (PCM → PCM, should be identity).
        let result = pipeline.transcode(
            &mut input,
            &mut output,
            &[CodecId::PcmS16Le],
        ).unwrap();

        assert!(result.packets_read > 0);
        assert!(result.packets_written > 0);

        // Verify output is valid WAV.
        let wav_out = output.into_io().into_vec().unwrap();
        assert!(wav_out.len() > 44);

        // Demux output and verify data matches.
        let mut verify = InputContext::new(
            IOContext::from_memory(wav_out),
            Box::new(WavDemuxer::new()),
        );
        verify.open().unwrap();
        assert_eq!(verify.streams[0].params.codec_id, CodecId::PcmS16Le);

        let out_pkt = verify.read_packet().unwrap();
        assert_eq!(out_pkt.data.data(), &pcm_data);
    }

    #[test]
    fn e2e_pipeline_with_defaults() {
        let pipeline = Pipeline::with_defaults();
        // Verify PCM codecs are registered.
        let pcm_data = vec![0u8; 200];
        let wav_in = build_wav(44100, 1, &pcm_data);

        let mut input = InputContext::new(IOContext::from_memory(wav_in), Box::new(WavDemuxer::new()));
        let mut output = OutputContext::new(IOContext::memory_writer(), Box::new(WavMuxer::new()));

        let result = pipeline.transcode(&mut input, &mut output, &[CodecId::PcmS16Le]).unwrap();
        assert_eq!(result.packets_read, result.packets_written);
    }

    // ── Negative ──

    #[test]
    fn e2e_unsupported_codec() {
        let pipeline = Pipeline::with_defaults();
        let wav_in = build_wav(44100, 1, &vec![0u8; 100]);

        let mut input = InputContext::new(IOContext::from_memory(wav_in), Box::new(WavDemuxer::new()));
        let mut output = OutputContext::new(IOContext::memory_writer(), Box::new(WavMuxer::new()));

        // Try to encode as AAC (stub decoder, will fail on open).
        let result = pipeline.transcode(&mut input, &mut output, &[CodecId::Aac]);
        assert!(result.is_err());
    }

    #[test]
    fn e2e_empty_input() {
        let pipeline = Pipeline::with_defaults();

        let mut input = InputContext::new(
            IOContext::from_memory(vec![]),
            Box::new(WavDemuxer::new()),
        );
        let mut output = OutputContext::new(IOContext::memory_writer(), Box::new(WavMuxer::new()));

        assert!(pipeline.transcode(&mut input, &mut output, &[]).is_err());
    }
}
