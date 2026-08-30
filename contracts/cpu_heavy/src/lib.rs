#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, Env, Vec};

#[cfg(test)]
mod test;

/// Errors returned by the `CpuHeavyContract` benchmark entry points.
///
/// Every entry point validates its inputs against a hard cap *before* doing any
/// work, so an oversized request fails with one of these codes instead of
/// running until the host aborts the invocation with
/// `Error(Budget, ExceededLimit)`. A budget abort surfaces to the caller as an
/// opaque `UnreachableCodeReached` VM trap and says nothing about which
/// argument was unreasonable; these codes do.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// `n` exceeded [`MAX_FIB`].
    FibonacciInputTooLarge = 1,
    /// The input list was longer than [`MAX_SORT`].
    SortInputTooLarge = 2,
    /// `limit` exceeded [`MAX_PRIME`].
    PrimeLimitTooLarge = 3,
    /// `outer * inner` exceeded [`MAX_LOOP_OPS`].
    LoopOpsTooLarge = 4,
    /// One of the combined-benchmark arguments exceeded its sub-cap.
    CombinedInputTooLarge = 5,
}

// Hard caps. These sit below the point where a benchmark would exhaust the
// default network budget (100_000_000 CPU instructions / 41_943_040 memory
// bytes), measured against the release WASM build:
//
//   fibonacci_iterative(50_000)    1.6M CPU              (pure arithmetic loop)
//   bubble_sort(100)              60.0M CPU / 10.2MB     (host Vec get/set)
//   count_primes(20_000)          22.7M CPU
//
// `bubble_sort` is quadratic in *host* calls, so its cap is by far the
// tightest: at 150 elements it costs 136.8M CPU and blows the budget outright,
// which is why the previous cap of 300 was never reachable in practice.

/// Largest `n` accepted by [`CpuHeavyContract::fibonacci_iterative`].
pub const MAX_FIB: u32 = 50_000;
/// Longest list accepted by [`CpuHeavyContract::bubble_sort`].
pub const MAX_SORT: u32 = 100;
/// Largest `limit` accepted by [`CpuHeavyContract::count_primes`].
pub const MAX_PRIME: u32 = 20_000;
/// Largest `outer * inner` product accepted by
/// [`CpuHeavyContract::nested_loop_burn`].
pub const MAX_LOOP_OPS: u32 = 500_000;

// Sub-caps for `combined_benchmark`, which pays for all three workloads in one
// invocation and so has to stay well inside each individual cap.
/// Largest `fib_n` accepted by [`CpuHeavyContract::combined_benchmark`].
pub const MAX_COMBINED_FIB: u32 = 10_000;
/// Largest `sort_size` accepted by [`CpuHeavyContract::combined_benchmark`].
pub const MAX_COMBINED_SORT: u32 = 50;
/// Largest `prime_limit` accepted by [`CpuHeavyContract::combined_benchmark`].
pub const MAX_COMBINED_PRIME: u32 = 5_000;

#[contract]
pub struct CpuHeavyContract;

#[contractimpl]
impl CpuHeavyContract {
    /// Iterative Fibonacci, wrapping on `u64` overflow.
    ///
    /// Returns [`Error::FibonacciInputTooLarge`] if `n` exceeds [`MAX_FIB`].
    pub fn fibonacci_iterative(_env: Env, n: u32) -> Result<u64, Error> {
        if n > MAX_FIB {
            return Err(Error::FibonacciInputTooLarge);
        }

        let mut a: u64 = 0;
        let mut b: u64 = 1;
        for _ in 0..n {
            let temp = a.wrapping_add(b);
            a = b;
            b = temp;
        }
        Ok(a)
    }

    /// Bubble-sorts a host `Vec` in place, exercising O(n²) host calls.
    ///
    /// Returns [`Error::SortInputTooLarge`] if the list is longer than
    /// [`MAX_SORT`].
    pub fn bubble_sort(_env: Env, values: Vec<u32>) -> Result<Vec<u32>, Error> {
        if values.len() > MAX_SORT {
            return Err(Error::SortInputTooLarge);
        }

        let mut arr = values;
        let n = arr.len();
        for i in 0..n {
            // `n - i - 1` would underflow on the last pass, and on an empty
            // list there is nothing to compare at all.
            for j in 0..n.saturating_sub(i + 1) {
                let val_j = arr.get(j).unwrap();
                let val_next = arr.get(j + 1).unwrap();
                if val_j > val_next {
                    arr.set(j, val_next);
                    arr.set(j + 1, val_j);
                }
            }
        }
        Ok(arr)
    }

    /// Counts primes in `2..=limit` by trial division.
    ///
    /// Returns [`Error::PrimeLimitTooLarge`] if `limit` exceeds [`MAX_PRIME`].
    pub fn count_primes(_env: Env, limit: u32) -> Result<u32, Error> {
        if limit > MAX_PRIME {
            return Err(Error::PrimeLimitTooLarge);
        }

        let mut count = 0;
        for num in 2..=limit {
            let mut is_prime = true;
            let mut i = 2;
            // `MAX_PRIME` is far below `sqrt(u32::MAX)`, so `i * i` cannot
            // overflow for any accepted `limit`.
            while i * i <= num {
                if num % i == 0 {
                    is_prime = false;
                    break;
                }
                i += 1;
            }
            if is_prime {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Burns `outer * inner` iterations of trivial arithmetic.
    ///
    /// Returns [`Error::LoopOpsTooLarge`] if the product exceeds
    /// [`MAX_LOOP_OPS`]. The product uses `saturating_mul` so a caller cannot
    /// slip past the guard by overflowing it.
    pub fn nested_loop_burn(_env: Env, outer: u32, inner: u32) -> Result<u64, Error> {
        if outer.saturating_mul(inner) > MAX_LOOP_OPS {
            return Err(Error::LoopOpsTooLarge);
        }

        let mut sum: u64 = 0;
        for i in 0..outer {
            for j in 0..inner {
                sum = sum.wrapping_add(i as u64).wrapping_add(j as u64);
            }
        }
        Ok(sum)
    }

    /// Runs all three workloads in one invocation and returns
    /// `[fibonacci, prime_count]`.
    ///
    /// Returns [`Error::CombinedInputTooLarge`] if any argument exceeds its
    /// sub-cap.
    pub fn combined_benchmark(
        env: Env,
        fib_n: u32,
        sort_size: u32,
        prime_limit: u32,
    ) -> Result<Vec<u64>, Error> {
        if fib_n > MAX_COMBINED_FIB
            || sort_size > MAX_COMBINED_SORT
            || prime_limit > MAX_COMBINED_PRIME
        {
            return Err(Error::CombinedInputTooLarge);
        }

        let mut results = Vec::new(&env);
        results.push_back(Self::fibonacci_iterative(env.clone(), fib_n)?);

        let mut to_sort = Vec::new(&env);
        for i in (0..sort_size).rev() {
            to_sort.push_back(i);
        }
        Self::bubble_sort(env.clone(), to_sort)?;

        results.push_back(Self::count_primes(env.clone(), prime_limit)? as u64);

        Ok(results)
    }
}
