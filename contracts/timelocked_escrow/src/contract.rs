use crate::escrow::{
    clear_admin_release_vote, has_config, read_admin, read_admin_release_vote, read_config,
    require_active, require_funded, write_admin, write_admin_release_vote, write_config,
};
use crate::guardian::{
    approval_count, guardian_index, has_quorum, read_approvals, read_epoch, read_guardians,
    read_recovery_approvals, set_approval, validate_guardians, write_approvals, write_epoch,
    write_guardians, write_recovery_approvals, REQUIRED_APPROVALS,
};
use crate::storage_types::{
    AdminReleaseEvent, AdminReleaseVoteEvent, ApprovalEvent, CancelEvent, DepositEvent, Error,
    EscrowConfig, EscrowStatus, GuardianRotationEvent, MutualCancelEvent, ReleaseEvent,
    StatusChangeEvent,
};
use soroban_sdk::{contract, contractimpl, token, Address, Env, Symbol, Vec};

/// Delay between an admin `vote_release` and `execute_release`:
/// 72 hours at ~5s per ledger (72 * 60 * 12).
pub const ADMIN_RELEASE_DELAY_LEDGERS: u32 = 51_840;

/// Minimum lock period in ledgers (≈ 8 minutes on Stellar).
pub const MIN_LOCK_LEDGERS: u32 = 100;

#[contract]
pub struct TimelockEscrow;

#[contractimpl]
impl TimelockEscrow {
    pub const MIN_LOCK_LEDGERS: u32 = MIN_LOCK_LEDGERS;

    /// Initializes the escrow with depositor, beneficiary, token, guardians, and lock duration.
    /// The timelock is computed as current ledger sequence + `lock_ledgers`.
    /// `cancellation_fee_bps` is the fee (in basis points) deducted on cancel, capped at 10000.
    /// `protocol_vault` receives the cancellation fee.
    /// `admin` may start an emergency release (`vote_release` then, after a
    /// 72-hour delay, `execute_release`) when the beneficiary key is lost.
    #[allow(clippy::too_many_arguments)]
    pub fn initialize(
        e: Env,
        depositor: Address,
        beneficiary: Address,
        token: Address,
        guardians: Vec<Address>,
        lock_ledgers: u32,
        cancellation_fee_bps: u32,
        protocol_vault: Address,
        admin: Address,
    ) -> Result<(), Error> {
        if has_config(&e) {
            return Err(Error::AlreadyInitialized);
        }
        if lock_ledgers < Self::MIN_LOCK_LEDGERS {
            return Err(Error::LockPeriodTooShort);
        }
        validate_guardians(&guardians)?;
        if cancellation_fee_bps > 10_000 {
            return Err(Error::CancellationFeeTooHigh);
        }

        let unlock_ledger = e.ledger().sequence().saturating_add(lock_ledgers);
        if unlock_ledger < e.ledger().sequence().saturating_add(Self::MIN_LOCK_LEDGERS) {
            return Err(Error::LockPeriodTooShort);
        }

        let config = EscrowConfig {
            depositor,
            beneficiary,
            token,
            amount: 0,
            unlock_ledger: e.ledger().sequence().saturating_add(lock_ledgers),
            is_released: false,
            is_recovered: false,
            is_cancelled: false,
            cancellation_fee_bps,
            protocol_vault,
        };

        write_config(&e, &config);
        write_guardians(&e, &guardians);
        write_approvals(&e, 0u32);
        write_recovery_approvals(&e, 0u32);
        write_epoch(&e, 0u32);

        // Store the admin address so release_by_admin can authenticate the caller.
        write_admin(&e, &admin);

        e.storage().instance().extend_ttl(100, 100);
        Ok(())
    }

    /// Creates the escrow with an absolute unlock ledger sequence, enforcing the minimum lock period.
    pub fn create_escrow(
        e: Env,
        depositor: Address,
        beneficiary: Address,
        token: Address,
        guardians: Vec<Address>,
        unlock_ledger: u32,
        cancellation_fee_bps: u32,
        protocol_vault: Address,
    ) -> Result<(), Error> {
        let current_sequence = e.ledger().sequence();
        if unlock_ledger < current_sequence.saturating_add(Self::MIN_LOCK_LEDGERS) {
            return Err(Error::LockPeriodTooShort);
        }
        let lock_ledgers = unlock_ledger.saturating_sub(current_sequence);
        let admin = depositor.clone();
        Self::initialize(
            e,
            depositor,
            beneficiary,
            token,
            guardians,
            lock_ledgers,
            cancellation_fee_bps,
            protocol_vault,
            admin,
        )
    }

