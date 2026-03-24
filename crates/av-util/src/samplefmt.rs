/// Audio sample formats.
///
/// Describes the data type and layout (interleaved vs planar) of audio samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SampleFormat {
    /// Unsigned 8-bit, interleaved.
    U8,
    /// Signed 16-bit, interleaved.
    S16,
    /// Signed 32-bit, interleaved.
    S32,
    /// 32-bit float, interleaved.
    F32,
    /// 64-bit float, interleaved.
    F64,

    /// Unsigned 8-bit, planar (one buffer per channel).
    U8p,
    /// Signed 16-bit, planar.
    S16p,
    /// Signed 32-bit, planar.
    S32p,
    /// 32-bit float, planar.
    F32p,
    /// 64-bit float, planar.
    F64p,
}

impl SampleFormat {
    /// Bytes per sample for a single channel.
    pub const fn bytes_per_sample(&self) -> usize {
        match self {
            Self::U8 | Self::U8p => 1,
            Self::S16 | Self::S16p => 2,
            Self::S32 | Self::S32p | Self::F32 | Self::F32p => 4,
            Self::F64 | Self::F64p => 8,
        }
    }

    /// True if samples are stored in separate per-channel buffers.
    pub const fn is_planar(&self) -> bool {
        matches!(self, Self::U8p | Self::S16p | Self::S32p | Self::F32p | Self::F64p)
    }

    /// True if samples are interleaved in a single buffer.
    pub const fn is_interleaved(&self) -> bool {
        !self.is_planar()
    }

    /// True if samples are floating-point.
    pub const fn is_float(&self) -> bool {
        matches!(self, Self::F32 | Self::F64 | Self::F32p | Self::F64p)
    }

    /// Get the planar counterpart (returns self if already planar).
    pub const fn to_planar(&self) -> Self {
        match self {
            Self::U8 => Self::U8p,
            Self::S16 => Self::S16p,
            Self::S32 => Self::S32p,
            Self::F32 => Self::F32p,
            Self::F64 => Self::F64p,
            other => *other,
        }
    }

    /// Get the interleaved counterpart (returns self if already interleaved).
    pub const fn to_interleaved(&self) -> Self {
        match self {
            Self::U8p => Self::U8,
            Self::S16p => Self::S16,
            Self::S32p => Self::S32,
            Self::F32p => Self::F32,
            Self::F64p => Self::F64,
            other => *other,
        }
    }

    /// Format name as a string.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::U8 => "u8", Self::S16 => "s16", Self::S32 => "s32",
            Self::F32 => "flt", Self::F64 => "dbl",
            Self::U8p => "u8p", Self::S16p => "s16p", Self::S32p => "s32p",
            Self::F32p => "fltp", Self::F64p => "dblp",
        }
    }

    /// Parse from name string.
    pub fn from_name(name: &str) -> Option<Self> {
        ALL_SAMPLE_FORMATS.iter().find(|f| f.name() == name).copied()
    }

    /// Compute the buffer size needed for `nb_samples` samples across
    /// `channels` channels. For interleaved, this is a single buffer size.
    /// For planar, this is the size of one per-channel buffer.
    pub const fn buffer_size(&self, nb_samples: u32, channels: u16) -> usize {
        let sample_bytes = self.bytes_per_sample();
        if self.is_planar() {
            sample_bytes * nb_samples as usize
        } else {
            sample_bytes * nb_samples as usize * channels as usize
        }
    }
}

impl core::fmt::Display for SampleFormat {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// All defined sample formats, for iteration / lookup.
pub const ALL_SAMPLE_FORMATS: &[SampleFormat] = &[
    SampleFormat::U8, SampleFormat::S16, SampleFormat::S32,
    SampleFormat::F32, SampleFormat::F64,
    SampleFormat::U8p, SampleFormat::S16p, SampleFormat::S32p,
    SampleFormat::F32p, SampleFormat::F64p,
];

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn bytes_per_sample() {
        assert_eq!(SampleFormat::U8.bytes_per_sample(), 1);
        assert_eq!(SampleFormat::S16.bytes_per_sample(), 2);
        assert_eq!(SampleFormat::S32.bytes_per_sample(), 4);
        assert_eq!(SampleFormat::F32.bytes_per_sample(), 4);
        assert_eq!(SampleFormat::F64.bytes_per_sample(), 8);
    }

    #[test]
    fn planar_variants_match() {
        assert_eq!(SampleFormat::U8p.bytes_per_sample(), 1);
        assert_eq!(SampleFormat::S16p.bytes_per_sample(), 2);
        assert_eq!(SampleFormat::F32p.bytes_per_sample(), 4);
    }

    #[test]
    fn is_planar() {
        assert!(!SampleFormat::S16.is_planar());
        assert!(SampleFormat::S16p.is_planar());
        assert!(!SampleFormat::F32.is_planar());
        assert!(SampleFormat::F32p.is_planar());
    }

    #[test]
    fn is_interleaved() {
        assert!(SampleFormat::S16.is_interleaved());
        assert!(!SampleFormat::S16p.is_interleaved());
    }

    #[test]
    fn is_float() {
        assert!(!SampleFormat::S16.is_float());
        assert!(SampleFormat::F32.is_float());
        assert!(SampleFormat::F64p.is_float());
    }

    #[test]
    fn to_planar_and_back() {
        assert_eq!(SampleFormat::S16.to_planar(), SampleFormat::S16p);
        assert_eq!(SampleFormat::S16p.to_interleaved(), SampleFormat::S16);
        // Already planar → no change.
        assert_eq!(SampleFormat::F32p.to_planar(), SampleFormat::F32p);
        // Already interleaved → no change.
        assert_eq!(SampleFormat::F32.to_interleaved(), SampleFormat::F32);
    }

    #[test]
    fn buffer_size_interleaved() {
        // 1024 samples, 2 channels, s16 (2 bytes) = 4096 bytes.
        assert_eq!(SampleFormat::S16.buffer_size(1024, 2), 4096);
    }

    #[test]
    fn buffer_size_planar() {
        // 1024 samples, planar s16 = 2048 bytes per channel.
        assert_eq!(SampleFormat::S16p.buffer_size(1024, 2), 2048);
    }

    #[test]
    fn display_format() {
        assert_eq!(format!("{}", SampleFormat::S16), "s16");
        assert_eq!(format!("{}", SampleFormat::F32p), "fltp");
    }

    #[test]
    fn from_name_valid() {
        assert_eq!(SampleFormat::from_name("s16"), Some(SampleFormat::S16));
        assert_eq!(SampleFormat::from_name("fltp"), Some(SampleFormat::F32p));
    }

    #[test]
    fn all_formats_roundtrip_name() {
        for fmt in ALL_SAMPLE_FORMATS {
            assert_eq!(SampleFormat::from_name(fmt.name()), Some(*fmt));
        }
    }

    // ── Negative ──

    #[test]
    fn from_name_invalid() {
        assert_eq!(SampleFormat::from_name("pcm16"), None);
        assert_eq!(SampleFormat::from_name(""), None);
    }

    #[test]
    fn buffer_size_zero_samples() {
        assert_eq!(SampleFormat::S16.buffer_size(0, 2), 0);
    }
}
