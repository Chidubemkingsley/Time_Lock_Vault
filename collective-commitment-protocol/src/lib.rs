#![no_std]

mod types;
pub use types::*;

mod storage_types;
pub use storage_types::*;

mod storage;
pub use storage::*;

mod utils;
pub use utils::*;

mod tests;
#[cfg(test)]
mod integration_tests;

use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Map, Vec};

#[contract]
pub struct CcpContract;

#[contractimpl]
impl CcpContract {
    // ─── initialize ──────────────────────────────────────────────────────────

    pub fn initialize(
        env: Env,
        xlm_token: Address,
        usdc_token: Address,
        eurc_token: Address,
    ) -> Result<(), CcpError> {
        if env.storage().instance().has(&DataKey::SupportedTokens) {
            return Err(CcpError::AlreadyInitialized);
        }
        let mut tokens = Vec::new(&env);
        tokens.push_back(xlm_token);
        tokens.push_back(usdc_token);
        tokens.push_back(eurc_token);
        env.storage().instance().set(&DataKey::SupportedTokens, &tokens);
        env.storage().instance().set(&DataKey::VaultCounter, &0u64);
        Ok(())
    }

    // ─── create_group_vault ──────────────────────────────────────────────────

    pub fn create_group_vault(
        env: Env,
        creator: Address,
        token: Address,
        members: Vec<Address>,
        amounts: Vec<i128>,
        unlock_time: u64,
        funding_deadline: u64,
        lock_type: LockType,
        penalty_rate: u32,
    ) -> Result<u64, CcpError> {
        creator.require_auth();

        let member_count = members.len();
        if member_count < 5 || member_count > 100 {
            return Err(CcpError::InvalidMemberCount);
        }
        if amounts.len() != member_count {
            return Err(CcpError::MemberAmountMismatch);
        }
        for i in 0..amounts.len() {
            if amounts.get(i).unwrap() <= 0 {
                return Err(CcpError::InvalidObligationAmount);
            }
        }
        if !is_supported_token(&env, &token) {
            return Err(CcpError::UnsupportedToken);
        }
        let now = env.ledger().timestamp();
        if unlock_time <= now {
            return Err(CcpError::InvalidUnlockTime);
        }
        if funding_deadline <= now || funding_deadline >= unlock_time {
            return Err(CcpError::InvalidFundingDeadline);
        }
        match lock_type {
            LockType::Penalty => {
                if penalty_rate == 0 || penalty_rate > 10_000 {
                    return Err(CcpError::InvalidPenaltyRate);
                }
            }
            LockType::Strict => {}
        }

        // Build obligations map and compute total_size
        let mut obligations: Map<Address, i128> = Map::new(&env);
        let mut total_size: i128 = 0;
        for i in 0..member_count {
            let member = members.get(i).unwrap();
            let amount = amounts.get(i).unwrap();
            obligations.set(member, amount);
            total_size += amount;
        }

        // Fixed 5% creator commission (500 basis points)
        let commission_rate: u32 = 500;

        let vault_id = next_vault_id(&env);
        let vault = GroupVault {
            vault_id,
            creator: creator.clone(),
            token: token.clone(),
            members: members.clone(),
            obligations: obligations.clone(),
            unlock_time,
            funding_deadline,
            lock_type: lock_type.clone(),
            penalty_rate,
            state: VaultState::FundingOpen,
            total_size,
            deposited_count: 0,
            claimed_count: 0,
            eligible_claimers: 0,
            original_pool: 0,
            commission_rate,
        };
        save_group_vault(&env, vault_id, &vault);

        // Create MemberRecord for each member
        for i in 0..member_count {
            let member = members.get(i).unwrap();
            let amount = amounts.get(i).unwrap();
            save_member_record(&env, vault_id, &member, &MemberRecord {
                state: MemberState::Committed,
                amount,
            });
            // Index vault under member
            let mut mv = get_member_vaults(&env, &member);
            mv.push_back(vault_id);
            save_member_vaults(&env, &member, &mv);
        }

        // Index vault under creator
        let mut cv = get_creator_vaults(&env, &creator);
        cv.push_back(vault_id);
        save_creator_vaults(&env, &creator, &cv);

        env.events().publish(
            (symbol_short!("grp_crt"), vault_id),
            GroupVaultCreatedEvent {
                vault_id,
                creator,
                token,
                member_count: member_count as u32,
                total_vault_size: total_size,
                unlock_time,
                lock_type,
            },
        );

        Ok(vault_id)
    }

    // ─── deposit ─────────────────────────────────────────────────────────────

