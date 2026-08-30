use soroban_sdk::{contracttype, Address};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Balance(Address),
    Admin,
    // Metadata keys
    Name,
    Symbol,
    Decimals,
}