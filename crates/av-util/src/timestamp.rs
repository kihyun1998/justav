use crate::error::{Error, Result};
use crate::mathematics::{rescale_q, rescale_q_rnd, Rounding};
use crate::rational::Rational;
use core::cmp::Ordering;
use core::fmt;

/// Sentinel value meaning "no presentation timestamp".
pub const NOPTS: i64 = i64::MIN;

/// A timestamp with its associated time base.
///
/// Wraps a raw `i64` value and its [`Rational`] time base, providing
/// convenient conversion, comparison, and discontinuity detection.
#[derive(Clone, Copy, Eq)]
pub struct Timestamp {
    /// Raw timestamp value in time_base units. [`NOPTS`] means unset.
    pub value: i64,
    /// Time base (e.g. 1/90000 for MPEG-TS, 1/48000 for audio).
    pub time_base: Rational,
}

impl Timestamp {
    /// Create a new timestamp.
    pub const fn new(value: i64, time_base: Rational) -> Self {
        Self { value, time_base }
    }

    /// An unset / unknown timestamp.
    pub const NONE: Self = Self {
        value: NOPTS,
        time_base: Rational::UNKNOWN,
    };

    /// Returns true if this timestamp has a valid value (not NOPTS).
    pub const fn is_valid(&self) -> bool {
        self.value != NOPTS && self.time_base.den != 0
    }

    /// Convert this timestamp to a different time base.
    pub fn rescale(&self, dst_tb: Rational) -> Result<Self> {
        if !self.is_valid() {
            return Err(Error::InvalidArgument("cannot rescale invalid timestamp".into()));
        }
        let value = rescale_q(self.value, self.time_base, dst_tb)?;
        Ok(Self::new(value, dst_tb))
    }

    /// Convert with explicit rounding.
    pub fn rescale_rnd(&self, dst_tb: Rational, rnd: Rounding) -> Result<Self> {
        if !self.is_valid() {
            return Err(Error::InvalidArgument("cannot rescale invalid timestamp".into()));
        }
        let value = rescale_q_rnd(self.value, self.time_base, dst_tb, rnd)?;
        Ok(Self::new(value, dst_tb))
    }

    /// Convert to seconds as f64. Returns `None` if invalid.
    pub fn to_seconds(&self) -> Option<f64> {
        if !self.is_valid() {
            return None;
        }
        let tb = self.time_base.to_f64()?;
        Some(self.value as f64 * tb)
    }

    /// Create a timestamp from seconds.
    pub fn from_seconds(seconds: f64, time_base: Rational) -> Result<Self> {
        if !time_base.is_valid() {
            return Err(Error::InvalidArgument("invalid time base".into()));
        }
        let tb_f64 = time_base.to_f64().ok_or(Error::InvalidArgument("invalid time base".into()))?;
        if tb_f64 == 0.0 {
            return Err(Error::InvalidArgument("time base is zero".into()));
        }
        let value = (seconds / tb_f64).round() as i64;
        Ok(Self::new(value, time_base))
    }

    /// Detect a discontinuity between two timestamps.
    ///
    /// Returns `true` if the gap between `prev` and `self` exceeds
    /// `threshold_seconds`. Both timestamps must be valid.
    pub fn is_discontinuity(&self, prev: &Timestamp, threshold_seconds: f64) -> Option<bool> {
        let cur = self.to_seconds()?;
        let prv = prev.to_seconds()?;
        let gap = (cur - prv).abs();
        Some(gap > threshold_seconds)
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.is_valid() {
            write!(f, "Timestamp(NOPTS)")
        } else {
            write!(f, "Timestamp({} @ {})", self.value, self.time_base)
        }
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_seconds() {
            Some(s) => write!(f, "{s:.6}s"),
            None => write!(f, "NOPTS"),
        }
    }
}

impl PartialEq for Timestamp {
    fn eq(&self, other: &Self) -> bool {
        if !self.is_valid() || !other.is_valid() {
            return false;
        }
        let lhs = (self.value as i128) * (self.time_base.num as i128) * (other.time_base.den as i128);
        let rhs = (other.value as i128) * (other.time_base.num as i128) * (self.time_base.den as i128);
        lhs == rhs
    }
}

#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for Timestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if !self.is_valid() || !other.is_valid() {
            return None;
        }
        let lhs = (self.value as i128) * (self.time_base.num as i128) * (other.time_base.den as i128);
        let rhs = (other.value as i128) * (other.time_base.num as i128) * (self.time_base.den as i128);
        Some(lhs.cmp(&rhs))
    }
}

