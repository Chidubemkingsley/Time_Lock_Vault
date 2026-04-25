use soroban_sdk::{Address, Env, Vec};
use crate::{DataKey, GroupVault, MemberRecord};

pub const LEDGER_BUMP_AMOUNT: u32 = 535_000;

pub fn next_vault_id(env: &Env) -> u64 {
    let counter: u64 = env.storage().instance().get(&DataKey::VaultCounter).unwrap_or(0);
    env.storage().instance().set(&DataKey::VaultCounter, &(counter + 1));
    counter
}

pub fn is_supported_token(env: &Env, token: &Address) -> bool {
    let tokens: Vec<Address> = match env.storage().instance().get(&DataKey::SupportedTokens) {
        Some(t) => t,
        None => return false,
    };
    tokens.contains(token)
}

pub fn get_group_vault_unchecked(env: &Env, vault_id: u64) -> Option<GroupVault> {
    env.storage().persistent().get(&DataKey::GroupVault(vault_id))
}

pub fn save_group_vault(env: &Env, vault_id: u64, vault: &GroupVault) {
    let key = DataKey::GroupVault(vault_id);
    env.storage().persistent().set(&key, vault);
    env.storage().persistent().extend_ttl(&key, LEDGER_BUMP_AMOUNT, LEDGER_BUMP_AMOUNT);
}

pub fn get_member_record(env: &Env, vault_id: u64, member: &Address) -> Option<MemberRecord> {
    env.storage().persistent().get(&DataKey::MemberRecord(vault_id, member.clone()))
}

pub fn save_member_record(env: &Env, vault_id: u64, member: &Address, record: &MemberRecord) {
    let key = DataKey::MemberRecord(vault_id, member.clone());
    env.storage().persistent().set(&key, record);
    env.storage().persistent().extend_ttl(&key, LEDGER_BUMP_AMOUNT, LEDGER_BUMP_AMOUNT);
}

pub fn get_pool(env: &Env, vault_id: u64) -> i128 {
    env.storage().instance().get(&DataKey::CommunityPool(vault_id)).unwrap_or(0)
}

pub fn add_to_pool(env: &Env, vault_id: u64, amount: i128) {
    let current = get_pool(env, vault_id);
    env.storage().instance().set(&DataKey::CommunityPool(vault_id), &(current + amount));
}

pub fn set_pool(env: &Env, vault_id: u64, amount: i128) {
    env.storage().instance().set(&DataKey::CommunityPool(vault_id), &amount);
}

pub fn get_creator_vaults(env: &Env, creator: &Address) -> Vec<u64> {
    let key = DataKey::CreatorVaults(creator.clone());
    env.storage().persistent().get(&key).unwrap_or_else(|| Vec::new(env))
}

pub fn save_creator_vaults(env: &Env, creator: &Address, ids: &Vec<u64>) {
    let key = DataKey::CreatorVaults(creator.clone());
    env.storage().persistent().set(&key, ids);
    env.storage().persistent().extend_ttl(&key, LEDGER_BUMP_AMOUNT, LEDGER_BUMP_AMOUNT);
}

pub fn get_member_vaults(env: &Env, member: &Address) -> Vec<u64> {
    let key = DataKey::MemberVaults(member.clone());
    env.storage().persistent().get(&key).unwrap_or_else(|| Vec::new(env))
}

pub fn save_member_vaults(env: &Env, member: &Address, ids: &Vec<u64>) {
    let key = DataKey::MemberVaults(member.clone());
    env.storage().persistent().set(&key, ids);
    env.storage().persistent().extend_ttl(&key, LEDGER_BUMP_AMOUNT, LEDGER_BUMP_AMOUNT);
}
