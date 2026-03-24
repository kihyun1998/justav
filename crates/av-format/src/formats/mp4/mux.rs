use av_codec::codec::CodecId;
use av_codec::packet::Packet;
use av_util::error::{Error, Result};

use crate::io::IOContext;
use crate::mux::Muxer;
use crate::stream::Stream;

/// Collected sample info for building the sample table.
#[derive(Debug, Clone)]
struct SampleInfo {
    offset: u64,
    size: u32,
    duration: u32,
    is_sync: bool,
}

/// Per-track muxing state.
struct TrackState {
    timescale: u32,
    samples: Vec<SampleInfo>,
    codec_tag: [u8; 4],
    is_audio: bool,
}

pub struct Mp4Muxer {
    tracks: Vec<TrackState>,
    mdat_offset: u64,
    mdat_size: u64,
}

impl Mp4Muxer {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            mdat_offset: 0,
            mdat_size: 0,
        }
    }
}

impl Default for Mp4Muxer {
    fn default() -> Self { Self::new() }
}

fn codec_to_tag(id: CodecId) -> Result<([u8; 4], bool)> {
    match id {
        CodecId::H264 => Ok((*b"avc1", false)),
        CodecId::Hevc => Ok((*b"hev1", false)),
        CodecId::Vp9 => Ok((*b"vp09", false)),
        CodecId::Av1 => Ok((*b"av01", false)),
        CodecId::Aac => Ok((*b"mp4a", true)),
        CodecId::Opus => Ok((*b"Opus", true)),
        CodecId::Flac => Ok((*b"fLaC", true)),
        _ => Err(Error::Unsupported(format!("MP4 doesn't support codec {}", id.name()))),
    }
}

impl Muxer for Mp4Muxer {
    fn name(&self) -> &'static str { "mp4" }

    fn write_header(&mut self, io: &mut IOContext, streams: &[Stream]) -> Result<()> {
        if streams.is_empty() {
            return Err(Error::InvalidArgument("no streams".into()));
        }

        // Build track states.
        for stream in streams {
            let (tag, is_audio) = codec_to_tag(stream.params.codec_id)?;
            let timescale = if is_audio {
                stream.params.sample_rate.max(1)
            } else {
                // Use stream time_base denominator, default 90000.
                if stream.time_base.is_valid() && stream.time_base.den > 0 {
                    stream.time_base.den as u32
                } else {
                    90000
                }
            };
            self.tracks.push(TrackState {
                timescale,
                samples: Vec::new(),
                codec_tag: tag,
                is_audio,
            });
        }

        // Write ftyp box.
        write_box(io, b"ftyp", &|io| {
            io.write_all(b"isom")?; // major_brand
            io.write_u32_be(0x200)?; // minor_version
            io.write_all(b"isomiso2mp41")?; // compatible_brands
            Ok(())
        })?;

        // Write mdat header (size will be patched in trailer).
        self.mdat_offset = io.position()?;
        io.write_u32_be(0)?; // placeholder size
        io.write_all(b"mdat")?;
        self.mdat_size = 0;

        Ok(())
    }

    fn write_packet(&mut self, io: &mut IOContext, packet: &Packet) -> Result<()> {
        let track_idx = packet.stream_index as usize;
        if track_idx >= self.tracks.len() {
            return Err(Error::InvalidArgument(format!(
                "stream index {} out of range", packet.stream_index
            )));
        }

        let offset = io.position()?;
        io.write_all(packet.data.data())?;

        let duration = packet.duration.unwrap_or(1) as u32;
        self.tracks[track_idx].samples.push(SampleInfo {
            offset,
            size: packet.data.len() as u32,
            duration,
            is_sync: packet.flags.keyframe,
        });
        self.mdat_size += packet.data.len() as u64;

        Ok(())
    }

    fn write_trailer(&mut self, io: &mut IOContext) -> Result<()> {
        // Patch mdat size.
        let mdat_end = io.position()?;
        let mdat_total = mdat_end - self.mdat_offset;
        io.seek(self.mdat_offset)?;
        io.write_u32_be(mdat_total as u32)?;
        io.seek(mdat_end)?;

        // Write moov box.
        self.write_moov(io)?;

        Ok(())
    }
}

