use av_codec::packet::Packet;
use av_util::error::Result;

use crate::io::IOContext;
use crate::stream::Stream;
use crate::metadata::Metadata;

/// Trait for a demuxer (container reader) implementation.
pub trait Demuxer: Send {
    /// Format name (e.g. "wav", "matroska", "mp4").
    fn name(&self) -> &'static str;

    /// Read the container header and populate streams.
    fn read_header(&mut self, io: &mut IOContext) -> Result<DemuxHeader>;

    /// Read the next packet from the container.
    /// Returns `Error::Eof` when no more packets are available.
    fn read_packet(&mut self, io: &mut IOContext) -> Result<Packet>;
}

/// Information extracted from the container header.
#[derive(Debug, Clone)]
pub struct DemuxHeader {
    /// Streams found in the container.
    pub streams: Vec<Stream>,
    /// Container-level metadata.
    pub metadata: Metadata,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demux_trait_is_object_safe() {
        fn _accepts(_d: &dyn Demuxer) {}
    }
}
