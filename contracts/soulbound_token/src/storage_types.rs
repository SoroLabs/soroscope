use soroban_sdk::{contracttype, Address};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Balance(Address),
    Admin,
    Name,
    Symbol,
    Decimals,
    IdentityMetadata(Address),
    NextTokenId,
    OwnerToken(Address),
    TokenOwner(u64),
    TokenIssuer(u64),
    TokenRevoked(u64),
}