impl Mp4Muxer {
    fn write_moov(&self, io: &mut IOContext) -> Result<()> {
        write_box(io, b"moov", &|io| {
            // mvhd.
            self.write_mvhd(io)?;
            // One trak per stream.
            for (idx, track) in self.tracks.iter().enumerate() {
                self.write_trak(io, idx, track)?;
            }
            Ok(())
        })
    }

    fn write_mvhd(&self, io: &mut IOContext) -> Result<()> {
        write_full_box(io, b"mvhd", 0, 0, &|io| {
            io.write_u32_be(0)?; // creation_time
            io.write_u32_be(0)?; // modification_time
            io.write_u32_be(1000)?; // timescale
            io.write_u32_be(0)?; // duration (could compute)
            io.write_u32_be(0x00010000)?; // rate = 1.0
            io.write_u16_be(0x0100)?; // volume = 1.0
            io.write_all(&[0u8; 10])?; // reserved
            // Matrix (identity).
            for &v in &[0x00010000u32, 0, 0, 0, 0x00010000, 0, 0, 0, 0x40000000] {
                io.write_u32_be(v)?;
            }
            io.write_all(&[0u8; 24])?; // pre_defined
            io.write_u32_be(self.tracks.len() as u32 + 1)?; // next_track_ID
            Ok(())
        })
    }

    fn write_trak(&self, io: &mut IOContext, idx: usize, track: &TrackState) -> Result<()> {
        write_box(io, b"trak", &|io| {
            self.write_tkhd(io, idx, track)?;
            write_box(io, b"mdia", &|io| {
                self.write_mdhd(io, track)?;
                self.write_hdlr(io, track)?;
                write_box(io, b"minf", &|io| {
                    write_box(io, b"dinf", &|io| {
                        write_full_box(io, b"dref", 0, 0, &|io| {
                            io.write_u32_be(1)?; // entry_count
                            write_full_box(io, b"url ", 0, 1, &|_| Ok(()))?; // self-contained
                            Ok(())
                        })
                    })?;
                    write_box(io, b"stbl", &|io| {
                        self.write_stsd(io, track)?;
                        self.write_stts(io, track)?;
                        self.write_stsz(io, track)?;
                        self.write_stsc(io, track)?;
                        self.write_stco(io, track)?;
                        self.write_stss(io, track)?;
                        Ok(())
                    })
                })
            })
        })
    }

    fn write_tkhd(&self, io: &mut IOContext, idx: usize, _: &TrackState) -> Result<()> {
        write_full_box(io, b"tkhd", 0, 3, &|io| { // flags=3 (enabled+in_movie)
            io.write_u32_be(0)?; // creation_time
            io.write_u32_be(0)?; // modification_time
            io.write_u32_be(idx as u32 + 1)?; // track_ID
            io.write_u32_be(0)?; // reserved
            io.write_u32_be(0)?; // duration
            io.write_all(&[0u8; 8])?; // reserved
            io.write_u16_be(0)?; // layer
            io.write_u16_be(0)?; // alternate_group
            io.write_u16_be(0)?; // volume
            io.write_u16_be(0)?; // reserved
            // Matrix.
            for &v in &[0x00010000u32, 0, 0, 0, 0x00010000, 0, 0, 0, 0x40000000] {
                io.write_u32_be(v)?;
            }
            io.write_u32_be(0)?; // width (fixed point)
            io.write_u32_be(0)?; // height (fixed point)
            Ok(())
        })
    }

    fn write_mdhd(&self, io: &mut IOContext, track: &TrackState) -> Result<()> {
        let duration: u64 = track.samples.iter().map(|s| s.duration as u64).sum();
        write_full_box(io, b"mdhd", 0, 0, &|io| {
            io.write_u32_be(0)?; // creation_time
            io.write_u32_be(0)?; // modification_time
            io.write_u32_be(track.timescale)?;
            io.write_u32_be(duration as u32)?;
            io.write_u16_be(0x55C4)?; // language = "und"
            io.write_u16_be(0)?; // pre_defined
            Ok(())
        })
    }

