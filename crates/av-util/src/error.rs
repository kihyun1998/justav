use core::fmt;

/// All possible errors in justav.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// End of file / stream.
    Eof,
    /// Invalid data encountered (corrupted input, bad header, etc.).
    InvalidData(String),
    /// Requested feature or codec is not supported.
    Unsupported(String),
    /// I/O error (wraps a description since `std::io::Error` is not Clone/Eq).
    Io(String),
    /// Resource not found (file, stream, codec, etc.).
    NotFound(String),
    /// Operation would require more data (try again).
    Again,
    /// Numeric overflow during computation.
    Overflow,
    /// Invalid argument passed to a function.
    InvalidArgument(String),
    /// Operation not permitted in current state.
    InvalidState(String),
    /// Out of memory or buffer too small.
    NoMemory,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Eof => write!(f, "end of file"),
            Error::InvalidData(msg) => write!(f, "invalid data: {msg}"),
            Error::Unsupported(msg) => write!(f, "unsupported: {msg}"),
            Error::Io(msg) => write!(f, "I/O error: {msg}"),
            Error::NotFound(msg) => write!(f, "not found: {msg}"),
            Error::Again => write!(f, "resource temporarily unavailable"),
            Error::Overflow => write!(f, "numeric overflow"),
            Error::InvalidArgument(msg) => write!(f, "invalid argument: {msg}"),
            Error::InvalidState(msg) => write!(f, "invalid state: {msg}"),
            Error::NoMemory => write!(f, "out of memory"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::UnexpectedEof => Error::Eof,
            std::io::ErrorKind::NotFound => Error::NotFound(e.to_string()),
            std::io::ErrorKind::OutOfMemory => Error::NoMemory,
            std::io::ErrorKind::InvalidData => Error::InvalidData(e.to_string()),
            std::io::ErrorKind::InvalidInput => Error::InvalidArgument(e.to_string()),
            _ => Error::Io(e.to_string()),
        }
    }
}

/// Convenience result type for justav operations.
pub type Result<T> = core::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn display_eof() {
        assert_eq!(Error::Eof.to_string(), "end of file");
    }

    #[test]
    fn display_invalid_data() {
        let e = Error::InvalidData("bad header".into());
        assert_eq!(e.to_string(), "invalid data: bad header");
    }

    #[test]
    fn display_all_variants() {
        // Ensure every variant formats without panic.
        let variants: Vec<Error> = vec![
            Error::Eof,
            Error::InvalidData("x".into()),
            Error::Unsupported("x".into()),
            Error::Io("x".into()),
            Error::NotFound("x".into()),
            Error::Again,
            Error::Overflow,
            Error::InvalidArgument("x".into()),
            Error::InvalidState("x".into()),
            Error::NoMemory,
        ];
        for v in &variants {
            assert!(!v.to_string().is_empty());
        }
    }

    #[test]
    fn result_ok() {
        let r: Result<i32> = Ok(42);
        assert_eq!(r.unwrap(), 42);
    }

    #[test]
    fn result_err() {
        let r: Result<i32> = Err(Error::Eof);
        assert!(r.is_err());
    }

    // ── Negative ──

    #[test]
    fn from_io_unexpected_eof() {
        let io_err = std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "eof");
        let e: Error = io_err.into();
        assert_eq!(e, Error::Eof);
    }

    #[test]
    fn from_io_not_found() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let e: Error = io_err.into();
        assert!(matches!(e, Error::NotFound(_)));
    }

    #[test]
    fn from_io_other() {
        let io_err = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
        let e: Error = io_err.into();
        assert!(matches!(e, Error::Io(_)));
    }

    #[test]
    fn error_is_clone_and_eq() {
        let a = Error::Overflow;
        let b = a.clone();
        assert_eq!(a, b);
    }
}
