use av_util::error::Result;
use crate::io::IOContext;
use crate::protocol::Protocol;
use std::io::{self, Read, Seek, SeekFrom, Write};

/// Pipe protocol for reading from stdin or writing to stdout.
///
/// Pipe sources are non-seekable.
pub struct PipeProtocol;

impl PipeProtocol {
    pub fn new() -> Self { Self }
}

impl Default for PipeProtocol {
    fn default() -> Self { Self::new() }
}

impl Protocol for PipeProtocol {
    fn name(&self) -> &'static str { "pipe" }

    fn can_handle(&self, url: &str) -> bool {
        url == "pipe:" || url == "pipe:0" || url == "pipe:1" || url.starts_with("pipe://")
    }

    fn open(&self, _url: &str, write: bool) -> Result<IOContext> {
        if write {
            let wrapper = PipeWriter(Box::new(io::stdout()));
            let mut ctx = IOContext::from_read_write_seek(wrapper, -1);
            ctx.seekable = false;
            Ok(ctx)
        } else {
            let wrapper = PipeReader(Box::new(io::stdin()));
            let mut ctx = IOContext::from_read_write_seek(wrapper, -1);
            ctx.seekable = false;
            Ok(ctx)
        }
    }
}

/// Wrapper that implements Read+Write+Seek for a Read source (Seek fails).
struct PipeReader(Box<dyn Read + Send>);

impl Read for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for PipeReader {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "pipe is read-only"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "pipe is read-only"))
    }
}

impl Seek for PipeReader {
    fn seek(&mut self, _pos: SeekFrom) -> io::Result<u64> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "pipe is not seekable"))
    }
}

/// Wrapper that implements Read+Write+Seek for a Write sink (Seek fails).
struct PipeWriter(Box<dyn Write + Send>);

impl Read for PipeWriter {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "pipe is write-only"))
    }
}

impl Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl Seek for PipeWriter {
    fn seek(&mut self, _pos: SeekFrom) -> io::Result<u64> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "pipe is not seekable"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_handle() {
        let p = PipeProtocol::new();
        assert!(p.can_handle("pipe:"));
        assert!(p.can_handle("pipe:0"));
        assert!(p.can_handle("pipe:1"));
        assert!(p.can_handle("pipe://stdin"));
        assert!(!p.can_handle("file.wav"));
        assert!(!p.can_handle("http://example.com"));
    }

    #[test]
    fn name() {
        assert_eq!(PipeProtocol::new().name(), "pipe");
    }

    #[test]
    fn pipe_reader_not_seekable() {
        // Simulate a pipe reader with a Cursor (pretend it's stdin).
        let data = vec![1u8, 2, 3, 4, 5];
        let cursor = std::io::Cursor::new(data.clone());
        let reader = PipeReader(Box::new(cursor));
        let mut ctx = IOContext::from_read_write_seek(reader, -1);
        ctx.seekable = false;

        // Read works.
        let mut buf = [0u8; 3];
        ctx.read_exact(&mut buf).unwrap();
        assert_eq!(buf, [1, 2, 3]);

        // Seek fails (non-seekable).
        assert!(ctx.seek(0).is_err());
    }

    #[test]
    fn pipe_writer_not_seekable() {
        let sink = Vec::<u8>::new();
        let cursor = std::io::Cursor::new(sink);
        let writer = PipeWriter(Box::new(cursor));
        let mut ctx = IOContext::from_read_write_seek(writer, -1);
        ctx.seekable = false;

        // Seek fails.
        assert!(ctx.seek(0).is_err());
    }
}
