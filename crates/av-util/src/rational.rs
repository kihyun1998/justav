use crate::error::{Error, Result};
use core::cmp::Ordering;
use core::fmt;
use core::ops::{Add, Div, Mul, Neg, Sub};

/// A rational number represented as numerator / denominator.
///
/// Used for timestamps, frame rates, sample aspect ratios, and time bases.
/// The denominator is always kept positive after construction. Zero denominator
/// is allowed only as a sentinel (representing "unknown" / invalid).
#[derive(Clone, Copy, Eq)]
pub struct Rational {
    pub num: i32,
    pub den: i32,
}

impl Rational {
    /// Create a new rational. Denominator of 0 represents "unknown".
    pub const fn new(num: i32, den: i32) -> Self {
        Self { num, den }
    }

    /// The zero rational (0/1).
    pub const ZERO: Self = Self { num: 0, den: 1 };

    /// An "unknown" / invalid rational (0/0).
    pub const UNKNOWN: Self = Self { num: 0, den: 0 };

    /// Returns true if this rational is valid (denominator != 0).
    pub const fn is_valid(&self) -> bool {
        self.den != 0
    }

    /// Reduce this rational to lowest terms.
    ///
    /// Returns `Error::InvalidArgument` if denominator is 0.
    pub fn reduce(&self) -> Result<Self> {
        if self.den == 0 {
            return Err(Error::InvalidArgument("denominator is zero".into()));
        }
        if self.num == 0 {
            return Ok(Self::new(0, 1));
        }

        let g = gcd(
            self.num.unsigned_abs() as u64,
            self.den.unsigned_abs() as u64,
        );
        let mut num = self.num / g as i32;
        let mut den = self.den / g as i32;

        // Keep denominator positive.
        if den < 0 {
            num = num.checked_neg().ok_or(Error::Overflow)?;
            den = den.checked_neg().ok_or(Error::Overflow)?;
        }

        Ok(Self::new(num, den))
    }

    /// Convert to f64. Returns `None` if denominator is 0.
    pub fn to_f64(self) -> Option<f64> {
        if self.den == 0 {
            None
        } else {
            Some(self.num as f64 / self.den as f64)
        }
    }

    /// Invert (swap numerator and denominator).
    ///
    /// Returns `Error::InvalidArgument` if numerator is 0.
    pub fn invert(&self) -> Result<Self> {
        if self.num == 0 {
            return Err(Error::InvalidArgument("cannot invert zero rational".into()));
        }
        Ok(Self::new(self.den, self.num))
    }

    /// Convert an f64 to the nearest Rational with denominator ≤ `max_den`.
    ///
    /// Uses continued fraction expansion for best approximation.
    pub fn from_f64(val: f64, max_den: i32) -> Result<Self> {
        if val.is_nan() {
            return Err(Error::InvalidArgument(
                "NaN cannot be converted to rational".into(),
            ));
        }
        if val.is_infinite() {
            return Err(Error::InvalidArgument(
                "infinity cannot be converted to rational".into(),
            ));
        }
        if max_den <= 0 {
            return Err(Error::InvalidArgument("max_den must be positive".into()));
        }

        let neg = val < 0.0;
        let val = val.abs();

        // Continued fraction approximation (Stern-Brocot / mediants).
        let mut p0: i64 = 0;
        let mut q0: i64 = 1;
        let mut p1: i64 = 1;
        let mut q1: i64 = 0;
        let mut x = val;

        let max_den = max_den as i64;

        for _ in 0..64 {
            let a = x as i64;
            let p2 = a.saturating_mul(p1).saturating_add(p0);
            let q2 = a.saturating_mul(q1).saturating_add(q0);

            if q2 > max_den {
                break;
            }

            p0 = p1;
            q0 = q1;
            p1 = p2;
            q1 = q2;

            let frac = x - a as f64;
            if frac < 1e-12 {
                break;
            }
            x = 1.0 / frac;

            if x > 1e15 {
                break;
            }
        }

        let num = if neg { -p1 } else { p1 };

        // Clamp to i32 range.
        let num = i32::try_from(num).map_err(|_| Error::Overflow)?;
        let den = i32::try_from(q1).map_err(|_| Error::Overflow)?;

        Ok(Self::new(num, den))
    }
}

