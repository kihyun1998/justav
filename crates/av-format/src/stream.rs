use av_codec::codec::CodecId;
use av_codec::codec_par::CodecParameters;
use av_util::dict::Dict;
use av_util::rational::Rational;

/// A single media stream within a container.
#[derive(Debug, Clone)]
pub struct Stream {
    /// Stream index (0-based).
    pub index: u32,
    /// Codec parameters for this stream.
    pub params: CodecParameters,
    /// Stream time base.
    pub time_base: Rational,
    /// Duration in time_base units (None if unknown).
    pub duration: Option<i64>,
    /// Number of frames (None if unknown).
    pub nb_frames: Option<u64>,
    /// Stream-level metadata.
    pub metadata: Dict,
    /// Stream disposition flags.
    pub disposition: StreamDisposition,
}

/// Stream disposition flags.
#[derive(Debug, Clone, Default)]
pub struct StreamDisposition {
    /// This is the default stream for its type.
    pub default: bool,
    /// Contains attached pictures (album art).
    pub attached_pic: bool,
    /// Contains forced subtitles.
    pub forced: bool,
    /// Stream is for hearing impaired.
    pub hearing_impaired: bool,
    /// Stream is for visually impaired.
    pub visual_impaired: bool,
}

impl Stream {
    /// Create a new stream with default values.
    pub fn new(index: u32, params: CodecParameters) -> Self {
        Self {
            index,
            params,
            time_base: Rational::UNKNOWN,
            duration: None,
            nb_frames: None,
            metadata: Dict::new(),
            disposition: StreamDisposition::default(),
        }
    }

    /// Codec ID of this stream.
    pub fn codec_id(&self) -> CodecId {
        self.params.codec_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use av_util::frame::MediaType;

    #[test]
    fn new_stream() {
        let params = CodecParameters::new_video(CodecId::H264, 1920, 1080).unwrap();
        let s = Stream::new(0, params);
        assert_eq!(s.index, 0);
        assert_eq!(s.codec_id(), CodecId::H264);
        assert_eq!(s.params.media_type, MediaType::Video);
    }

    #[test]
    fn stream_metadata() {
        let params = CodecParameters::new_audio(CodecId::Aac, 48000, 2).unwrap();
        let mut s = Stream::new(1, params);
        s.metadata.set("language", "eng");
        assert_eq!(s.metadata.get("language"), Some("eng"));
    }

    #[test]
    fn disposition_defaults() {
        let d = StreamDisposition::default();
        assert!(!d.default);
        assert!(!d.attached_pic);
    }
}
