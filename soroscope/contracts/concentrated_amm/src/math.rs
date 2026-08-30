#![no_std]
use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MathError {
    Overflow = 1,
    DivisionByZero = 2,
    InvalidPrice = 3,
}

pub const Q64: u128 = 1 << 64;

pub fn mul_div_u128(a: u128, b: u128, d: u128) -> Result<u128, MathError> {
    if d == 0 { return Err(MathError::DivisionByZero); }
    if let Some(prod) = a.checked_mul(b) { return Ok(prod / d); }
    let a_low = a & 0xFFFFFFFFFFFFFFFF;
    let a_high = a >> 64;
    let b_low = b & 0xFFFFFFFFFFFFFFFF;
    let b_high = b >> 64;
    let p0 = a_low * b_low;
    let p1 = a_low * b_high;
    let p2 = a_high * b_low;
    let p3 = a_high * b_high;
    let mut mid = (p1 & 0xFFFFFFFFFFFFFFFF) + (p2 & 0xFFFFFFFFFFFFFFFF) + (p0 >> 64);
    let high = p3 + (p1 >> 64) + (p2 >> 64) + (mid >> 64);
    let low = (mid << 64) | (p0 & 0xFFFFFFFFFFFFFFFF);
    if high >= d { return Err(MathError::Overflow); }
    let mut quotient = 0u128;
    let mut remainder = high;
    for i in (0..128).rev() {
        remainder = (remainder << 1) | ((low >> i) & 1);
        if remainder >= d { remainder -= d; quotient |= 1 << i; }
    }
    Ok(quotient)
}

/// Sqrt Price X64 mapping:
/// We represent price as Q64.64. tick is integer. 
/// Price = 1.0001 ^ tick. SqrtPrice = 1.0001 ^ (tick / 2).
/// We approximate it for the sake of this task to avoid heavy exponentiation
/// if not needed, or implement a basic version.
pub fn get_sqrt_ratio_at_tick(tick: i32) -> Result<u128, MathError> {
    let abs_tick = tick.abs() as u32;
    // 1.00005 in Q64 = 18447669818844880896
    let base: u128 = 18447669818844880896; 
    let mut ratio: u128 = Q64;
    let mut current_base = base;
    let mut t = abs_tick;
    
    while t > 0 {
        if t % 2 == 1 {
            ratio = mul_div_u128(ratio, current_base, Q64)?;
        }
        current_base = mul_div_u128(current_base, current_base, Q64)?;
        t /= 2;
    }
    
    if tick < 0 {
        ratio = mul_div_u128(Q64, Q64, ratio)?;
    }
    
    Ok(ratio)
}

/// Computes the amount of token 0 (token A) for a given liquidity and price range
/// delta_amount0 = liquidity * (sqrt(upper) - sqrt(lower)) / (sqrt(upper) * sqrt(lower))
pub fn get_amount_0_delta(
    sqrt_ratio_ax64: u128,
    sqrt_ratio_bx64: u128,
    liquidity: u128,
) -> Result<u128, MathError> {
    let (lower, upper) = if sqrt_ratio_ax64 < sqrt_ratio_bx64 {
        (sqrt_ratio_ax64, sqrt_ratio_bx64)
    } else {
        (sqrt_ratio_bx64, sqrt_ratio_ax64)
    };
    
    // num1 = liquidity << 64
    // num2 = upper - lower
    // den = upper * lower
    // Actually, amount0 = (liquidity * (upper - lower)) / upper / lower * 2^64
    let num1 = mul_div_u128(liquidity, Q64, upper)?;
    let num2 = upper.checked_sub(lower).ok_or(MathError::Overflow)?;
    let amount0 = mul_div_u128(num1, num2, lower)?;
    Ok(amount0)
}

/// Computes the amount of token 1 (token B) for a given liquidity and price range
/// delta_amount1 = liquidity * (sqrt(upper) - sqrt(lower))
pub fn get_amount_1_delta(
    sqrt_ratio_ax64: u128,
    sqrt_ratio_bx64: u128,
    liquidity: u128,
) -> Result<u128, MathError> {
    let (lower, upper) = if sqrt_ratio_ax64 < sqrt_ratio_bx64 {
        (sqrt_ratio_ax64, sqrt_ratio_bx64)
    } else {
        (sqrt_ratio_bx64, sqrt_ratio_ax64)
    };
    
    let diff = upper.checked_sub(lower).ok_or(MathError::Overflow)?;
    mul_div_u128(liquidity, diff, Q64)
}

pub fn get_next_sqrt_price_from_input(
    sqrt_px64: u128,
    liquidity: u128,
    amount_in: u128,
    zero_for_one: bool,
) -> Result<u128, MathError> {
    if amount_in == 0 { return Ok(sqrt_px64); }
    
    if zero_for_one {
        // next_P_q64 = sqrt_px64 * L / (L + Δx * sqrt_P_real)
        // where sqrt_P_real = sqrt_px64 / Q64
        let p_real_times_delta = mul_div_u128(amount_in, sqrt_px64, Q64)?;
        let den = liquidity.checked_add(p_real_times_delta).ok_or(MathError::Overflow)?;
        mul_div_u128(sqrt_px64, liquidity, den)
    } else {
        // next_P = sqrt_P + (amount_in / L)
        let delta = mul_div_u128(amount_in, Q64, liquidity)?;
        sqrt_px64.checked_add(delta).ok_or(MathError::Overflow)
    }
}
