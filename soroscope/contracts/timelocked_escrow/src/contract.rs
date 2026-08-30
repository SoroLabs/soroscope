use crate::escrow::{has_config, read_config, require_active, require_funded, write_config};
use crate::guardian::{
    approval_count, guardian_index, has_quorum, read_approvals, read_epoch, read_guardians,
    set_approval, validate_guardians, write_approvals, write_epoch, write_guardians,
    REQUIRED_APPROVALS,
};
use crate::storage_types::{
    ApprovalEvent, CancelEvent, DepositEvent, Error, EscrowConfig, GuardianRotationEvent,
    ReleaseEvent,
};
use soroban_sdk::{contract, contractimpl, token, Address, Env, String, Vec};

#[contract]
pub struct TimelockEscrow;

#[contractimpl]
impl TimelockEscrow {
    /// Initializes the escrow with depositor, beneficiary, token, guardians, and lock duration.
    /// The timelock is computed as current ledger sequence + `lock_ledgers`.
    pub fn initialize(
        e: Env,
        depositor: Address,
        beneficiary: Address,
        token: Address,
        guardians: Vec<Address>,
        lock_ledgers: u32,
    ) -> Result<(), Error> {
        if has_config(&e) {
            return Err(Error::AlreadyInitialized);
        }
        validate_guardians(&guardians)?;

        let config = EscrowConfig {
            depositor,
            beneficiary,
            token,
            amount: 0,
            unlock_ledger: e.ledger().sequence().saturating_add(lock_ledgers),
            is_released: false,
            is_cancelled: false,
        };

        write_config(&e, &config);
        write_guardians(&e, &guardians);
        write_approvals(&e, 0u32);
        write_epoch(&e, 0u32);

        e.storage().instance().extend_ttl(100, 100);
        Ok(())
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

        e.events().publish(
            (String::from_str(&e, "deposit"), config.depositor.clone()),
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
            (String::from_str(&e, "approval"), guardian.clone()),
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

        e.events().publish(
            (String::from_str(&e, "release"), config.beneficiary.clone()),
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

    /// Depositor cancels the escrow and reclaims funds.
    /// Allowed before timelock, or after timelock only if zero approvals exist.
    pub fn cancel(e: Env) -> Result<(), Error> {
        let mut config = read_config(&e)?;
        require_active(&config)?;
        require_funded(&config)?;

        config.depositor.require_auth();

        if e.ledger().sequence() >= config.unlock_ledger && approval_count(read_approvals(&e)) > 0 {
            return Err(Error::AlreadyFinalized);
        }

        token::Client::new(&e, &config.token).transfer(
            &e.current_contract_address(),
            &config.depositor,
            &config.amount,
        );

        let cancelled_amount = config.amount;
        config.amount = 0;
        config.is_cancelled = true;
        write_config(&e, &config);

        e.events().publish(
            (String::from_str(&e, "cancel"), config.depositor.clone()),
            CancelEvent {
                depositor: config.depositor,
                token: config.token,
                amount: cancelled_amount,
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
        let new_epoch = read_epoch(&e) + 1;
        write_epoch(&e, new_epoch);

        e.events().publish(
            (String::from_str(&e, "guardian_rotation"),),
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

    pub fn is_releasable(e: Env) -> bool {
        match read_config(&e) {
            Ok(config) => {
                !config.is_released
                    && !config.is_cancelled
                    && config.amount > 0
                    && e.ledger().sequence() >= config.unlock_ledger
                    && has_quorum(read_approvals(&e))
            }
            Err(_) => false,
        }
    }
}
