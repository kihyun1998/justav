use av_codec::codec::CodecId;
use av_codec::codec_par::CodecParameters;
use av_codec::packet::Packet;
use av_util::buffer::Buffer;
use av_util::error::{Error, Result};
use av_util::rational::Rational;

use crate::demux::{DemuxHeader, Demuxer};
use crate::formats::mp4::boxes::{read_box_header, read_full_box_header, skip_box, is_container_box};
use crate::io::IOContext;
use crate::metadata::Metadata;
use crate::stream::Stream;

/// Sample entry from the sample table.
#[derive(Debug, Clone)]
struct SampleEntry {
    offset: u64,
    size: u32,
    duration: u32,
    is_sync: bool,
}

/// Track info extracted from moov.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TrackInfo {
    track_id: u32,
    timescale: u32,
    codec_id: CodecId,
    params: CodecParameters,
    samples: Vec<SampleEntry>,
}

pub struct Mp4Demuxer {
    tracks: Vec<TrackInfo>,
    current_track: usize,
    current_sample: usize,
}

impl Mp4Demuxer {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            current_track: 0,
            current_sample: 0,
        }
    }
}

impl Default for Mp4Demuxer {
    fn default() -> Self { Self::new() }
}

impl Demuxer for Mp4Demuxer {
    fn name(&self) -> &'static str { "mp4" }

    fn read_header(&mut self, io: &mut IOContext) -> Result<DemuxHeader> {
        let mut streams = Vec::new();

        // Scan top-level boxes for moov.
        while let Ok(header) = read_box_header(io) {
            if header.is_type(b"moov") {
                self.parse_moov(io, header.payload_offset, header.payload_size)?;
            } else {
                skip_box(io, &header)?;
            }
        }

        // Build streams from parsed tracks.
        for (idx, track) in self.tracks.iter().enumerate() {
            let tb = Rational::new(1, track.timescale as i32);
            let total_duration: u64 = track.samples.iter().map(|s| s.duration as u64).sum();
            let mut stream = Stream::new(idx as u32, track.params.clone());
            stream.time_base = tb;
            stream.duration = Some(total_duration as i64);
            stream.nb_frames = Some(track.samples.len() as u64);
            streams.push(stream);
        }

        if streams.is_empty() {
            return Err(Error::InvalidData("no tracks found in MP4".into()));
        }

        Ok(DemuxHeader {
            streams,
            metadata: Metadata::new(),
        })
    }

    fn read_packet(&mut self, io: &mut IOContext) -> Result<Packet> {
        // Simple sequential reading: iterate all samples across all tracks.
        // Find the next sample with the earliest DTS.
        if self.tracks.is_empty() {
            return Err(Error::Eof);
        }

        // For simplicity, read sequentially from track 0, then track 1, etc.
        while self.current_track < self.tracks.len() {
            let track = &self.tracks[self.current_track];
            if self.current_sample < track.samples.len() {
                let sample = &track.samples[self.current_sample];

                io.seek(sample.offset)?;
                let data = io.read_bytes(sample.size as usize)?;

                // Compute PTS from cumulative durations.
                let pts: i64 = track.samples[..self.current_sample]
                    .iter()
                    .map(|s| s.duration as i64)
                    .sum();

                let mut pkt = Packet::new(Buffer::from_vec(data));
                pkt.pts = Some(pts);
                pkt.dts = Some(pts);
                pkt.duration = Some(sample.duration as i64);
                pkt.stream_index = self.current_track as u32;
                pkt.flags.keyframe = sample.is_sync;
                pkt.time_base = Rational::new(1, track.timescale as i32);

                self.current_sample += 1;
                return Ok(pkt);
            }
            self.current_track += 1;
            self.current_sample = 0;
        }

        Err(Error::Eof)
    }
}