    pub fn deposit(env: Env, caller: Address, vault_id: u64) -> Result<(), CcpError> {
        caller.require_auth();

        let mut vault = get_group_vault_unchecked(&env, vault_id)
            .ok_or(CcpError::VaultNotFound)?;

        if vault.state != VaultState::FundingOpen {
            return Err(CcpError::WrongVaultState);
        }
        if env.ledger().timestamp() > vault.funding_deadline {
            return Err(CcpError::FundingDeadlinePassed);
        }

        let mut record = get_member_record(&env, vault_id, &caller)
            .ok_or(CcpError::NotMember)?;

        if record.state != MemberState::Committed {
            return Err(CcpError::WrongMemberState);
        }

        let amount = record.amount;

        // Split deposit: commission to creator, remainder locked in contract
        let commission = amount * (vault.commission_rate as i128) / 10_000;
        let locked_amount = amount - commission;

        // Transfer full amount from member to contract first
        token_client(&env, &vault.token).transfer(
            &caller,
            &env.current_contract_address(),
            &amount,
        );

        // Immediately forward commission to creator
        if commission > 0 {
            token_client(&env, &vault.token).transfer(
                &env.current_contract_address(),
                &vault.creator,
                &commission,
            );
        }

        // Store the net locked amount in the member record
        record.state = MemberState::Deposited;
        record.amount = locked_amount;
        save_member_record(&env, vault_id, &caller, &record);

        vault.deposited_count += 1;
        save_group_vault(&env, vault_id, &vault);

        env.events().publish(
            (symbol_short!("mem_dep"), vault_id),
            MemberDepositedEvent { vault_id, member: caller.clone(), amount: locked_amount },
        );

        // Check if fully funded → activate
        if vault.deposited_count == vault.members.len() as u32 {
            // Transition all Deposited → Active
            for member in vault.members.iter() {
                let mut mr = get_member_record(&env, vault_id, &member).unwrap();
                mr.state = MemberState::Active;
                save_member_record(&env, vault_id, &member, &mr);
            }
            vault.state = VaultState::ActiveLocked;
            save_group_vault(&env, vault_id, &vault);

            env.events().publish(
                (symbol_short!("vlt_act"), vault_id),
                VaultActivatedEvent { vault_id },
            );
        }

        Ok(())
    }

    // ─── cancel ──────────────────────────────────────────────────────────────

    pub fn cancel(env: Env, vault_id: u64) -> Result<(), CcpError> {
        let mut vault = get_group_vault_unchecked(&env, vault_id)
            .ok_or(CcpError::VaultNotFound)?;

        if vault.state != VaultState::FundingOpen {
            return Err(CcpError::WrongVaultState);
        }
        if env.ledger().timestamp() <= vault.funding_deadline {
            return Err(CcpError::FundingDeadlineNotPassed);
        }

        vault.state = VaultState::Cancelled;
        save_group_vault(&env, vault_id, &vault);

        env.events().publish(
            (symbol_short!("vlt_can"), vault_id),
            VaultCancelledEvent { vault_id },
        );

        Ok(())
    }

    // ─── withdraw ────────────────────────────────────────────────────────────

    pub fn withdraw(env: Env, caller: Address, vault_id: u64) -> Result<(), CcpError> {
        caller.require_auth();

        let mut vault = get_group_vault_unchecked(&env, vault_id)
            .ok_or(CcpError::VaultNotFound)?;

        // Lazy SettlementReady transition
        maybe_transition_to_settlement_ready(&env, vault_id, &mut vault);

        match vault.state {
            VaultState::Cancelled => {
                // Refund path
                let mut record = get_member_record(&env, vault_id, &caller)
                    .ok_or(CcpError::NotMember)?;
                if record.state != MemberState::Deposited {
                    return Err(CcpError::WrongMemberState);
                }
                let amount = record.amount;
                token_client(&env, &vault.token).transfer(
                    &env.current_contract_address(),
                    &caller,
                    &amount,
                );
                record.state = MemberState::Withdrawn;
                save_member_record(&env, vault_id, &caller, &record);
                env.events().publish(
                    (symbol_short!("mem_wdr"), vault_id),
                    MemberWithdrawnEvent { vault_id, member: caller, amount },
                );
            }
            VaultState::SettlementReady => {
                // Mature withdrawal
                let mut record = get_member_record(&env, vault_id, &caller)
                    .ok_or(CcpError::NotMember)?;
                if record.state != MemberState::Active {
                    return Err(CcpError::WrongMemberState);
                }
                let amount = record.amount;
                token_client(&env, &vault.token).transfer(
                    &env.current_contract_address(),
                    &caller,
                    &amount,
                );
                record.state = MemberState::Withdrawn;
                save_member_record(&env, vault_id, &caller, &record);
                env.events().publish(
                    (symbol_short!("mem_wdr"), vault_id),
                    MemberWithdrawnEvent { vault_id, member: caller, amount },
                );
            }
            VaultState::ActiveLocked => {
                // Early exit
                let mut record = get_member_record(&env, vault_id, &caller)
                    .ok_or(CcpError::NotMember)?;
                if record.state != MemberState::Active {
                    return Err(CcpError::WrongMemberState);
                }
                if vault.lock_type == LockType::Strict {
                    return Err(CcpError::EarlyExitNotAllowed);
                }
                let (payout, penalty) = calculate_penalty(record.amount, vault.penalty_rate);
                token_client(&env, &vault.token).transfer(
                    &env.current_contract_address(),
                    &caller,
                    &payout,
                );
                add_to_pool(&env, vault_id, penalty);
                record.state = MemberState::Exited;
                save_member_record(&env, vault_id, &caller, &record);
                // NOTE: do NOT call maybe_transition_to_settlement_ready here —
                // early exit never triggers settlement (unlock_time not reached)
                env.events().publish(
                    (symbol_short!("mem_exit"), vault_id),
                    MemberEarlyExitEvent {
                        vault_id,
                        member: caller,
                        payout,
                        penalty,
                    },
                );
            }
            VaultState::FundingOpen | VaultState::Resolved => {
                return Err(CcpError::WrongVaultState);
            }
        }

        Ok(())
    }

