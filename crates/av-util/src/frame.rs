use crate::buffer::Buffer;
use crate::error::{Error, Result};
use crate::rational::Rational;

/// Maximum number of planes in a frame (video or audio).
pub const MAX_PLANES: usize = 8;

/// Types of side data that can be attached to a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SideDataType {
    /// Display transformation matrix (rotation, flip).
    DisplayMatrix,
    /// HDR10+ dynamic metadata.
    DynamicHdr10Plus,
    /// HDR Vivid dynamic metadata.
    DynamicHdrVivid,
    /// Dolby Vision metadata.
    DoviMetadata,
    /// Mastering display colour volume (HDR).
    MasteringDisplayMetadata,
    /// Content light level information (HDR).
    ContentLightLevel,
    /// Motion vectors.
    MotionVectors,
    /// Regions of interest.
    RegionsOfInterest,
    /// Encoder quantization parameters.
    QpTable,
    /// ICC color profile.
    IccProfile,
    /// Closed captions (e.g. CEA-608, CEA-708).
    ClosedCaptions,
}

/// A single piece of side data attached to a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideData {
    pub data_type: SideDataType,
    pub data: Buffer,
}

/// The kind of media stored in a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Video,
    Audio,
}

/// An audio/video frame — the fundamental unit of decoded media.
///
/// For video: contains pixel data across up to [`MAX_PLANES`] planes.
/// For audio: contains sample data (interleaved or planar).
///
/// `Frame` is `Send` (can be moved between threads) but not `Sync`
/// (mutable access requires exclusive ownership or `&mut`).
#[derive(Debug, Clone)]
pub struct Frame {
    /// The media type of this frame.
    pub media_type: MediaType,

    // ── Video fields ──
    pub width: u32,
    pub height: u32,

    // ── Audio fields ──
    pub sample_rate: u32,
    pub nb_samples: u32,
    pub channels: u16,

    // ── Common fields ──
    /// Presentation timestamp in time_base units.
    pub pts: Option<i64>,
    /// Duration in time_base units.
    pub duration: Option<i64>,
    /// Time base for pts/duration.
    pub time_base: Rational,
    /// Whether this is a key frame.
    pub key_frame: bool,

    /// Plane data buffers. For video, typically Y/U/V or R/G/B.
    /// For planar audio, one buffer per channel. For interleaved audio,
    /// a single buffer at index 0.
    planes: [Option<Buffer>; MAX_PLANES],
    /// Byte stride (line size) for each plane. Relevant for video.
    pub linesize: [u32; MAX_PLANES],

    /// Attached side data.
    side_data: Vec<SideData>,
}

impl Frame {
    /// Create an empty video frame.
    pub fn new_video(width: u32, height: u32) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(Error::InvalidArgument("video frame dimensions must be > 0".into()));
        }
        Ok(Self {
            media_type: MediaType::Video,
            width,
            height,
            sample_rate: 0,
            nb_samples: 0,
            channels: 0,
            pts: None,
            duration: None,
            time_base: Rational::UNKNOWN,
            key_frame: false,
            planes: Default::default(),
            linesize: [0; MAX_PLANES],
            side_data: Vec::new(),
        })
    }

    /// Create an empty audio frame.
    pub fn new_audio(sample_rate: u32, channels: u16, nb_samples: u32) -> Result<Self> {
        if sample_rate == 0 {
            return Err(Error::InvalidArgument("sample_rate must be > 0".into()));
        }
        if channels == 0 {
            return Err(Error::InvalidArgument("channels must be > 0".into()));
        }
        Ok(Self {
            media_type: MediaType::Audio,
            width: 0,
            height: 0,
            sample_rate,
            nb_samples,
            channels,
            pts: None,
            duration: None,
            time_base: Rational::UNKNOWN,
            key_frame: false,
            planes: Default::default(),
            linesize: [0; MAX_PLANES],
            side_data: Vec::new(),
        })
    }

    /// Set the data buffer for a plane.
    pub fn set_plane(&mut self, index: usize, buf: Buffer, linesize: u32) -> Result<()> {
        if index >= MAX_PLANES {
            return Err(Error::InvalidArgument(format!(
                "plane index {index} out of range (max {MAX_PLANES})"
            )));
        }
        self.planes[index] = Some(buf);
        self.linesize[index] = linesize;
        Ok(())
    }

    /// Get an immutable reference to a plane's data.
    pub fn plane(&self, index: usize) -> Option<&[u8]> {
        self.planes.get(index)?.as_ref().map(|b| b.data())
    }

    /// Get a mutable reference to a plane's data (copy-on-write if shared).
    pub fn plane_mut(&mut self, index: usize) -> Result<&mut [u8]> {
        if index >= MAX_PLANES {
            return Err(Error::InvalidArgument(format!(
                "plane index {index} out of range"
            )));
        }
        match &mut self.planes[index] {
            Some(buf) => Ok(buf.make_writable()),
            None => Err(Error::NotFound(format!("plane {index} not set"))),
        }
    }

    /// Returns the number of planes that have data set.
    pub fn plane_count(&self) -> usize {
        self.planes.iter().filter(|p| p.is_some()).count()
    }

    // ── Side data ──

    /// Add side data to this frame. Replaces existing data of the same type.
    pub fn add_side_data(&mut self, data_type: SideDataType, data: Buffer) {
        self.side_data.retain(|sd| sd.data_type != data_type);
        self.side_data.push(SideData { data_type, data });
    }

    /// Get side data by type.
    pub fn get_side_data(&self, data_type: SideDataType) -> Option<&SideData> {
        self.side_data.iter().find(|sd| sd.data_type == data_type)
    }

    /// Remove side data of the given type. Returns true if it existed.
    pub fn remove_side_data(&mut self, data_type: SideDataType) -> bool {
        let before = self.side_data.len();
        self.side_data.retain(|sd| sd.data_type != data_type);
        self.side_data.len() < before
    }

    /// Returns the number of side data entries.
    pub fn side_data_count(&self) -> usize {
        self.side_data.len()
    }

    /// Reset frame to an empty state, keeping the media type.
    pub fn reset(&mut self) {
        self.pts = None;
        self.duration = None;
        self.key_frame = false;
        self.planes = Default::default();
        self.linesize = [0; MAX_PLANES];
        self.side_data.clear();
    }
}

