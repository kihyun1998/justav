use av_util::error::{Error, Result};

use crate::codec_par::CodecParameters;
use crate::packet::Packet;

/// Trait for a bitstream filter.
///
/// Bitstream filters transform encoded packets without full decoding
/// (e.g. converting H.264 from Annex B to AVCC format).
pub trait BitstreamFilter: Send {
    /// Name of this filter.
    fn name(&self) -> &'static str;

    /// Initialize with codec parameters.
    fn init(&mut self, params: &CodecParameters) -> Result<()>;

    /// Send a packet for filtering.
    fn send_packet(&mut self, packet: &Packet) -> Result<()>;

    /// Receive a filtered packet.
    /// Returns `Error::Again` if more input is needed.
    fn receive_packet(&mut self, packet: &mut Packet) -> Result<()>;

    /// Flush remaining packets.
    fn flush(&mut self);
}

/// A chain of bitstream filters applied in sequence.
pub struct BsfChain {
    filters: Vec<Box<dyn BitstreamFilter>>,
}

impl BsfChain {
    /// Create an empty chain (passthrough).
    pub fn new() -> Self {
        Self { filters: Vec::new() }
    }

    /// Append a filter to the chain.
    pub fn push(&mut self, filter: Box<dyn BitstreamFilter>) {
        self.filters.push(filter);
    }

    /// Number of filters in the chain.
    pub fn len(&self) -> usize {
        self.filters.len()
    }

    /// Returns true if no filters are registered.
    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }

    /// Send a packet through the entire chain.
    /// For an empty chain, this is a no-op (passthrough).
    pub fn send_packet(&mut self, packet: &Packet) -> Result<()> {
        for filter in &mut self.filters {
            filter.send_packet(packet)?;
        }
        Ok(())
    }
}

impl Default for BsfChain {
    fn default() -> Self {
        Self::new()
    }
}

/// A null bitstream filter (passthrough).
pub struct NullBsf {
    pending: Option<Packet>,
}

impl NullBsf {
    pub fn new() -> Self {
        Self { pending: None }
    }
}

impl Default for NullBsf {
    fn default() -> Self {
        Self::new()
    }
}

impl BitstreamFilter for NullBsf {
    fn name(&self) -> &'static str { "null" }

    fn init(&mut self, _params: &CodecParameters) -> Result<()> { Ok(()) }

    fn send_packet(&mut self, packet: &Packet) -> Result<()> {
        if self.pending.is_some() {
            return Err(Error::Again);
        }
        self.pending = Some(packet.clone());
        Ok(())
    }

    fn receive_packet(&mut self, packet: &mut Packet) -> Result<()> {
        match self.pending.take() {
            Some(p) => {
                *packet = p;
                Ok(())
            }
            None => Err(Error::Again),
        }
    }

    fn flush(&mut self) {
        self.pending = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use av_util::buffer::Buffer;

    // ── Positive ──

    #[test]
    fn null_bsf_passthrough() {
        let mut bsf = NullBsf::new();
        let params = CodecParameters::new();
        bsf.init(&params).unwrap();

        let pkt = Packet::new(Buffer::from_vec(vec![1, 2, 3]));
        bsf.send_packet(&pkt).unwrap();

        let mut out = Packet::empty();
        bsf.receive_packet(&mut out).unwrap();
        assert_eq!(out.data.data(), &[1, 2, 3]);
    }

    #[test]
    fn chain_empty_is_passthrough() {
        let mut chain = BsfChain::new();
        assert!(chain.is_empty());
        let pkt = Packet::new(Buffer::from_vec(vec![1]));
        chain.send_packet(&pkt).unwrap(); // no-op
    }

    #[test]
    fn chain_with_null_bsf() {
        let mut chain = BsfChain::new();
        chain.push(Box::new(NullBsf::new()));
        assert_eq!(chain.len(), 1);
    }

    // ── Negative ──

    #[test]
    fn null_bsf_send_twice_without_receive() {
        let mut bsf = NullBsf::new();
        let pkt = Packet::new(Buffer::from_vec(vec![1]));
        bsf.send_packet(&pkt).unwrap();
        assert!(bsf.send_packet(&pkt).is_err()); // pending not drained
    }

    #[test]
    fn null_bsf_receive_without_send() {
        let mut bsf = NullBsf::new();
        let mut out = Packet::empty();
        assert!(bsf.receive_packet(&mut out).is_err());
    }
}
