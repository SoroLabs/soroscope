#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Env};

pub use soroscope_error_codes::ContractError as MathError;

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct Fixed(pub i128);

pub const SCALE: i128 = 1_000_000_000_000_000_000; // 18 decimals
pub const LN2: i128 = 693_147_180_559_945_309; // ln(2) * SCALE

/// WAD — 10^18, the standard 18-decimal fixed-point unit (same as `SCALE`).
pub const WAD: i128 = SCALE;
/// RAY — 10^27, the high-precision 27-decimal fixed-point unit.
pub const RAY: i128 = 1_000_000_000_000_000_000_000_000_000;

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

/// Multiply two WAD fixed-point numbers: `(a * b) / WAD`.
/// The intermediate product is computed with full 256-bit precision, so
/// results are exact up to the final truncation. Overflow returns
/// `MathError::Overflow` instead of panicking.
pub fn wad_mul(a: i128, b: i128) -> Result<i128, MathError> {
    mul_div(a, b, WAD).ok_or(MathError::Overflow)
}

/// Divide two WAD fixed-point numbers: `(a * WAD) / b`.
/// A zero divisor returns `MathError::DivisionByZero`.
pub fn wad_div(a: i128, b: i128) -> Result<i128, MathError> {
    if b == 0 {
        return Err(MathError::DivisionByZero);
    }
    mul_div(a, WAD, b).ok_or(MathError::Overflow)
}

/// Multiply two RAY fixed-point numbers: `(a * b) / RAY`.
pub fn ray_mul(a: i128, b: i128) -> Result<i128, MathError> {
    mul_div(a, b, RAY).ok_or(MathError::Overflow)
}