    /// Depositor transfers tokens into the escrow. Single deposit only.
    pub fn deposit(e: Env, amount: i128) -> Result<(), Error> {
        let mut config = read_config(&e)?;
        require_active(&config)?;
        if config.amount > 0 {
            return Err(Error::AlreadyDeposited);
        }

        config.depositor.require_auth();

        token::Client::new(&e, &config.token).transfer(
            &config.depositor,
            &e.current_contract_address(),
            &amount,
        );

        config.amount = amount;
        write_config(&e, &config);

        publish_status_change(&e, EscrowStatus::Initialized, EscrowStatus::Funded);

        e.events().publish(
            (Symbol::new(&e, "deposit"), config.depositor.clone()),
            DepositEvent {
                depositor: config.depositor,
                token: config.token,
                amount,
                unlock_ledger: config.unlock_ledger,
            },
        );

        e.storage().instance().extend_ttl(100, 100);
        Ok(())
    }

    /// Guardian casts an approval vote. Returns the current approval count.
    pub fn approve(e: Env, guardian: Address) -> Result<u32, Error> {
        let config = read_config(&e)?;
        require_active(&config)?;
        require_funded(&config)?;

        guardian.require_auth();

        let guardians = read_guardians(&e);
        let idx = guardian_index(&guardians, &guardian)?;

        let bitmap = read_approvals(&e);
        let new_bitmap = set_approval(bitmap, idx)?;
        write_approvals(&e, new_bitmap);

        let count = approval_count(new_bitmap);

        e.events().publish(
            (Symbol::new(&e, "approval"), guardian.clone()),
            ApprovalEvent {
                guardian,
                guardian_index: idx,
                approval_count: count,
            },
        );

        e.storage().instance().extend_ttl(100, 100);
        Ok(count)
    }

    /// Guardian explicitly approves returning the escrow to the depositor.
    pub fn approve_recovery(e: Env, guardian: Address) -> Result<u32, Error> {
        let config = read_config(&e)?;
        require_active(&config)?;
        require_funded(&config)?;

        guardian.require_auth();
        let guardians = read_guardians(&e);
        let idx = guardian_index(&guardians, &guardian)?;
        let bitmap = read_recovery_approvals(&e);
        let new_bitmap = set_approval(bitmap, idx)?;
        write_recovery_approvals(&e, new_bitmap);

        let count = approval_count(new_bitmap);
        e.events().publish(
            (Symbol::new(&e, "recovery_approval"), guardian.clone()),
            ApprovalEvent {
                guardian,
                guardian_index: idx,
                approval_count: count,
            },
        );
        e.storage().instance().extend_ttl(100, 100);
        Ok(count)
    }

    /// Releases funds to the beneficiary. Permissionless — anyone can trigger
    /// once the timelock has expired and quorum (3-of-5) is reached.
    pub fn release(e: Env) -> Result<(), Error> {
        let mut config = read_config(&e)?;
        require_active(&config)?;
        require_funded(&config)?;

        if e.ledger().sequence() < config.unlock_ledger {
            return Err(Error::TimelockNotExpired);
        }

        let bitmap = read_approvals(&e);
        if !has_quorum(bitmap) {
            return Err(Error::InsufficientApprovals);
        }

        token::Client::new(&e, &config.token).transfer(
            &e.current_contract_address(),
            &config.beneficiary,
            &config.amount,
        );

        let released_amount = config.amount;
        config.amount = 0;
        config.is_released = true;
        write_config(&e, &config);
        publish_status_change(&e, EscrowStatus::Funded, EscrowStatus::Released);

        e.events().publish(
            (Symbol::new(&e, "release"), config.beneficiary.clone()),
            ReleaseEvent {
                beneficiary: config.beneficiary,
                token: config.token,
                amount: released_amount,
                approval_count: approval_count(bitmap),
            },
        );

        e.storage().instance().extend_ttl(100, 100);
        Ok(())
    }

    /// Returns escrowed funds to the original depositor after expiry when the
    /// configured guardian quorum has approved. Anyone may trigger the transfer.
    pub fn recover(e: Env) -> Result<(), Error> {
        let mut config = read_config(&e)?;
        require_active(&config)?;
        require_funded(&config)?;

        if e.ledger().sequence() < config.unlock_ledger {
            return Err(Error::TimelockNotExpired);
        }
        let bitmap = read_recovery_approvals(&e);
        if !has_quorum(bitmap) {
            return Err(Error::InsufficientApprovals);
        }

        token::Client::new(&e, &config.token).transfer(
            &e.current_contract_address(),
            &config.depositor,
            &config.amount,
        );

        config.amount = 0;
        config.is_recovered = true;
        write_config(&e, &config);
        publish_status_change(&e, EscrowStatus::Funded, EscrowStatus::Recovered);
        e.storage().instance().extend_ttl(100, 100);
        Ok(())
    }

