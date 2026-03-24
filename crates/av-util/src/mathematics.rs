use crate::error::{Error, Result};
use crate::rational::Rational;
use core::cmp::Ordering;

/// Rounding modes for rescale operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rounding {
    /// Round toward zero.
    Zero,
    /// Round away from zero.
    Inf,
    /// Round toward negative infinity.
    Down,
    /// Round toward positive infinity.
    Up,
    /// Round to nearest, ties away from zero.
    NearInf,
}

/// Compute the greatest common divisor of two values.
pub fn gcd(a: i64, b: i64) -> i64 {
    let mut a = a.unsigned_abs();
    let mut b = b.unsigned_abs();
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a.max(1) as i64
}

/// Rescale a value from one timebase to another: `value * src / dst`.
///
/// Uses 128-bit intermediate arithmetic to avoid overflow.
pub fn rescale(value: i64, src: i64, dst: i64) -> Result<i64> {
    rescale_rnd(value, src, dst, Rounding::NearInf)
}

/// Rescale a value: `value * src / dst` with the given rounding mode.
pub fn rescale_rnd(value: i64, src: i64, dst: i64, rnd: Rounding) -> Result<i64> {
    if dst == 0 {
        return Err(Error::InvalidArgument("rescale destination is zero".into()));
    }
    if src == 0 {
        return Ok(0);
    }

    let neg = (value < 0) ^ (src < 0) ^ (dst < 0);
    let value = value.unsigned_abs() as u128;
    let src = src.unsigned_abs() as u128;
    let dst = dst.unsigned_abs() as u128;

    let product = value * src;
    let mut result = product / dst;
    let remainder = product % dst;

    // Apply rounding.
    match rnd {
        Rounding::Zero => {}
        Rounding::Inf => {
            if remainder > 0 {
                result += 1;
            }
        }
        Rounding::Down => {
            if neg && remainder > 0 {
                result += 1;
            }
        }
        Rounding::Up => {
            if !neg && remainder > 0 {
                result += 1;
            }
        }
        Rounding::NearInf => {
            if remainder * 2 >= dst {
                result += 1;
            }
        }
    }

    let result = i64::try_from(result).map_err(|_| Error::Overflow)?;
    Ok(if neg { -result } else { result })
}

/// Rescale using two `Rational` time bases: `value * src_tb / dst_tb`.
pub fn rescale_q(value: i64, src_tb: Rational, dst_tb: Rational) -> Result<i64> {
    rescale_q_rnd(value, src_tb, dst_tb, Rounding::NearInf)
}

/// Rescale using two `Rational` time bases with explicit rounding.
pub fn rescale_q_rnd(value: i64, src_tb: Rational, dst_tb: Rational, rnd: Rounding) -> Result<i64> {
    if !src_tb.is_valid() || !dst_tb.is_valid() {
        return Err(Error::InvalidArgument("invalid timebase".into()));
    }
    let src = (src_tb.num as i64) * (dst_tb.den as i64);
    let dst = (src_tb.den as i64) * (dst_tb.num as i64);
    rescale_rnd(value, src, dst, rnd)
}

