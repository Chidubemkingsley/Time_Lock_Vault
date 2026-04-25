use soroban_sdk::{contracttype, Address, Map, Vec};

// ─── Enums ────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum VaultState {
    FundingOpen,
    ActiveLocked,
    SettlementReady,
    Resolved,
    Cancelled,
}

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum MemberState {
    Committed,
    Deposited,
    Active,
    Exited,
    Withdrawn,
    Claimed,
}

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum LockType {
    Strict,
    Penalty,
}

// ─── Core structs ─────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct GroupVault {
    pub vault_id: u64,
    pub creator: Address,
    pub token: Address,
    pub members: Vec<Address>,
    pub obligations: Map<Address, i128>,
    pub unlock_time: u64,
    pub funding_deadline: u64,
    pub lock_type: LockType,
    pub penalty_rate: u32,
    pub state: VaultState,
    pub total_size: i128,
    pub deposited_count: u32,
    pub claimed_count: u32,
    /// Set when vault transitions to SettlementReady — total members eligible to claim pool
    pub eligible_claimers: u32,
    /// Set when vault transitions to SettlementReady — original pool balance for equal distribution
    pub original_pool: i128,
    /// Creator commission in basis points (e.g. 500 = 5%). Deducted from each deposit.
    pub commission_rate: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MemberRecord {
    pub state: MemberState,
    pub amount: i128,
}

// ─── Event structs ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct GroupVaultCreatedEvent {
    pub vault_id: u64,
    pub creator: Address,
    pub token: Address,
    pub member_count: u32,
    pub total_vault_size: i128,
    pub unlock_time: u64,
    pub lock_type: LockType,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MemberDepositedEvent {
    pub vault_id: u64,
    pub member: Address,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct VaultActivatedEvent {
    pub vault_id: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct VaultCancelledEvent {
    pub vault_id: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MemberEarlyExitEvent {
    pub vault_id: u64,
    pub member: Address,
    pub payout: i128,
    pub penalty: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MemberWithdrawnEvent {
    pub vault_id: u64,
    pub member: Address,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PoolClaimedEvent {
    pub vault_id: u64,
    pub member: Address,
    pub claimed: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct VaultResolvedEvent {
    pub vault_id: u64,
}
