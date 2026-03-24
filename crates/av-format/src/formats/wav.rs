use av_codec::codec::CodecId;
use av_codec::codec_par::CodecParameters;
use av_codec::packet::Packet;
use av_util::buffer::Buffer;
use av_util::error::{Error, Result};
use av_util::frame::MediaType;
use av_util::rational::Rational;

use crate::demux::{DemuxHeader, Demuxer};
use crate::io::IOContext;
use crate::metadata::Metadata;
use crate::mux::Muxer;
use crate::stream::Stream;

const RIFF_TAG: &[u8; 4] = b"RIFF";
const WAVE_TAG: &[u8; 4] = b"WAVE";
const FMT_TAG: &[u8; 4] = b"fmt ";
const DATA_TAG: &[u8; 4] = b"data";

const WAVE_FORMAT_PCM: u16 = 1;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;

/// Map (format_tag, bits_per_sample) to CodecId.
fn wav_to_codec_id(format_tag: u16, bits_per_sample: u16) -> Result<CodecId> {
    match (format_tag, bits_per_sample) {
        (WAVE_FORMAT_PCM, 8) => Ok(CodecId::PcmU8),
        (WAVE_FORMAT_PCM, 16) => Ok(CodecId::PcmS16Le),
        (WAVE_FORMAT_IEEE_FLOAT, 32) => Ok(CodecId::PcmF32Le),
        _ => Err(Error::Unsupported(format!(
            "WAV format_tag={format_tag} bps={bits_per_sample}"
        ))),
    }
}

fn codec_id_to_wav(id: CodecId) -> Result<(u16, u16)> {
    match id {
        CodecId::PcmU8 => Ok((WAVE_FORMAT_PCM, 8)),
        CodecId::PcmS16Le => Ok((WAVE_FORMAT_PCM, 16)),
        CodecId::PcmF32Le => Ok((WAVE_FORMAT_IEEE_FLOAT, 32)),
        _ => Err(Error::Unsupported(format!("WAV doesn't support codec {}", id.name()))),
    }
}

// ────────────────────────────────────────────────────────────────────────────
// WAV Demuxer
// ────────────────────────────────────────────────────────────────────────────

pub struct WavDemuxer {
    data_offset: u64,
    data_size: u64,
    data_read: u64,
    block_align: u16,
}

impl WavDemuxer {
    pub fn new() -> Self {
        Self {
            data_offset: 0,
            data_size: 0,
            data_read: 0,
            block_align: 0,
        }
    }
}

impl Default for WavDemuxer {
    fn default() -> Self { Self::new() }
}

impl Demuxer for WavDemuxer {
    fn name(&self) -> &'static str { "wav" }

    fn read_header(&mut self, io: &mut IOContext) -> Result<DemuxHeader> {
        // RIFF header.
        let mut tag = [0u8; 4];
        io.read_exact(&mut tag)?;
        if &tag != RIFF_TAG {
            return Err(Error::InvalidData("not a RIFF file".into()));
        }
        let _file_size = io.read_u32_le()?;
        io.read_exact(&mut tag)?;
        if &tag != WAVE_TAG {
            return Err(Error::InvalidData("not a WAVE file".into()));
        }

        // Read chunks until we find fmt and data.
        let mut params: Option<CodecParameters> = None;
        let mut sample_rate = 0u32;

        loop {
            let chunk_id = match io.read_bytes(4) {
                Ok(id) => id,
                Err(Error::Eof) => break,
                Err(e) => return Err(e),
            };
            let chunk_size = io.read_u32_le()?;

            if chunk_id == FMT_TAG {
                let format_tag = io.read_u16_le()?;
                let channels = io.read_u16_le()?;
                sample_rate = io.read_u32_le()?;
                let _byte_rate = io.read_u32_le()?;
                self.block_align = io.read_u16_le()?;
                let bits_per_sample = io.read_u16_le()?;

                let codec_id = wav_to_codec_id(format_tag, bits_per_sample)?;
                let mut par = CodecParameters::new_audio(codec_id, sample_rate, channels)?;
                par.bit_rate = (_byte_rate as u64) * 8;
                params = Some(par);

                // Skip any extra fmt bytes.
                let fmt_read = 16u32;
                if chunk_size > fmt_read {
                    io.skip((chunk_size - fmt_read) as i64)?;
                }
            } else if chunk_id == DATA_TAG {
                self.data_offset = io.position()?;
                self.data_size = chunk_size as u64;
                break; // Start reading data.
            } else {
                // Skip unknown chunk.
                io.skip(chunk_size as i64)?;
            }

            // Pad byte for odd chunk sizes.
            if chunk_size % 2 != 0 {
                io.skip(1)?;
            }
        }

        let params = params.ok_or_else(|| Error::InvalidData("missing fmt chunk".into()))?;

        let time_base = Rational::new(1, sample_rate as i32);
        let duration = if self.block_align > 0 {
            Some((self.data_size / self.block_align as u64) as i64)
        } else {
            None
        };

        let mut stream = Stream::new(0, params);
        stream.time_base = time_base;
        stream.duration = duration;

        Ok(DemuxHeader {
            streams: vec![stream],
            metadata: Metadata::new(),
        })
    }

