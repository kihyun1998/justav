use av_util::error::{Error, Result};
use crate::codec::CodecId;
use crate::codec_par::CodecParameters;

/// H.264 NAL unit types (ITU-T H.264 Table 7-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NalType {
    /// Non-IDR slice (P/B frame).
    Slice,
    /// Slice data partition A.
    SlicePartA,
    /// Slice data partition B.
    SlicePartB,
    /// Slice data partition C.
    SlicePartC,
    /// IDR slice (keyframe).
    SliceIdr,
    /// Supplemental enhancement information.
    Sei,
    /// Sequence parameter set.
    Sps,
    /// Picture parameter set.
    Pps,
    /// Access unit delimiter.
    Aud,
    /// End of sequence.
    EndSequence,
    /// End of stream.
    EndStream,
    /// Filler data.
    Filler,
    /// SPS extension.
    SpsExt,
    /// Coded slice of an auxiliary coded picture.
    AuxSlice,
    /// Coded slice extension (SVC/MVC).
    SliceExt,
    /// Unknown / reserved type.
    Unknown(u8),
}

impl NalType {
    pub fn from_byte(b: u8) -> Self {
        match b & 0x1F {
            1 => Self::Slice,
            2 => Self::SlicePartA,
            3 => Self::SlicePartB,
            4 => Self::SlicePartC,
            5 => Self::SliceIdr,
            6 => Self::Sei,
            7 => Self::Sps,
            8 => Self::Pps,
            9 => Self::Aud,
            10 => Self::EndSequence,
            11 => Self::EndStream,
            12 => Self::Filler,
            13 => Self::SpsExt,
            19 => Self::AuxSlice,
            20 => Self::SliceExt,
            other => Self::Unknown(other),
        }
    }

    /// True if this NAL is a VCL (video coding layer) unit.
    pub fn is_vcl(&self) -> bool {
        matches!(
            self,
            Self::Slice | Self::SlicePartA | Self::SlicePartB
                | Self::SlicePartC | Self::SliceIdr | Self::AuxSlice | Self::SliceExt
        )
    }

    /// True if this is a keyframe NAL.
    pub fn is_idr(&self) -> bool {
        matches!(self, Self::SliceIdr)
    }
}

/// A parsed NAL unit.
#[derive(Debug, Clone)]
pub struct NalUnit {
    /// NAL type.
    pub nal_type: NalType,
    /// NAL reference IDC (0-3).
    pub nal_ref_idc: u8,
    /// Raw RBSP data (without start code or length prefix, including NAL header byte).
    pub data: Vec<u8>,
}

/// Split an Annex B byte stream into individual NAL units.
///
/// Annex B format uses start codes: `0x000001` (3-byte) or `0x00000001` (4-byte).
pub fn split_annex_b(data: &[u8]) -> Vec<NalUnit> {
    // Collect all start code positions first.
    let mut positions = Vec::new(); // (data_start, _code_len)
    let mut search_pos = 0;
    while let Some((data_start, _code_len)) = find_start_code(data, search_pos) {
        positions.push(data_start);
        search_pos = data_start; // continue searching after this NAL data start
    }

    let mut nals = Vec::new();
    for (idx, &nal_start) in positions.iter().enumerate() {
        let nal_end = if idx + 1 < positions.len() {
            // End is at the start of the next start code's zero bytes.
            let next_data_start = positions[idx + 1];
            // Walk back from next_data_start to skip the start code and its leading zeros.
            let mut end = next_data_start;
            // The start code is either 00 00 01 (3 bytes before data) or 00 00 00 01 (4 bytes).
            // Walk back to find where the zeros begin.
            if end >= 4 && data[end - 4] == 0 && data[end - 3] == 0 && data[end - 2] == 0 && data[end - 1] == 1 {
                end -= 4;
            } else if end >= 3 && data[end - 3] == 0 && data[end - 2] == 0 && data[end - 1] == 1 {
                end -= 3;
            }
            end
        } else {
            data.len()
        };

        if nal_start < nal_end {
            let nal_data = &data[nal_start..nal_end];
            let header = nal_data[0];
            nals.push(NalUnit {
                nal_type: NalType::from_byte(header),
                nal_ref_idc: (header >> 5) & 0x03,
                data: nal_data.to_vec(),
            });
        }
    }
    nals
}

