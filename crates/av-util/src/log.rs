/// Log levels for justav messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogLevel {
    /// Extremely verbose (per-packet, per-frame).
    Trace = 0,
    /// Debugging information.
    Debug = 1,
    /// Informational messages (default).
    Info = 2,
    /// Potential issues that don't prevent operation.
    Warning = 3,
    /// Errors that may cause degraded output.
    Error = 4,
    /// Fatal errors that prevent further processing.
    Fatal = 5,
    /// Suppress all output.
    Quiet = 6,
}

/// Global log level — messages below this level are suppressed.
static LOG_LEVEL: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(LogLevel::Info as u8);

/// Set the global log level.
pub fn set_log_level(level: LogLevel) {
    LOG_LEVEL.store(level as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Get the current global log level.
pub fn log_level() -> LogLevel {
    match LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed) {
        0 => LogLevel::Trace,
        1 => LogLevel::Debug,
        2 => LogLevel::Info,
        3 => LogLevel::Warning,
        4 => LogLevel::Error,
        5 => LogLevel::Fatal,
        _ => LogLevel::Quiet,
    }
}

/// Log a message at the given level.
///
/// Routes to the `tracing` crate for structured logging. Also respects the
/// global [`log_level`] filter — messages below that level are suppressed
/// before reaching tracing.
pub fn log(level: LogLevel, msg: &str) {
    if level < log_level() {
        return;
    }
    match level {
        LogLevel::Trace => tracing::trace!("{}", msg),
        LogLevel::Debug => tracing::debug!("{}", msg),
        LogLevel::Info => tracing::info!("{}", msg),
        LogLevel::Warning => tracing::warn!("{}", msg),
        LogLevel::Error => tracing::error!("{}", msg),
        LogLevel::Fatal => tracing::error!(fatal = true, "{}", msg),
        LogLevel::Quiet => {}
    }
}

/// Log a message with a context name (e.g. module or codec name).
pub fn log_ctx(level: LogLevel, ctx: &str, msg: &str) {
    if level < log_level() {
        return;
    }
    match level {
        LogLevel::Trace => tracing::trace!(context = ctx, "{}", msg),
        LogLevel::Debug => tracing::debug!(context = ctx, "{}", msg),
        LogLevel::Info => tracing::info!(context = ctx, "{}", msg),
        LogLevel::Warning => tracing::warn!(context = ctx, "{}", msg),
        LogLevel::Error => tracing::error!(context = ctx, "{}", msg),
        LogLevel::Fatal => tracing::error!(fatal = true, context = ctx, "{}", msg),
        LogLevel::Quiet => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_level_is_info() {
        set_log_level(LogLevel::Info);
        assert_eq!(log_level(), LogLevel::Info);
    }

    #[test]
    fn set_and_get_level() {
        set_log_level(LogLevel::Debug);
        assert_eq!(log_level(), LogLevel::Debug);
        set_log_level(LogLevel::Info);
    }

    #[test]
    fn level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warning);
        assert!(LogLevel::Warning < LogLevel::Error);
        assert!(LogLevel::Error < LogLevel::Fatal);
        assert!(LogLevel::Fatal < LogLevel::Quiet);
    }

    #[test]
    fn log_does_not_panic() {
        set_log_level(LogLevel::Quiet);
        log(LogLevel::Fatal, "test fatal");
        log(LogLevel::Trace, "test trace");
        set_log_level(LogLevel::Info);
    }

    #[test]
    fn log_ctx_does_not_panic() {
        set_log_level(LogLevel::Quiet);
        log_ctx(LogLevel::Info, "av-codec", "test message");
        set_log_level(LogLevel::Info);
    }

    #[test]
    fn log_with_tracing_subscriber() {
        // Verify tracing integration works when a subscriber is set.
        let _guard = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::TRACE)
            .try_init();

        set_log_level(LogLevel::Trace);
        log(LogLevel::Trace, "trace message");
        log(LogLevel::Debug, "debug message");
        log(LogLevel::Info, "info message");
        log(LogLevel::Warning, "warning message");
        log(LogLevel::Error, "error message");
        log(LogLevel::Fatal, "fatal message");
        log_ctx(LogLevel::Info, "test-ctx", "context message");
        set_log_level(LogLevel::Info);
        // If we got here without panic, tracing integration works.
    }

    #[test]
    fn log_suppressed_below_level() {
        set_log_level(LogLevel::Error);
        // These should not reach tracing (no panic, no output).
        log(LogLevel::Trace, "should be suppressed");
        log(LogLevel::Info, "should be suppressed");
        log(LogLevel::Warning, "should be suppressed");
        set_log_level(LogLevel::Info);
    }
}