    fn read_packet(&mut self, io: &mut IOContext) -> Result<Packet> {
        if self.data_read >= self.data_size {
            return Err(Error::Eof);
        }

        // Read up to 4096 samples worth per packet.
        let max_bytes = (4096 * self.block_align as u64).min(self.data_size - self.data_read);
        if max_bytes == 0 {
            return Err(Error::Eof);
        }

        let data = io.read_bytes(max_bytes as usize)?;
        let nb_samples = if self.block_align > 0 {
            data.len() as i64 / self.block_align as i64
        } else {
            0
        };

        let pts = if self.block_align > 0 {
            Some((self.data_read / self.block_align as u64) as i64)
        } else {
            None
        };

        self.data_read += data.len() as u64;

        let mut pkt = Packet::new(Buffer::from_vec(data));
        pkt.pts = pts;
        pkt.dts = pts;
        pkt.duration = Some(nb_samples);
        pkt.stream_index = 0;
        Ok(pkt)
    }
}

/// Probe function for WAV format.
pub fn probe(data: &[u8]) -> u32 {
    if data.len() >= 12 && &data[0..4] == RIFF_TAG && &data[8..12] == WAVE_TAG {
        90
    } else {
        0
    }
}

// ────────────────────────────────────────────────────────────────────────────
// WAV Muxer
// ────────────────────────────────────────────────────────────────────────────

pub struct WavMuxer {
    data_offset: u64,
    data_size: u64,
}

impl WavMuxer {
    pub fn new() -> Self {
        Self { data_offset: 0, data_size: 0 }
    }
}

impl Default for WavMuxer {
    fn default() -> Self { Self::new() }
}

impl Muxer for WavMuxer {
    fn name(&self) -> &'static str { "wav" }

    fn write_header(&mut self, io: &mut IOContext, streams: &[Stream]) -> Result<()> {
        if streams.len() != 1 || streams[0].params.media_type != MediaType::Audio {
            return Err(Error::InvalidArgument("WAV requires exactly 1 audio stream".into()));
        }
        let par = &streams[0].params;
        let (format_tag, bits_per_sample) = codec_id_to_wav(par.codec_id)?;
        let channels = par.channels;
        let sample_rate = par.sample_rate;
        let block_align = channels * (bits_per_sample / 8);
        let byte_rate = sample_rate * block_align as u32;

        // RIFF header (size placeholder = 0, filled in trailer).
        io.write_all(RIFF_TAG)?;
        io.write_u32_le(0)?; // file size - 8 (placeholder)
        io.write_all(WAVE_TAG)?;

        // fmt chunk.
        io.write_all(FMT_TAG)?;
        io.write_u32_le(16)?; // chunk size
        io.write_u16_le(format_tag)?;
        io.write_u16_le(channels)?;
        io.write_u32_le(sample_rate)?;
        io.write_u32_le(byte_rate)?;
        io.write_u16_le(block_align)?;
        io.write_u16_le(bits_per_sample)?;

        // data chunk header (size placeholder).
        io.write_all(DATA_TAG)?;
        io.write_u32_le(0)?; // data size (placeholder)
        self.data_offset = io.position()?;
        self.data_size = 0;

        Ok(())
    }

    fn write_packet(&mut self, io: &mut IOContext, packet: &Packet) -> Result<()> {
        io.write_all(packet.data.data())?;
        self.data_size += packet.data.len() as u64;
        Ok(())
    }