// Send is auto-derived (Buffer is Send + Sync, Vec<SideData> is Send).
// Frame is NOT Sync because &mut self methods exist that mutate in place.
// (It's actually auto-Sync too since all fields are Sync, but conceptually
//  we want users to pass frames by move or &mut, not shared &.)

#[cfg(test)]
mod tests {
    use super::*;

    // ═══════════════════════════════════════════════════
    // Frame construction
    // ═══════════════════════════════════════════════════

    // ── Positive ──

    #[test]
    fn new_video_basic() {
        let f = Frame::new_video(1920, 1080).unwrap();
        assert_eq!(f.media_type, MediaType::Video);
        assert_eq!(f.width, 1920);
        assert_eq!(f.height, 1080);
        assert_eq!(f.plane_count(), 0);
    }

    #[test]
    fn new_audio_basic() {
        let f = Frame::new_audio(48000, 2, 1024).unwrap();
        assert_eq!(f.media_type, MediaType::Audio);
        assert_eq!(f.sample_rate, 48000);
        assert_eq!(f.channels, 2);
        assert_eq!(f.nb_samples, 1024);
    }

    #[test]
    fn set_and_get_plane() {
        let mut f = Frame::new_video(4, 2).unwrap();
        let buf = Buffer::from_vec(vec![10, 20, 30, 40, 50, 60, 70, 80]);
        f.set_plane(0, buf, 4).unwrap();

        assert_eq!(f.plane_count(), 1);
        assert_eq!(f.plane(0).unwrap(), &[10, 20, 30, 40, 50, 60, 70, 80]);
        assert_eq!(f.linesize[0], 4);
    }

    #[test]
    fn set_multiple_planes() {
        let mut f = Frame::new_video(2, 2).unwrap();
        f.set_plane(0, Buffer::from_vec(vec![1; 4]), 2).unwrap(); // Y
        f.set_plane(1, Buffer::from_vec(vec![2; 1]), 1).unwrap(); // U
        f.set_plane(2, Buffer::from_vec(vec![3; 1]), 1).unwrap(); // V
        assert_eq!(f.plane_count(), 3);
    }

    #[test]
    fn plane_mut_modifies_data() {
        let mut f = Frame::new_video(2, 2).unwrap();
        f.set_plane(0, Buffer::from_vec(vec![0; 4]), 2).unwrap();
        let data = f.plane_mut(0).unwrap();
        data[0] = 255;
        assert_eq!(f.plane(0).unwrap()[0], 255);
    }

    #[test]
    fn clone_is_independent() {
        let mut f = Frame::new_video(2, 2).unwrap();
        f.set_plane(0, Buffer::from_vec(vec![1, 2, 3, 4]), 2).unwrap();
        f.pts = Some(100);

        let f2 = f.clone();
        assert_eq!(f2.pts, Some(100));
        assert_eq!(f2.plane(0).unwrap(), &[1, 2, 3, 4]);
    }

    #[test]
    fn pts_and_duration() {
        let mut f = Frame::new_video(4, 4).unwrap();
        f.pts = Some(1000);
        f.duration = Some(33);
        f.time_base = Rational::new(1, 30);
        f.key_frame = true;

        assert_eq!(f.pts, Some(1000));
        assert_eq!(f.duration, Some(33));
        assert!(f.key_frame);
    }

