#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Vec};

#[contract]
pub struct CpuHeavyContract;

// Constants to keep execution deterministic and bounded
const MAX_FIB: u32 = 50_000;
const MAX_SORT: u32 = 300;
const MAX_PRIME: u32 = 20_000;
const MAX_LOOP_OPS: u32 = 500_000;

#[contractimpl]
impl CpuHeavyContract {
    
    pub fn fibonacci_iterative(_env: Env, n: u32) -> u64 {
        if n > MAX_FIB { panic!("input too large"); } 
        
        let mut a: u64 = 0;
        let mut b: u64 = 1;
        for _ in 0..n {
            let temp = a.wrapping_add(b);
            a = b;
            b = temp;
        }
        a
    }

    pub fn bubble_sort(_env: Env, values: Vec<u32>) -> Vec<u32> {
        if values.len() > MAX_SORT { panic!("list too long"); } 
        
        let mut arr = values;
        let n = arr.len();
        for i in 0..n {
            for j in 0..n - i - 1 {
                let val_j = arr.get(j).unwrap();
                let val_next = arr.get(j + 1).unwrap();
                if val_j > val_next {
                    arr.set(j, val_next);
                    arr.set(j + 1, val_j);
                }
            }
        }
        arr
    }

    pub fn count_primes(_env: Env, limit: u32) -> u32 {
        if limit > MAX_PRIME { panic!("limit too large"); } 
        
        let mut count = 0;
        for num in 2..=limit {
            let mut is_prime = true;
            let mut i = 2;
            while i * i <= num {
                if num % i == 0 {
                    is_prime = false;
                    break;
                }
                i += 1;
            }
            if is_prime { count += 1; }
        }
        count
    }

    pub fn nested_loop_burn(_env: Env, outer: u32, inner: u32) -> u64 {
        if outer.checked_mul(inner).unwrap_or(u32::MAX) > MAX_LOOP_OPS {
            panic!("total ops too large");
        }

        let mut sum: u64 = 0;
        for i in 0..outer {
            for j in 0..inner {
                sum = sum.wrapping_add(i as u64).wrapping_add(j as u64);
            }
        }
        sum
    }

    pub fn combined_benchmark(
        env: Env, 
        fib_n: u32, 
        sort_size: u32, 
        prime_limit: u32
    ) -> Vec<u64> {
        if fib_n > 10_000 || sort_size > 100 || prime_limit > 5_000 {
            panic!("combined inputs too large");
        }

        let mut results = Vec::new(&env);
        results.push_back(Self::fibonacci_iterative(env.clone(), fib_n));
        
        let mut to_sort = Vec::new(&env);
        for i in (0..sort_size).rev() {
            to_sort.push_back(i);
        }
        Self::bubble_sort(env.clone(), to_sort);
        
        results.push_back(Self::count_primes(env.clone(), prime_limit) as u64);
        
        results
    }
}

mod test;