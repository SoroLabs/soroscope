use soroban_sdk::contracterror;

/// Errors raised by the token contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u32)]
pub enum Error {
    /// Minting would push the total supply above the configured `max_supply`.
    SupplyCapExceeded = 1,
    /// The sender or the recipient account has been deauthorized (frozen).
    AccountFrozen = 2,
    /// The account does not hold enough tokens for the clawback.
    InsufficientBalance = 3,
}