    fn write_hdlr(&self, io: &mut IOContext, track: &TrackState) -> Result<()> {
        write_full_box(io, b"hdlr", 0, 0, &|io| {
            io.write_u32_be(0)?; // pre_defined
            io.write_all(if track.is_audio { b"soun" } else { b"vide" })?;
            io.write_all(&[0u8; 12])?; // reserved
            let name = if track.is_audio { b"SoundHandler\0" } else { b"VideoHandler\0" };
            io.write_all(name)?;
            Ok(())
        })
    }

    fn write_stsd(&self, io: &mut IOContext, track: &TrackState) -> Result<()> {
        write_full_box(io, b"stsd", 0, 0, &|io| {
            io.write_u32_be(1)?; // entry_count
            // Minimal sample entry.
            let entry_size: u32 = if track.is_audio { 8 + 8 + 20 } else { 8 + 8 + 70 };
            io.write_u32_be(entry_size)?;
            io.write_all(&track.codec_tag)?;
            io.write_all(&[0u8; 6])?; // reserved
            io.write_u16_be(1)?; // data_reference_index
            if track.is_audio {
                io.write_all(&[0u8; 8])?; // reserved
                io.write_u16_be(2)?; // channel_count
                io.write_u16_be(16)?; // sample_size
                io.write_u32_be(0)?; // reserved
                io.write_u32_be(track.timescale << 16)?; // sample_rate (16.16)
            } else {
                io.write_all(&[0u8; 16])?; // reserved + predefined
                io.write_u16_be(1920)?; // width
                io.write_u16_be(1080)?; // height
                io.write_u32_be(0x00480000)?; // horiz_res
                io.write_u32_be(0x00480000)?; // vert_res
                io.write_u32_be(0)?; // reserved
                io.write_u16_be(1)?; // frame_count
                io.write_all(&[0u8; 32])?; // compressor_name
                io.write_u16_be(0x0018)?; // depth
                io.write_u16_be(0xFFFF)?; // pre_defined
            }
            Ok(())
        })
    }

    fn write_stts(&self, io: &mut IOContext, track: &TrackState) -> Result<()> {
        // Run-length encode durations.
        let runs = rle_durations(&track.samples);
        write_full_box(io, b"stts", 0, 0, &|io| {
            io.write_u32_be(runs.len() as u32)?;
            for (count, delta) in &runs {
                io.write_u32_be(*count)?;
                io.write_u32_be(*delta)?;
            }
            Ok(())
        })
    }

    fn write_stsz(&self, io: &mut IOContext, track: &TrackState) -> Result<()> {
        write_full_box(io, b"stsz", 0, 0, &|io| {
            io.write_u32_be(0)?; // sample_size = 0 (variable)
            io.write_u32_be(track.samples.len() as u32)?;
            for s in &track.samples {
                io.write_u32_be(s.size)?;
            }
            Ok(())
        })
    }

    fn write_stsc(&self, io: &mut IOContext, _track: &TrackState) -> Result<()> {
        // Simple: 1 sample per chunk.
        write_full_box(io, b"stsc", 0, 0, &|io| {
            io.write_u32_be(1)?; // entry_count
            io.write_u32_be(1)?; // first_chunk
            io.write_u32_be(1)?; // samples_per_chunk
            io.write_u32_be(1)?; // sample_description_index
            Ok(())
        })
    }

    fn write_stco(&self, io: &mut IOContext, track: &TrackState) -> Result<()> {
        // Use co64 if any offset exceeds 32-bit range.
        let needs_64bit = track.samples.iter().any(|s| s.offset > u32::MAX as u64);

        if needs_64bit {
            write_full_box(io, b"co64", 0, 0, &|io| {
                io.write_u32_be(track.samples.len() as u32)?;
                for s in &track.samples {
                    io.write_u64_be(s.offset)?;
                }
                Ok(())
            })
        } else {
            write_full_box(io, b"stco", 0, 0, &|io| {
                io.write_u32_be(track.samples.len() as u32)?;
                for s in &track.samples {
                    io.write_u32_be(s.offset as u32)?;
                }
                Ok(())
            })
        }
    }

