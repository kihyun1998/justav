use av_util::error::Result;

use crate::codec::CodecId;

/// Trait for a stream parser.
///
/// Parsers split a raw byte stream into individual codec frames/packets.
/// They are used when reading raw streams (e.g. Annex B H.264) that don't
/// have clear packet boundaries from the container.
pub trait Parser: Send {
    /// The codec this parser handles.
    fn codec_id(&self) -> CodecId;

    /// Feed data into the parser. Returns how many bytes were consumed.
    ///
    /// After calling `parse`, check `pull_packet` for complete packets.
    fn parse(&mut self, data: &[u8]) -> Result<usize>;

    /// Pull a complete packet out of the parser, if available.
    ///
    /// Returns `None` if no complete packet is ready yet.
    fn pull_packet(&mut self) -> Option<Vec<u8>>;

    /// Signal end-of-stream. After this, `pull_packet` may return
    /// remaining buffered data.
    fn flush(&mut self);

    /// Reset the parser state.
    fn reset(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Parser tests will be added when concrete implementations
    // (e.g. H.264 NAL parser) are built in Phase 3.

    #[test]
    fn parser_trait_is_object_safe() {
        // Verify Parser can be used as trait object.
        fn accepts_parser(_p: &dyn Parser) {}
        // Compiles = object safe.
    }
}