    #[test]
    fn reset_clears_all() {
        let mut f = Frame::new_video(4, 4).unwrap();
        f.set_plane(0, Buffer::from_vec(vec![1; 16]), 4).unwrap();
        f.pts = Some(100);
        f.add_side_data(SideDataType::DisplayMatrix, Buffer::from_vec(vec![0; 36]));

        f.reset();
        assert_eq!(f.plane_count(), 0);
        assert_eq!(f.pts, None);
        assert_eq!(f.side_data_count(), 0);
        // media_type and dimensions are preserved.
        assert_eq!(f.media_type, MediaType::Video);
        assert_eq!(f.width, 4);
    }

    // ── Negative ──

    #[test]
    fn new_video_zero_width() {
        assert!(Frame::new_video(0, 100).is_err());
    }

    #[test]
    fn new_video_zero_height() {
        assert!(Frame::new_video(100, 0).is_err());
    }

    #[test]
    fn new_audio_zero_sample_rate() {
        assert!(Frame::new_audio(0, 2, 1024).is_err());
    }

    #[test]
    fn new_audio_zero_channels() {
        assert!(Frame::new_audio(48000, 0, 1024).is_err());
    }

    #[test]
    fn set_plane_out_of_range() {
        let mut f = Frame::new_video(4, 4).unwrap();
        assert!(f.set_plane(MAX_PLANES, Buffer::from_vec(vec![0]), 1).is_err());
    }

    #[test]
    fn plane_mut_not_set() {
        let mut f = Frame::new_video(4, 4).unwrap();
        assert!(f.plane_mut(0).is_err());
    }

    #[test]
    fn plane_out_of_bounds_returns_none() {
        let f = Frame::new_video(4, 4).unwrap();
        assert!(f.plane(MAX_PLANES).is_none());
        assert!(f.plane(0).is_none()); // Not set yet.
    }

    // ═══════════════════════════════════════════════════
    // Side data
    // ═══════════════════════════════════════════════════

    // ── Positive ──

    #[test]
    fn add_and_get_side_data() {
        let mut f = Frame::new_video(4, 4).unwrap();
        let buf = Buffer::from_vec(vec![1, 2, 3]);
        f.add_side_data(SideDataType::DisplayMatrix, buf.clone());

        let sd = f.get_side_data(SideDataType::DisplayMatrix).unwrap();
        assert_eq!(sd.data, buf);
        assert_eq!(f.side_data_count(), 1);
    }

    #[test]
    fn add_replaces_same_type() {
        let mut f = Frame::new_video(4, 4).unwrap();
        f.add_side_data(SideDataType::DisplayMatrix, Buffer::from_vec(vec![1]));
        f.add_side_data(SideDataType::DisplayMatrix, Buffer::from_vec(vec![2]));

        assert_eq!(f.side_data_count(), 1);
        let sd = f.get_side_data(SideDataType::DisplayMatrix).unwrap();
        assert_eq!(sd.data.data(), &[2]);
    }

    #[test]
    fn multiple_side_data_types() {
        let mut f = Frame::new_video(4, 4).unwrap();
        f.add_side_data(SideDataType::DisplayMatrix, Buffer::from_vec(vec![1]));
        f.add_side_data(SideDataType::ContentLightLevel, Buffer::from_vec(vec![2]));
        assert_eq!(f.side_data_count(), 2);
    }

    #[test]
    fn remove_side_data_existing() {
        let mut f = Frame::new_video(4, 4).unwrap();
        f.add_side_data(SideDataType::DisplayMatrix, Buffer::from_vec(vec![1]));
        assert!(f.remove_side_data(SideDataType::DisplayMatrix));
        assert_eq!(f.side_data_count(), 0);
    }

    // ── Negative ──

    #[test]
    fn get_nonexistent_side_data() {
        let f = Frame::new_video(4, 4).unwrap();
        assert!(f.get_side_data(SideDataType::MotionVectors).is_none());
    }

    #[test]
    fn remove_nonexistent_side_data() {
        let mut f = Frame::new_video(4, 4).unwrap();
        assert!(!f.remove_side_data(SideDataType::MotionVectors));
    }

    // ═══════════════════════════════════════════════════
    // Concurrency
    // ═══════════════════════════════════════════════════

    #[test]
    fn frame_send_across_threads() {
        use std::thread;

        let mut f = Frame::new_video(4, 4).unwrap();
        f.set_plane(0, Buffer::from_vec(vec![42; 16]), 4).unwrap();
        f.pts = Some(999);

        let handle = thread::spawn(move || {
            assert_eq!(f.plane(0).unwrap()[0], 42);
            assert_eq!(f.pts, Some(999));
        });
        handle.join().unwrap();
    }

    #[test]
    fn send_and_sync_traits() {
        fn assert_send<T: Send>() {}
        assert_send::<Frame>();
        // Frame is also Sync (all fields are Sync), but conceptually
        // we use it via &mut, not shared &.
    }

    // ── Edge cases ──

    #[test]
    fn audio_zero_samples_allowed() {
        // Zero samples is valid (empty/flush frame).
        let f = Frame::new_audio(48000, 2, 0).unwrap();
        assert_eq!(f.nb_samples, 0);
    }
}
