#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String};

mod math;
use math::*;

#[cfg(test)]
mod test;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidTickRange = 3,
    InsufficientAmount = 4,
    SlippageExceeded = 5,
    InvalidFee = 6,
    MathError = 7,
}

impl From<MathError> for Error {
    fn from(_: MathError) -> Self {
        Error::MathError
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TickInfo {
    pub liquidity_gross: u128,
    pub liquidity_net: i128,
    pub fee_growth_outside_0: u128,
    pub fee_growth_outside_1: u128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionInfo {
    pub liquidity: u128,
    pub fee_growth_inside_0_last: u128,
    pub fee_growth_inside_1_last: u128,
    pub tokens_owed_0: u128,
    pub tokens_owed_1: u128,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    TokenA,
    TokenB,
    FeeBps,
    TickSpacing,
    CurrentTick,
    CurrentSqrtPrice,
    Liquidity,
    FeeGrowthGlobal0,
    FeeGrowthGlobal1,
    Tick(i32),
    TickBitmap(i32),
    Position(Address, i32, i32), // Owner, TickLower, TickUpper
}

#[contract]
pub struct ConcentratedAmm;

#[contractimpl]
impl ConcentratedAmm {
    pub fn initialize(
        e: Env,
        token_a: Address,
        token_b: Address,
        fee_bps: u32,
        tick_spacing: i32,
        initial_sqrt_price_x64: u128,
        initial_tick: i32,
    ) -> Result<(), Error> {
        if e.storage().instance().has(&DataKey::TokenA) {
            return Err(Error::AlreadyInitialized);
        }
        
        e.storage().instance().set(&DataKey::TokenA, &token_a);
        e.storage().instance().set(&DataKey::TokenB, &token_b);
        e.storage().instance().set(&DataKey::FeeBps, &fee_bps);
        e.storage().instance().set(&DataKey::TickSpacing, &tick_spacing);
        e.storage().instance().set(&DataKey::CurrentTick, &initial_tick);
        e.storage().instance().set(&DataKey::CurrentSqrtPrice, &initial_sqrt_price_x64);
        e.storage().instance().set(&DataKey::Liquidity, &0u128);
        e.storage().instance().set(&DataKey::FeeGrowthGlobal0, &0u128);
        e.storage().instance().set(&DataKey::FeeGrowthGlobal1, &0u128);
        
        Ok(())
    }

    pub fn mint(
        e: Env,
        to: Address,
        tick_lower: i32,
        tick_upper: i32,
        amount_a_desired: u128,
        amount_b_desired: u128,
    ) -> Result<(u128, u128, u128), Error> {
        to.require_auth();

        let tick_spacing: i32 = e.storage().instance().get(&DataKey::TickSpacing).ok_or(Error::NotInitialized)?;
        if tick_lower >= tick_upper || tick_lower % tick_spacing != 0 || tick_upper % tick_spacing != 0 {
            return Err(Error::InvalidTickRange);
        }

        let current_tick: i32 = e.storage().instance().get(&DataKey::CurrentTick).unwrap();
        let current_sqrt_price: u128 = e.storage().instance().get(&DataKey::CurrentSqrtPrice).unwrap();

        let sqrt_ratio_ax64 = get_sqrt_ratio_at_tick(tick_lower)?;
        let sqrt_ratio_bx64 = get_sqrt_ratio_at_tick(tick_upper)?;

        // Use sqrt-price comparisons (mirrors Uniswap V3) so boundary ticks (e.g. tick_lower ==
        // current_tick) don't cause division by zero in the diff_b denominator.
        let liquidity: u128;
        let amount_a: u128;
        let amount_b: u128;

        if current_sqrt_price <= sqrt_ratio_ax64 {
            // Current price is at or below the range: only token A is needed.
            let num1 = mul_div_u128(amount_a_desired, sqrt_ratio_bx64, Q64)?;
            let num2 = mul_div_u128(num1, sqrt_ratio_ax64, Q64)?;
            let den = sqrt_ratio_bx64.checked_sub(sqrt_ratio_ax64).ok_or(Error::MathError)?;
            liquidity = mul_div_u128(num2, Q64, den)?;
            amount_a = amount_a_desired;
            amount_b = 0;
        } else if current_sqrt_price >= sqrt_ratio_bx64 {
            // Current price is at or above the range: only token B is needed.
            let diff = sqrt_ratio_bx64.checked_sub(sqrt_ratio_ax64).ok_or(Error::MathError)?;
            liquidity = mul_div_u128(amount_b_desired, Q64, diff)?;
            amount_a = 0;
            amount_b = amount_b_desired;
        } else {
            // Current price is inside the range: both tokens are needed.
            let num1 = mul_div_u128(amount_a_desired, sqrt_ratio_bx64, Q64)?;
            let num2 = mul_div_u128(num1, current_sqrt_price, Q64)?;
            let den_a = sqrt_ratio_bx64.checked_sub(current_sqrt_price).ok_or(Error::MathError)?;
            let liq_a = mul_div_u128(num2, Q64, den_a)?;

            let diff_b = current_sqrt_price.checked_sub(sqrt_ratio_ax64).ok_or(Error::MathError)?;
            let liq_b = mul_div_u128(amount_b_desired, Q64, diff_b)?;

            liquidity = if liq_a < liq_b { liq_a } else { liq_b };
            amount_a = get_amount_0_delta(current_sqrt_price, sqrt_ratio_bx64, liquidity)?;
            amount_b = get_amount_1_delta(sqrt_ratio_ax64, current_sqrt_price, liquidity)?;
        }

        if liquidity == 0 {
            return Err(Error::InsufficientAmount);
        }

        let token_a: Address = e.storage().instance().get(&DataKey::TokenA).unwrap();
        let token_b: Address = e.storage().instance().get(&DataKey::TokenB).unwrap();
        
        if amount_a > 0 {
            let client_a = soroban_sdk::token::Client::new(&e, &token_a);
            client_a.transfer(&to, &e.current_contract_address(), &(amount_a as i128));
        }
        if amount_b > 0 {
            let client_b = soroban_sdk::token::Client::new(&e, &token_b);
            client_b.transfer(&to, &e.current_contract_address(), &(amount_b as i128));
        }

        let fg0: u128 = e.storage().instance().get(&DataKey::FeeGrowthGlobal0).unwrap();
        let fg1: u128 = e.storage().instance().get(&DataKey::FeeGrowthGlobal1).unwrap();

        let flipped_lower = Self::update_tick(&e, tick_lower, current_tick, liquidity as i128, fg0, fg1, false);
        let flipped_upper = Self::update_tick(&e, tick_upper, current_tick, liquidity as i128, fg0, fg1, true);

        if flipped_lower { Self::flip_tick_bitmap(&e, tick_lower, tick_spacing); }
        if flipped_upper { Self::flip_tick_bitmap(&e, tick_upper, tick_spacing); }

        let (fee_growth_inside_0, fee_growth_inside_1) = Self::get_fee_growth_inside(&e, tick_lower, tick_upper, current_tick, fg0, fg1);

        let pos_key = DataKey::Position(to.clone(), tick_lower, tick_upper);
        let mut pos: PositionInfo = e.storage().persistent().get(&pos_key).unwrap_or(PositionInfo { 
            liquidity: 0, fee_growth_inside_0_last: fee_growth_inside_0, fee_growth_inside_1_last: fee_growth_inside_1, tokens_owed_0: 0, tokens_owed_1: 0 
        });

        // Update tokens owed
        pos.tokens_owed_0 += mul_div_u128(pos.liquidity, fee_growth_inside_0.wrapping_sub(pos.fee_growth_inside_0_last), Q64).unwrap_or(0);
        pos.tokens_owed_1 += mul_div_u128(pos.liquidity, fee_growth_inside_1.wrapping_sub(pos.fee_growth_inside_1_last), Q64).unwrap_or(0);
        
        pos.liquidity += liquidity;
        pos.fee_growth_inside_0_last = fee_growth_inside_0;
        pos.fee_growth_inside_1_last = fee_growth_inside_1;

        e.storage().persistent().set(&pos_key, &pos);
        e.storage().persistent().extend_ttl(&pos_key, 100, 100);

        if current_tick >= tick_lower && current_tick < tick_upper {
            let current_liq: u128 = e.storage().instance().get(&DataKey::Liquidity).unwrap_or(0);
            e.storage().instance().set(&DataKey::Liquidity, &(current_liq + liquidity));
        }

        Ok((liquidity, amount_a, amount_b))
    }

    pub fn burn(
        e: Env,
        from: Address,
        tick_lower: i32,
        tick_upper: i32,
        liquidity: u128,
    ) -> Result<(u128, u128), Error> {
        from.require_auth();

        let pos_key = DataKey::Position(from.clone(), tick_lower, tick_upper);
        let mut pos: PositionInfo = e.storage().persistent().get(&pos_key).ok_or(Error::InsufficientAmount)?;
        
        if pos.liquidity < liquidity {
            return Err(Error::InsufficientAmount);
        }

        let current_tick: i32 = e.storage().instance().get(&DataKey::CurrentTick).unwrap();
        let current_sqrt_price: u128 = e.storage().instance().get(&DataKey::CurrentSqrtPrice).unwrap();
        let fg0: u128 = e.storage().instance().get(&DataKey::FeeGrowthGlobal0).unwrap();
        let fg1: u128 = e.storage().instance().get(&DataKey::FeeGrowthGlobal1).unwrap();
        
        let sqrt_ratio_ax64 = get_sqrt_ratio_at_tick(tick_lower)?;
        let sqrt_ratio_bx64 = get_sqrt_ratio_at_tick(tick_upper)?;

        let mut amount_a: u128 = 0;
        let mut amount_b: u128 = 0;

        if current_tick < tick_lower {
            amount_a = get_amount_0_delta(sqrt_ratio_ax64, sqrt_ratio_bx64, liquidity)?;
        } else if current_tick >= tick_upper {
            amount_b = get_amount_1_delta(sqrt_ratio_ax64, sqrt_ratio_bx64, liquidity)?;
        } else {
            amount_a = get_amount_0_delta(current_sqrt_price, sqrt_ratio_bx64, liquidity)?;
            amount_b = get_amount_1_delta(sqrt_ratio_ax64, current_sqrt_price, liquidity)?;
        }

        let (fee_growth_inside_0, fee_growth_inside_1) = Self::get_fee_growth_inside(&e, tick_lower, tick_upper, current_tick, fg0, fg1);

        pos.tokens_owed_0 += mul_div_u128(pos.liquidity, fee_growth_inside_0.wrapping_sub(pos.fee_growth_inside_0_last), Q64).unwrap_or(0);
        pos.tokens_owed_1 += mul_div_u128(pos.liquidity, fee_growth_inside_1.wrapping_sub(pos.fee_growth_inside_1_last), Q64).unwrap_or(0);

        pos.liquidity -= liquidity;
        pos.fee_growth_inside_0_last = fee_growth_inside_0;
        pos.fee_growth_inside_1_last = fee_growth_inside_1;

        if pos.liquidity == 0 && pos.tokens_owed_0 == 0 && pos.tokens_owed_1 == 0 {
            e.storage().persistent().remove(&pos_key);
        } else {
            e.storage().persistent().set(&pos_key, &pos);
            e.storage().persistent().extend_ttl(&pos_key, 100, 100);
        }

        let tick_spacing: i32 = e.storage().instance().get(&DataKey::TickSpacing).unwrap();
        let flipped_lower = Self::update_tick(&e, tick_lower, current_tick, -(liquidity as i128), fg0, fg1, false);
        let flipped_upper = Self::update_tick(&e, tick_upper, current_tick, -(liquidity as i128), fg0, fg1, true);

        if flipped_lower { Self::flip_tick_bitmap(&e, tick_lower, tick_spacing); }
        if flipped_upper { Self::flip_tick_bitmap(&e, tick_upper, tick_spacing); }

        if current_tick >= tick_lower && current_tick < tick_upper {
            let current_liq: u128 = e.storage().instance().get(&DataKey::Liquidity).unwrap_or(0);
            e.storage().instance().set(&DataKey::Liquidity, &(current_liq - liquidity));
        }

        let token_a: Address = e.storage().instance().get(&DataKey::TokenA).unwrap();
        let token_b: Address = e.storage().instance().get(&DataKey::TokenB).unwrap();
        
        if amount_a > 0 {
            let client_a = soroban_sdk::token::Client::new(&e, &token_a);
            client_a.transfer(&e.current_contract_address(), &from, &(amount_a as i128));
        }
        if amount_b > 0 {
            let client_b = soroban_sdk::token::Client::new(&e, &token_b);
            client_b.transfer(&e.current_contract_address(), &from, &(amount_b as i128));
        }

        Ok((amount_a, amount_b))
    }

    pub fn collect_fees(
        e: Env,
        from: Address,
        tick_lower: i32,
        tick_upper: i32,
    ) -> Result<(u128, u128), Error> {
        from.require_auth();

        let pos_key = DataKey::Position(from.clone(), tick_lower, tick_upper);
        let mut pos: PositionInfo = e.storage().persistent().get(&pos_key).ok_or(Error::InsufficientAmount)?;

        let current_tick: i32 = e.storage().instance().get(&DataKey::CurrentTick).unwrap();
        let fg0: u128 = e.storage().instance().get(&DataKey::FeeGrowthGlobal0).unwrap();
        let fg1: u128 = e.storage().instance().get(&DataKey::FeeGrowthGlobal1).unwrap();
        
        let (fee_growth_inside_0, fee_growth_inside_1) = Self::get_fee_growth_inside(&e, tick_lower, tick_upper, current_tick, fg0, fg1);

        pos.tokens_owed_0 += mul_div_u128(pos.liquidity, fee_growth_inside_0.wrapping_sub(pos.fee_growth_inside_0_last), Q64).unwrap_or(0);
        pos.tokens_owed_1 += mul_div_u128(pos.liquidity, fee_growth_inside_1.wrapping_sub(pos.fee_growth_inside_1_last), Q64).unwrap_or(0);

        let amount_0 = pos.tokens_owed_0;
        let amount_1 = pos.tokens_owed_1;

        pos.tokens_owed_0 = 0;
        pos.tokens_owed_1 = 0;
        pos.fee_growth_inside_0_last = fee_growth_inside_0;
        pos.fee_growth_inside_1_last = fee_growth_inside_1;

        if pos.liquidity == 0 {
            e.storage().persistent().remove(&pos_key);
        } else {
            e.storage().persistent().set(&pos_key, &pos);
            e.storage().persistent().extend_ttl(&pos_key, 100, 100);
        }

        let token_a: Address = e.storage().instance().get(&DataKey::TokenA).unwrap();
        let token_b: Address = e.storage().instance().get(&DataKey::TokenB).unwrap();
        
        if amount_0 > 0 {
            let client_a = soroban_sdk::token::Client::new(&e, &token_a);
            client_a.transfer(&e.current_contract_address(), &from, &(amount_0 as i128));
        }
        if amount_1 > 0 {
            let client_b = soroban_sdk::token::Client::new(&e, &token_b);
            client_b.transfer(&e.current_contract_address(), &from, &(amount_1 as i128));
        }

        Ok((amount_0, amount_1))
    }

    pub fn swap(
        e: Env,
        to: Address,
        zero_for_one: bool,
        amount_specified: u128,
    ) -> Result<(u128, u128), Error> {
        to.require_auth();

        let token_a: Address = e.storage().instance().get(&DataKey::TokenA).ok_or(Error::NotInitialized)?;
        let token_b: Address = e.storage().instance().get(&DataKey::TokenB).ok_or(Error::NotInitialized)?;
        let fee_bps: u32 = e.storage().instance().get(&DataKey::FeeBps).unwrap();
        let tick_spacing: i32 = e.storage().instance().get(&DataKey::TickSpacing).unwrap();
        
        let mut current_sqrt_price: u128 = e.storage().instance().get(&DataKey::CurrentSqrtPrice).unwrap();
        let mut current_tick: i32 = e.storage().instance().get(&DataKey::CurrentTick).unwrap();
        let mut liquidity: u128 = e.storage().instance().get(&DataKey::Liquidity).unwrap();
        let mut fg0: u128 = e.storage().instance().get(&DataKey::FeeGrowthGlobal0).unwrap();
        let mut fg1: u128 = e.storage().instance().get(&DataKey::FeeGrowthGlobal1).unwrap();

        let mut amount_remaining = amount_specified;
        let mut amount_in_total: u128 = 0;
        let mut amount_out_total: u128 = 0;

        let token_in = if zero_for_one { &token_a } else { &token_b };
        let token_out = if zero_for_one { &token_b } else { &token_a };
        
        let client_in = soroban_sdk::token::Client::new(&e, token_in);
        let client_out = soroban_sdk::token::Client::new(&e, token_out);

        while amount_remaining > 0 {
            if liquidity == 0 {
                // No liquidity at current price; advance to next initialized tick or stop.
                let (next_tick, initialized) = Self::next_initialized_tick_within_one_word(&e, current_tick, tick_spacing, zero_for_one);
                if !initialized {
                    break;
                }
                // Cross the empty tick to activate its liquidity, then continue.
                let tick_key = DataKey::Tick(next_tick);
                if let Some(mut tick_info) = e.storage().persistent().get::<_, TickInfo>(&tick_key) {
                    tick_info.fee_growth_outside_0 = fg0.wrapping_sub(tick_info.fee_growth_outside_0);
                    tick_info.fee_growth_outside_1 = fg1.wrapping_sub(tick_info.fee_growth_outside_1);
                    e.storage().persistent().set(&tick_key, &tick_info);
                    let net = if zero_for_one { -tick_info.liquidity_net } else { tick_info.liquidity_net };
                    if net < 0 {
                        liquidity = liquidity.saturating_sub((-net) as u128);
                    } else {
                        liquidity = liquidity.saturating_add(net as u128);
                    }
                }
                current_tick = if zero_for_one { next_tick - 1 } else { next_tick };
                current_sqrt_price = get_sqrt_ratio_at_tick(current_tick)?;
                continue;
            }

            let (next_tick, initialized) = Self::next_initialized_tick_within_one_word(&e, current_tick, tick_spacing, zero_for_one);
            let sqrt_price_next = get_sqrt_ratio_at_tick(next_tick)?;

            let amount_remaining_less_fee = mul_div_u128(amount_remaining, 10000 - (fee_bps as u128), 10000)?;

            let max_amount_in = if zero_for_one {
                get_amount_0_delta(current_sqrt_price, sqrt_price_next, liquidity).unwrap_or(u128::MAX)
            } else {
                get_amount_1_delta(current_sqrt_price, sqrt_price_next, liquidity).unwrap_or(u128::MAX)
            };

            let crossed = amount_remaining_less_fee >= max_amount_in && initialized;
            let amount_in_step = if crossed { max_amount_in } else { amount_remaining_less_fee };
            
            let amount_out_step = if crossed {
                if zero_for_one {
                    get_amount_1_delta(current_sqrt_price, sqrt_price_next, liquidity)?
                } else {
                    get_amount_0_delta(current_sqrt_price, sqrt_price_next, liquidity)?
                }
            } else {
                let next_p = get_next_sqrt_price_from_input(current_sqrt_price, liquidity, amount_in_step, zero_for_one)?;
                if zero_for_one {
                    get_amount_1_delta(current_sqrt_price, next_p, liquidity)?
                } else {
                    get_amount_0_delta(current_sqrt_price, next_p, liquidity)?
                }
            };

            // Calculate exact fee collected during this step
            let fee_amount = if crossed {
                mul_div_u128(amount_in_step, fee_bps as u128, 10000 - (fee_bps as u128))?
            } else {
                amount_remaining - amount_remaining_less_fee
            };

            if liquidity > 0 {
                let fee_growth = mul_div_u128(fee_amount, Q64, liquidity)?;
                if zero_for_one {
                    fg0 = fg0.wrapping_add(fee_growth);
                } else {
                    fg1 = fg1.wrapping_add(fee_growth);
                }
            }

            amount_in_total += amount_in_step + fee_amount;
            amount_out_total += amount_out_step;
            amount_remaining = amount_remaining.saturating_sub(amount_in_step + fee_amount);

            if crossed {
                current_sqrt_price = sqrt_price_next;
                current_tick = if zero_for_one { next_tick - 1 } else { next_tick };

                let mut tick_info: TickInfo = e.storage().persistent().get(&DataKey::Tick(next_tick)).unwrap();
                tick_info.fee_growth_outside_0 = fg0.wrapping_sub(tick_info.fee_growth_outside_0);
                tick_info.fee_growth_outside_1 = fg1.wrapping_sub(tick_info.fee_growth_outside_1);
                e.storage().persistent().set(&DataKey::Tick(next_tick), &tick_info);

                let net = if zero_for_one { -tick_info.liquidity_net } else { tick_info.liquidity_net };
                if net < 0 {
                    liquidity = liquidity.saturating_sub((-net) as u128);
                } else {
                    liquidity = liquidity.saturating_add(net as u128);
                }
            } else {
                current_sqrt_price = get_next_sqrt_price_from_input(current_sqrt_price, liquidity, amount_in_step, zero_for_one)?;
            }
        }

        e.storage().instance().set(&DataKey::CurrentSqrtPrice, &current_sqrt_price);
        e.storage().instance().set(&DataKey::CurrentTick, &current_tick);
        e.storage().instance().set(&DataKey::Liquidity, &liquidity);
        e.storage().instance().set(&DataKey::FeeGrowthGlobal0, &fg0);
        e.storage().instance().set(&DataKey::FeeGrowthGlobal1, &fg1);

        client_in.transfer(&to, &e.current_contract_address(), &(amount_in_total as i128));
        client_out.transfer(&e.current_contract_address(), &to, &(amount_out_total as i128));

        Ok((amount_in_total, amount_out_total))
    }

    fn update_tick(e: &Env, tick: i32, current_tick: i32, liquidity_delta: i128, fg0: u128, fg1: u128, is_upper: bool) -> bool {
        let tick_key = DataKey::Tick(tick);
        let mut info: TickInfo = e.storage().persistent().get(&tick_key).unwrap_or(TickInfo { 
            liquidity_gross: 0, liquidity_net: 0, fee_growth_outside_0: 0, fee_growth_outside_1: 0 
        });
        
        let liquidity_gross_before = info.liquidity_gross;
        
        if liquidity_gross_before == 0 {
            if tick <= current_tick {
                info.fee_growth_outside_0 = fg0;
                info.fee_growth_outside_1 = fg1;
            }
        }

        let delta = if is_upper { -liquidity_delta } else { liquidity_delta };
        info.liquidity_net += delta;
        if liquidity_delta < 0 {
            info.liquidity_gross = info.liquidity_gross.saturating_sub((-liquidity_delta) as u128);
        } else {
            info.liquidity_gross = info.liquidity_gross.saturating_add(liquidity_delta as u128);
        }

        let flipped = (liquidity_gross_before == 0 && info.liquidity_gross > 0) || (liquidity_gross_before > 0 && info.liquidity_gross == 0);

        if info.liquidity_gross == 0 {
            e.storage().persistent().remove(&tick_key);
        } else {
            e.storage().persistent().set(&tick_key, &info);
            e.storage().persistent().extend_ttl(&tick_key, 100, 100);
        }
        
        flipped
    }

    fn flip_tick_bitmap(e: &Env, tick: i32, tick_spacing: i32) {
        let compressed = tick / tick_spacing;
        let word_pos = compressed >> 6;
        let bit_pos = (compressed & 0x3F) as u64;
        let mask = 1u64 << bit_pos;

        let key = DataKey::TickBitmap(word_pos);
        let mut word: u64 = e.storage().persistent().get(&key).unwrap_or(0);
        word ^= mask;
        
        if word == 0 {
            e.storage().persistent().remove(&key);
        } else {
            e.storage().persistent().set(&key, &word);
            e.storage().persistent().extend_ttl(&key, 100, 100);
        }
    }

    fn next_initialized_tick_within_one_word(e: &Env, tick: i32, tick_spacing: i32, lte: bool) -> (i32, bool) {
        let compressed = if tick < 0 && tick % tick_spacing != 0 {
            (tick / tick_spacing) - 1
        } else {
            tick / tick_spacing
        };

        if lte {
            let word_pos = compressed >> 6;
            let bit_pos = (compressed & 0x3F) as u32;
            let mask = (1u64 << bit_pos) - 1 + (1u64 << bit_pos);
            let word: u64 = e.storage().persistent().get(&DataKey::TickBitmap(word_pos)).unwrap_or(0);
            let masked = word & mask;

            if masked != 0 {
                let bit = 63 - masked.leading_zeros();
                ((word_pos * 64 + bit as i32) * tick_spacing, true)
            } else {
                ((word_pos * 64) * tick_spacing, false)
            }
        } else {
            let compressed_next = compressed + 1;
            let word_pos = compressed_next >> 6;
            let bit_pos = (compressed_next & 0x3F) as u32;
            let mask = !((1u64 << bit_pos) - 1);
            let word: u64 = e.storage().persistent().get(&DataKey::TickBitmap(word_pos)).unwrap_or(0);
            let masked = word & mask;

            if masked != 0 {
                let bit = masked.trailing_zeros();
                ((word_pos * 64 + bit as i32) * tick_spacing, true)
            } else {
                (((word_pos + 1) * 64 - 1) * tick_spacing, false)
            }
        }
    }

    fn get_fee_growth_inside(
        e: &Env, 
        tick_lower: i32, 
        tick_upper: i32, 
        tick_current: i32, 
        fg0: u128, 
        fg1: u128
    ) -> (u128, u128) {
        let lower_info: TickInfo = e.storage().persistent().get(&DataKey::Tick(tick_lower)).unwrap_or(TickInfo { liquidity_gross: 0, liquidity_net: 0, fee_growth_outside_0: 0, fee_growth_outside_1: 0 });
        let upper_info: TickInfo = e.storage().persistent().get(&DataKey::Tick(tick_upper)).unwrap_or(TickInfo { liquidity_gross: 0, liquidity_net: 0, fee_growth_outside_0: 0, fee_growth_outside_1: 0 });

        let (fee_below_0, fee_below_1) = if tick_current >= tick_lower {
            (lower_info.fee_growth_outside_0, lower_info.fee_growth_outside_1)
        } else {
            (fg0.wrapping_sub(lower_info.fee_growth_outside_0), fg1.wrapping_sub(lower_info.fee_growth_outside_1))
        };

        let (fee_above_0, fee_above_1) = if tick_current < tick_upper {
            (upper_info.fee_growth_outside_0, upper_info.fee_growth_outside_1)
        } else {
            (fg0.wrapping_sub(upper_info.fee_growth_outside_0), fg1.wrapping_sub(upper_info.fee_growth_outside_1))
        };

        (
            fg0.wrapping_sub(fee_below_0).wrapping_sub(fee_above_0),
            fg1.wrapping_sub(fee_below_1).wrapping_sub(fee_above_1),
        )
    }
}