    fn write_trailer(&mut self, io: &mut IOContext) -> Result<()> {
        let file_end = io.position()?;

        // Patch RIFF size (offset 4).
        io.seek(4)?;
        io.write_u32_le((file_end - 8) as u32)?;

        // Patch data chunk size (data_offset - 4).
        io.seek(self.data_offset - 4)?;
        io.write_u32_le(self.data_size as u32)?;

        // Seek back to end.
        io.seek(file_end)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{InputContext, OutputContext};

    // ── Positive ──

    #[test]
    fn wav_probe_valid() {
        let mut data = vec![0u8; 44];
        data[0..4].copy_from_slice(RIFF_TAG);
        data[8..12].copy_from_slice(WAVE_TAG);
        assert_eq!(probe(&data), 90);
    }

    #[test]
    fn wav_probe_invalid() {
        assert_eq!(probe(b"NOT_RIFF_DATA_"), 0);
    }

    #[test]
    fn wav_roundtrip_s16le() {
        // Create WAV via muxer.
        let params = CodecParameters::new_audio(CodecId::PcmS16Le, 44100, 2).unwrap();
        let stream = Stream::new(0, params.clone());
        let mux_io = IOContext::memory_writer();
        let mut out = OutputContext::new(mux_io, Box::new(WavMuxer::new()));
        out.add_stream(stream);
        out.write_header().unwrap();

        // Write some PCM data (100 samples * 2 channels * 2 bytes = 400 bytes).
        let pcm_data: Vec<u8> = (0..400).map(|i| (i % 256) as u8).collect();
        let pkt = Packet::new(Buffer::from_vec(pcm_data.clone()));
        out.write_packet(&pkt).unwrap();
        out.write_trailer().unwrap();

        // Get the WAV bytes.
        let wav_bytes = out.into_io().into_vec().unwrap();
        assert!(wav_bytes.len() > 44); // header + data

        // Demux the WAV.
        let demux_io = IOContext::from_memory(wav_bytes);
        let mut input = InputContext::new(demux_io, Box::new(WavDemuxer::new()));
        input.open().unwrap();

        assert_eq!(input.nb_streams(), 1);
        assert_eq!(input.streams[0].params.codec_id, CodecId::PcmS16Le);
        assert_eq!(input.streams[0].params.sample_rate, 44100);
        assert_eq!(input.streams[0].params.channels, 2);

        // Read packets back.
        let read_pkt = input.read_packet().unwrap();
        assert_eq!(read_pkt.data.data(), &pcm_data);
    }

    #[test]
    fn wav_roundtrip_u8() {
        let params = CodecParameters::new_audio(CodecId::PcmU8, 8000, 1).unwrap();
        let stream = Stream::new(0, params);
        let mux_io = IOContext::memory_writer();
        let mut out = OutputContext::new(mux_io, Box::new(WavMuxer::new()));
        out.add_stream(stream);
        out.write_header().unwrap();

        let pcm_data = vec![128u8; 200]; // 200 samples
        out.write_packet(&Packet::new(Buffer::from_vec(pcm_data.clone()))).unwrap();
        out.write_trailer().unwrap();

        let wav_bytes = out.into_io().into_vec().unwrap();
        let demux_io = IOContext::from_memory(wav_bytes);
        let mut input = InputContext::new(demux_io, Box::new(WavDemuxer::new()));
        input.open().unwrap();

        assert_eq!(input.streams[0].params.codec_id, CodecId::PcmU8);
        let read_pkt = input.read_packet().unwrap();
        assert_eq!(read_pkt.data.data(), &pcm_data);
    }

    #[test]
    fn wav_roundtrip_f32le() {
        let params = CodecParameters::new_audio(CodecId::PcmF32Le, 48000, 2).unwrap();
        let stream = Stream::new(0, params);
        let mux_io = IOContext::memory_writer();
        let mut out = OutputContext::new(mux_io, Box::new(WavMuxer::new()));
        out.add_stream(stream);
        out.write_header().unwrap();

        let pcm_data = vec![0u8; 48000 * 2 * 4]; // 1 second of silence
        out.write_packet(&Packet::new(Buffer::from_vec(pcm_data.clone()))).unwrap();
        out.write_trailer().unwrap();

        let wav_bytes = out.into_io().into_vec().unwrap();
        let demux_io = IOContext::from_memory(wav_bytes);
        let mut input = InputContext::new(demux_io, Box::new(WavDemuxer::new()));
        input.open().unwrap();

        assert_eq!(input.streams[0].params.codec_id, CodecId::PcmF32Le);
        assert_eq!(input.streams[0].params.sample_rate, 48000);
    }

    #[test]
    fn wav_packet_pts() {
        let params = CodecParameters::new_audio(CodecId::PcmS16Le, 44100, 1).unwrap();
        let stream = Stream::new(0, params);
        let mux_io = IOContext::memory_writer();
        let mut out = OutputContext::new(mux_io, Box::new(WavMuxer::new()));
        out.add_stream(stream);
        out.write_header().unwrap();

        // Write 2 packets of 100 samples each.
        let pcm = vec![0u8; 200]; // 100 samples * 1ch * 2bytes
        out.write_packet(&Packet::new(Buffer::from_vec(pcm.clone()))).unwrap();
        out.write_packet(&Packet::new(Buffer::from_vec(pcm))).unwrap();
        out.write_trailer().unwrap();

        let wav_bytes = out.into_io().into_vec().unwrap();
        let mut input = InputContext::new(IOContext::from_memory(wav_bytes), Box::new(WavDemuxer::new()));
        input.open().unwrap();

        let pkt1 = input.read_packet().unwrap();
        assert_eq!(pkt1.pts, Some(0));
        // PTS of second read depends on packet size.
    }

    #[test]
    fn wav_eof_after_data() {
        let params = CodecParameters::new_audio(CodecId::PcmS16Le, 44100, 1).unwrap();
        let stream = Stream::new(0, params);
        let mux_io = IOContext::memory_writer();
        let mut out = OutputContext::new(mux_io, Box::new(WavMuxer::new()));
        out.add_stream(stream);
        out.write_header().unwrap();

        let pcm = vec![0u8; 20]; // 10 samples
        out.write_packet(&Packet::new(Buffer::from_vec(pcm))).unwrap();
        out.write_trailer().unwrap();

        let wav_bytes = out.into_io().into_vec().unwrap();
        let mut input = InputContext::new(IOContext::from_memory(wav_bytes), Box::new(WavDemuxer::new()));
        input.open().unwrap();

        let _pkt1 = input.read_packet().unwrap();
        assert!(matches!(input.read_packet(), Err(Error::Eof)));
    }

    // ── Negative ──

    #[test]
    fn demux_not_riff() {
        let mut input = InputContext::new(
            IOContext::from_memory(b"NOT_RIFF_DATA_AT_ALL".to_vec()),
            Box::new(WavDemuxer::new()),
        );
        assert!(input.open().is_err());
    }

    #[test]
    fn demux_riff_not_wave() {
        let mut data = vec![0u8; 12];
        data[0..4].copy_from_slice(RIFF_TAG);
        data[4..8].copy_from_slice(&100u32.to_le_bytes());
        data[8..12].copy_from_slice(b"AVI ");
        let mut input = InputContext::new(
            IOContext::from_memory(data),
            Box::new(WavDemuxer::new()),
        );
        assert!(input.open().is_err());
    }

    #[test]
    fn mux_no_streams() {
        let mux_io = IOContext::memory_writer();
        let mut out = OutputContext::new(mux_io, Box::new(WavMuxer::new()));
        assert!(out.write_header().is_err());
    }

    #[test]
    fn mux_video_stream_rejected() {
        let params = CodecParameters::new_video(CodecId::H264, 1920, 1080).unwrap();
        let stream = Stream::new(0, params);
        let mux_io = IOContext::memory_writer();
        let mut out = OutputContext::new(mux_io, Box::new(WavMuxer::new()));
        out.add_stream(stream);
        assert!(out.write_header().is_err());
    }

    #[test]
    fn mux_unsupported_codec() {
        let params = CodecParameters::new_audio(CodecId::Aac, 48000, 2).unwrap();
        let stream = Stream::new(0, params);
        let mux_io = IOContext::memory_writer();
        let mut out = OutputContext::new(mux_io, Box::new(WavMuxer::new()));
        out.add_stream(stream);
        assert!(out.write_header().is_err());
    }

    #[test]
    fn demux_truncated_riff_header() {
        // Less than 12 bytes — too short for RIFF+size+WAVE.
        let mut input = InputContext::new(
            IOContext::from_memory(b"RIFF".to_vec()),
            Box::new(WavDemuxer::new()),
        );
        assert!(input.open().is_err());
    }

    #[test]
    fn demux_riff_missing_fmt_chunk() {
        // Valid RIFF/WAVE header but goes straight to data without fmt.
        let mut data = Vec::new();
        data.extend_from_slice(RIFF_TAG);
        data.extend_from_slice(&100u32.to_le_bytes()); // file size
        data.extend_from_slice(WAVE_TAG);
        data.extend_from_slice(DATA_TAG);
        data.extend_from_slice(&20u32.to_le_bytes()); // data chunk size
        data.extend_from_slice(&vec![0u8; 20]); // data

        let mut input = InputContext::new(
            IOContext::from_memory(data),
            Box::new(WavDemuxer::new()),
        );
        assert!(input.open().is_err()); // missing fmt → error
    }

    #[test]
    fn demux_empty_data_chunk() {
        // Valid WAV with 0-byte data chunk.
        let params = CodecParameters::new_audio(CodecId::PcmS16Le, 44100, 1).unwrap();
        let stream = Stream::new(0, params);
        let mux_io = IOContext::memory_writer();
        let mut out = OutputContext::new(mux_io, Box::new(WavMuxer::new()));
        out.add_stream(stream);
        out.write_header().unwrap();
        // No packets written.
        out.write_trailer().unwrap();

        let wav_bytes = out.into_io().into_vec().unwrap();
        let mut input = InputContext::new(IOContext::from_memory(wav_bytes), Box::new(WavDemuxer::new()));
        input.open().unwrap();
        // Should immediately EOF.
        assert!(matches!(input.read_packet(), Err(Error::Eof)));
    }
}
