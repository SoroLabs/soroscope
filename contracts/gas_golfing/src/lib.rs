#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum GasGolfingError {
    PositionNotFound = 1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnpackedPosition {
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub fee_bps: u16,
    pub flags: u16,
    pub timestamp: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    UnpackedPos(Address),
    BitpackedPos(Address),
}

const MASK_16: u128 = 0xFFFF;
const MASK_32: u128 = 0xFFFF_FFFF;

const SHIFT_TICK_LOWER: u32 = 0;
const SHIFT_TICK_UPPER: u32 = 32;
const SHIFT_FEE_BPS: u32 = 64;
const SHIFT_FLAGS: u32 = 80;
const SHIFT_TIMESTAMP: u32 = 96;

/// Packs `(tick_lower, tick_upper, fee_bps, flags, timestamp)` into a single `u128`.
///
/// Bit layout:
/// - `[0..32)`   : `tick_lower` (`i32` as `u32`)
/// - `[32..64)`  : `tick_upper` (`i32` as `u32`)
/// - `[64..80)`  : `fee_bps` (`u16`)
/// - `[80..96)`  : `flags` (`u16`)
/// - `[96..128)` : `timestamp` (`u32`)
pub fn pack_position(
    tick_lower: i32,
    tick_upper: i32,
    fee_bps: u16,
    flags: u16,
    timestamp: u32,
) -> u128 {
    let lo = (tick_lower as u32) as u128;
    let hi = (tick_upper as u32) as u128;
    let fee = fee_bps as u128;
    let flg = flags as u128;
    let ts = timestamp as u128;

    (lo << SHIFT_TICK_LOWER)
        | (hi << SHIFT_TICK_UPPER)
        | (fee << SHIFT_FEE_BPS)
        | (flg << SHIFT_FLAGS)
        | (ts << SHIFT_TIMESTAMP)
}

/// Unpacks a `u128` bit-packed position into `(tick_lower, tick_upper, fee_bps, flags, timestamp)`.
pub fn unpack_position(packed: u128) -> (i32, i32, u16, u16, u32) {
    let tick_lower = (((packed >> SHIFT_TICK_LOWER) & MASK_32) as u32) as i32;
    let tick_upper = (((packed >> SHIFT_TICK_UPPER) & MASK_32) as u32) as i32;
    let fee_bps = ((packed >> SHIFT_FEE_BPS) & MASK_16) as u16;
    let flags = ((packed >> SHIFT_FLAGS) & MASK_16) as u16;
    let timestamp = ((packed >> SHIFT_TIMESTAMP) & MASK_32) as u32;

    (tick_lower, tick_upper, fee_bps, flags, timestamp)
}

#[contract]
pub struct GasGolfingContract;

#[contractimpl]
impl GasGolfingContract {
    pub fn standard_loop(_env: Env, n: u32) -> u64 {
        let mut accumulator = 0u64;
        for i in 0..n {
            accumulator += i as u64;
        }
        accumulator
    }

    pub fn unrolled_loop(_env: Env, n: u32) -> u64 {
        let mut accumulator = 0u64;
        let mut i = 0u32;

        while n - i >= 4 {
            accumulator += i as u64;
            accumulator += (i + 1) as u64;
            accumulator += (i + 2) as u64;
            accumulator += (i + 3) as u64;
            i += 4;
        }

        while i < n {
            accumulator += i as u64;
            i += 1;
        }

        accumulator
    }

    /// Writes position fields individually inside a `#[contracttype]` struct.
    pub fn write_unpacked_position(
        env: Env,
        owner: Address,
        tick_lower: i32,
        tick_upper: i32,
        fee_bps: u16,
        flags: u16,
        timestamp: u32,
    ) {
        let position = UnpackedPosition {
            tick_lower,
            tick_upper,
            fee_bps,
            flags,
            timestamp,
        };
        env.storage()
            .persistent()
            .set(&DataKey::UnpackedPos(owner), &position);
    }

    /// Packs position fields into a single `u128` and persists it under `DataKey::BitpackedPos`.
    pub fn write_bitpacked_position(
        env: Env,
        owner: Address,
        tick_lower: i32,
        tick_upper: i32,
        fee_bps: u16,
        flags: u16,
        timestamp: u32,
    ) -> u128 {
        let packed = pack_position(tick_lower, tick_upper, fee_bps, flags, timestamp);
        env.storage()
            .persistent()
            .set(&DataKey::BitpackedPos(owner), &packed);
        packed
    }

