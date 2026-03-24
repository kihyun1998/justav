use av_codec::packet::Packet;
use av_util::rational::Rational;
use av_util::mathematics::rescale_q;

/// Interleaving mode for muxing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InterleaveMode {
    /// Sort packets by DTS across all streams.
    #[default]
    Dts,
    /// Write packets in the order they arrive (no reordering).
    Passthrough,
}

/// Compare two packets for DTS-based interleaving.
/// Returns `Ordering::Less` if `a` should be written before `b`.
pub fn compare_dts(a: &Packet, b: &Packet) -> std::cmp::Ordering {
    let a_dts = a.dts.or(a.pts).unwrap_or(i64::MAX);
    let b_dts = b.dts.or(b.pts).unwrap_or(i64::MAX);

    // If same timebase, compare directly.
    if a.time_base == b.time_base {
        return a_dts.cmp(&b_dts);
    }

    // Different timebases: rescale to a common base.
    let common_tb = Rational::new(1, 1_000_000); // microseconds
    let a_us = rescale_q(a_dts, a.time_base, common_tb).unwrap_or(i64::MAX);
    let b_us = rescale_q(b_dts, b.time_base, common_tb).unwrap_or(i64::MAX);
    a_us.cmp(&b_us)
}

/// A simple interleaving buffer.
///
/// Collects packets from multiple streams and yields them in DTS order.
pub struct InterleaveBuffer {
    packets: Vec<Packet>,
    mode: InterleaveMode,
}

impl InterleaveBuffer {
    pub fn new(mode: InterleaveMode) -> Self {
        Self {
            packets: Vec::new(),
            mode,
        }
    }

    /// Add a packet to the buffer.
    pub fn push(&mut self, packet: Packet) {
        self.packets.push(packet);
    }

