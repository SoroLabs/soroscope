# Pull Request: [Contract] Implement Custom Error Enums with Explicit Numerical Discriminants

## 🎯 Overview
This PR addresses **Issue #647** by annotating all contract error enums with explicit `#[repr(u32)]` numerical discriminants (e.g. `AlreadyInitialized = 1`) and exporting a standardized JSON schema specification. 

Previously, implicit error discriminant mappings caused cross-language mapping between Rust contracts, the SoroScope web dashboard (Next.js/TypeScript), and external clients to be fragile and prone to decoding mismatches when enums were extended. Explicit numerical discriminants guarantee a stable on-chain ABI and deterministic cross-language error decoding.

## ✨ Features Implemented
- **Explicit `#[repr(u32)]` Discriminants**:
  - Annotated `ContractError` in `contracts/error_codes/src/lib.rs` with explicit numerical discriminants (`1` to `17`).
  - Added conversion utility methods `as_u32(&self)` and `from_u32(code: u32) -> Option<Self>`.
- **Exported JSON Schema Specification**:
  - Defined `ERROR_SCHEMA_JSON` constant in `contracts/error_codes`.
  - Created [`contracts/error_codes/schema.json`](file:///c:/Users/chris/Desktop/d/soroscope/contracts/error_codes/schema.json) mapping variant names, integer codes, and descriptions.
- **Contract Error Enums Cleanup & Audit**:
  - Cleaned up duplicate variant declarations in [`contracts/liquidity_pool/src/lib.rs`](file:///c:/Users/chris/Desktop/d/soroscope/contracts/liquidity_pool/src/lib.rs).
  - Updated `CrossChainError` in [`contracts/cross_chain_payload/src/errors.rs`](file:///c:/Users/chris/Desktop/d/soroscope/contracts/cross_chain_payload/src/errors.rs) with `#[contracterror]`, `#[repr(u32)]`, and explicit variant discriminants matching `as_u32()`.
- **Dual-Directory Alignment**:
  - Synchronized changes across both root `contracts/` and nested `soroscope/contracts/` structures.

## 🔧 Technical Implementation
- `ContractError` enum definition:
  ```rust
  #[contracterror]
  #[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
  #[repr(u32)]
  pub enum ContractError {
      AlreadyInitialized = 1,
      NotInitialized = 2,
      Unauthorized = 3,
      InsufficientBalance = 4,
      InsufficientLiquidity = 5,
      InsufficientShares = 6,
      InsufficientAllowance = 7,
      SlippageExceeded = 8,
      InvalidFee = 9,
      NoPendingFeeUpdate = 10,
      TimelockNotElapsed = 11,
      OracleNotConfigured = 12,
      InvalidOraclePrice = 13,
      Paused = 14,
      Overflow = 15,
      DivisionByZero = 16,
      InvalidInput = 17,
  }
  ```
- Exported JSON Schema layout (`contracts/error_codes/schema.json`):
  ```json
  {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "ContractError",
    "type": "object",
    "description": "Explicit numerical discriminant mappings for SoroScope smart contract error codes.",
    "error_codes": [
      { "name": "AlreadyInitialized", "code": 1, "description": "Contract has already been initialized." },
      ...
    ]
  }
  ```

## 🧪 Step-by-Step Testing & Verification Process
Follow these steps to verify that the assignment was successfully completed:

1. **Verify Contract Error Code Discriminant Tests**:
   Run cargo test on the error codes crate:
   ```bash
   cargo test -p soroscope-error-codes
   ```
   *(If testing on Windows without MSVC linker installed, use the GNU toolchain)*:
   ```bash
   cargo +stable-x86_64-pc-windows-gnu test -p soroscope-error-codes
   ```
   *Expected Result*: All 3 tests (`test_explicit_numerical_discriminants`, `test_from_u32_conversion`, `test_schema_json_contains_all_variants`) pass cleanly.

2. **Verify JSON Schema Specification**:
   Inspect the generated schema file:
   ```bash
   cat contracts/error_codes/schema.json
   ```
   *Expected Result*: Valid JSON structure mapping error variants (`AlreadyInitialized` through `InvalidInput`) to their corresponding integer codes (`1` through `17`).

3. **Verify Contract Workspace Checks**:
   Check contract compilation across the workspace:
   ```bash
   cargo +stable-x86_64-pc-windows-gnu check
   ```

4. **Verify Web Frontend Diagnostics**:
   Ensure web application lints pass:
   ```bash
   cd web
   npm run lint
   ```

## 📁 Files Changed
- `contracts/error_codes/src/lib.rs` - Added explicit numerical discriminants, conversion helpers, JSON schema export, and tests.
- `contracts/error_codes/schema.json` - Added exported JSON schema specification.
- `contracts/liquidity_pool/src/lib.rs` - Removed duplicate enum variants.
- `contracts/cross_chain_payload/src/errors.rs` - Added `#[contracterror]`, `#[repr(u32)]`, and explicit discriminants.
- `soroscope/contracts/error_codes/src/lib.rs` - Synchronized nested crate file.
- `soroscope/contracts/error_codes/schema.json` - Synchronized nested schema file.
- `soroscope/contracts/cross_chain_payload/src/errors.rs` - Synchronized nested cross-chain errors.
- `docs/PR_DESCRIPTION_ISSUE_647.md` - Added PR documentation.

## ✅ Acceptance Criteria Checklist
- [x] Implementation completed according to specification.
- [x] Code passes all linting and formatting checks.
- [x] Unit and Integration tests added and passing.

## 🔗 Related Issues
Closes #647