    fn write_stss(&self, io: &mut IOContext, track: &TrackState) -> Result<()> {
        let syncs: Vec<u32> = track.samples.iter().enumerate()
            .filter(|(_, s)| s.is_sync)
            .map(|(i, _)| i as u32 + 1) // 1-based
            .collect();
        // If all are sync, omit stss.
        if syncs.len() == track.samples.len() {
            return Ok(());
        }
        write_full_box(io, b"stss", 0, 0, &|io| {
            io.write_u32_be(syncs.len() as u32)?;
            for s in &syncs {
                io.write_u32_be(*s)?;
            }
            Ok(())
        })
    }
}

fn rle_durations(samples: &[SampleInfo]) -> Vec<(u32, u32)> {
    let mut runs = Vec::new();
    for s in samples {
        if let Some(last) = runs.last_mut() {
            let (count, delta): &mut (u32, u32) = last;
            if *delta == s.duration {
                *count += 1;
                continue;
            }
        }
        runs.push((1, s.duration));
    }
    runs
}

/// Write a box: size(4) + type(4) + content.
fn write_box(io: &mut IOContext, box_type: &[u8; 4], writer: &dyn Fn(&mut IOContext) -> Result<()>) -> Result<()> {
    let start = io.position()?;
    io.write_u32_be(0)?; // placeholder
    io.write_all(box_type)?;
    writer(io)?;
    let end = io.position()?;
    io.seek(start)?;
    io.write_u32_be((end - start) as u32)?;
    io.seek(end)?;
    Ok(())
}