impl Mp4Demuxer {
    fn parse_moov(&mut self, io: &mut IOContext, offset: u64, size: u64) -> Result<()> {
        let end = offset + size;
        io.seek(offset)?;

        while io.position()? < end {
            let header = match read_box_header(io) {
                Ok(h) => h,
                Err(_) => break,
            };

            if header.is_type(b"trak") {
                if let Ok(track) = self.parse_trak(io, header.payload_offset, header.payload_size) {
                    self.tracks.push(track);
                }
            } else {
                skip_box(io, &header)?;
            }
        }
        Ok(())
    }

    fn parse_trak(&self, io: &mut IOContext, offset: u64, size: u64) -> Result<TrackInfo> {
        let end = offset + size;
        io.seek(offset)?;

        let mut track_id = 0u32;
        let mut timescale = 1000u32;
        let mut codec_id = CodecId::None;
        let mut width = 0u32;
        let mut height = 0u32;
        let mut sample_rate = 0u32;
        let mut channels = 0u16;
        let mut is_audio = false;

        // Sample table data.
        let mut sample_sizes: Vec<u32> = Vec::new();
        let mut chunk_offsets: Vec<u64> = Vec::new();
        let mut sample_durations: Vec<(u32, u32)> = Vec::new(); // (count, delta)
        let mut samples_per_chunk: Vec<(u32, u32)> = Vec::new(); // (first_chunk, samples_per_chunk)
        let mut sync_samples: Option<Vec<u32>> = None;

        // Recursive box parsing.
        self.parse_boxes(io, end, &mut |io, header| {
            match &header.box_type {
                b"tkhd" => {
                    let (version, _flags) = read_full_box_header(io)?;
                    if version == 1 {
                        io.skip(8)?; // creation_time
                        io.skip(8)?; // modification_time
                        track_id = io.read_u32_be()?;
                    } else {
                        io.skip(4)?;
                        io.skip(4)?;
                        track_id = io.read_u32_be()?;
                    }
                    Ok(false) // don't recurse
                }
                b"mdhd" => {
                    let (version, _) = read_full_box_header(io)?;
                    if version == 1 {
                        io.skip(8)?; io.skip(8)?;
                        timescale = io.read_u32_be()?;
                    } else {
                        io.skip(4)?; io.skip(4)?;
                        timescale = io.read_u32_be()?;
                    }
                    Ok(false)
                }
                b"hdlr" => {
                    let _ = read_full_box_header(io)?;
                    io.skip(4)?; // pre_defined
                    let mut handler = [0u8; 4];
                    io.read_exact(&mut handler)?;
                    is_audio = &handler == b"soun";
                    Ok(false)
                }
                b"stsd" => {
                    let _ = read_full_box_header(io)?;
                    let entry_count = io.read_u32_be()?;
                    if entry_count > 0 {
                        let entry_header = read_box_header(io)?;
                        let tag = entry_header.box_type;
                        // Skip 6 reserved + 2 data_ref_index.
                        io.skip(8)?;

                        if is_audio {
                            // Audio sample entry.
                            io.skip(8)?; // reserved
                            channels = io.read_u16_be()?;
                            io.skip(2)?; // sample_size
                            io.skip(4)?; // reserved
                            sample_rate = io.read_u32_be()? >> 16; // 16.16 fixed point
                            codec_id = match &tag {
                                b"mp4a" => CodecId::Aac,
                                b"Opus" => CodecId::Opus,
                                b"fLaC" => CodecId::Flac,
                                _ => CodecId::None,
                            };
                        } else {
                            // Video sample entry.
                            io.skip(16)?; // reserved + predefined
                            width = io.read_u16_be()? as u32;
                            height = io.read_u16_be()? as u32;
                            codec_id = match &tag {
                                b"avc1" | b"avc3" => CodecId::H264,
                                b"hvc1" | b"hev1" => CodecId::Hevc,
                                b"vp09" => CodecId::Vp9,
                                b"av01" => CodecId::Av1,
                                _ => CodecId::None,
                            };
                        }
                    }
                    Ok(false)
                }
                b"stsz" => {
                    let _ = read_full_box_header(io)?;
                    let default_size = io.read_u32_be()?;
                    let count = io.read_u32_be()?;
                    if default_size == 0 {
                        for _ in 0..count {
                            sample_sizes.push(io.read_u32_be()?);
                        }
                    } else {
                        sample_sizes = vec![default_size; count as usize];
                    }
                    Ok(false)
                }
                b"stco" => {
                    let _ = read_full_box_header(io)?;
                    let count = io.read_u32_be()?;
                    for _ in 0..count {
                        chunk_offsets.push(io.read_u32_be()? as u64);
                    }
                    Ok(false)
                }
                b"co64" => {
                    let _ = read_full_box_header(io)?;
                    let count = io.read_u32_be()?;
                    for _ in 0..count {
                        chunk_offsets.push(io.read_u64_be()?);
                    }
                    Ok(false)
                }
                b"stts" => {
                    let _ = read_full_box_header(io)?;
                    let count = io.read_u32_be()?;
                    for _ in 0..count {
                        let sample_count = io.read_u32_be()?;
                        let sample_delta = io.read_u32_be()?;
                        sample_durations.push((sample_count, sample_delta));
                    }
                    Ok(false)
                }
                b"stsc" => {
                    let _ = read_full_box_header(io)?;
                    let count = io.read_u32_be()?;
                    for _ in 0..count {
                        let first_chunk = io.read_u32_be()?;
                        let spc = io.read_u32_be()?;
                        let _desc_idx = io.read_u32_be()?;
                        samples_per_chunk.push((first_chunk, spc));
                    }
                    Ok(false)
                }
                b"stss" => {
                    let _ = read_full_box_header(io)?;
                    let count = io.read_u32_be()?;
                    let mut syncs = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        syncs.push(io.read_u32_be()?);
                    }
                    sync_samples = Some(syncs);
                    Ok(false)
                }
                _ if is_container_box(&header.box_type) => Ok(true), // recurse
                _ => Ok(false), // skip
            }
        })?;

