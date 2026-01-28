use soroban_sdk::{contracttype, Address};

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

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Allowance(AllowanceDataKey),
    Balance(Address),
    Admin,
    State(Address), // User state (Deauthorized, etc - though standard token usually just has balance/allowance/admin/metadata)
    // Metadata keys
    Name,
    Symbol,
    Decimals,
}
