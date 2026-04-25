use soroban_sdk::{contracttype, contracterror, Address};

#[contracttype]
pub enum DataKey {
    // Instance storage
    VaultCounter,
    SupportedTokens,
    CommunityPool(u64),

    // Persistent storage
    GroupVault(u64),
    MemberRecord(u64, Address),
    CreatorVaults(Address),
    MemberVaults(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CcpError {
    // Initialization
    AlreadyInitialized       = 1,
    NotInitialized           = 2,

    // Input validation
    InvalidMemberCount       = 10,
    MemberAmountMismatch     = 11,
    InvalidObligationAmount  = 12,
    UnsupportedToken         = 13,
    InvalidUnlockTime        = 14,
    InvalidFundingDeadline   = 15,
    InvalidPenaltyRate       = 16,

    // Vault lifecycle
    VaultNotFound            = 20,
    NotMember                = 21,
    WrongVaultState          = 22,
    WrongMemberState         = 23,
    FundingDeadlinePassed    = 24,
    FundingDeadlineNotPassed = 25,
    EarlyExitNotAllowed      = 26,

    // Access control
    Unauthorized             = 30,

    // Token transfer
    TransferFailed           = 40,
}