/// Find the next start code (`00 00 01` or `00 00 00 01`) at or after `pos`.
/// Returns `Some((data_start, code_len))` — `data_start` is the byte after the start code.
fn find_start_code(data: &[u8], mut pos: usize) -> Option<(usize, usize)> {
    while pos + 2 < data.len() {
        if data[pos] == 0 && data[pos + 1] == 0 {
            if data[pos + 2] == 1 {
                return Some((pos + 3, 3));
            }
            if pos + 3 < data.len() && data[pos + 2] == 0 && data[pos + 3] == 1 {
                return Some((pos + 4, 4));
            }
        }
        pos += 1;
    }
    None
}

/// Split AVCC (length-prefixed) NAL units.
///
/// `length_size` is the number of bytes for each length prefix (typically 4).
pub fn split_avcc(data: &[u8], length_size: usize) -> Result<Vec<NalUnit>> {
    if length_size == 0 || length_size > 4 {
        return Err(Error::InvalidArgument(format!(
            "invalid AVCC length_size: {length_size}"
        )));
    }

    let mut nals = Vec::new();
    let mut pos = 0;

    while pos + length_size <= data.len() {
        let mut nal_len: u32 = 0;
        for j in 0..length_size {
            nal_len = (nal_len << 8) | data[pos + j] as u32;
        }
        pos += length_size;

        let nal_len = nal_len as usize;
        if pos + nal_len > data.len() {
            return Err(Error::InvalidData(format!(
                "AVCC NAL length {nal_len} exceeds remaining data {}", data.len() - pos
            )));
        }
        if nal_len == 0 {
            continue;
        }

        let nal_data = &data[pos..pos + nal_len];
        let header = nal_data[0];
        nals.push(NalUnit {
            nal_type: NalType::from_byte(header),
            nal_ref_idc: (header >> 5) & 0x03,
            data: nal_data.to_vec(),
        });
        pos += nal_len;
    }
    Ok(nals)
}

/// Minimal SPS info extracted from parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpsInfo {
    pub profile_idc: u8,
    pub level_idc: u8,
    pub sps_id: u32,
    pub width: u32,
    pub height: u32,
}

/// Parse basic SPS fields (profile, level, resolution).
///
/// This is a simplified parser — it reads the fixed-position fields and
/// uses Exp-Golomb decoding for the variable fields needed to get dimensions.
pub fn parse_sps_basic(data: &[u8]) -> Result<SpsInfo> {
    if data.len() < 4 {
        return Err(Error::InvalidData("SPS too short".into()));
    }

    // Skip NAL header byte (data[0]).
    let profile_idc = data[1];
    // data[2] = constraint_set flags
    let level_idc = data[3];

    // Exp-Golomb reader starting at byte 4, bit 0.
    let mut reader = ExpGolombReader::new(&data[4..]);
    let sps_id = reader.read_ue()?; // seq_parameter_set_id

    // For High profile and above, there are extra fields to skip.
    if matches!(profile_idc, 100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135) {
        let chroma_format_idc = reader.read_ue()?;
        if chroma_format_idc == 3 {
            reader.skip_bits(1)?; // separate_colour_plane_flag
        }
        let _bit_depth_luma = reader.read_ue()?;
        let _bit_depth_chroma = reader.read_ue()?;
        reader.skip_bits(1)?; // qpprime_y_zero_transform_bypass_flag
        let scaling_matrix_present = reader.read_bits(1)?;
        if scaling_matrix_present != 0 {
            let count = if chroma_format_idc != 3 { 8 } else { 12 };
            for _ in 0..count {
                let present = reader.read_bits(1)?;
                if present != 0 {
                    let size = if count < 6 { 16 } else { 64 };
                    // Skip scaling list.
                    let mut last = 8i32;
                    let mut next = 8i32;
                    for _ in 0..size {
                        if next != 0 {
                            let delta = reader.read_se()?;
                            next = (last + delta + 256) % 256;
                        }
                        last = if next == 0 { last } else { next };
                    }
                }
            }
        }
    }

    let _log2_max_frame_num = reader.read_ue()? + 4;
    let pic_order_cnt_type = reader.read_ue()?;

    match pic_order_cnt_type {
        0 => { let _log2_max_poc_lsb = reader.read_ue()? + 4; }
        1 => {
            reader.skip_bits(1)?; // delta_pic_order_always_zero_flag
            let _offset_non_ref = reader.read_se()?;
            let _offset_top_to_bottom = reader.read_se()?;
            let num_ref_frames = reader.read_ue()?;
            for _ in 0..num_ref_frames {
                let _offset = reader.read_se()?;
            }
        }
        _ => {} // type 2 has no extra fields
    }

    let _max_num_ref_frames = reader.read_ue()?;
    reader.skip_bits(1)?; // gaps_in_frame_num_value_allowed_flag

    let pic_width_in_mbs = reader.read_ue()? + 1;
    let pic_height_in_map_units = reader.read_ue()? + 1;
    let frame_mbs_only_flag = reader.read_bits(1)?;

    let width = pic_width_in_mbs * 16;
    let height = (2 - frame_mbs_only_flag) * pic_height_in_map_units * 16;

    Ok(SpsInfo {
        profile_idc,
        level_idc,
        sps_id,
        width,
        height,
    })
}

