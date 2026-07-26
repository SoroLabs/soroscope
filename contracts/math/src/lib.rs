#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Env};

pub use soroscope_error_codes::ContractError as MathError;

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct Fixed(pub i128);

pub const SCALE: i128 = 1_000_000_000_000_000_000; // 18 decimals
pub const LN2: i128 = 693_147_180_559_945_309; // ln(2) * SCALE

#[allow(clippy::should_implement_trait)]
impl Fixed {
    pub const ZERO: Fixed = Fixed(0);
    pub const ONE: Fixed = Fixed(SCALE);

    pub fn from_int(v: i128) -> Result<Self, MathError> {
        v.checked_mul(SCALE).map(Fixed).ok_or(MathError::Overflow)
    }

    pub fn to_int(self) -> i128 {
        self.0 / SCALE
    }

    pub fn add(self, other: Fixed) -> Result<Fixed, MathError> {
        self.0
            .checked_add(other.0)
            .map(Fixed)
            .ok_or(MathError::Overflow)
    }

    pub fn sub(self, other: Fixed) -> Result<Fixed, MathError> {
        self.0
            .checked_sub(other.0)
            .map(Fixed)
            .ok_or(MathError::Overflow)
    }

    pub fn mul(self, other: Fixed) -> Result<Fixed, MathError> {
        mul_div(self.0, other.0, SCALE)
            .map(Fixed)
            .ok_or(MathError::Overflow)
    }

    pub fn div(self, other: Fixed) -> Result<Fixed, MathError> {
        if other.0 == 0 {
            return Err(MathError::DivisionByZero);
        }
        mul_div(self.0, SCALE, other.0)
            .map(Fixed)
            .ok_or(MathError::Overflow)
    }

    /// Exponential function e^x
    /// Uses range reduction: e^x = 2^n * e^r where r = x - n*ln(2)
    pub fn exp(self) -> Result<Fixed, MathError> {
        if self.0 == 0 {
            return Ok(Fixed::ONE);
        }
        if self.0 < -42 * SCALE {
            return Ok(Fixed::ZERO);
        } // e^-42 is very small
        if self.0 > 88 * SCALE {
            return Err(MathError::Overflow);
        } // e^88 overflows i128

        let x = self.0;
        let n = x / LN2;
        let r = x % LN2;

        // e^r using Taylor series (r is in [0, ln(2)])
        let mut result = SCALE;
        let mut term = SCALE;

        for i in 1..25 {
            term = mul_div(term, r, i as i128 * SCALE).ok_or(MathError::Overflow)?;
            if term == 0 {
                break;
            }
            result = result.checked_add(term).ok_or(MathError::Overflow)?;
        }

        // Multiply by 2^n
        if n >= 0 {
            result = result.checked_shl(n as u32).ok_or(MathError::Overflow)?;
        } else {
            result >>= (-n) as u32;
        }

        Ok(Fixed(result))
    }

    /// Natural logarithm ln(x)
    /// Uses Newton's method with a good initial guess
    pub fn ln(self) -> Result<Fixed, MathError> {
        if self.0 <= 0 {
            return Err(MathError::InvalidInput);
        }

        let mut x = self.0;
        let mut n = 0i128;

        // Range reduction: ln(x) = ln(x / 2^n) + n*ln(2)
        // Bring x to [1, 2] range
        while x > 2 * SCALE {
            x >>= 1;
            n += 1;
        }
        while x < SCALE {
            x <<= 1;
            n -= 1;
        }

        // ln(x) for x in [1, 2] using Newton's method
        // y_{n+1} = y_n + 2 * (x - e^y_n) / (x + e^y_n)
        let mut y = 0i128; // ln(1) = 0 is a good start for [1, 2]

        for _ in 0..8 {
            let ey = Fixed(y).exp()?;
            let num = (x - ey.0).checked_mul(2).ok_or(MathError::Overflow)?;
            let den = x + ey.0;
            // (num * SCALE) / den
            let delta = mul_div(num, SCALE, den).ok_or(MathError::Overflow)?;
            y = y.checked_add(delta).ok_or(MathError::Overflow)?;
            if delta.abs() <= 1 {
                break;
            }
        }

        // Add n * ln(2)
        let nln2 = n.checked_mul(LN2).ok_or(MathError::Overflow)?;
        y.checked_add(nln2).map(Fixed).ok_or(MathError::Overflow)
    }

    pub fn pow(self, y: Fixed) -> Result<Fixed, MathError> {
        if self.0 == 0 {
            return if y.0 == 0 {
                Ok(Fixed::ONE)
            } else {
                Ok(Fixed::ZERO)
            };
        }
        if self.0 < 0 {
            return Err(MathError::InvalidInput);
        }

        let lnx = self.ln()?;
        let ylnx = y.mul(lnx)?;
        ylnx.exp()
    }
}

