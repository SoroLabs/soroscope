# EmergencyGuard - Standardized Emergency Controls

## Overview

This implementation provides a standardized, reusable `EmergencyGuard` trait and implementation for handling paused states across all Soroban contracts in the workspace. It features:

✅ **Granular Pausing** - Pause specific operations independently (swaps, deposits, withdrawals, transfers, minting, burning)  
✅ **Multi-Signature Support** - Built-in architecture for multi-admin authorization  
✅ **Admin Rotation** - Secure admin authority transfer without moving funds  
✅ **Efficient Storage** - Bitmask-based pause state (32 operations in single u32)  
✅ **Event Logging** - All administrative actions logged for audits  
✅ **Code Reuse** - Single implementation shared across all contracts

## Files Created

```
contracts/emergency_guard/
├── Cargo.toml                 # Crate dependencies and metadata
├── src/
│   ├── lib.rs                 # Main trait, types, and default implementation
│   └── test.rs                # Unit tests
├── examples/
│   └── simple_token.rs        # Example: Token contract using EmergencyGuard
└── README.md                  # Detailed API documentation

contracts/EMERGENCY_GUARD_INTEGRATION.md  # Integration guide for existing contracts
```