        // Build sample table from stsc + stco + stsz + stts.
        let samples = build_sample_table(
            &sample_sizes,
            &chunk_offsets,
            &sample_durations,
            &samples_per_chunk,
            &sync_samples,
        )?;

        let params = if is_audio {
            let sr = if sample_rate > 0 { sample_rate } else { 44100 };
            let ch = if channels > 0 { channels } else { 2 };
            CodecParameters::new_audio(codec_id, sr, ch)?
        } else {
            let w = if width > 0 { width } else { 1920 };
            let h = if height > 0 { height } else { 1080 };
            CodecParameters::new_video(codec_id, w, h)?
        };

        Ok(TrackInfo {
            track_id,
            timescale,
            codec_id,
            params,
            samples,
        })
    }

    fn parse_boxes(
        &self,
        io: &mut IOContext,
        end: u64,
        handler: &mut dyn FnMut(&mut IOContext, &crate::formats::mp4::boxes::BoxHeader) -> Result<bool>,
    ) -> Result<()> {
        while io.position()? < end {
            let header = match read_box_header(io) {
                Ok(h) => h,
                Err(_) => break,
            };

            let box_end = header.payload_offset + header.payload_size;
            let recurse = handler(io, &header)?;

            if recurse {
                self.parse_boxes(io, box_end, handler)?;
            }

            io.seek(box_end)?;
        }
        Ok(())
    }
}