impl fmt::Debug for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.num, self.den)
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.num, self.den)
    }
}

impl PartialEq for Rational {
    fn eq(&self, other: &Self) -> bool {
        if !self.is_valid() || !other.is_valid() {
            return false;
        }
        // Cross-multiply to avoid overflow for common cases.
        (self.num as i64) * (other.den as i64) == (other.num as i64) * (self.den as i64)
    }
}

#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if !self.is_valid() || !other.is_valid() {
            return None;
        }
        let lhs = (self.num as i64) * (other.den as i64);
        let rhs = (other.num as i64) * (self.den as i64);

        // If denominators have different signs, comparison flips.
        let sign = (self.den as i64).signum() * (other.den as i64).signum();
        if sign > 0 {
            Some(lhs.cmp(&rhs))
        } else {
            Some(rhs.cmp(&lhs))
        }
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

impl Add for Rational {
    type Output = Result<Self>;

    fn add(self, rhs: Self) -> Result<Self> {
        if !self.is_valid() || !rhs.is_valid() {
            return Err(Error::InvalidArgument(
                "cannot add invalid rationals".into(),
            ));
        }
        let num = (self.num as i64) * (rhs.den as i64) + (rhs.num as i64) * (self.den as i64);
        let den = (self.den as i64) * (rhs.den as i64);

        let g = gcd(num.unsigned_abs(), den.unsigned_abs());
        let num = i32::try_from(num / g as i64).map_err(|_| Error::Overflow)?;
        let den = i32::try_from(den / g as i64).map_err(|_| Error::Overflow)?;

        Self::new(num, den).reduce()
    }
}

impl Sub for Rational {
    type Output = Result<Self>;

    fn sub(self, rhs: Self) -> Result<Self> {
        self + Rational::new(-rhs.num, rhs.den)
    }
}

impl Mul for Rational {
    type Output = Result<Self>;

    fn mul(self, rhs: Self) -> Result<Self> {
        if !self.is_valid() || !rhs.is_valid() {
            return Err(Error::InvalidArgument(
                "cannot multiply invalid rationals".into(),
            ));
        }
        let num = (self.num as i64) * (rhs.num as i64);
        let den = (self.den as i64) * (rhs.den as i64);

        let g = gcd(num.unsigned_abs(), den.unsigned_abs());
        let num = i32::try_from(num / g as i64).map_err(|_| Error::Overflow)?;
        let den = i32::try_from(den / g as i64).map_err(|_| Error::Overflow)?;

        Self::new(num, den).reduce()
    }
}

impl Div for Rational {
    type Output = Result<Self>;

    fn div(self, rhs: Self) -> Result<Self> {
        if rhs.num == 0 {
            return Err(Error::InvalidArgument("division by zero".into()));
        }
        self * Rational::new(rhs.den, rhs.num)
    }
}

impl Neg for Rational {
    type Output = Self;

    fn neg(self) -> Self {
        Self::new(-self.num, self.den)
    }
}