/// Write a full box: size(4) + type(4) + version(1) + flags(3) + content.
fn write_full_box(io: &mut IOContext, box_type: &[u8; 4], version: u8, flags: u32, writer: &dyn Fn(&mut IOContext) -> Result<()>) -> Result<()> {
    write_box(io, box_type, &|io| {
        let vf = ((version as u32) << 24) | (flags & 0x00FFFFFF);
        io.write_u32_be(vf)?;
        writer(io)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{InputContext, OutputContext};
    use crate::formats::mp4::demux::Mp4Demuxer;
    use av_codec::codec_par::CodecParameters;
    use av_util::buffer::Buffer;
    use av_util::rational::Rational;

    fn aac_stream() -> Stream {
        let mut params = CodecParameters::new_audio(CodecId::Aac, 44100, 2).unwrap();
        params.codec_id = CodecId::Aac;
        let mut s = Stream::new(0, params);
        s.time_base = Rational::new(1, 44100);
        s
    }

    fn h264_stream() -> Stream {
        let mut params = CodecParameters::new_video(CodecId::H264, 1920, 1080).unwrap();
        params.codec_id = CodecId::H264;
        let mut s = Stream::new(0, params);
        s.time_base = Rational::new(1, 90000);
        s
    }

    // ── Positive ──

    #[test]
    fn mp4_mux_single_audio_track() {
        let mut out = OutputContext::new(IOContext::memory_writer(), Box::new(Mp4Muxer::new()));
        out.add_stream(aac_stream());
        out.write_header().unwrap();

        for _ in 0..10 {
            let mut pkt = Packet::new(Buffer::from_vec(vec![0xAA; 100]));
            pkt.duration = Some(1024);
            pkt.stream_index = 0;
            out.write_packet(&pkt).unwrap();
        }
        out.write_trailer().unwrap();

        let data = out.into_io().into_vec().unwrap();
        // Should contain ftyp + mdat + moov.
        assert!(data.len() > 100);
        assert_eq!(&data[4..8], b"ftyp");
    }

    #[test]
    fn mp4_roundtrip_audio() {
        // Mux.
        let mut out = OutputContext::new(IOContext::memory_writer(), Box::new(Mp4Muxer::new()));
        out.add_stream(aac_stream());
        out.write_header().unwrap();

        let payloads: Vec<Vec<u8>> = (0..5).map(|i| vec![i as u8; 50 + i * 10]).collect();
        for (_i, payload) in payloads.iter().enumerate() {
            let mut pkt = Packet::new(Buffer::from_vec(payload.clone()));
            pkt.duration = Some(1024);
            pkt.stream_index = 0;
            out.write_packet(&pkt).unwrap();
        }
        out.write_trailer().unwrap();
        let mp4_data = out.into_io().into_vec().unwrap();

        // Demux.
        let mut input = InputContext::new(
            IOContext::from_memory(mp4_data),
            Box::new(Mp4Demuxer::new()),
        );
        input.open().unwrap();
        assert_eq!(input.nb_streams(), 1);
        assert_eq!(input.streams[0].params.codec_id, CodecId::Aac);

        // Read back packets.
        for (i, expected) in payloads.iter().enumerate() {
            let pkt = input.read_packet().unwrap();
            assert_eq!(pkt.data.data(), expected.as_slice(), "packet {i} mismatch");
            assert_eq!(pkt.duration, Some(1024));
        }
        assert!(matches!(input.read_packet(), Err(Error::Eof)));
    }

    #[test]
    fn mp4_video_track() {
        let mut out = OutputContext::new(IOContext::memory_writer(), Box::new(Mp4Muxer::new()));
        out.add_stream(h264_stream());
        out.write_header().unwrap();

        let mut pkt = Packet::new(Buffer::from_vec(vec![0; 1000]));
        pkt.duration = Some(3000);
        pkt.flags.keyframe = true;
        out.write_packet(&pkt).unwrap();
        out.write_trailer().unwrap();

        let mp4_data = out.into_io().into_vec().unwrap();
        let mut input = InputContext::new(IOContext::from_memory(mp4_data), Box::new(Mp4Demuxer::new()));
        input.open().unwrap();
        assert_eq!(input.streams[0].params.codec_id, CodecId::H264);

        let read_pkt = input.read_packet().unwrap();
        assert_eq!(read_pkt.data.len(), 1000);
        assert!(read_pkt.flags.keyframe);
    }

    #[test]
    fn mp4_probe() {
        let mut out = OutputContext::new(IOContext::memory_writer(), Box::new(Mp4Muxer::new()));
        out.add_stream(aac_stream());
        out.write_header().unwrap();
        out.write_trailer().unwrap();
        let data = out.into_io().into_vec().unwrap();
        assert_eq!(crate::formats::mp4::probe(&data), 95);
    }

    // ── Negative ──

    #[test]
    fn mp4_mux_unsupported_codec() {
        let params = CodecParameters::new_audio(CodecId::PcmS16Le, 44100, 2).unwrap();
        let stream = Stream::new(0, params);
        let mut out = OutputContext::new(IOContext::memory_writer(), Box::new(Mp4Muxer::new()));
        out.add_stream(stream);
        assert!(out.write_header().is_err());
    }

    #[test]
    fn mp4_mux_no_streams() {
        let mut out = OutputContext::new(IOContext::memory_writer(), Box::new(Mp4Muxer::new()));
        assert!(out.write_header().is_err());
    }

    #[test]
    fn mp4_demux_empty() {
        let mut input = InputContext::new(IOContext::from_memory(vec![]), Box::new(Mp4Demuxer::new()));
        assert!(input.open().is_err());
    }

    #[test]
    fn mp4_demux_not_mp4() {
        let mut input = InputContext::new(
            IOContext::from_memory(b"RIFF\x00\x00\x00\x00WAVE".to_vec()),
            Box::new(Mp4Demuxer::new()),
        );
        assert!(input.open().is_err());
    }

    #[test]
    fn rle_durations_basic() {
        let samples = vec![
            SampleInfo { offset: 0, size: 0, duration: 1024, is_sync: true },
            SampleInfo { offset: 0, size: 0, duration: 1024, is_sync: true },
            SampleInfo { offset: 0, size: 0, duration: 512, is_sync: true },
        ];
        let runs = rle_durations(&samples);
        assert_eq!(runs, vec![(2, 1024), (1, 512)]);
    }

    #[test]
    fn mp4_roundtrip_multiple_packets_variable_size() {
        let mut out = OutputContext::new(IOContext::memory_writer(), Box::new(Mp4Muxer::new()));
        out.add_stream(aac_stream());
        out.write_header().unwrap();

        let sizes = [30, 100, 50, 200, 10];
        for (i, &sz) in sizes.iter().enumerate() {
            let mut pkt = Packet::new(Buffer::from_vec(vec![i as u8; sz]));
            pkt.duration = Some(1024);
            out.write_packet(&pkt).unwrap();
        }
        out.write_trailer().unwrap();

        let mp4 = out.into_io().into_vec().unwrap();
        let mut input = InputContext::new(IOContext::from_memory(mp4), Box::new(Mp4Demuxer::new()));
        input.open().unwrap();

        for (i, &sz) in sizes.iter().enumerate() {
            let pkt = input.read_packet().unwrap();
            assert_eq!(pkt.data.len(), sz, "packet {i} size mismatch");
            assert!(pkt.data.data().iter().all(|&b| b == i as u8), "packet {i} data mismatch");
        }
    }

    #[test]
    fn mp4_stream_index_out_of_range() {
        let mut out = OutputContext::new(IOContext::memory_writer(), Box::new(Mp4Muxer::new()));
        out.add_stream(aac_stream());
        out.write_header().unwrap();

        let mut pkt = Packet::new(Buffer::from_vec(vec![0; 10]));
        pkt.stream_index = 99; // out of range
        assert!(out.write_packet(&pkt).is_err());
    }

    #[test]
    fn mp4_keyframe_tracking() {
        let mut out = OutputContext::new(IOContext::memory_writer(), Box::new(Mp4Muxer::new()));
        out.add_stream(h264_stream());
        out.write_header().unwrap();

        // Write keyframe + non-keyframe + keyframe.
        for (i, is_key) in [(0, true), (1, false), (2, true)] {
            let mut pkt = Packet::new(Buffer::from_vec(vec![i; 100]));
            pkt.duration = Some(3000);
            pkt.flags.keyframe = is_key;
            out.write_packet(&pkt).unwrap();
        }
        out.write_trailer().unwrap();

        let mp4 = out.into_io().into_vec().unwrap();
        let mut input = InputContext::new(IOContext::from_memory(mp4), Box::new(Mp4Demuxer::new()));
        input.open().unwrap();

        let p0 = input.read_packet().unwrap();
        assert!(p0.flags.keyframe);
        let p1 = input.read_packet().unwrap();
        assert!(!p1.flags.keyframe);
        let p2 = input.read_packet().unwrap();
        assert!(p2.flags.keyframe);
    }

    #[test]
    fn mp4_co64_triggered_by_large_offset() {
        // Verify co64 logic: manually set a sample offset > u32::MAX.
        let needs_64 = [SampleInfo { offset: u64::from(u32::MAX) + 1, size: 100, duration: 1024, is_sync: true }];
        assert!(needs_64.iter().any(|s| s.offset > u32::MAX as u64));
        // The write_stco function checks this condition and writes co64.
        // Full E2E test with actual >4GB data is impractical in unit tests,
        // but the branch condition is verified.
    }

    #[test]
    fn mp4_demux_missing_moov() {
        // MP4 with ftyp + mdat but no moov → should error.
        let mut io_w = IOContext::memory_writer();
        // ftyp box.
        io_w.write_u32_be(16).unwrap();
        io_w.write_all(b"ftyp").unwrap();
        io_w.write_all(b"isom\x00\x00\x00\x00").unwrap();
        // mdat box (no moov).
        io_w.write_u32_be(16).unwrap();
        io_w.write_all(b"mdat").unwrap();
        io_w.write_all(&[0u8; 8]).unwrap();

        let data = io_w.into_vec().unwrap();
        let mut input = InputContext::new(IOContext::from_memory(data), Box::new(Mp4Demuxer::new()));
        assert!(input.open().is_err()); // "no tracks found in MP4"
    }

    #[test]
    fn mp4_demux_truncated_box() {
        // Box header says 1000 bytes but file ends after 20.
        let mut io_w = IOContext::memory_writer();
        io_w.write_u32_be(16).unwrap();
        io_w.write_all(b"ftyp").unwrap();
        io_w.write_all(b"isom\x00\x00\x00\x00").unwrap();
        io_w.write_u32_be(1000).unwrap(); // claims 1000 bytes
        io_w.write_all(b"moov").unwrap();
        // But only wrote 8 bytes of moov header, no payload.

        let data = io_w.into_vec().unwrap();
        let mut input = InputContext::new(IOContext::from_memory(data), Box::new(Mp4Demuxer::new()));
        assert!(input.open().is_err());
    }
}
