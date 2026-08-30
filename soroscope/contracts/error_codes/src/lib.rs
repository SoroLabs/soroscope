#![no_std]
use soroban_sdk::contracterror;

/// Unified error codes shared across all SoroScope contracts.
///
/// Each variant maps to a stable `u32` discriminant so that the SoroScope UI
/// can decode errors consistently regardless of which contract emitted them.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    // ── Initialisation ──────────────────────────────────────────────────────
    AlreadyInitialized = 1,
    NotInitialized = 2,

    // ── Authorisation ───────────────────────────────────────────────────────
    Unauthorized = 3,

    // ── Balances & liquidity ─────────────────────────────────────────────────
    InsufficientBalance = 4,
    InsufficientLiquidity = 5,
    InsufficientShares = 6,
    InsufficientAllowance = 7,

    // ── Swap / pricing ───────────────────────────────────────────────────────
    SlippageExceeded = 8,

    // ── Fee management ───────────────────────────────────────────────────────
    InvalidFee = 9,
    NoPendingFeeUpdate = 10,
    TimelockNotElapsed = 11,

    // ── Oracle ───────────────────────────────────────────────────────────────
    OracleNotConfigured = 12,
    InvalidOraclePrice = 13,

    // ── Circuit-breaker ──────────────────────────────────────────────────────
    Paused = 14,

    // ── Math ─────────────────────────────────────────────────────────────────
    Overflow = 15,
    DivisionByZero = 16,
    InvalidInput = 17,
}

impl ContractError {
    /// Convert the error variant into its explicit numerical u32 discriminant.
    pub fn as_u32(&self) -> u32 {
        *self as u32
    }

    /// Try to construct a `ContractError` from a numerical u32 discriminant.
    pub fn from_u32(code: u32) -> Option<Self> {
        match code {
            1 => Some(Self::AlreadyInitialized),
            2 => Some(Self::NotInitialized),
            3 => Some(Self::Unauthorized),
            4 => Some(Self::InsufficientBalance),
            5 => Some(Self::InsufficientLiquidity),
            6 => Some(Self::InsufficientShares),
            7 => Some(Self::InsufficientAllowance),
            8 => Some(Self::SlippageExceeded),
            9 => Some(Self::InvalidFee),
            10 => Some(Self::NoPendingFeeUpdate),
            11 => Some(Self::TimelockNotElapsed),
            12 => Some(Self::OracleNotConfigured),
            13 => Some(Self::InvalidOraclePrice),
            14 => Some(Self::Paused),
            15 => Some(Self::Overflow),
            16 => Some(Self::DivisionByZero),
            17 => Some(Self::InvalidInput),
            _ => None,
        }
    }
}

/// JSON schema specification export for cross-language error decoding.
pub const ERROR_SCHEMA_JSON: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "ContractError",
  "type": "object",
  "description": "Explicit numerical discriminant mappings for SoroScope smart contract error codes.",
  "error_codes": [
    { "name": "AlreadyInitialized", "code": 1, "description": "Contract has already been initialized." },
    { "name": "NotInitialized", "code": 2, "description": "Contract has not been initialized yet." },
    { "name": "Unauthorized", "code": 3, "description": "Caller is not authorized to perform this operation." },
    { "name": "InsufficientBalance", "code": 4, "description": "Account balance is insufficient for requested transaction." },
    { "name": "InsufficientLiquidity", "code": 5, "description": "Liquidity pool reserve is insufficient." },
    { "name": "InsufficientShares", "code": 6, "description": "LP share balance is insufficient." },
    { "name": "InsufficientAllowance", "code": 7, "description": "Approved token allowance is insufficient." },
    { "name": "SlippageExceeded", "code": 8, "description": "Slippage tolerance was exceeded during swap." },
    { "name": "InvalidFee", "code": 9, "description": "Fee parameter exceeds allowed threshold." },
    { "name": "NoPendingFeeUpdate", "code": 10, "description": "No pending fee update was found." },
    { "name": "TimelockNotElapsed", "code": 11, "description": "Timelock period has not yet elapsed." },
    { "name": "OracleNotConfigured", "code": 12, "description": "Price oracle is not configured." },
    { "name": "InvalidOraclePrice", "code": 13, "description": "Price returned by oracle is invalid or stale." },
    { "name": "Paused", "code": 14, "description": "Contract execution is currently paused by emergency guard." },
    { "name": "Overflow", "code": 15, "description": "Arithmetic overflow occurred." },
    { "name": "DivisionByZero", "code": 16, "description": "Attempted division by zero." },
    { "name": "InvalidInput", "code": 17, "description": "Input argument provided is invalid." }
  ]
}"#;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_explicit_numerical_discriminants() {
        assert_eq!(ContractError::AlreadyInitialized.as_u32(), 1);
        assert_eq!(ContractError::NotInitialized.as_u32(), 2);
        assert_eq!(ContractError::Unauthorized.as_u32(), 3);
        assert_eq!(ContractError::InsufficientBalance.as_u32(), 4);
        assert_eq!(ContractError::InsufficientLiquidity.as_u32(), 5);
        assert_eq!(ContractError::InsufficientShares.as_u32(), 6);
        assert_eq!(ContractError::InsufficientAllowance.as_u32(), 7);
        assert_eq!(ContractError::SlippageExceeded.as_u32(), 8);
        assert_eq!(ContractError::InvalidFee.as_u32(), 9);
        assert_eq!(ContractError::NoPendingFeeUpdate.as_u32(), 10);
        assert_eq!(ContractError::TimelockNotElapsed.as_u32(), 11);
        assert_eq!(ContractError::OracleNotConfigured.as_u32(), 12);
        assert_eq!(ContractError::InvalidOraclePrice.as_u32(), 13);
        assert_eq!(ContractError::Paused.as_u32(), 14);
        assert_eq!(ContractError::Overflow.as_u32(), 15);
        assert_eq!(ContractError::DivisionByZero.as_u32(), 16);
        assert_eq!(ContractError::InvalidInput.as_u32(), 17);
    }

    #[test]
    fn test_from_u32_conversion() {
        for code in 1..=17 {
            let err = ContractError::from_u32(code).expect("Valid discriminant should convert");
            assert_eq!(err.as_u32(), code);
        }
        assert_eq!(ContractError::from_u32(0), None);
        assert_eq!(ContractError::from_u32(18), None);
    }

    #[test]
    fn test_schema_json_contains_all_variants() {
        assert!(ERROR_SCHEMA_JSON.contains("AlreadyInitialized"));
        assert!(ERROR_SCHEMA_JSON.contains("InvalidInput"));
        assert!(ERROR_SCHEMA_JSON.contains("\"code\": 1"));
        assert!(ERROR_SCHEMA_JSON.contains("\"code\": 17"));
    }
}
