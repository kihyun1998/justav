use av_util::buffer::Buffer;
use av_util::error::{Error, Result};
use av_util::mathematics::rescale_q;
use av_util::rational::Rational;

/// Types of side data that can be attached to a packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PacketSideDataType {
    /// Palette for indexed-color video.
    Palette,
    /// New extradata (codec-specific header update).
    NewExtradata,
    /// Parameter change (sample rate, channel layout, etc.).
    ParamChange,
    /// Skip samples at start/end of packet.
    SkipSamples,
    /// Mastering display metadata.
    MasteringDisplayMetadata,
    /// Content light level.
    ContentLightLevel,
    /// Encryption initialization info.
    EncryptionInitInfo,
    /// Encryption info for this specific packet.
    EncryptionInfo,
}

/// A single piece of side data attached to a packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketSideData {
    pub data_type: PacketSideDataType,
    pub data: Buffer,
}

/// Packet flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PacketFlags {
    /// This packet contains a keyframe.
    pub keyframe: bool,
    /// This packet is corrupt (but may still be decodable).
    pub corrupt: bool,
    /// This packet should be discarded after decoding.
    pub discard: bool,
}

/// An encoded media packet — the unit of data passed between
/// demuxer/decoder and encoder/muxer.
#[derive(Debug, Clone)]
pub struct Packet {
    /// The encoded data.
    pub data: Buffer,
    /// Presentation timestamp in `time_base` units.
    pub pts: Option<i64>,
    /// Decompression timestamp in `time_base` units.
    pub dts: Option<i64>,
    /// Duration in `time_base` units.
    pub duration: Option<i64>,
    /// Time base for pts/dts/duration.
    pub time_base: Rational,
    /// Stream index this packet belongs to.
    pub stream_index: u32,
    /// Flags.
    pub flags: PacketFlags,
    /// Byte position of this packet in the stream (-1 if unknown).
    pub pos: i64,
    /// Attached side data.
    side_data: Vec<PacketSideData>,
}

impl Packet {
    /// Create a new packet with the given data.
    pub fn new(data: Buffer) -> Self {
        Self {
            data,
            pts: None,
            dts: None,
            duration: None,
            time_base: Rational::UNKNOWN,
            stream_index: 0,
            flags: PacketFlags::default(),
            pos: -1,
            side_data: Vec::new(),
        }
    }

    /// Create an empty packet (e.g. for flush signaling).
    pub fn empty() -> Self {
        Self::new(Buffer::from_vec(Vec::new()))
    }

    /// Returns the size of the packet data in bytes.
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Returns true if the packet has no data.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns true if this is a keyframe packet.
    pub fn is_keyframe(&self) -> bool {
        self.flags.keyframe
    }

    /// Rescale timestamps from the current time_base to `dst_tb`.
    pub fn rescale_ts(&mut self, dst_tb: Rational) -> Result<()> {
        if !self.time_base.is_valid() || !dst_tb.is_valid() {
            return Err(Error::InvalidArgument("invalid time base".into()));
        }
        if let Some(pts) = self.pts {
            self.pts = Some(rescale_q(pts, self.time_base, dst_tb)?);
        }
        if let Some(dts) = self.dts {
            self.dts = Some(rescale_q(dts, self.time_base, dst_tb)?);
        }
        if let Some(dur) = self.duration {
            self.duration = Some(rescale_q(dur, self.time_base, dst_tb)?);
        }
        self.time_base = dst_tb;
        Ok(())
    }

    // ── Side data ──

    /// Add side data. Replaces existing data of the same type.
    pub fn add_side_data(&mut self, data_type: PacketSideDataType, data: Buffer) {
        self.side_data.retain(|sd| sd.data_type != data_type);
        self.side_data.push(PacketSideData { data_type, data });
    }

    /// Get side data by type.
    pub fn get_side_data(&self, data_type: PacketSideDataType) -> Option<&PacketSideData> {
        self.side_data.iter().find(|sd| sd.data_type == data_type)
    }

    /// Remove side data of the given type.
    pub fn remove_side_data(&mut self, data_type: PacketSideDataType) -> bool {
        let before = self.side_data.len();
        self.side_data.retain(|sd| sd.data_type != data_type);
        self.side_data.len() < before
    }

    /// Number of side data entries.
    pub fn side_data_count(&self) -> usize {
        self.side_data.len()
    }