    /// The original depositor may extend the lock, but cannot shorten it.
    pub fn extend_timelock(e: Env, new_unlock_ledger: u32) -> Result<(), Error> {
        let mut config = read_config(&e)?;
        require_active(&config)?;
        config.depositor.require_auth();
        if new_unlock_ledger <= config.unlock_ledger {
            return Err(Error::InvalidTimelockExtension);
        }
        config.unlock_ledger = new_unlock_ledger;
        write_config(&e, &config);
        e.storage().instance().extend_ttl(100, 100);
        Ok(())
    }

    /// Depositor cancels the escrow and reclaims funds.
    /// Allowed before timelock, or after timelock only if zero approvals exist.
    /// A configurable cancellation fee (in basis points) is deducted and sent
    /// to the protocol vault; the remainder is returned to the depositor.
    pub fn cancel(e: Env) -> Result<(), Error> {
        let mut config = read_config(&e)?;
        require_active(&config)?;
        require_funded(&config)?;

        config.depositor.require_auth();

        if e.ledger().sequence() >= config.unlock_ledger && approval_count(read_approvals(&e)) > 0 {
            return Err(Error::AlreadyFinalized);
        }

        let fee = config.amount * i128::from(config.cancellation_fee_bps) / 10_000;
        let refund = config.amount - fee;

        if fee > 0 {
            token::Client::new(&e, &config.token).transfer(
                &e.current_contract_address(),
                &config.protocol_vault,
                &fee,
            );
        }

        token::Client::new(&e, &config.token).transfer(
            &e.current_contract_address(),
            &config.depositor,
            &refund,
        );

        let cancelled_amount = config.amount;
        config.amount = 0;
        config.is_cancelled = true;
        write_config(&e, &config);
        publish_status_change(&e, EscrowStatus::Funded, EscrowStatus::Cancelled);

        e.events().publish(
            (Symbol::new(&e, "cancel"), config.depositor.clone()),
            CancelEvent {
                depositor: config.depositor,
                token: config.token,
                amount: cancelled_amount,
                fee,
                protocol_vault: config.protocol_vault,
            },
        );

        e.storage().instance().extend_ttl(100, 100);
        Ok(())
    }

    /// Both depositor and beneficiary mutually agree to cancel the escrow.
    /// Works regardless of timelock or approval state. The cancellation fee is
    /// deducted and sent to the protocol vault; the remainder is returned to the depositor.
    pub fn mutual_cancel(e: Env) -> Result<(), Error> {
        let mut config = read_config(&e)?;
        require_active(&config)?;
        require_funded(&config)?;

        config.depositor.require_auth();
        config.beneficiary.require_auth();

        let fee = config.amount * i128::from(config.cancellation_fee_bps) / 10_000;
        let refund = config.amount - fee;

        if fee > 0 {
            token::Client::new(&e, &config.token).transfer(
                &e.current_contract_address(),
                &config.protocol_vault,
                &fee,
            );
        }

        token::Client::new(&e, &config.token).transfer(
            &e.current_contract_address(),
            &config.depositor,
            &refund,
        );

        let cancelled_amount = config.amount;
        config.amount = 0;
        config.is_cancelled = true;
        write_config(&e, &config);
        publish_status_change(&e, EscrowStatus::Funded, EscrowStatus::Cancelled);

        e.events().publish(
            (
                Symbol::new(&e, "mutual_cancel"),
                config.depositor.clone(),
                config.beneficiary.clone(),
            ),
            MutualCancelEvent {
                depositor: config.depositor,
                beneficiary: config.beneficiary,
                token: config.token,
                amount: cancelled_amount,
                fee,
                protocol_vault: config.protocol_vault,
            },
        );

        e.storage().instance().extend_ttl(100, 100);
        Ok(())
    }

    /// Rotates the guardian set. Requires exactly 3 current guardians to authorize.
    /// Resets all pending approvals and bumps the epoch.
    pub fn rotate_guardians(
        e: Env,
        new_guardians: Vec<Address>,
        approving_guardians: Vec<Address>,
    ) -> Result<(), Error> {
        let config = read_config(&e)?;
        require_active(&config)?;

        validate_guardians(&new_guardians)?;

        if approving_guardians.len() != REQUIRED_APPROVALS {
            return Err(Error::InsufficientApprovals);
        }

        let current_guardians = read_guardians(&e);

        // Verify each approving guardian is current and unique, require their auth
        let mut approver_bitmap = 0u32;
        for i in 0..approving_guardians.len() {
            let g = approving_guardians.get(i).unwrap();
            g.require_auth();
            let idx = guardian_index(&current_guardians, &g)?;
            let mask = 1u32 << idx;
            if approver_bitmap & mask != 0 {
                return Err(Error::DuplicateGuardian);
            }
            approver_bitmap |= mask;
        }

        let old_guardians = current_guardians;
        write_guardians(&e, &new_guardians);
        write_approvals(&e, 0u32);
        write_recovery_approvals(&e, 0u32);
        let new_epoch = read_epoch(&e) + 1;
        write_epoch(&e, new_epoch);

        e.events().publish(
            (Symbol::new(&e, "guardian_rotation"),),
            GuardianRotationEvent {
                old_guardians,
                new_guardians,
                new_epoch,
            },
        );

        e.storage().instance().extend_ttl(100, 100);
        Ok(())
    }