/// Build a flat sample table from the MP4 sample table boxes.
fn build_sample_table(
    sizes: &[u32],
    chunk_offsets: &[u64],
    durations: &[(u32, u32)],
    samples_per_chunk: &[(u32, u32)],
    sync_samples: &Option<Vec<u32>>,
) -> Result<Vec<SampleEntry>> {
    let num_samples = sizes.len();
    if num_samples == 0 {
        return Ok(Vec::new());
    }

    // Expand durations.
    let mut dur_expanded = Vec::with_capacity(num_samples);
    for &(count, delta) in durations {
        for _ in 0..count {
            dur_expanded.push(delta);
        }
    }
    // Pad if needed.
    while dur_expanded.len() < num_samples {
        dur_expanded.push(dur_expanded.last().copied().unwrap_or(1));
    }

    // Expand sample-to-chunk mapping to get (chunk_index, sample_offset_in_chunk).
    let mut sample_chunk_map: Vec<(usize, usize)> = Vec::with_capacity(num_samples);
    if samples_per_chunk.is_empty() || chunk_offsets.is_empty() {
        // Fallback: one sample per chunk.
        for i in 0..num_samples {
            let chunk_idx = i.min(chunk_offsets.len().saturating_sub(1));
            sample_chunk_map.push((chunk_idx, 0));
        }
    } else {
        let mut sample_idx = 0;
        let num_chunks = chunk_offsets.len();
        for chunk_idx in 0..num_chunks {
            let chunk_num = chunk_idx as u32 + 1; // 1-based
            // Find how many samples in this chunk.
            let spc = samples_per_chunk
                .iter()
                .rev()
                .find(|&&(first, _)| first <= chunk_num)
                .map(|&(_, spc)| spc)
                .unwrap_or(1);

            for s in 0..spc as usize {
                if sample_idx >= num_samples {
                    break;
                }
                sample_chunk_map.push((chunk_idx, s));
                sample_idx += 1;
            }
        }
    }

    // Build sample entries with offsets.
    let mut samples = Vec::with_capacity(num_samples);
    let sync_set: Option<std::collections::HashSet<u32>> =
        sync_samples.as_ref().map(|v| v.iter().copied().collect());

    for i in 0..num_samples {
        let (chunk_idx, offset_in_chunk) = if i < sample_chunk_map.len() {
            sample_chunk_map[i]
        } else {
            (0, 0)
        };

        let chunk_offset = chunk_offsets.get(chunk_idx).copied().unwrap_or(0);
        // Offset within chunk = sum of sizes of previous samples in this chunk.
        let intra_offset: u64 = if offset_in_chunk > 0 {
            // Find the first sample in this chunk, then sum sizes up to our position.
            let first_in_chunk = sample_chunk_map
                .iter()
                .position(|&(ci, _)| ci == chunk_idx)
                .unwrap_or(i);
            sizes[first_in_chunk..first_in_chunk + offset_in_chunk]
                .iter()
                .map(|&s| s as u64)
                .sum()
        } else {
            0
        };

        let is_sync = match &sync_set {
            Some(set) => set.contains(&(i as u32 + 1)), // stss is 1-based
            None => true, // No stss = all samples are sync.
        };

        samples.push(SampleEntry {
            offset: chunk_offset + intra_offset,
            size: sizes[i],
            duration: dur_expanded[i],
            is_sync,
        });
    }

    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_sample_table_basic() {
        let sizes = vec![100, 200, 150];
        let offsets = vec![1000u64]; // 1 chunk
        let durations = vec![(3, 1024)]; // 3 samples, each 1024
        let spc = vec![(1, 3)]; // chunk 1 has 3 samples
        let table = build_sample_table(&sizes, &offsets, &durations, &spc, &None).unwrap();

        assert_eq!(table.len(), 3);
        assert_eq!(table[0].offset, 1000);
        assert_eq!(table[0].size, 100);
        assert_eq!(table[1].offset, 1100); // 1000 + 100
        assert_eq!(table[2].offset, 1300); // 1000 + 100 + 200
        assert!(table[0].is_sync); // no stss = all sync
    }

    #[test]
    fn build_sample_table_with_sync() {
        let sizes = vec![100; 5];
        let offsets = vec![0u64];
        let durations = vec![(5, 512)];
        let spc = vec![(1, 5)];
        let syncs = Some(vec![1, 3]); // samples 1 and 3 are sync (1-based)
        let table = build_sample_table(&sizes, &offsets, &durations, &spc, &syncs).unwrap();

        assert!(table[0].is_sync);  // sample 1
        assert!(!table[1].is_sync); // sample 2
        assert!(table[2].is_sync);  // sample 3
        assert!(!table[3].is_sync); // sample 4
    }

    #[test]
    fn build_sample_table_empty() {
        let table = build_sample_table(&[], &[], &[], &[], &None).unwrap();
        assert!(table.is_empty());
    }
}
