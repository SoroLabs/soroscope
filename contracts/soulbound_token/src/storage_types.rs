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
}