    // ── View Functions ──────────────────────────────────────

    pub fn get_config(e: Env) -> Result<EscrowConfig, Error> {
        read_config(&e)
    }

    pub fn get_guardians(e: Env) -> Vec<Address> {
        read_guardians(&e)
    }

    pub fn get_approval_bitmap(e: Env) -> u32 {
        read_approvals(&e)
    }

    pub fn get_approval_count(e: Env) -> u32 {
        approval_count(read_approvals(&e))
    }

    pub fn get_recovery_approval_count(e: Env) -> u32 {
        approval_count(read_recovery_approvals(&e))
    }

    pub fn is_releasable(e: Env) -> bool {
        match read_config(&e) {
            Ok(config) => {
                !config.is_released
                    && !config.is_recovered
                    && !config.is_cancelled
                    && config.amount > 0
                    && e.ledger().sequence() >= config.unlock_ledger
                    && has_quorum(read_approvals(&e))
            }
            Err(_) => false,
        }
    }

    /// Returns the stored contract admin address.
    pub fn get_admin(e: Env) -> Result<Address, Error> {
        read_admin(&e)
    }

    /// Returns the ledger at which the admin voted for an emergency release, if any.
    pub fn get_admin_release_vote(e: Env) -> Option<u32> {
        read_admin_release_vote(&e)
    }

    /// Step 1 of the admin emergency release (recovery path for a lost
    /// beneficiary key). Records the vote and starts a 72-hour
    /// (`ADMIN_RELEASE_DELAY_LEDGERS`) delay before `execute_release` may run.
    pub fn vote_release(e: Env) -> Result<u32, Error> {
        let admin = read_admin(&e)?;
        admin.require_auth();

        let config = read_config(&e)?;
        require_active(&config)?;
        require_funded(&config)?;

        if read_admin_release_vote(&e).is_some() {
            return Err(Error::AdminReleaseAlreadyVoted);
        }

        let voted_at_ledger = e.ledger().sequence();
        let executable_at_ledger = voted_at_ledger.saturating_add(ADMIN_RELEASE_DELAY_LEDGERS);
        write_admin_release_vote(&e, voted_at_ledger);

        e.events().publish(
            (Symbol::new(&e, "admin_vote_release"), admin.clone()),
            AdminReleaseVoteEvent {
                admin,
                voted_at_ledger,
                executable_at_ledger,
            },
        );

        // Keep the contract live for the whole delay so execute_release can run.
        let ttl = ADMIN_RELEASE_DELAY_LEDGERS + 100;
        e.storage().instance().extend_ttl(ttl, ttl);
        Ok(executable_at_ledger)
    }

    /// Step 2 of the admin emergency release. Callable only by the admin, and
    /// only once `ADMIN_RELEASE_DELAY_LEDGERS` have passed since `vote_release`.
    /// Sends the full escrowed amount to the beneficiary, bypassing the
    /// guardian quorum and the timelock.
    pub fn execute_release(e: Env) -> Result<(), Error> {
        let admin = read_admin(&e)?;
        admin.require_auth();

        let mut config = read_config(&e)?;
        require_active(&config)?;
        require_funded(&config)?;

        let voted_at_ledger = read_admin_release_vote(&e).ok_or(Error::AdminReleaseNotVoted)?;
        if e.ledger().sequence() < voted_at_ledger.saturating_add(ADMIN_RELEASE_DELAY_LEDGERS) {
            return Err(Error::AdminReleaseDelayNotElapsed);
        }

        // Effects before the token transfer.
        let released_amount = config.amount;
        config.amount = 0;
        config.is_released = true;
        write_config(&e, &config);
        clear_admin_release_vote(&e);

        token::Client::new(&e, &config.token).transfer(
            &e.current_contract_address(),
            &config.beneficiary,
            &released_amount,
        );

        publish_status_change(&e, EscrowStatus::Funded, EscrowStatus::Released);
        e.events().publish(
            (Symbol::new(&e, "admin_execute_release"), admin.clone()),
            AdminReleaseEvent {
                admin,
                beneficiary: config.beneficiary,
                token: config.token,
                amount: released_amount,
            },
        );

        e.storage().instance().extend_ttl(100, 100);
        Ok(())
    }
}

fn publish_status_change(e: &Env, previous_status: EscrowStatus, new_status: EscrowStatus) {
    e.events().publish(
        (Symbol::new(e, "status_change"),),
        StatusChangeEvent {
            previous_status,
            new_status,
        },
    );
}
