use crate::error::{Error, Result};

/// A generic ring buffer (FIFO queue) for bytes.
///
/// Fixed capacity, supports read/write/peek/drain without allocation
/// after construction.
pub struct Fifo {
    buf: Vec<u8>,
    read_pos: usize,
    write_pos: usize,
    count: usize,
}

impl Fifo {
    /// Create a FIFO with the given capacity in bytes.
    pub fn new(capacity: usize) -> Result<Self> {
        if capacity == 0 {
            return Err(Error::InvalidArgument("fifo capacity must be > 0".into()));
        }
        Ok(Self {
            buf: vec![0u8; capacity],
            read_pos: 0,
            write_pos: 0,
            count: 0,
        })
    }

    /// Total capacity.
    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    /// Number of bytes available to read.
    pub fn readable(&self) -> usize {
        self.count
    }

    /// Number of bytes of free space for writing.
    pub fn writable(&self) -> usize {
        self.buf.len() - self.count
    }

    /// Returns true if the FIFO is empty.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Returns true if the FIFO is full.
    pub fn is_full(&self) -> bool {
        self.count == self.buf.len()
    }

    /// Write data into the FIFO. Returns error if not enough space.
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        if data.len() > self.writable() {
            return Err(Error::NoMemory);
        }
        for &byte in data {
            self.buf[self.write_pos] = byte;
            self.write_pos = (self.write_pos + 1) % self.buf.len();
        }
        self.count += data.len();
        Ok(())
    }

    /// Read data from the FIFO into `dst`. Returns error if not enough data.
    pub fn read(&mut self, dst: &mut [u8]) -> Result<()> {
        if dst.len() > self.count {
            return Err(Error::Eof);
        }
        for byte in dst.iter_mut() {
            *byte = self.buf[self.read_pos];
            self.read_pos = (self.read_pos + 1) % self.buf.len();
        }
        self.count -= dst.len();
        Ok(())
    }

    /// Peek at `len` bytes without consuming them.
    pub fn peek(&self, len: usize) -> Result<Vec<u8>> {
        if len > self.count {
            return Err(Error::Eof);
        }
        let mut out = Vec::with_capacity(len);
        let mut pos = self.read_pos;
        for _ in 0..len {
            out.push(self.buf[pos]);
            pos = (pos + 1) % self.buf.len();
        }
        Ok(out)
    }

    /// Discard `len` bytes from the read side.
    pub fn drain(&mut self, len: usize) -> Result<()> {
        if len > self.count {
            return Err(Error::Eof);
        }
        self.read_pos = (self.read_pos + len) % self.buf.len();
        self.count -= len;
        Ok(())
    }

    /// Reset to empty state.
    pub fn reset(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
        self.count = 0;
    }
}

impl std::fmt::Debug for Fifo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fifo")
            .field("capacity", &self.capacity())
            .field("readable", &self.readable())
            .field("writable", &self.writable())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn write_and_read() {
        let mut f = Fifo::new(16).unwrap();
        f.write(&[1, 2, 3]).unwrap();
        let mut buf = [0u8; 3];
        f.read(&mut buf).unwrap();
        assert_eq!(buf, [1, 2, 3]);
        assert!(f.is_empty());
    }

    #[test]
    fn wrap_around() {
        let mut f = Fifo::new(4).unwrap();
        f.write(&[1, 2, 3]).unwrap();
        let mut buf = [0u8; 2];
        f.read(&mut buf).unwrap(); // read [1,2], read_pos=2
        f.write(&[4, 5]).unwrap(); // wraps around
        let mut buf2 = [0u8; 3];
        f.read(&mut buf2).unwrap();
        assert_eq!(buf2, [3, 4, 5]);
    }

    #[test]
    fn peek_does_not_consume() {
        let mut f = Fifo::new(8).unwrap();
        f.write(&[10, 20, 30]).unwrap();
        let peeked = f.peek(2).unwrap();
        assert_eq!(peeked, vec![10, 20]);
        assert_eq!(f.readable(), 3); // unchanged
    }

    #[test]
    fn drain() {
        let mut f = Fifo::new(8).unwrap();
        f.write(&[1, 2, 3, 4]).unwrap();
        f.drain(2).unwrap();
        assert_eq!(f.readable(), 2);
        let mut buf = [0u8; 2];
        f.read(&mut buf).unwrap();
        assert_eq!(buf, [3, 4]);
    }

    #[test]
    fn reset() {
        let mut f = Fifo::new(8).unwrap();
        f.write(&[1, 2]).unwrap();
        f.reset();
        assert!(f.is_empty());
        assert_eq!(f.writable(), 8);
    }

    #[test]
    fn fill_to_capacity() {
        let mut f = Fifo::new(4).unwrap();
        f.write(&[1, 2, 3, 4]).unwrap();
        assert!(f.is_full());
        assert_eq!(f.writable(), 0);
    }

    // ── Negative ──

    #[test]
    fn write_overflow() {
        let mut f = Fifo::new(4).unwrap();
        f.write(&[1, 2, 3]).unwrap();
        assert!(f.write(&[4, 5]).is_err()); // only 1 byte free
    }

    #[test]
    fn read_underflow() {
        let mut f = Fifo::new(4).unwrap();
        let mut buf = [0u8; 1];
        assert!(f.read(&mut buf).is_err());
    }

    #[test]
    fn peek_underflow() {
        let f = Fifo::new(4).unwrap();
        assert!(f.peek(1).is_err());
    }

    #[test]
    fn drain_underflow() {
        let mut f = Fifo::new(4).unwrap();
        assert!(f.drain(1).is_err());
    }

    #[test]
    fn zero_capacity() {
        assert!(Fifo::new(0).is_err());
    }

    // ── Edge ──

    #[test]
    fn single_byte_fifo() {
        let mut f = Fifo::new(1).unwrap();
        f.write(&[42]).unwrap();
        assert!(f.is_full());
        let mut buf = [0u8; 1];
        f.read(&mut buf).unwrap();
        assert_eq!(buf, [42]);
        assert!(f.is_empty());
    }
}