fn mul_div(a: i128, b: i128, d: i128) -> Option<i128> {
    if d == 0 {
        return None;
    }
    let a_abs = a.unsigned_abs();
    let b_abs = b.unsigned_abs();
    let d_abs = d.unsigned_abs();

    let (res_abs, overflow) = mul_div_u128(a_abs, b_abs, d_abs);
    if overflow || res_abs > (i128::MAX as u128) {
        return None;
    }

    let res = res_abs as i128;
    if (a < 0) ^ (b < 0) ^ (d < 0) {
        Some(-res)
    } else {
        Some(res)
    }
}

/// Represents a 256-bit unsigned integer as high and low 128-bit parts
#[derive(Debug, Clone, Copy, Default)]
struct U256 {
    high: u128,
    low: u128,
}

impl U256 {
    const ZERO: U256 = U256 { high: 0, low: 0 };
    
    /// Multiplies two u128 values to produce a 256-bit result
    fn from_mul(a: u128, b: u128) -> Self {
        let a_low = a & 0xFFFFFFFFFFFFFFFF;
        let a_high = a >> 64;
        let b_low = b & 0xFFFFFFFFFFFFFFFF;
        let b_high = b >> 64;
        
        let p0 = a_low * b_low;
        let p1 = a_low * b_high;
        let p2 = a_high * b_low;
        let p3 = a_high * b_high;
        
        let mid = (p1 & 0xFFFFFFFFFFFFFFFF) + (p2 & 0xFFFFFFFFFFFFFFFF) + (p0 >> 64);
        let high = p3 + (p1 >> 64) + (p2 >> 64) + (mid >> 64);
        let low = (mid << 64) | (p0 & 0xFFFFFFFFFFFFFFFF);
        
        U256 { high, low }
    }
    
    /// Divides this 256-bit number by a 128-bit divisor
    /// Returns (quotient, overflow) where overflow is true if result doesn't fit in u128
    fn div_u128(self, d: u128) -> (u128, bool) {
        if d == 0 {
            return (0, true);
        }
        
        // If high part is >= divisor, result would overflow u128
        if self.high >= d {
            return (0, true);
        }
        
        // Binary long division: divide 256-bit by 128-bit
        let mut quotient = 0u128;
        let mut remainder = self.high;
        
        for i in (0..128).rev() {
            remainder = (remainder << 1) | ((self.low >> i) & 1);
            if remainder >= d {
                remainder -= d;
                quotient |= 1 << i;
            }
        }
        
        (quotient, false)
    }
}

fn mul_div_u128(a: u128, b: u128, d: u128) -> (u128, bool) {
    if d == 0 {
        return (0, true);
    }
    
    // Try simple multiplication first for small values
    if let Some(prod) = a.checked_mul(b) {
        return (prod / d, false);
    }
    
    // Use 256-bit intermediate for large values
    let product = U256::from_mul(a, b);
    product.div_u128(d)
}

#[contract]
pub struct Math;

