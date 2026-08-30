use crate::admin::{has_administrator, read_administrator, write_administrator};
use crate::balance::{read_balance, receive_balance, spend_balance};
use crate::metadata::{
    read_decimal, read_identity_metadata, read_name, read_symbol, remove_identity_metadata,
    write_decimal, write_identity_metadata, write_name, write_symbol,
};
use soroban_sdk::{contract, contracterror, contractimpl, Address, Env, String};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    AlreadyMinted = 4,
    TokenNotFound = 5,
}

pub trait SoulboundTokenTrait {
    fn initialize(e: Env, admin: Address, decimal: u32, name: String, symbol: String);
    fn mint(e: Env, to: Address);
    fn mint_identity(e: Env, to: Address, metadata: String);
    fn set_admin(e: Env, new_admin: Address);
    fn balance(e: Env, id: Address) -> i128;
    fn transfer(e: Env, from: Address, to: Address, amount: i128) -> Result<(), Error>;
    fn transfer_from(
        e: Env,
        spender: Address,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), Error>;
    fn burn(e: Env, from: Address);
    fn admin_transfer(e: Env, from: Address, to: Address);
    fn revoke(e: Env, from: Address);
    fn update_metadata(e: Env, to: Address, metadata: String);
    fn identity_metadata(e: Env, id: Address) -> String;
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

    /// Mint a soulbound identity badge bound to `to`. Holders may own at most one.
    fn mint(e: Env, to: Address) {
        let admin = read_administrator(&e);
        admin.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        receive_balance(&e, to.clone(), 1);
        write_identity_metadata(&e, to, &String::from_str(&e, ""));
    }

    /// Mint a soulbound identity badge with issuer-provided metadata (URI or claims).
    fn mint_identity(e: Env, to: Address, metadata: String) {
        let admin = read_administrator(&e);
        admin.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        receive_balance(&e, to.clone(), 1);
        write_identity_metadata(&e, to, &metadata);
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

    /// Soulbound badges are non-transferable.
    fn transfer(_e: Env, _from: Address, _to: Address, _amount: i128) -> Result<(), Error> {
        Err(Error::Unauthorized)
    }

    /// Soulbound badges cannot be moved via allowance either.
    fn transfer_from(
        _e: Env,
        _spender: Address,
        _from: Address,
        _to: Address,
        _amount: i128,
    ) -> Result<(), Error> {
        Err(Error::Unauthorized)
    }

    fn burn(e: Env, from: Address) {
        from.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        spend_balance(&e, from.clone(), 1);
        remove_identity_metadata(&e, from);
    }

    fn admin_transfer(e: Env, from: Address, to: Address) {
        let admin = read_administrator(&e);
        admin.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        let metadata = read_identity_metadata(&e, from.clone());
        spend_balance(&e, from.clone(), 1);
        remove_identity_metadata(&e, from);
        receive_balance(&e, to.clone(), 1);
        write_identity_metadata(&e, to, &metadata);
    }

    /// Issuer revocation: burn the holder's identity badge.
    fn revoke(e: Env, from: Address) {
        let admin = read_administrator(&e);
        admin.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        let balance = read_balance(&e, from.clone());
        if balance == 0 {
            panic!("no token to revoke");
        }

        spend_balance(&e, from.clone(), 1);
        remove_identity_metadata(&e, from);
    }

    /// Issuer updates metadata for an existing identity badge.
    fn update_metadata(e: Env, to: Address, metadata: String) {
        let admin = read_administrator(&e);
        admin.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        if read_balance(&e, to.clone()) == 0 {
            panic!("no token to update");
        }
        write_identity_metadata(&e, to, &metadata);
    }

    fn identity_metadata(e: Env, id: Address) -> String {
        e.storage().instance().extend_ttl(100, 100);
        read_identity_metadata(&e, id)
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