    /// Reads the raw packed `u128` for `owner`.
    pub fn get_bitpacked_position(env: Env, owner: Address) -> Result<u128, GasGolfingError> {
        env.storage()
            .persistent()
            .get(&DataKey::BitpackedPos(owner))
            .ok_or(GasGolfingError::PositionNotFound)
    }

    /// Reads and unpacks the bit-packed position for `owner` as `(tick_lower, tick_upper, fee_bps, flags, timestamp)`.
    pub fn get_unpacked_from_bitpacked(
        env: Env,
        owner: Address,
    ) -> Result<(i32, i32, u16, u16, u32), GasGolfingError> {
        let packed = Self::get_bitpacked_position(env, owner)?;
        Ok(unpack_position(packed))
    }

    /// Reads the unpacked struct position for `owner`.
    pub fn get_unpacked_position(
        env: Env,
        owner: Address,
    ) -> Result<UnpackedPosition, GasGolfingError> {
        env.storage()
            .persistent()
            .get(&DataKey::UnpackedPos(owner))
            .ok_or(GasGolfingError::PositionNotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_bitpacked_roundtrip_negative_and_positive_ticks_and_timestamps() {
        let env = Env::default();
        let contract_id = env.register(GasGolfingContract, ());
        let client = GasGolfingContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);

        let cases: &[(i32, i32, u16, u16, u32)] = &[
            (-887_272, 887_272, 3_000, 0b0000_0000_0000_0101, 1_700_000_000),
            (i32::MIN, i32::MAX, u16::MAX, u16::MAX, u32::MAX),
            (-1, 1, 0, 0, 0),
            (0, 0, 500, 1, 42),
        ];

        for &(tick_lower, tick_upper, fee_bps, flags, timestamp) in cases {
            let packed = client.write_bitpacked_position(
                &owner,
                &tick_lower,
                &tick_upper,
                &fee_bps,
                &flags,
                &timestamp,
            );
            assert_eq!(
                packed,
                pack_position(tick_lower, tick_upper, fee_bps, flags, timestamp)
            );
            assert_eq!(
                client.get_unpacked_from_bitpacked(&owner),
                (tick_lower, tick_upper, fee_bps, flags, timestamp)
            );
        }
    }

    #[test]
    fn test_bitpacked_consumes_fewer_instructions_and_write_bytes_than_unpacked() {
        let env = Env::default();
        let contract_id = env.register(GasGolfingContract, ());
        let client = GasGolfingContractClient::new(&env, &contract_id);

        let owner_unpacked = Address::generate(&env);
        let owner_packed = Address::generate(&env);

        let tick_lower = -120_400i32;
        let tick_upper = 120_400i32;
        let fee_bps = 2_500u16;
        let flags = 3u16;
        let timestamp = 1_750_000_123u32;

        env.cost_estimate().reset_unlimited();
        client.write_unpacked_position(
            &owner_unpacked,
            &tick_lower,
            &tick_upper,
            &fee_bps,
            &flags,
            &timestamp,
        );
        let unpacked_resources = env.cost_estimate().resources();

        env.cost_estimate().reset_unlimited();
        client.write_bitpacked_position(
            &owner_packed,
            &tick_lower,
            &tick_upper,
            &fee_bps,
            &flags,
            &timestamp,
        );
        let packed_resources = env.cost_estimate().resources();

        assert_eq!(
            client.get_unpacked_from_bitpacked(&owner_packed),
            (tick_lower, tick_upper, fee_bps, flags, timestamp)
        );
        assert!(
            packed_resources.write_bytes < unpacked_resources.write_bytes,
            "expected bitpacked write_bytes ({}) < unpacked write_bytes ({})",
            packed_resources.write_bytes,
            unpacked_resources.write_bytes
        );
        assert!(
            packed_resources.instructions < unpacked_resources.instructions,
            "expected bitpacked instructions ({}) < unpacked instructions ({})",
            packed_resources.instructions,
            unpacked_resources.instructions
        );
    }
}