impl Ord for Timestamp {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tb_ms() -> Rational { Rational::new(1, 1000) }
    fn tb_us() -> Rational { Rational::new(1, 1_000_000) }
    fn tb_90k() -> Rational { Rational::new(1, 90000) }

    // ── Positive ──

    #[test]
    fn new_and_valid() {
        let ts = Timestamp::new(1000, tb_ms());
        assert!(ts.is_valid());
        assert_eq!(ts.value, 1000);
    }

    #[test]
    fn to_seconds() {
        let ts = Timestamp::new(1500, tb_ms());
        let s = ts.to_seconds().unwrap();
        assert!((s - 1.5).abs() < 1e-9);
    }

    #[test]
    fn from_seconds() {
        let ts = Timestamp::from_seconds(2.5, tb_ms()).unwrap();
        assert_eq!(ts.value, 2500);
    }

    #[test]
    fn rescale_ms_to_us() {
        let ts = Timestamp::new(500, tb_ms());
        let ts2 = ts.rescale(tb_us()).unwrap();
        assert_eq!(ts2.value, 500_000);
        assert_eq!(ts2.time_base, tb_us());
    }

    #[test]
    fn rescale_90k_to_ms() {
        let ts = Timestamp::new(90000, tb_90k());
        let ts2 = ts.rescale(tb_ms()).unwrap();
        assert_eq!(ts2.value, 1000);
    }

    #[test]
    fn equality_different_timebases() {
        let a = Timestamp::new(500, tb_ms());
        let b = Timestamp::new(500_000, tb_us());
        assert_eq!(a, b);
    }

    #[test]
    fn ordering_different_timebases() {
        let a = Timestamp::new(499, tb_ms());
        let b = Timestamp::new(500_000, tb_us());
        assert!(a < b);
    }

    #[test]
    fn display_valid() {
        let ts = Timestamp::new(1500, tb_ms());
        let s = format!("{ts}");
        assert!(s.contains("1.5"));
    }

    #[test]
    fn display_nopts() {
        assert_eq!(format!("{}", Timestamp::NONE), "NOPTS");
    }

    #[test]
    fn debug_valid() {
        let ts = Timestamp::new(100, tb_ms());
        let s = format!("{ts:?}");
        assert!(s.contains("100"));
        assert!(s.contains("1/1000"));
    }

    #[test]
    fn debug_nopts() {
        let s = format!("{:?}", Timestamp::NONE);
        assert!(s.contains("NOPTS"));
    }

    #[test]
    fn discontinuity_detected() {
        let a = Timestamp::new(1000, tb_ms()); // 1.0s
        let b = Timestamp::new(5000, tb_ms()); // 5.0s
        assert_eq!(b.is_discontinuity(&a, 2.0), Some(true));
    }

    #[test]
    fn no_discontinuity() {
        let a = Timestamp::new(1000, tb_ms()); // 1.0s
        let b = Timestamp::new(1033, tb_ms()); // 1.033s
        assert_eq!(b.is_discontinuity(&a, 1.0), Some(false));
    }

    // ── Negative ──

    #[test]
    fn none_is_not_valid() {
        assert!(!Timestamp::NONE.is_valid());
    }

    #[test]
    fn nopts_to_seconds_is_none() {
        assert!(Timestamp::NONE.to_seconds().is_none());
    }

    #[test]
    fn rescale_invalid_timestamp() {
        assert!(Timestamp::NONE.rescale(tb_ms()).is_err());
    }

    #[test]
    fn from_seconds_invalid_tb() {
        assert!(Timestamp::from_seconds(1.0, Rational::UNKNOWN).is_err());
    }

    #[test]
    fn nopts_not_equal_to_itself() {
        assert_ne!(Timestamp::NONE, Timestamp::NONE);
    }

    #[test]
    fn nopts_no_ordering() {
        assert!(Timestamp::NONE.partial_cmp(&Timestamp::new(0, tb_ms())).is_none());
    }

    #[test]
    fn discontinuity_with_invalid() {
        let valid = Timestamp::new(1000, tb_ms());
        assert!(valid.is_discontinuity(&Timestamp::NONE, 1.0).is_none());
    }

    // ── Edge ──

    #[test]
    fn zero_timestamp() {
        let ts = Timestamp::new(0, tb_ms());
        assert!(ts.is_valid());
        assert!((ts.to_seconds().unwrap()).abs() < 1e-12);
    }

    #[test]
    fn negative_timestamp() {
        let ts = Timestamp::new(-1000, tb_ms());
        assert!(ts.is_valid());
        assert!((ts.to_seconds().unwrap() + 1.0).abs() < 1e-9);
    }
}
