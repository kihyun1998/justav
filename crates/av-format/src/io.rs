use av_util::error::{Error, Result};
use std::io::{self, Read, Seek, SeekFrom, Write};

/// Buffered I/O context wrapping a reader/writer/seeker.
///
/// Provides endian-aware read/write helpers and buffered access.
/// This is the low-level I/O layer that protocols and format implementations use.
pub struct IOContext {
    inner: Box<dyn ReadWriteSeek>,
    /// True if this context supports seeking.
    pub seekable: bool,
    /// Total size in bytes (-1 if unknown, e.g. pipe/network).
    pub size: i64,
}

/// Combined trait for Read + Write + Seek.
pub trait ReadWriteSeek: Read + Write + Seek + Send {}
impl<T: Read + Write + Seek + Send> ReadWriteSeek for T {}

impl IOContext {
    /// Create from a Read+Write+Seek source (e.g. File).
    pub fn from_read_write_seek(inner: impl ReadWriteSeek + 'static, size: i64) -> Self {
        Self {
            inner: Box::new(inner),
            seekable: true,
            size,
        }
    }

    /// Create from an in-memory buffer (read+write+seek).
    pub fn from_memory(data: Vec<u8>) -> Self {
        let size = data.len() as i64;
        Self {
            inner: Box::new(io::Cursor::new(data)),
            seekable: true,
            size,
        }
    }

    /// Create an empty writable memory buffer.
    pub fn memory_writer() -> Self {
        Self {
            inner: Box::new(io::Cursor::new(Vec::new())),
            seekable: true,
            size: 0,
        }
    }

    // ── Read helpers ──

