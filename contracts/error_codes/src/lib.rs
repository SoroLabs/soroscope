#![no_std]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ErrorCategory {
    Auth = 1,
    Math = 2,
    Storage = 3,
    Liquidity = 4,
    Oracle = 5,
    Governance = 6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SharedError {
    // Auth (100..=199)
    Unauthorized = 100,
    InvalidCaller = 101,

    // Math (200..=299)
    Overflow = 200,
    Underflow = 201,
    DivisionByZero = 202,

    // Storage (300..=399)
    MissingEntry = 300,
    StorageCorrupted = 301,

    // Liquidity (400..=499)
    InsufficientLiquidity = 400,
    SlippageExceeded = 401,

    // Oracle (500..=599)
    StalePrice = 500,
    InvalidOracleFeed = 501,

    // Governance (600..=699)
    ProposalNotFound = 600,
    VotingClosed = 601,
}

impl SharedError {
    pub const ALL: [Self; 12] = [
        Self::Unauthorized,
        Self::InvalidCaller,
        Self::Overflow,
        Self::Underflow,
        Self::DivisionByZero,
        Self::MissingEntry,
        Self::StorageCorrupted,
        Self::InsufficientLiquidity,
        Self::SlippageExceeded,
        Self::StalePrice,
        Self::InvalidOracleFeed,
        Self::ProposalNotFound,
        // Note: adjust array length to match exact variant count
    ];

    pub const fn code(self) -> u32 {
        self as u32
    }

    pub const fn category(self) -> ErrorCategory {
        match self {
            Self::Unauthorized | Self::InvalidCaller => ErrorCategory::Auth,
            Self::Overflow | Self::Underflow | Self::DivisionByZero => ErrorCategory::Math,
            Self::MissingEntry | Self::StorageCorrupted => ErrorCategory::Storage,
            Self::InsufficientLiquidity | Self::SlippageExceeded => ErrorCategory::Liquidity,
            Self::StalePrice | Self::InvalidOracleFeed => ErrorCategory::Oracle,
            Self::ProposalNotFound | Self::VotingClosed => ErrorCategory::Governance,
        }
    }

    pub const fn is_retryable(self) -> bool {
        match self {
            // Transient / state or market conditions that may succeed later
            Self::InsufficientLiquidity | Self::SlippageExceeded | Self::StalePrice => true,
            // Deterministic or permission/state failures unless inputs/state change
            Self::Unauthorized
            | Self::InvalidCaller
            | Self::Overflow
            | Self::Underflow
            | Self::DivisionByZero
            | Self::MissingEntry
            | Self::StorageCorrupted
            | Self::InvalidOracleFeed
            | Self::ProposalNotFound
            | Self::VotingClosed => false,
        }
    }

    pub const fn from_code(code: u32) -> Option<Self> {
        match code {
            100 => Some(Self::Unauthorized),
            101 => Some(Self::InvalidCaller),
            200 => Some(Self::Overflow),
            201 => Some(Self::Underflow),
            202 => Some(Self::DivisionByZero),
            300 => Some(Self::MissingEntry),
            301 => Some(Self::StorageCorrupted),
            400 => Some(Self::InsufficientLiquidity),
            401 => Some(Self::SlippageExceeded),
            500 => Some(Self::StalePrice),
            501 => Some(Self::InvalidOracleFeed),
            600 => Some(Self::ProposalNotFound),
            601 => Some(Self::VotingClosed),
            _ => None,
        }
    }
}

impl TryFrom<u32> for SharedError {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::from_code(value).ok_or(())
    }
}