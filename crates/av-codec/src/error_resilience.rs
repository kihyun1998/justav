/// Policy for handling corrupt or missing data during decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ErrorPolicy {
    /// Return an error immediately on corrupt data.
    #[default]
    Fail,
    /// Skip the corrupt unit and continue with the next one.
    Skip,
    /// Attempt to conceal the error (e.g. repeat previous frame).
    Conceal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_fail() {
        assert_eq!(ErrorPolicy::default(), ErrorPolicy::Fail);
    }

    #[test]
    fn all_variants_debug() {
        for v in [ErrorPolicy::Fail, ErrorPolicy::Skip, ErrorPolicy::Conceal] {
            assert!(!format!("{v:?}").is_empty());
        }
    }
}
