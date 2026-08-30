use crate::admin::{has_administrator, read_administrator, write_administrator};
use crate::balance::{read_balance, receive_balance, spend_balance};
use crate::metadata::{
    read_decimal, read_name, read_symbol, write_decimal, write_name, write_symbol,
};
use soroban_sdk::{contract, contractimpl, Address, Env, String};

pub trait SoulboundTokenTrait {
    fn initialize(e: Env, admin: Address, decimal: u32, name: String, symbol: String);
    fn mint(e: Env, to: Address);
    fn set_admin(e: Env, new_admin: Address);
    fn balance(e: Env, id: Address) -> i128;
    fn transfer(e: Env, from: Address, to: Address, amount: i128);
    fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128);
    fn burn(e: Env, from: Address);
    fn admin_transfer(e: Env, from: Address, to: Address);
    fn decimals(e: Env) -> u32;
    fn name(e: Env) -> String;
    fn symbol(e: Env) -> String;
}

#[contract]
pub struct SoulboundToken;

#[contractimpl]
impl SoulboundTokenTrait for SoulboundToken {
    fn initialize(e: Env, admin: Address, decimal: u32, name: String, symbol: String) {
        if has_administrator(&e) {
            panic!("already initialized");
        }
        write_administrator(&e, &admin);
        write_decimal(&e, decimal);
        write_name(&e, &name);
        write_symbol(&e, &symbol);
    }

    fn mint(e: Env, to: Address) {
        let admin = read_administrator(&e);
        admin.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        receive_balance(&e, to, 1);
    }

    fn set_admin(e: Env, new_admin: Address) {
        let admin = read_administrator(&e);
        admin.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        write_administrator(&e, &new_admin);
    }

    fn balance(e: Env, id: Address) -> i128 {
        e.storage().instance().extend_ttl(100, 100);
        read_balance(&e, id)
    }

    fn transfer(e: Env, _from: Address, _to: Address, _amount: i128) {
        panic!("soulbound tokens cannot be transferred");
    }

    fn transfer_from(e: Env, _spender: Address, _from: Address, _to: Address, _amount: i128) {
        panic!("soulbound tokens cannot be transferred");
    }

    fn burn(e: Env, from: Address) {
        from.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        spend_balance(&e, from, 1);
    }

    fn admin_transfer(e: Env, from: Address, to: Address) {
        let admin = read_administrator(&e);
        admin.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        spend_balance(&e, from, 1);
        receive_balance(&e, to, 1);
    }

    fn decimals(e: Env) -> u32 {
        read_decimal(&e)
    }

    fn name(e: Env) -> String {
        read_name(&e)
    }

    fn symbol(e: Env) -> String {
        read_symbol(&e)
    }
}