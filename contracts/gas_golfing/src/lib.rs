#![no_std]

use soroban_sdk::{contract, contractimpl, Env};

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
}
