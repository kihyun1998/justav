pub mod file;
pub mod pipe;

use av_util::error::Result;
use crate::io::IOContext;

/// Trait for I/O protocols (file, HTTP, pipe, etc.).
///
/// A protocol knows how to open a URL/path and return an `IOContext`.
pub trait Protocol: Send {
    /// Protocol name (e.g. "file", "http", "pipe").
    fn name(&self) -> &'static str;

    /// Check if this protocol can handle the given URL.
    fn can_handle(&self, url: &str) -> bool;

    /// Open a URL and return an IOContext.
    fn open(&self, url: &str, write: bool) -> Result<IOContext>;
}