    /// Read exactly `buf.len()` bytes.
    pub fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        self.inner.read_exact(buf).map_err(Error::from)
    }

    /// Read a single byte.
    pub fn read_u8(&mut self) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    /// Read u16 little-endian.
    pub fn read_u16_le(&mut self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    /// Read u16 big-endian.
    pub fn read_u16_be(&mut self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.read_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }

    /// Read u32 little-endian.
    pub fn read_u32_le(&mut self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    /// Read u32 big-endian.
    pub fn read_u32_be(&mut self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }

    /// Read u64 little-endian.
    pub fn read_u64_le(&mut self) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.read_exact(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }

    /// Read u64 big-endian.
    pub fn read_u64_be(&mut self) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.read_exact(&mut buf)?;
        Ok(u64::from_be_bytes(buf))
    }

    /// Read `len` bytes into a new Vec.
    pub fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }

    // ── Write helpers ──

    /// Write all bytes from `buf`.
    pub fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        self.inner.write_all(buf).map_err(Error::from)
    }

    pub fn write_u8(&mut self, v: u8) -> Result<()> {
        self.write_all(&[v])
    }

    pub fn write_u16_le(&mut self, v: u16) -> Result<()> {
        self.write_all(&v.to_le_bytes())
    }

    pub fn write_u16_be(&mut self, v: u16) -> Result<()> {
        self.write_all(&v.to_be_bytes())
    }

    pub fn write_u32_le(&mut self, v: u32) -> Result<()> {
        self.write_all(&v.to_le_bytes())
    }

    pub fn write_u32_be(&mut self, v: u32) -> Result<()> {
        self.write_all(&v.to_be_bytes())
    }

    pub fn write_u64_le(&mut self, v: u64) -> Result<()> {
        self.write_all(&v.to_le_bytes())
    }

    pub fn write_u64_be(&mut self, v: u64) -> Result<()> {
        self.write_all(&v.to_be_bytes())
    }

    /// Flush the underlying writer.
    pub fn flush(&mut self) -> Result<()> {
        self.inner.flush().map_err(Error::from)
    }

    // ── Seek helpers ──

    /// Seek to an absolute byte position.
    pub fn seek(&mut self, pos: u64) -> Result<u64> {
        if !self.seekable {
            return Err(Error::Unsupported("non-seekable source".into()));
        }
        self.inner.seek(SeekFrom::Start(pos)).map_err(Error::from)
    }

    /// Get the current byte position.
    pub fn position(&mut self) -> Result<u64> {
        self.inner.stream_position().map_err(Error::from)
    }

    /// Skip `n` bytes forward.
    pub fn skip(&mut self, n: i64) -> Result<u64> {
        self.inner.seek(SeekFrom::Current(n)).map_err(Error::from)
    }

    /// Consume the context and return the underlying bytes (memory contexts only).
    ///
    /// This works by reading all data from position 0. Returns the full content.
    pub fn into_vec(mut self) -> Result<Vec<u8>> {
        self.seek(0)?;
        let mut buf = Vec::new();
        self.inner.read_to_end(&mut buf).map_err(Error::from)?;
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn memory_read_write() {
        let mut io = IOContext::memory_writer();
        io.write_u32_le(0xDEADBEEF).unwrap();
        io.seek(0).unwrap();
        assert_eq!(io.read_u32_le().unwrap(), 0xDEADBEEF);
    }

    #[test]
    fn endian_roundtrip_le() {
        let mut io = IOContext::memory_writer();
        io.write_u16_le(0x1234).unwrap();
        io.write_u32_le(0xAABBCCDD).unwrap();
        io.write_u64_le(0x0102030405060708).unwrap();
        io.seek(0).unwrap();
        assert_eq!(io.read_u16_le().unwrap(), 0x1234);
        assert_eq!(io.read_u32_le().unwrap(), 0xAABBCCDD);
        assert_eq!(io.read_u64_le().unwrap(), 0x0102030405060708);
    }

    #[test]
    fn endian_roundtrip_be() {
        let mut io = IOContext::memory_writer();
        io.write_u16_be(0x1234).unwrap();
        io.write_u32_be(0xAABBCCDD).unwrap();
        io.write_u64_be(0x0102030405060708).unwrap();
        io.seek(0).unwrap();
        assert_eq!(io.read_u16_be().unwrap(), 0x1234);
        assert_eq!(io.read_u32_be().unwrap(), 0xAABBCCDD);
        assert_eq!(io.read_u64_be().unwrap(), 0x0102030405060708);
    }

    #[test]
    fn read_u8_and_bytes() {
        let data = vec![0x01, 0x02, 0x03, 0x04, 0x05];
        let mut io = IOContext::from_memory(data);
        assert_eq!(io.read_u8().unwrap(), 0x01);
        let rest = io.read_bytes(4).unwrap();
        assert_eq!(rest, vec![0x02, 0x03, 0x04, 0x05]);
    }

    #[test]
    fn seek_and_position() {
        let data = vec![0u8; 100];
        let mut io = IOContext::from_memory(data);
        io.seek(50).unwrap();
        assert_eq!(io.position().unwrap(), 50);
        io.skip(10).unwrap();
        assert_eq!(io.position().unwrap(), 60);
    }

    #[test]
    fn from_memory_size() {
        let io = IOContext::from_memory(vec![0u8; 256]);
        assert_eq!(io.size, 256);
        assert!(io.seekable);
    }

    #[test]
    fn from_cursor() {
        let cursor = std::io::Cursor::new(vec![1u8, 2, 3]);
        let mut io = IOContext::from_read_write_seek(cursor, 3);
        assert!(io.seekable);
        assert_eq!(io.read_u8().unwrap(), 1);
    }

    #[test]
    fn into_vec() {
        let mut io = IOContext::memory_writer();
        io.write_all(b"hello").unwrap();
        let data = io.into_vec().unwrap();
        assert_eq!(data, b"hello");
    }

    // ── Negative ──

    #[test]
    fn read_past_eof() {
        let mut io = IOContext::from_memory(vec![1, 2]);
        io.read_bytes(2).unwrap();
        assert!(io.read_u8().is_err());
    }

    #[test]
    fn read_u32_insufficient_data() {
        let mut io = IOContext::from_memory(vec![1, 2]);
        assert!(io.read_u32_le().is_err());
    }

    // ── Edge ──

    #[test]
    fn empty_memory() {
        let mut io = IOContext::from_memory(vec![]);
        assert_eq!(io.size, 0);
        assert!(io.read_u8().is_err());
    }

    #[test]
    fn write_and_reread() {
        let mut io = IOContext::memory_writer();
        for i in 0u8..10 {
            io.write_u8(i).unwrap();
        }
        io.seek(0).unwrap();
        for i in 0u8..10 {
            assert_eq!(io.read_u8().unwrap(), i);
        }
    }
}