impl SpsInfo {
    /// Convert SPS info to CodecParameters.
    pub fn to_codec_parameters(&self) -> Result<CodecParameters> {
        let mut par = CodecParameters::new_video(CodecId::H264, self.width, self.height)?;
        par.codec_id = CodecId::H264;
        Ok(par)
    }
}

/// Simple bitstream reader for Exp-Golomb coded values.
struct ExpGolombReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8, // 0-7, MSB first
}

impl<'a> ExpGolombReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, byte_pos: 0, bit_pos: 0 }
    }

    fn read_bit(&mut self) -> Result<u32> {
        if self.byte_pos >= self.data.len() {
            return Err(Error::Eof);
        }
        let bit = (self.data[self.byte_pos] >> (7 - self.bit_pos)) & 1;
        self.bit_pos += 1;
        if self.bit_pos >= 8 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
        Ok(bit as u32)
    }

    fn read_bits(&mut self, n: u32) -> Result<u32> {
        let mut val = 0u32;
        for _ in 0..n {
            val = (val << 1) | self.read_bit()?;
        }
        Ok(val)
    }

    fn skip_bits(&mut self, n: u32) -> Result<()> {
        for _ in 0..n {
            self.read_bit()?;
        }
        Ok(())
    }

    /// Read unsigned Exp-Golomb coded value.
    fn read_ue(&mut self) -> Result<u32> {
        let mut leading_zeros = 0u32;
        loop {
            let bit = self.read_bit()?;
            if bit == 1 {
                break;
            }
            leading_zeros += 1;
            if leading_zeros > 31 {
                return Err(Error::InvalidData("Exp-Golomb overflow".into()));
            }
        }
        if leading_zeros == 0 {
            return Ok(0);
        }
        let val = self.read_bits(leading_zeros)?;
        Ok((1 << leading_zeros) - 1 + val)
    }

    /// Read signed Exp-Golomb coded value.
    fn read_se(&mut self) -> Result<i32> {
        let ue = self.read_ue()?;
        let val = (ue.div_ceil(2)) as i32;
        if ue % 2 == 0 { Ok(-val) } else { Ok(val) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Annex B splitting ──

    #[test]
    fn split_annex_b_basic() {
        // Two NAL units with 4-byte start codes.
        let data = [
            0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1E, // SPS
            0x00, 0x00, 0x00, 0x01, 0x68, 0x01, 0x02,       // PPS
        ];
        let nals = split_annex_b(&data);
        assert_eq!(nals.len(), 2);
        assert_eq!(nals[0].nal_type, NalType::Sps);
        assert_eq!(nals[1].nal_type, NalType::Pps);
    }

    #[test]
    fn split_annex_b_3byte_start_code() {
        let data = [
            0x00, 0x00, 0x01, 0x67, 0x42, // SPS with 3-byte start code
            0x00, 0x00, 0x01, 0x65, 0xAA, // IDR
        ];
        let nals = split_annex_b(&data);
        assert_eq!(nals.len(), 2);
        assert_eq!(nals[0].nal_type, NalType::Sps);
        assert_eq!(nals[1].nal_type, NalType::SliceIdr);
    }

    #[test]
    fn split_annex_b_single_nal() {
        let data = [0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x02]; // SEI
        let nals = split_annex_b(&data);
        assert_eq!(nals.len(), 1);
        assert_eq!(nals[0].nal_type, NalType::Sei);
    }

    #[test]
    fn split_annex_b_empty() {
        let nals = split_annex_b(&[]);
        assert!(nals.is_empty());
    }

    #[test]
    fn split_annex_b_no_start_code() {
        let nals = split_annex_b(&[0x67, 0x42, 0x00]);
        assert!(nals.is_empty());
    }

    // ── AVCC splitting ──

    #[test]
    fn split_avcc_basic() {
        let data = [
            0x00, 0x00, 0x00, 0x03, 0x67, 0x42, 0x00, // SPS (len=3)
            0x00, 0x00, 0x00, 0x02, 0x68, 0x01,       // PPS (len=2)
        ];
        let nals = split_avcc(&data, 4).unwrap();
        assert_eq!(nals.len(), 2);
        assert_eq!(nals[0].nal_type, NalType::Sps);
        assert_eq!(nals[0].data.len(), 3);
        assert_eq!(nals[1].nal_type, NalType::Pps);
    }

    #[test]
    fn split_avcc_2byte_length() {
        let data = [0x00, 0x02, 0x65, 0xAA]; // IDR (len=2, 2-byte prefix)
        let nals = split_avcc(&data, 2).unwrap();
        assert_eq!(nals.len(), 1);
        assert_eq!(nals[0].nal_type, NalType::SliceIdr);
    }

    #[test]
    fn split_avcc_truncated() {
        let data = [0x00, 0x00, 0x00, 0xFF, 0x67]; // len=255 but only 1 byte
        assert!(split_avcc(&data, 4).is_err());
    }

    #[test]
    fn split_avcc_invalid_length_size() {
        assert!(split_avcc(&[], 0).is_err());
        assert!(split_avcc(&[], 5).is_err());
    }

    #[test]
    fn split_avcc_empty() {
        let nals = split_avcc(&[], 4).unwrap();
        assert!(nals.is_empty());
    }

    // ── NAL type classification ──

    #[test]
    fn nal_type_from_byte() {
        assert_eq!(NalType::from_byte(0x67), NalType::Sps);   // 0x67 & 0x1F = 7
        assert_eq!(NalType::from_byte(0x68), NalType::Pps);   // 8
        assert_eq!(NalType::from_byte(0x65), NalType::SliceIdr); // 5
        assert_eq!(NalType::from_byte(0x41), NalType::Slice);   // 1
        assert_eq!(NalType::from_byte(0x09), NalType::Aud);     // 9
    }

    #[test]
    fn nal_type_vcl() {
        assert!(NalType::SliceIdr.is_vcl());
        assert!(NalType::Slice.is_vcl());
        assert!(!NalType::Sps.is_vcl());
        assert!(!NalType::Pps.is_vcl());
        assert!(!NalType::Sei.is_vcl());
    }

    #[test]
    fn nal_type_idr() {
        assert!(NalType::SliceIdr.is_idr());
        assert!(!NalType::Slice.is_idr());
    }

    // ── SPS parsing ──

    #[test]
    fn parse_sps_baseline_720p() {
        // 1280/16=80 mbs, minus1=79. 720/16=45, minus1=44.
        let sps_data = build_test_sps(66, 31, 79, 44, true);
        let info = parse_sps_basic(&sps_data).unwrap();
        assert_eq!(info.profile_idc, 66);
        assert_eq!(info.level_idc, 31);
        assert_eq!(info.width, 1280);
        assert_eq!(info.height, 720);
    }

    #[test]
    fn parse_sps_baseline_1080p() {
        // 1920/16=120, minus1=119. 1088/16=68, minus1=67.
        let sps_data = build_test_sps(66, 40, 119, 67, true);
        let info = parse_sps_basic(&sps_data).unwrap();
        assert_eq!(info.width, 1920);
        assert_eq!(info.height, 1088); // 68*16=1088 (cropped to 1080 via crop rect)
    }

    #[test]
    fn parse_sps_too_short() {
        assert!(parse_sps_basic(&[0x67, 0x42]).is_err());
    }

    // ── Exp-Golomb ──

    #[test]
    fn exp_golomb_ue() {
        // 1 → ue(0), 010 → ue(1), 011 → ue(2), 00100 → ue(3)
        let data = [0b1_010_011_0, 0b0100_0000];
        let mut r = ExpGolombReader::new(&data);
        assert_eq!(r.read_ue().unwrap(), 0);
        assert_eq!(r.read_ue().unwrap(), 1);
        assert_eq!(r.read_ue().unwrap(), 2);
        assert_eq!(r.read_ue().unwrap(), 3);
    }

    #[test]
    fn exp_golomb_se() {
        // se: ue=0→0, ue=1→1, ue=2→-1, ue=3→2, ue=4→-2
        let data = [0b1_010_011_0, 0b0100_0010, 0b1_0000000];
        let mut r = ExpGolombReader::new(&data);
        assert_eq!(r.read_se().unwrap(), 0);
        assert_eq!(r.read_se().unwrap(), 1);
        assert_eq!(r.read_se().unwrap(), -1);
        assert_eq!(r.read_se().unwrap(), 2);
        assert_eq!(r.read_se().unwrap(), -2);
    }

    /// Build a minimal test SPS bitstream.
    fn build_test_sps(
        profile_idc: u8, level_idc: u8,
        pic_width_in_mbs_minus1: u32, pic_height_in_map_units_minus1: u32,
        frame_mbs_only: bool,
    ) -> Vec<u8> {
        let mut bits = BitWriter::new();
        // NAL header.
        bits.push_byte(0x67); // forbidden=0, nal_ref_idc=3, type=7(SPS)
        bits.push_byte(profile_idc);
        bits.push_byte(0x00); // constraint flags
        bits.push_byte(level_idc);
        // sps_id = 0 (ue: 1)
        bits.push_ue(0);
        // log2_max_frame_num_minus4 = 0
        bits.push_ue(0);
        // pic_order_cnt_type = 0
        bits.push_ue(0);
        // log2_max_pic_order_cnt_lsb_minus4 = 0
        bits.push_ue(0);
        // max_num_ref_frames = 1
        bits.push_ue(1);
        // gaps_in_frame_num_value_allowed_flag = 0
        bits.push_bit(0);
        // pic_width_in_mbs_minus1
        bits.push_ue(pic_width_in_mbs_minus1);
        // pic_height_in_map_units_minus1
        bits.push_ue(pic_height_in_map_units_minus1);
        // frame_mbs_only_flag
        bits.push_bit(if frame_mbs_only { 1 } else { 0 });

        bits.finish()
    }

    /// Minimal bit writer for building test bitstreams.
    struct BitWriter {
        data: Vec<u8>,
        current: u8,
        bit_pos: u8,
    }

    impl BitWriter {
        fn new() -> Self { Self { data: Vec::new(), current: 0, bit_pos: 0 } }

        fn push_bit(&mut self, b: u8) {
            self.current |= (b & 1) << (7 - self.bit_pos);
            self.bit_pos += 1;
            if self.bit_pos >= 8 {
                self.data.push(self.current);
                self.current = 0;
                self.bit_pos = 0;
            }
        }

        fn push_byte(&mut self, b: u8) {
            assert_eq!(self.bit_pos, 0, "push_byte requires byte alignment");
            self.data.push(b);
        }

        fn push_ue(&mut self, val: u32) {
            let code = val + 1;
            let bits = 32 - code.leading_zeros();
            // Leading zeros.
            for _ in 0..bits - 1 {
                self.push_bit(0);
            }
            // Value bits.
            for i in (0..bits).rev() {
                self.push_bit(((code >> i) & 1) as u8);
            }
        }

        fn finish(mut self) -> Vec<u8> {
            if self.bit_pos > 0 {
                self.data.push(self.current);
            }
            self.data
        }
    }
}
