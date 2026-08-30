#![no_std]

use soroban_sdk::{contract, contractimpl, symbol_short, vec, Env, Symbol, Vec};

// A minimal Soroban contract used as a clean reference template.
// The contract stores no state and simply responds with a greeting vector.
#[contract]
pub struct HelloContract;

#[contractimpl]
impl HelloContract {
    // Every public method in a Soroban contract must receive an `Env` first.
    // This example accepts a symbol and returns a greeting with that symbol.
    pub fn hello(env: Env, to: Symbol) -> Vec<Symbol> {
        // Build the response explicitly so the structure is easy to follow.
        vec![&env, symbol_short!("Hello"), to]
    }
}

mod test;