/// Compare two timestamps expressed in different time bases.
///
/// Returns:
/// - `Ordering::Less` if `ts_a` < `ts_b`
/// - `Ordering::Equal` if `ts_a` == `ts_b`
/// - `Ordering::Greater` if `ts_a` > `ts_b`
///
/// Returns `Error` if either timebase is invalid.
pub fn compare_ts(ts_a: i64, tb_a: Rational, ts_b: i64, tb_b: Rational) -> Result<Ordering> {
    if !tb_a.is_valid() || !tb_b.is_valid() {
        return Err(Error::InvalidArgument("invalid timebase".into()));
    }
    let lhs = (ts_a as i128) * (tb_a.num as i128) * (tb_b.den as i128);
    let rhs = (ts_b as i128) * (tb_b.num as i128) * (tb_a.den as i128);
    Ok(lhs.cmp(&rhs))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn gcd_basic() {
        assert_eq!(gcd(12, 8), 4);
        assert_eq!(gcd(7, 13), 1);
        assert_eq!(gcd(0, 5), 5);
    }

    #[test]
    fn gcd_negative() {
        assert_eq!(gcd(-12, 8), 4);
        assert_eq!(gcd(12, -8), 4);
    }

    #[test]
    fn rescale_simple() {
        // 90000 ticks at 1/90000 → seconds at 1/1000 = 1000
        let result = rescale(90000, 1000, 90000).unwrap();
        assert_eq!(result, 1000);
    }

    #[test]
    fn rescale_rounding_near() {
        // 1 * 2 / 3 = 0.666... → rounds to 1
        let result = rescale_rnd(1, 2, 3, Rounding::NearInf).unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn rescale_rounding_zero() {
        // 1 * 2 / 3 = 0.666... → truncates to 0
        let result = rescale_rnd(1, 2, 3, Rounding::Zero).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn rescale_rounding_up() {
        // 1 * 2 / 3 = 0.666... → positive, rounds up to 1
        let result = rescale_rnd(1, 2, 3, Rounding::Up).unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn rescale_rounding_down() {
        // 1 * 2 / 3 = 0.666... → positive, rounds down to 0
        let result = rescale_rnd(1, 2, 3, Rounding::Down).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn rescale_negative_value() {
        let result = rescale(-90000, 1000, 90000).unwrap();
        assert_eq!(result, -1000);
    }

    #[test]
    fn rescale_q_basic() {
        // 48000 samples at 1/48000 → milliseconds at 1/1000
        let src_tb = Rational::new(1, 48000);
        let dst_tb = Rational::new(1, 1000);
        let result = rescale_q(48000, src_tb, dst_tb).unwrap();
        assert_eq!(result, 1000);
    }

    #[test]
    fn rescale_q_frame_rate_conversion() {
        // Frame 30 at 1/30 fps → frame at 1/60 fps
        let src_tb = Rational::new(1, 30);
        let dst_tb = Rational::new(1, 60);
        let result = rescale_q(30, src_tb, dst_tb).unwrap();
        assert_eq!(result, 60);
    }

    #[test]
    fn compare_ts_equal() {
        let tb_ms = Rational::new(1, 1000);
        let tb_us = Rational::new(1, 1_000_000);
        // 500ms == 500000us
        assert_eq!(
            compare_ts(500, tb_ms, 500_000, tb_us).unwrap(),
            Ordering::Equal
        );
    }

    #[test]
    fn compare_ts_less() {
        let tb_ms = Rational::new(1, 1000);
        let tb_us = Rational::new(1, 1_000_000);
        assert_eq!(
            compare_ts(499, tb_ms, 500_000, tb_us).unwrap(),
            Ordering::Less
        );
    }

    #[test]
    fn compare_ts_greater() {
        let tb_ms = Rational::new(1, 1000);
        let tb_us = Rational::new(1, 1_000_000);
        assert_eq!(
            compare_ts(501, tb_ms, 500_000, tb_us).unwrap(),
            Ordering::Greater
        );
    }

    #[test]
    fn rescale_zero_src() {
        // 0 * anything = 0
        let result = rescale(100, 0, 1).unwrap();
        assert_eq!(result, 0);
    }

    // ── Negative ──

    #[test]
    fn rescale_zero_dst() {
        assert!(rescale(100, 1, 0).is_err());
    }

    #[test]
    fn rescale_q_invalid_timebase() {
        let valid = Rational::new(1, 1000);
        let invalid = Rational::UNKNOWN;
        assert!(rescale_q(100, valid, invalid).is_err());
        assert!(rescale_q(100, invalid, valid).is_err());
    }

    #[test]
    fn compare_ts_invalid_timebase() {
        let valid = Rational::new(1, 1000);
        let invalid = Rational::UNKNOWN;
        assert!(compare_ts(100, valid, 100, invalid).is_err());
    }

    // ── Edge Cases ──

    #[test]
    fn rescale_large_values_no_overflow() {
        // Large timestamp that would overflow i64 without 128-bit intermediate.
        let result = rescale(i64::MAX / 2, 2, 1).unwrap();
        assert_eq!(result, i64::MAX - 1);
    }

    #[test]
    fn rescale_identity() {
        // value * 1 / 1 = value
        assert_eq!(rescale(12345, 1, 1).unwrap(), 12345);
    }

    #[test]
    fn gcd_both_zero() {
        // By convention, gcd(0,0) returns 1 (our sentinel).
        assert_eq!(gcd(0, 0), 1);
    }

    #[test]
    fn compare_ts_zero_timestamps() {
        let tb = Rational::new(1, 1000);
        assert_eq!(compare_ts(0, tb, 0, tb).unwrap(), Ordering::Equal);
    }

    #[test]
    fn compare_ts_negative() {
        let tb = Rational::new(1, 1000);
        assert_eq!(compare_ts(-100, tb, 100, tb).unwrap(), Ordering::Less);
    }
}