/// Compute greatest common divisor of two unsigned values.
fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Positive ──

    #[test]
    fn new_and_display() {
        let r = Rational::new(1, 30);
        assert_eq!(r.to_string(), "1/30");
    }

    #[test]
    fn reduce_basic() {
        let r = Rational::new(6, 4).reduce().unwrap();
        assert_eq!(r, Rational::new(3, 2));
    }

    #[test]
    fn reduce_negative_den() {
        let r = Rational::new(3, -4).reduce().unwrap();
        assert_eq!(r, Rational::new(-3, 4));
    }

    #[test]
    fn reduce_zero_num() {
        let r = Rational::new(0, 5).reduce().unwrap();
        assert_eq!(r, Rational::new(0, 1));
    }

    #[test]
    fn to_f64_normal() {
        let r = Rational::new(1, 4);
        assert!((r.to_f64().unwrap() - 0.25).abs() < 1e-12);
    }

    #[test]
    fn add_basic() {
        let a = Rational::new(1, 4);
        let b = Rational::new(1, 4);
        let c = (a + b).unwrap();
        assert_eq!(c, Rational::new(1, 2));
    }

    #[test]
    fn sub_basic() {
        let a = Rational::new(3, 4);
        let b = Rational::new(1, 4);
        let c = (a - b).unwrap();
        assert_eq!(c, Rational::new(1, 2));
    }

    #[test]
    fn mul_basic() {
        let a = Rational::new(2, 3);
        let b = Rational::new(3, 4);
        let c = (a * b).unwrap();
        assert_eq!(c, Rational::new(1, 2));
    }

    #[test]
    fn div_basic() {
        let a = Rational::new(1, 2);
        let b = Rational::new(3, 4);
        let c = (a / b).unwrap();
        assert_eq!(c, Rational::new(2, 3));
    }

    #[test]
    fn neg_basic() {
        let r = -Rational::new(3, 4);
        assert_eq!(r, Rational::new(-3, 4));
    }

    #[test]
    fn invert_basic() {
        let r = Rational::new(3, 4).invert().unwrap();
        assert_eq!(r, Rational::new(4, 3));
    }

    #[test]
    fn from_f64_simple() {
        let r = Rational::from_f64(0.5, 100).unwrap();
        assert_eq!(r, Rational::new(1, 2));
    }

    #[test]
    fn from_f64_negative() {
        let r = Rational::from_f64(-0.25, 100).unwrap();
        assert_eq!(r, Rational::new(-1, 4));
    }

    #[test]
    fn from_f64_irrational_approximation() {
        // pi ≈ 355/113 with max_den=1000
        let r = Rational::from_f64(std::f64::consts::PI, 1000).unwrap();
        let val = r.to_f64().unwrap();
        assert!((val - std::f64::consts::PI).abs() < 0.001);
    }

    #[test]
    fn equality_cross_multiply() {
        assert_eq!(Rational::new(2, 4), Rational::new(1, 2));
        assert_eq!(Rational::new(-1, 2), Rational::new(1, -2));
    }

    #[test]
    fn ordering() {
        assert!(Rational::new(1, 3) < Rational::new(1, 2));
        assert!(Rational::new(2, 3) > Rational::new(1, 2));
    }

    #[test]
    fn is_valid() {
        assert!(Rational::new(1, 2).is_valid());
        assert!(Rational::ZERO.is_valid());
    }

    #[test]
    fn send_and_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<Rational>();
        assert_sync::<Rational>();
    }

    // ── Negative ──

    #[test]
    fn reduce_zero_den() {
        let r = Rational::new(1, 0).reduce();
        assert!(r.is_err());
    }

    #[test]
    fn to_f64_zero_den() {
        assert!(Rational::new(1, 0).to_f64().is_none());
    }

    #[test]
    fn div_by_zero() {
        let a = Rational::new(1, 2);
        let b = Rational::new(0, 1);
        assert!((a / b).is_err());
    }

    #[test]
    fn invert_zero() {
        assert!(Rational::new(0, 1).invert().is_err());
    }

    #[test]
    fn add_invalid() {
        let a = Rational::new(1, 2);
        let b = Rational::UNKNOWN;
        assert!((a + b).is_err());
    }

    #[test]
    fn from_f64_nan() {
        assert!(Rational::from_f64(f64::NAN, 100).is_err());
    }

    #[test]
    fn from_f64_infinity() {
        assert!(Rational::from_f64(f64::INFINITY, 100).is_err());
    }

    #[test]
    fn from_f64_zero_max_den() {
        assert!(Rational::from_f64(0.5, 0).is_err());
    }

    #[test]
    fn unknown_is_not_valid() {
        assert!(!Rational::UNKNOWN.is_valid());
    }

    #[test]
    fn unknown_not_equal_to_itself() {
        // Invalid rationals are never equal, similar to NaN.
        assert_ne!(Rational::UNKNOWN, Rational::UNKNOWN);
    }

    #[test]
    fn unknown_no_ordering() {
        assert!(
            Rational::UNKNOWN
                .partial_cmp(&Rational::new(1, 2))
                .is_none()
        );
    }

    // ── Edge Cases ──

    #[test]
    fn i32_max_reduce() {
        let r = Rational::new(i32::MAX, i32::MAX).reduce().unwrap();
        assert_eq!(r, Rational::new(1, 1));
    }

    #[test]
    fn large_values_multiply() {
        // 30000/1 * 1/30000 = 1/1 — tests i64 intermediate
        let a = Rational::new(30000, 1);
        let b = Rational::new(1, 30000);
        let c = (a * b).unwrap();
        assert_eq!(c, Rational::new(1, 1));
    }
}