    /// Take the next packet that should be written, based on interleave mode.
    /// Returns `None` if the buffer is empty.
    pub fn pop(&mut self) -> Option<Packet> {
        if self.packets.is_empty() {
            return None;
        }
        match self.mode {
            InterleaveMode::Passthrough => Some(self.packets.remove(0)),
            InterleaveMode::Dts => {
                let idx = self.packets
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| compare_dts(a, b))
                    .map(|(i, _)| i)
                    .unwrap();
                Some(self.packets.remove(idx))
            }
        }
    }

    /// Number of buffered packets.
    pub fn len(&self) -> usize {
        self.packets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    /// Drain all remaining packets in order.
    pub fn drain(&mut self) -> Vec<Packet> {
        match self.mode {
            InterleaveMode::Passthrough => std::mem::take(&mut self.packets),
            InterleaveMode::Dts => {
                let mut result = Vec::with_capacity(self.packets.len());
                while let Some(pkt) = self.pop() {
                    result.push(pkt);
                }
                result
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use av_util::buffer::Buffer;

    fn make_packet(dts: i64, stream: u32) -> Packet {
        let mut pkt = Packet::new(Buffer::from_vec(vec![0]));
        pkt.dts = Some(dts);
        pkt.stream_index = stream;
        pkt.time_base = Rational::new(1, 1000);
        pkt
    }

    // ── Positive ──

    #[test]
    fn dts_interleave_orders_correctly() {
        let mut buf = InterleaveBuffer::new(InterleaveMode::Dts);
        buf.push(make_packet(300, 0)); // video
        buf.push(make_packet(100, 1)); // audio
        buf.push(make_packet(200, 0)); // video

        assert_eq!(buf.pop().unwrap().dts, Some(100));
        assert_eq!(buf.pop().unwrap().dts, Some(200));
        assert_eq!(buf.pop().unwrap().dts, Some(300));
    }

    #[test]
    fn passthrough_preserves_order() {
        let mut buf = InterleaveBuffer::new(InterleaveMode::Passthrough);
        buf.push(make_packet(300, 0));
        buf.push(make_packet(100, 1));
        buf.push(make_packet(200, 0));

        assert_eq!(buf.pop().unwrap().dts, Some(300));
        assert_eq!(buf.pop().unwrap().dts, Some(100));
        assert_eq!(buf.pop().unwrap().dts, Some(200));
    }

    #[test]
    fn drain_dts() {
        let mut buf = InterleaveBuffer::new(InterleaveMode::Dts);
        buf.push(make_packet(30, 0));
        buf.push(make_packet(10, 1));
        buf.push(make_packet(20, 0));

        let pkts = buf.drain();
        let dts_values: Vec<i64> = pkts.iter().map(|p| p.dts.unwrap()).collect();
        assert_eq!(dts_values, vec![10, 20, 30]);
    }

    // ── Negative / Edge ──

    #[test]
    fn pop_empty() {
        let mut buf = InterleaveBuffer::new(InterleaveMode::Dts);
        assert!(buf.pop().is_none());
    }

    #[test]
    fn len_tracking() {
        let mut buf = InterleaveBuffer::new(InterleaveMode::Dts);
        assert!(buf.is_empty());
        buf.push(make_packet(10, 0));
        assert_eq!(buf.len(), 1);
        buf.pop();
        assert!(buf.is_empty());
    }

    #[test]
    fn dts_interleave_different_timebases() {
        let mut buf = InterleaveBuffer::new(InterleaveMode::Dts);

        // Packet A: DTS=2000 at 1/1000 = 2.0 seconds.
        let mut pkt_a = Packet::new(Buffer::from_vec(vec![0]));
        pkt_a.dts = Some(2000);
        pkt_a.time_base = Rational::new(1, 1000);
        pkt_a.stream_index = 0;

        // Packet B: DTS=48000 at 1/48000 = 1.0 second.
        let mut pkt_b = Packet::new(Buffer::from_vec(vec![0]));
        pkt_b.dts = Some(48000);
        pkt_b.time_base = Rational::new(1, 48000);
        pkt_b.stream_index = 1;

        buf.push(pkt_a);
        buf.push(pkt_b);

        // B (1.0s) should come before A (2.0s).
        let first = buf.pop().unwrap();
        assert_eq!(first.stream_index, 1);
        let second = buf.pop().unwrap();
        assert_eq!(second.stream_index, 0);
    }

    #[test]
    fn dts_none_treated_as_max() {
        let mut buf = InterleaveBuffer::new(InterleaveMode::Dts);

        let mut pkt_a = Packet::new(Buffer::from_vec(vec![0]));
        pkt_a.dts = None; // unknown DTS → goes last
        pkt_a.time_base = Rational::new(1, 1000);

        buf.push(pkt_a);
        buf.push(make_packet(100, 0));

        // Packet with DTS=100 should come first; None → last.
        let first = buf.pop().unwrap();
        assert_eq!(first.dts, Some(100));
        let second = buf.pop().unwrap();
        assert_eq!(second.dts, None);
    }

    #[test]
    fn dts_reverse_order_corrected() {
        let mut buf = InterleaveBuffer::new(InterleaveMode::Dts);
        // Push packets in reverse DTS order.
        buf.push(make_packet(300, 0));
        buf.push(make_packet(200, 0));
        buf.push(make_packet(100, 0));

        // DTS mode should still yield them in ascending order.
        assert_eq!(buf.pop().unwrap().dts, Some(100));
        assert_eq!(buf.pop().unwrap().dts, Some(200));
        assert_eq!(buf.pop().unwrap().dts, Some(300));
    }

    #[test]
    fn dts_large_gap() {
        let mut buf = InterleaveBuffer::new(InterleaveMode::Dts);
        buf.push(make_packet(0, 0));
        buf.push(make_packet(1_000_000_000, 0)); // 1M seconds gap

        assert_eq!(buf.pop().unwrap().dts, Some(0));
        assert_eq!(buf.pop().unwrap().dts, Some(1_000_000_000));
    }
}
