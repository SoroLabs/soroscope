use soroban_sdk::{contracttype, Address, String};

#[derive(Clone)]
#[contracttype]
pub struct AllowanceDataKey {
    pub from: Address,
    pub spender: Address,
}

#[derive(Clone)]
#[contracttype]
pub struct AllowanceValue {
    pub amount: i128,
    pub expiration_ledger: u32,
}

/// Token metadata grouped into a single instance-storage entry.
/// Replaces 3 separate DataKey variants: Name, Symbol, Decimals.
#[derive(Clone)]
#[contracttype]
pub struct TokenMetadata {
    pub name: String,
    pub symbol: String,
    pub decimals: u32,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Allowance(AllowanceDataKey),
    Balance(Address),
    Admin,
    State(Address),
    /// Single instance key replacing Name + Symbol + Decimals.
    Metadata,
}