    /// Clear all fields, keeping the allocation for reuse.
    pub fn reset(&mut self) {
        self.data = Buffer::from_vec(Vec::new());
        self.pts = None;
        self.dts = None;
        self.duration = None;
        self.time_base = Rational::UNKNOWN;
        self.stream_index = 0;
        self.flags = PacketFlags::default();
        self.pos = -1;
        self.side_data.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tb_ms() -> Rational { Rational::new(1, 1000) }
    fn tb_us() -> Rational { Rational::new(1, 1_000_000) }

    // ── Positive ──

    #[test]
    fn new_packet() {
        let pkt = Packet::new(Buffer::from_vec(vec![0xAA; 100]));
        assert_eq!(pkt.size(), 100);
        assert!(!pkt.is_empty());
        assert!(!pkt.is_keyframe());
        assert_eq!(pkt.pos, -1);
    }

    #[test]
    fn empty_packet() {
        let pkt = Packet::empty();
        assert!(pkt.is_empty());
        assert_eq!(pkt.size(), 0);
    }

    #[test]
    fn clone_packet() {
        let mut pkt = Packet::new(Buffer::from_vec(vec![1, 2, 3]));
        pkt.pts = Some(100);
        pkt.stream_index = 1;
        let pkt2 = pkt.clone();
        assert_eq!(pkt2.pts, Some(100));
        assert_eq!(pkt2.data.data(), &[1, 2, 3]);
        assert_eq!(pkt2.stream_index, 1);
    }

    #[test]
    fn keyframe_flag() {
        let mut pkt = Packet::empty();
        pkt.flags.keyframe = true;
        assert!(pkt.is_keyframe());
    }

    #[test]
    fn rescale_ts_ms_to_us() {
        let mut pkt = Packet::new(Buffer::from_vec(vec![0]));
        pkt.pts = Some(500);
        pkt.dts = Some(490);
        pkt.duration = Some(33);
        pkt.time_base = tb_ms();

        pkt.rescale_ts(tb_us()).unwrap();
        assert_eq!(pkt.pts, Some(500_000));
        assert_eq!(pkt.dts, Some(490_000));
        assert_eq!(pkt.duration, Some(33_000));
        assert_eq!(pkt.time_base, tb_us());
    }

    #[test]
    fn rescale_ts_none_fields_stay_none() {
        let mut pkt = Packet::new(Buffer::from_vec(vec![0]));
        pkt.time_base = tb_ms();
        // pts/dts/duration are all None.
        pkt.rescale_ts(tb_us()).unwrap();
        assert_eq!(pkt.pts, None);
        assert_eq!(pkt.dts, None);
        assert_eq!(pkt.duration, None);
    }

    #[test]
    fn side_data_crud() {
        let mut pkt = Packet::empty();
        pkt.add_side_data(PacketSideDataType::Palette, Buffer::from_vec(vec![1, 2]));
        assert_eq!(pkt.side_data_count(), 1);

        let sd = pkt.get_side_data(PacketSideDataType::Palette).unwrap();
        assert_eq!(sd.data.data(), &[1, 2]);

        // Replace.
        pkt.add_side_data(PacketSideDataType::Palette, Buffer::from_vec(vec![3]));
        assert_eq!(pkt.side_data_count(), 1);
        assert_eq!(pkt.get_side_data(PacketSideDataType::Palette).unwrap().data.data(), &[3]);

        // Remove.
        assert!(pkt.remove_side_data(PacketSideDataType::Palette));
        assert_eq!(pkt.side_data_count(), 0);
    }

    #[test]
    fn reset_clears_all() {
        let mut pkt = Packet::new(Buffer::from_vec(vec![1; 100]));
        pkt.pts = Some(42);
        pkt.flags.keyframe = true;
        pkt.add_side_data(PacketSideDataType::Palette, Buffer::from_vec(vec![0]));

        pkt.reset();
        assert!(pkt.is_empty());
        assert_eq!(pkt.pts, None);
        assert!(!pkt.is_keyframe());
        assert_eq!(pkt.side_data_count(), 0);
    }

    #[test]
    fn send_across_threads() {
        fn assert_send<T: Send>() {}
        assert_send::<Packet>();

        let pkt = Packet::new(Buffer::from_vec(vec![42]));
        let handle = std::thread::spawn(move || {
            assert_eq!(pkt.data.data(), &[42]);
        });
        handle.join().unwrap();
    }

    // ── Negative ──

    #[test]
    fn rescale_ts_invalid_src_tb() {
        let mut pkt = Packet::new(Buffer::from_vec(vec![0]));
        pkt.pts = Some(100);
        // time_base is UNKNOWN (default).
        assert!(pkt.rescale_ts(tb_ms()).is_err());
    }

    #[test]
    fn rescale_ts_invalid_dst_tb() {
        let mut pkt = Packet::new(Buffer::from_vec(vec![0]));
        pkt.pts = Some(100);
        pkt.time_base = tb_ms();
        assert!(pkt.rescale_ts(Rational::UNKNOWN).is_err());
    }

    #[test]
    fn get_nonexistent_side_data() {
        let pkt = Packet::empty();
        assert!(pkt.get_side_data(PacketSideDataType::Palette).is_none());
    }

    #[test]
    fn remove_nonexistent_side_data() {
        let mut pkt = Packet::empty();
        assert!(!pkt.remove_side_data(PacketSideDataType::Palette));
    }

    // ── Edge ──

    #[test]
    fn zero_size_data() {
        let pkt = Packet::new(Buffer::from_vec(vec![]));
        assert!(pkt.is_empty());
        assert_eq!(pkt.size(), 0);
    }
}
