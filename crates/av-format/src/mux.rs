use av_codec::packet::Packet;
use av_util::error::Result;

use crate::io::IOContext;
use crate::stream::Stream;

/// Trait for a muxer (container writer) implementation.
pub trait Muxer: Send {
    /// Format name.
    fn name(&self) -> &'static str;

    /// Write the container header.
    fn write_header(&mut self, io: &mut IOContext, streams: &[Stream]) -> Result<()>;

    /// Write a packet to the container.
    fn write_packet(&mut self, io: &mut IOContext, packet: &Packet) -> Result<()>;

    /// Write the container trailer (finalize).
    fn write_trailer(&mut self, io: &mut IOContext) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn muxer_trait_is_object_safe() {
        fn _accepts(_m: &dyn Muxer) {}
    }
}