/// Divide two RAY fixed-point numbers: `(a * RAY) / b`.
/// A zero divisor returns `MathError::DivisionByZero`.
pub fn ray_div(a: i128, b: i128) -> Result<i128, MathError> {
    if b == 0 {
        return Err(MathError::DivisionByZero);
    }
    mul_div(a, RAY, b).ok_or(MathError::Overflow)
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

fn mul_div_u128(a: u128, b: u128, d: u128) -> (u128, bool) {
    match mul_div_parts(a, b, d) {
        Some((quotient, _)) => (quotient, false),
        None => (0, true),
    }
}

/// Compute `a * b` as a full 256-bit value, returned as `(high, low)`.
fn mul_256(a: u128, b: u128) -> (u128, u128) {
    let a_low = a & u64::MAX as u128;
    let a_high = a >> 64;
    let b_low = b & u64::MAX as u128;
    let b_high = b >> 64;

    let p0 = a_low * b_low;
    let p1 = a_low * b_high;
    let p2 = a_high * b_low;
    let p3 = a_high * b_high;

    let mid = (p1 & u64::MAX as u128) + (p2 & u64::MAX as u128) + (p0 >> 64);
    let high = p3 + (p1 >> 64) + (p2 >> 64) + (mid >> 64);
    let low = (mid << 64) | (p0 & u64::MAX as u128);
    (high, low)
}

/// Divide the full 256-bit product `a * b` by `d`, returning `(quotient, remainder)`.
/// Returns `None` when the divisor is zero or the quotient does not fit in `u128`.
fn mul_div_parts(a: u128, b: u128, d: u128) -> Option<(u128, u128)> {
    if d == 0 {
        return None;
    }
    let (high, low) = mul_256(a, b);
    if high >= d {
        return None;
    }

    let mut quotient = 0u128;
    let mut remainder = high;
    for i in (0..128).rev() {
        let carry = remainder >> 127;
        remainder = (remainder << 1) | ((low >> i) & 1);
        if carry != 0 || remainder >= d {
            remainder = remainder.wrapping_sub(d);
            quotient |= 1u128 << i;
        }
    }
    Some((quotient, remainder))
}

fn signed_from_magnitude(magnitude: u128, negative: bool) -> Result<i128, MathError> {
    if negative {
        if magnitude == (i128::MAX as u128) + 1 {
            Ok(i128::MIN)
        } else if magnitude <= i128::MAX as u128 {
            Ok(-(magnitude as i128))
        } else {
            Err(MathError::Overflow)
        }
    } else if magnitude <= i128::MAX as u128 {
        Ok(magnitude as i128)
    } else {
        Err(MathError::Overflow)
    }
}

/// Compute `(x * y) / denominator` with a full 256-bit intermediate product.
///
/// The product is never narrowed to `i128`, so values above 10^19 stroops do
/// not suffer phantom overflow. The result truncates toward zero. A zero
/// denominator returns `MathError::DivisionByZero`; a result outside the
/// `i128` range returns `MathError::Overflow`.
pub fn mul_div_down(x: i128, y: i128, denominator: i128) -> Result<i128, MathError> {
    let negative = (x < 0) ^ (y < 0) ^ (denominator < 0);
    let parts = mul_div_parts(
        x.unsigned_abs(),
        y.unsigned_abs(),
        denominator.unsigned_abs(),
    )
    .ok_or(if denominator == 0 {
        MathError::DivisionByZero
    } else {
        MathError::Overflow
    })?;
    signed_from_magnitude(parts.0, negative)
}

/// Compute `ceil((x * y) / denominator)` with a full 256-bit intermediate.
///
/// Any non-zero remainder rounds the magnitude up. For a negative result the
/// truncation toward zero is already the ceiling, so no adjustment is made.
pub fn mul_div_up(x: i128, y: i128, denominator: i128) -> Result<i128, MathError> {
    let negative = (x < 0) ^ (y < 0) ^ (denominator < 0);
    let parts = mul_div_parts(
        x.unsigned_abs(),
        y.unsigned_abs(),
        denominator.unsigned_abs(),
    )
    .ok_or(if denominator == 0 {
        MathError::DivisionByZero
    } else {
        MathError::Overflow
    })?;
    let (mut quotient, remainder) = parts;
    if remainder != 0 && !negative {
        quotient = quotient.checked_add(1).ok_or(MathError::Overflow)?;
    }
    signed_from_magnitude(quotient, negative)
}

/// Deterministic integer square root via bounded Newton-Raphson iteration.
///
/// Returns `floor(sqrt(val))`. Negative inputs return `MathError::InvalidInput`.
pub fn sqrt(val: i128) -> Result<i128, MathError> {
    if val < 0 {
        return Err(MathError::InvalidInput);
    }
    if val == 0 {
        return Ok(0);
    }

    let n = val as u128;
    let bits = 128 - n.leading_zeros();
    let mut x = 1u128 << bits.div_ceil(2);

    for _ in 0..128 {
        let next = (x + n / x) >> 1;
        if next >= x {
            break;
        }
        x = next;
    }
    while x > 0 && x > n / x {
        x -= 1;
    }
    Ok(x as i128)
}

/// Raise a WAD fixed-point `base` to an integer `exp` using bounded binary
/// exponentiation. Each multiplication keeps a 256-bit intermediate via
/// [`mul_div_down`], so compounding does not overflow on large balances.
pub fn pow_wad(base: i128, exp: u32) -> Result<i128, MathError> {
    if exp == 0 {
        return Ok(WAD);
    }
    if base == 0 {
        return Ok(0);
    }

    let negative = base < 0 && exp % 2 == 1;
    let mut factor = base.unsigned_abs();
    let mut remaining = exp;
    let mut result = WAD as u128;

    while remaining > 0 {
        if remaining & 1 == 1 {
            result = mul_div_parts(result, factor, WAD as u128)
                .ok_or(MathError::Overflow)?
                .0;
        }
        remaining >>= 1;
        if remaining > 0 {
            factor = mul_div_parts(factor, factor, WAD as u128)
                .ok_or(MathError::Overflow)?
                .0;
        }
    }

    signed_from_magnitude(result, negative)
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
    pub fn wad_mul(_e: Env, a: i128, b: i128) -> Result<i128, MathError> {
        crate::wad_mul(a, b)
    }
    pub fn wad_div(_e: Env, a: i128, b: i128) -> Result<i128, MathError> {
        crate::wad_div(a, b)
    }
    pub fn ray_mul(_e: Env, a: i128, b: i128) -> Result<i128, MathError> {
        crate::ray_mul(a, b)
    }
    pub fn ray_div(_e: Env, a: i128, b: i128) -> Result<i128, MathError> {
        crate::ray_div(a, b)
    }
    pub fn mul_div_down(_e: Env, x: i128, y: i128, denominator: i128) -> Result<i128, MathError> {
        crate::mul_div_down(x, y, denominator)
    }
    pub fn mul_div_up(_e: Env, x: i128, y: i128, denominator: i128) -> Result<i128, MathError> {
        crate::mul_div_up(x, y, denominator)
    }
    pub fn sqrt(_e: Env, val: i128) -> Result<i128, MathError> {
        crate::sqrt(val)
    }
    pub fn pow_wad(_e: Env, base: i128, exp: u32) -> Result<i128, MathError> {
        crate::pow_wad(base, exp)
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
    fn test_wad_ray_constants() {
        assert_eq!(WAD, SCALE);
        assert_eq!(WAD, 1_000_000_000_000_000_000);
        assert_eq!(RAY, 1_000_000_000_000_000_000_000_000_000);
        let wad_pow = 10_i128.pow(18);
        let ray_pow = 10_i128.pow(27);
        assert_eq!(WAD, wad_pow);
        assert_eq!(RAY, ray_pow);
        assert!(ray_pow > wad_pow);
    }

    #[test]
    fn test_wad_mul() {
        assert_eq!(wad_mul(0, 0).unwrap(), 0);
        assert_eq!(wad_mul(0, 5 * WAD).unwrap(), 0);
        assert_eq!(wad_mul(5 * WAD, 0).unwrap(), 0);
        assert_eq!(wad_mul(WAD, WAD).unwrap(), WAD);
        assert_eq!(wad_mul(2 * WAD, 3 * WAD).unwrap(), 6 * WAD);
        assert_eq!(wad_mul(WAD / 2, WAD / 2).unwrap(), WAD / 4);
        assert_eq!(wad_mul(-2 * WAD, 3 * WAD).unwrap(), -6 * WAD);
        assert_eq!(wad_mul(2 * WAD, -3 * WAD).unwrap(), -6 * WAD);
        assert_eq!(wad_mul(-2 * WAD, -3 * WAD).unwrap(), 6 * WAD);
    }

    #[test]
    fn test_wad_div() {
        assert_eq!(wad_div(WAD, 2 * WAD).unwrap(), WAD / 2);
        assert_eq!(wad_div(-WAD, 2 * WAD).unwrap(), -(WAD / 2));
        assert_eq!(wad_div(10 * WAD, 2 * WAD).unwrap(), 5 * WAD);
        assert_eq!(wad_div(42 * WAD, 6 * WAD).unwrap(), 7 * WAD);
        assert_eq!(wad_div(0, 7 * WAD).unwrap(), 0);
        assert_eq!(wad_div(WAD / 4, WAD / 2).unwrap(), WAD / 2);
        assert_eq!(wad_div(-42 * WAD, -6 * WAD).unwrap(), 7 * WAD);
    }

    #[test]
    fn test_ray_mul_div() {
        assert_eq!(ray_mul(RAY, RAY).unwrap(), RAY);
        assert_eq!(ray_mul(2 * RAY, 3 * RAY).unwrap(), 6 * RAY);
        assert_eq!(ray_mul(-2 * RAY, 3 * RAY).unwrap(), -6 * RAY);
        assert_eq!(ray_mul(100 * RAY, 100 * RAY).unwrap(), 10_000 * RAY);
        assert_eq!(ray_div(RAY, 2 * RAY).unwrap(), RAY / 2);
        assert_eq!(
            ray_div(10 * RAY, 4 * RAY).unwrap(),
            2_500_000_000_000_000_000_000_000_000
        );
        assert_eq!(ray_div(0, 7 * RAY).unwrap(), 0);
    }

    #[test]
    fn test_zero_precision_loss_on_division() {
        // Exact divisions round-trip bit-for-bit: (a / b) * b == a.
        for x in [0i128, 1, 42, 10_000, i128::MAX / WAD] {
            for y in [1, 2, 4, 8, 100] {
                let q = wad_div(x * WAD, y * WAD).unwrap();
                assert_eq!(wad_mul(q, y * WAD).unwrap(), x * WAD);
            }
        }
        for x in [0i128, 1, 123_456] {
            for y in [1, 2, 4, 8, 100] {
                let q = ray_div(x * RAY, y * RAY).unwrap();
                assert_eq!(ray_mul(q, y * RAY).unwrap(), x * RAY);
            }
        }

        // RAY keeps ~9 extra decimal digits: scaling a RAY quotient down to
        // WAD restores the WAD quotient exactly.
        let wad_third = wad_div(WAD, 3 * WAD).unwrap();
        let ray_third = ray_div(RAY, 3 * RAY).unwrap();
        assert_eq!(wad_third, ray_third / 1_000_000_000);
    }

    #[test]
    fn test_div_by_zero() {
        assert_eq!(wad_div(1, 0), Err(MathError::DivisionByZero));
        assert_eq!(wad_div(0, 0), Err(MathError::DivisionByZero));
        assert_eq!(ray_div(1, 0), Err(MathError::DivisionByZero));
        assert_eq!(ray_div(0, 0), Err(MathError::DivisionByZero));
    }

    #[test]
    fn test_overflow_boundaries() {
        // Intermediate products that exceed the i128 range return errors
        // instead of panicking, even when computed through the 256-bit path.
        assert_eq!(wad_mul(i128::MAX, WAD).unwrap(), i128::MAX);
        assert_eq!(wad_mul(i128::MAX, 2 * WAD), Err(MathError::Overflow));
        assert_eq!(wad_mul(i128::MIN, WAD), Err(MathError::Overflow));
        assert_eq!(ray_mul(i128::MAX, 2 * RAY), Err(MathError::Overflow));
        assert_eq!(wad_div(i128::MAX, 1), Err(MathError::Overflow));
        assert_eq!(ray_div(i128::MAX, 1), Err(MathError::Overflow));
        assert_eq!(wad_div(i128::MAX, 2 * WAD).unwrap(), i128::MAX / 2);
        assert_eq!(ray_div(i128::MAX, 2 * RAY).unwrap(), i128::MAX / 2);
    }

    #[test]
    fn test_high_precision_large_numerators() {
        // Divisor-scaled division keeps full 256-bit numerator precision,
        // whereas a naive `a * WAD` in i128 would overflow.
        assert_eq!(wad_div(WAD, 7 * WAD).unwrap(), 142_857_142_857_142_857);
        assert_eq!(wad_div(i128::MAX, 100 * WAD).unwrap(), i128::MAX / 100);
        assert_eq!(
            ray_div(RAY, 7 * RAY).unwrap(),
            142_857_142_857_142_857_142_857_142
        );
    }

    #[test]
    fn test_contract_methods() {
        let env = Env::default();
        assert_eq!(Math::wad_mul(env.clone(), 2 * WAD, 3 * WAD), Ok(6 * WAD));
        assert_eq!(Math::wad_div(env.clone(), WAD, 4 * WAD), Ok(WAD / 4));
        assert_eq!(Math::ray_mul(env.clone(), 2 * RAY, 3 * RAY), Ok(6 * RAY));
        assert_eq!(Math::ray_div(env.clone(), RAY, 4 * RAY), Ok(RAY / 4));
        assert_eq!(
            Math::wad_div(env.clone(), 1, 0),
            Err(MathError::DivisionByZero)
        );
        assert_eq!(
            Math::mul_div_down(env.clone(), i128::MAX, i128::MAX, i128::MAX),
            Ok(i128::MAX)
        );
        assert_eq!(Math::mul_div_up(env.clone(), 1, 1, 2), Ok(1));
        assert_eq!(
            Math::sqrt(env.clone(), i128::MAX),
            Ok(13_043_817_825_332_782_212)
        );
        assert_eq!(Math::pow_wad(env.clone(), 2 * WAD, 2), Ok(4 * WAD));
    }

    #[test]
    fn test_mul_div_down_no_phantom_overflow() {
        let half = i128::MAX / 2;
        assert_eq!(mul_div_down(half, 2, half), Ok(2));
        assert_eq!(mul_div_down(half, half, half), Ok(half));
        assert_eq!(mul_div_down(i128::MAX, i128::MAX, i128::MAX), Ok(i128::MAX));
        assert_eq!(mul_div_down(1, 1, 2), Ok(0));
        assert_eq!(mul_div_down(0, i128::MAX, i128::MAX), Ok(0));
        assert_eq!(mul_div_down(half, half, 2 * half), Ok(half / 2));
    }

    #[test]
    fn test_mul_div_up_rounds_up_on_nonzero_remainder() {
        assert_eq!(mul_div_up(1, 1, 2), Ok(1));
        assert_eq!(mul_div_down(1, 1, 2), Ok(0));
        assert_eq!(mul_div_up(10, 1, 3), Ok(4));
        assert_eq!(mul_div_down(10, 1, 3), Ok(3));
        assert_eq!(mul_div_up(i128::MAX, 1, 2), Ok(i128::MAX / 2 + 1));
        let half = i128::MAX / 2;
        assert_eq!(mul_div_up(half, half, half - 1), Ok(half + 2));
        assert_eq!(mul_div_up(6, 7, 2), Ok(21));
    }

    #[test]
    fn test_mul_div_signed_and_error_boundaries() {
        assert_eq!(mul_div_down(-1, 3, 2), Ok(-1));
        assert_eq!(mul_div_up(-1, 3, 2), Ok(-1));
        assert_eq!(mul_div_up(-10, 1, 3), Ok(-3));
        assert_eq!(mul_div_down(1, -3, -2), Ok(1));
        assert_eq!(mul_div_down(i128::MIN, 1, 1), Ok(i128::MIN));
        assert_eq!(mul_div_down(i128::MIN, 1, -1), Err(MathError::Overflow));
        assert_eq!(mul_div_down(1, 1, 0), Err(MathError::DivisionByZero));
        assert_eq!(mul_div_up(1, 1, 0), Err(MathError::DivisionByZero));
    }

    #[test]
    fn test_sqrt_boundaries() {
        assert_eq!(sqrt(0), Ok(0));
        assert_eq!(sqrt(1), Ok(1));
        assert_eq!(sqrt(2), Ok(1));
        assert_eq!(sqrt(3), Ok(1));
        assert_eq!(sqrt(4), Ok(2));
        assert_eq!(sqrt(99), Ok(9));
        assert_eq!(sqrt(100), Ok(10));
        assert_eq!(sqrt(i128::MAX / 2), Ok(9_223_372_036_854_775_807));
        assert_eq!(sqrt(i128::MAX), Ok(13_043_817_825_332_782_212));
        assert_eq!(sqrt(-1), Err(MathError::InvalidInput));
        assert_eq!(sqrt(i128::MIN), Err(MathError::InvalidInput));

        for value in 0..1000i128 {
            let root = sqrt(value).unwrap();
            assert!(root * root <= value);
            assert!((root + 1) * (root + 1) > value);
        }
    }

    #[test]
    fn test_pow_wad_bounded_exponentiation() {
        assert_eq!(pow_wad(WAD, 0), Ok(WAD));
        assert_eq!(pow_wad(0, 3), Ok(0));
        assert_eq!(pow_wad(WAD, 7), Ok(WAD));
        assert_eq!(pow_wad(2 * WAD, 2), Ok(4 * WAD));
        assert_eq!(pow_wad(11 * WAD / 10, 2), Ok(121 * WAD / 100));
        assert_eq!(pow_wad(-2 * WAD, 3), Ok(-8 * WAD));
        assert_eq!(pow_wad(-2 * WAD, 2), Ok(4 * WAD));
        assert_eq!(pow_wad(i128::MAX, 2), Err(MathError::Overflow));
    }
}
