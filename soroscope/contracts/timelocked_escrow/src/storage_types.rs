use soroban_sdk::{contracterror, contracttype, Address, Vec};

// ── Storage Keys ──────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Config,
    Guardians,
    Approvals,
    ApprovalEpoch,
}

// ── Escrow Configuration ──────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct EscrowConfig {
    pub depositor: Address,
    pub beneficiary: Address,
    pub token: Address,
    pub amount: i128,
    pub unlock_ledger: u32,
    pub is_released: bool,
    pub is_cancelled: bool,
}

// ── Errors ────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    AlreadyDeposited = 3,
    NoDeposit = 4,
    TimelockNotExpired = 5,
    InsufficientApprovals = 6,
    NotGuardian = 7,
    AlreadyApproved = 8,
    AlreadyFinalized = 9,
    Unauthorized = 10,
    InvalidGuardianCount = 11,
    DuplicateGuardian = 12,
}

// ── Event Structs ─────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DepositEvent {
    pub depositor: Address,
    pub token: Address,
    pub amount: i128,
    pub unlock_ledger: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ApprovalEvent {
    pub guardian: Address,
    pub guardian_index: u32,
    pub approval_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReleaseEvent {
    pub beneficiary: Address,
    pub token: Address,
    pub amount: i128,
    pub approval_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CancelEvent {
    pub depositor: Address,
    pub token: Address,
    pub amount: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct GuardianRotationEvent {
    pub old_guardians: Vec<Address>,
    pub new_guardians: Vec<Address>,
    pub new_epoch: u32,
}
