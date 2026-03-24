use av_util::error::{Error, Result};
use crate::io::IOContext;
use crate::protocol::Protocol;

use std::fs::{File, OpenOptions};

/// Local file protocol.
pub struct FileProtocol;

impl FileProtocol {
    pub fn new() -> Self { Self }
}

impl Default for FileProtocol {
    fn default() -> Self { Self::new() }
}

impl Protocol for FileProtocol {
    fn name(&self) -> &'static str { "file" }

    fn can_handle(&self, url: &str) -> bool {
        // Accept plain paths and file:// URLs.
        !url.contains("://") || url.starts_with("file://")
    }

    fn open(&self, url: &str, write: bool) -> Result<IOContext> {
        let path = url.strip_prefix("file://").unwrap_or(url);
        if write {
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .read(true)
                .open(path)
                .map_err(Error::from)?;
            Ok(IOContext::from_read_write_seek(file, 0))
        } else {
            let file = File::open(path).map_err(Error::from)?;
            let size = file.metadata().map(|m| m.len() as i64).unwrap_or(-1);
            Ok(IOContext::from_read_write_seek(file, size))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_handle_paths() {
        let p = FileProtocol::new();
        assert!(p.can_handle("test.wav"));
        assert!(p.can_handle("/tmp/test.mp4"));
        assert!(p.can_handle("file:///tmp/test.mp4"));
        assert!(!p.can_handle("http://example.com/video.mp4"));
    }

    #[test]
    fn open_nonexistent_file() {
        let p = FileProtocol::new();
        assert!(p.open("/nonexistent/path/file.wav", false).is_err());
    }

    #[test]
    fn name() {
        assert_eq!(FileProtocol::new().name(), "file");
    }
}