#[contractimpl]
impl Math {
    pub fn exp(_e: Env, x: i128) -> Result<i128, MathError> {
        Fixed(x).exp().map(|f| f.0)
    }
    pub fn ln(_e: Env, x: i128) -> Result<i128, MathError> {
        Fixed(x).ln().map(|f| f.0)
    }
    pub fn pow(_e: Env, x: i128, y: i128) -> Result<i128, MathError> {
        Fixed(x).pow(Fixed(y)).map(|f| f.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_overflow_protection() {
        let max = Fixed(i128::MAX);
        let one = Fixed::ONE;
        assert_eq!(max.add(one), Err(MathError::Overflow));

        let large = Fixed(i128::MAX / 2 + 1);
        assert_eq!(large.add(large), Err(MathError::Overflow));

        let small = Fixed::from_int(1).unwrap();
        let _very_large = Fixed(i128::MAX / SCALE + 1);
        // This should overflow during mul_div if not careful, but mul_div handles it
        assert_eq!(small.mul(Fixed(i128::MAX)), Ok(Fixed(i128::MAX)));
        assert_eq!(
            Fixed(i128::MAX).mul(Fixed(2 * SCALE)),
            Err(MathError::Overflow)
        );
    }

    #[test]
    fn test_benchmarks() {
        // This is a "conceptual" benchmark since we can't easily measure time in no_std tests without std
        // But we can compare the complexity/results.

        let x_raw = 2 * SCALE;
        let y_raw = 3 * SCALE;

        // Raw arithmetic (limited to simple ops)
        let raw_mul = x_raw * y_raw / SCALE;

        // Fixed type
        let fixed_mul = Fixed(x_raw).mul(Fixed(y_raw)).unwrap().0;

        assert_eq!(raw_mul, fixed_mul);

        // Advanced ops (no raw equivalent easily)
        let fixed_exp = Fixed(SCALE).exp().unwrap();
        assert!(fixed_exp.0 > 2 * SCALE);
    }

    #[test]
    fn test_mul_div_basic() {
        // Test basic multiplication/division
        assert_eq!(mul_div(100, 200, 50), Some(400));
        assert_eq!(mul_div(100, 200, 100), Some(200));
        assert_eq!(mul_div(0, 100, 50), Some(0));
        assert_eq!(mul_div(100, 0, 50), Some(0));
    }

    #[test]
    fn test_mul_div_division_by_zero() {
        assert_eq!(mul_div(100, 200, 0), None);
    }

    #[test]
    fn test_mul_div_negative_numbers() {
        // Test negative number handling
        assert_eq!(mul_div(-100, 200, 50), Some(-400));
        assert_eq!(mul_div(100, -200, 50), Some(-400));
        assert_eq!(mul_div(100, 200, -50), Some(-400));
        assert_eq!(mul_div(-100, -200, 50), Some(400));
        assert_eq!(mul_div(-100, 200, -50), Some(400));
        assert_eq!(mul_div(100, -200, -50), Some(400));
        assert_eq!(mul_div(-100, -200, -50), Some(-400));
    }

    #[test]
    fn test_mul_div_large_values() {
        // Test with large values that would overflow i128 multiplication
        let large = i128::MAX / 2;
        let result = mul_div(large, 2, SCALE);
        assert!(result.is_some());
        
        // Test with values near i128::MAX
        let max = i128::MAX;
        let result = mul_div(max, 1, SCALE);
        assert!(result.is_some());
    }

    #[test]
    fn test_mul_div_overflow_detection() {
        // Test cases that should overflow
        let max = i128::MAX;
        assert_eq!(mul_div(max, max, 1), None); // Would overflow
        assert_eq!(mul_div(max, 2, SCALE), None); // Would overflow after division
    }

    #[test]
    fn test_mul_div_precision() {
        // Test precision preservation
        assert_eq!(mul_div(SCALE, SCALE, SCALE), Some(SCALE));
        assert_eq!(mul_div(SCALE * 2, SCALE * 3, SCALE), Some(SCALE * 6));
        
        // Test fractional precision
        let result = mul_div(SCALE, SCALE / 2, SCALE);
        assert_eq!(result, Some(SCALE / 2));
    }

    #[test]
    fn test_u256_from_mul() {
        // Test U256 multiplication
        let a = 0xFFFFFFFFFFFFFFFFu128;
        let b = 0xFFFFFFFFFFFFFFFFu128;
        let product = U256::from_mul(a, b);
        
        // a * b should be (2^64 - 1)^2 = 2^128 - 2^65 + 1
        assert_eq!(product.high, 0xFFFFFFFFFFFFFFFE);
        assert_eq!(product.low, 0x0000000000000001);
    }

    #[test]
    fn test_u256_div_u128() {
        // Test U256 division
        let product = U256 { high: 1, low: 0 };
        let (quotient, overflow) = product.div_u128(2);
        assert!(!overflow);
        assert_eq!(quotient, (1u128 << 127) / 2);
        
        // Test overflow case
        let (quotient, overflow) = product.div_u128(1);
        assert!(overflow);
        assert_eq!(quotient, 0);
    }

    #[test]
    fn test_mul_div_edge_cases() {
        // Test edge case: divisor equals one of the operands
        assert_eq!(mul_div(100, 50, 50), Some(100));
        assert_eq!(mul_div(50, 100, 50), Some(100));
        
        // Test edge case: result is zero
        assert_eq!(mul_div(1, 1, i128::MAX as i128), Some(0));
    }

    #[test]
    fn test_fixed_mul_div_integration() {
        // Test that Fixed operations correctly use mul_div
        let a = Fixed::from_int(2).unwrap();
        let b = Fixed::from_int(3).unwrap();
        let result = a.mul(b).unwrap();
        assert_eq!(result.to_int(), 6);
        
        // Test division
        let c = Fixed::from_int(6).unwrap();
        let d = Fixed::from_int(2).unwrap();
        let result = c.div(d).unwrap();
        assert_eq!(result.to_int(), 3);
    }

    #[test]
    fn test_mul_div_u128_overflow_protection() {
        // Test that mul_div_u128 correctly handles overflow
        let a = u128::MAX;
        let b = u128::MAX;
        let d = 2u128;
        
        let (result, overflow) = mul_div_u128(a, b, d);
        // Should not overflow since we're dividing by 2
        assert!(!overflow);
        assert!(result > 0);
    }
}