    // ─── claim_pool ──────────────────────────────────────────────────────────

    pub fn claim_pool(env: Env, caller: Address, vault_id: u64) -> Result<(), CcpError> {
        caller.require_auth();

        let mut vault = get_group_vault_unchecked(&env, vault_id)
            .ok_or(CcpError::VaultNotFound)?;

        // Lazy SettlementReady transition
        maybe_transition_to_settlement_ready(&env, vault_id, &mut vault);

        if vault.state != VaultState::SettlementReady {
            return Err(CcpError::WrongVaultState);
        }

        let mut record = get_member_record(&env, vault_id, &caller)
            .ok_or(CcpError::NotMember)?;

        if record.state != MemberState::Active && record.state != MemberState::Withdrawn {
            return Err(CcpError::WrongMemberState);
        }

        let pool_balance = get_pool(&env, vault_id);
        let claimable_count = vault.eligible_claimers;

        let claim_amount = if vault.original_pool == 0 || claimable_count == 0 {
            0i128
        } else {
            let base = vault.original_pool / (claimable_count as i128);
            let remainder = vault.original_pool % (claimable_count as i128);
            // First claimer (claimed_count == 0) gets base + remainder
            if vault.claimed_count == 0 { base + remainder } else { base }
        };

        if claim_amount > 0 {
            token_client(&env, &vault.token).transfer(
                &env.current_contract_address(),
                &caller,
                &claim_amount,
            );
            set_pool(&env, vault_id, pool_balance - claim_amount);
        }

        record.state = MemberState::Claimed;
        save_member_record(&env, vault_id, &caller, &record);

        vault.claimed_count += 1;
        save_group_vault(&env, vault_id, &vault);

        env.events().publish(
            (symbol_short!("pool_clm"), vault_id),
            PoolClaimedEvent { vault_id, member: caller, claimed: claim_amount },
        );

        // Resolve when all eligible claimers have claimed
        if vault.claimed_count == claimable_count {
            vault.state = VaultState::Resolved;
            save_group_vault(&env, vault_id, &vault);
            env.events().publish(
                (symbol_short!("vlt_res"), vault_id),
                VaultResolvedEvent { vault_id },
            );
        }

        Ok(())
    }

    // ─── Read-only queries ────────────────────────────────────────────────────

    pub fn get_group_vault(env: Env, vault_id: u64) -> Result<GroupVault, CcpError> {
        get_group_vault_unchecked(&env, vault_id).ok_or(CcpError::VaultNotFound)
    }

    pub fn get_member_state(
        env: Env,
        vault_id: u64,
        member: Address,
    ) -> Result<MemberRecord, CcpError> {
        get_group_vault_unchecked(&env, vault_id).ok_or(CcpError::VaultNotFound)?;
        get_member_record(&env, vault_id, &member).ok_or(CcpError::NotMember)
    }

    pub fn get_vaults_by_creator(env: Env, creator: Address) -> Vec<u64> {
        get_creator_vaults(&env, &creator)
    }

    pub fn get_vaults_by_member(env: Env, member: Address) -> Vec<u64> {
        get_member_vaults(&env, &member)
    }

    pub fn get_pool_balance(env: Env, vault_id: u64) -> i128 {
        get_pool(&env, vault_id)
    }

    pub fn get_member_claim_amount(env: Env, vault_id: u64, member: Address) -> i128 {
        let vault = match get_group_vault_unchecked(&env, vault_id) {
            Some(v) => v,
            None => return 0,
        };
        // Only valid for members in Active or Withdrawn state
        let record = match get_member_record(&env, vault_id, &member) {
            Some(r) => r,
            None => return 0,
        };
        if record.state != MemberState::Active && record.state != MemberState::Withdrawn {
            return 0;
        }
        let pool_balance = get_pool(&env, vault_id);
        let claimable_count = count_claimable_members(&env, vault_id, &vault);
        if pool_balance == 0 || claimable_count == 0 {
            return 0;
        }
        pool_balance / (claimable_count as i128)
    }
}
