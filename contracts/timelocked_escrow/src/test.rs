use crate::contract::TimelockEscrowClient;
use crate::guardian::{approval_count, has_quorum, set_approval};
use crate::TimelockEscrow;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env, Vec,
};

const LOCK_LEDGERS: u32 = 100;
const DEPOSIT_AMOUNT: i128 = 10_000;
const FEE_BPS: u32 = 100; // 1%

/// Creates a fully initialized test environment with token, escrow, depositor,
/// beneficiary, 5 guardians, a protocol vault, and a 1% cancellation fee.
fn setup() -> (
    Env,
    TimelockEscrowClient<'static>,
    Address,      // token
    Address,      // depositor
    Address,      // beneficiary
    Vec<Address>, // guardians
    Address,      // protocol_vault
) {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let depositor = Address::generate(&e);
    let beneficiary = Address::generate(&e);
    let protocol_vault = Address::generate(&e);

    // Register a Stellar asset and mint to depositor
    let token_admin = Address::generate(&e);
    let token_addr = e
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let sac = token::StellarAssetClient::new(&e, &token_addr);
    sac.mint(&depositor, &(DEPOSIT_AMOUNT * 2));

    // 5 guardians
    let mut guardians = Vec::new(&e);
    for _ in 0..5 {
        guardians.push_back(Address::generate(&e));
    }

    let contract_id = e.register(TimelockEscrow, ());
    let client = TimelockEscrowClient::new(&e, &contract_id);

    client.initialize(
        &depositor,
        &beneficiary,
        &token_addr,
        &guardians,
        &LOCK_LEDGERS,
        &FEE_BPS,
        &protocol_vault,
    );

    (
        e,
        client,
        token_addr,
        depositor,
        beneficiary,
        guardians,
        protocol_vault,
    )
}

fn advance_ledger(e: &Env, by: u32) {
    let mut info = e.ledger().get();
    info.sequence_number += by;
    e.ledger().set(info);
}

// ── 1. Full Lifecycle ─────────────────────────────────────────

#[test]
fn test_full_lifecycle() {
    let (e, client, token_addr, _depositor, beneficiary, guardians, _vault) = setup();
    let token_client = token::Client::new(&e, &token_addr);

    client.deposit(&DEPOSIT_AMOUNT);

    // 3 guardians approve
    client.approve(&guardians.get(0).unwrap());
    client.approve(&guardians.get(1).unwrap());
    let count = client.approve(&guardians.get(2).unwrap());
    assert_eq!(count, 3);

    assert!(!client.is_releasable());

    // Advance past timelock
    advance_ledger(&e, LOCK_LEDGERS + 1);

    assert!(client.is_releasable());

    client.release();

    assert_eq!(token_client.balance(&beneficiary), DEPOSIT_AMOUNT);

    let config = client.get_config();
    assert!(config.is_released);
    assert_eq!(config.amount, 0);
}

// ── 2. Release Before Timelock Fails ──────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_release_before_timelock_fails() {
    let (_, client, _, _, _, guardians, _) = setup();
    client.deposit(&DEPOSIT_AMOUNT);

    client.approve(&guardians.get(0).unwrap());
    client.approve(&guardians.get(1).unwrap());
    client.approve(&guardians.get(2).unwrap());

    // Don't advance ledger — timelock not expired
    client.release();
}

// ── 3. Release Insufficient Approvals ─────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_release_insufficient_approvals() {
    let (e, client, _, _, _, guardians, _) = setup();
    client.deposit(&DEPOSIT_AMOUNT);

    // Only 2 approvals
    client.approve(&guardians.get(0).unwrap());
    client.approve(&guardians.get(1).unwrap());

    advance_ledger(&e, LOCK_LEDGERS + 1);
    client.release();
}

// ── 4. Double Approve Fails ───────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_double_approve_fails() {
    let (_, client, _, _, _, guardians, _) = setup();
    client.deposit(&DEPOSIT_AMOUNT);

    let g = guardians.get(0).unwrap();
    client.approve(&g);
    client.approve(&g);
}

// ── 5. Non-Guardian Approve Fails ─────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_non_guardian_approve_fails() {
    let (e, client, _, _, _, _, _) = setup();
    client.deposit(&DEPOSIT_AMOUNT);

    let imposter = Address::generate(&e);
    client.approve(&imposter);
}

// ── 6. Cancel Before Timelock Deducts Fee ─────────────────────

#[test]
fn test_cancel_before_timelock_deducts_fee() {
    let (e, client, token_addr, depositor, _, _, protocol_vault) = setup();
    let token_client = token::Client::new(&e, &token_addr);

    client.deposit(&DEPOSIT_AMOUNT);

    // Cancel immediately — before timelock
    client.cancel();

    let config = client.get_config();
    assert!(config.is_cancelled);
    assert_eq!(config.amount, 0);

    // Fee = 1% of 10_000 = 100
    let expected_fee: i128 = DEPOSIT_AMOUNT * 100 / 10_000; // 100

    assert_eq!(token_client.balance(&protocol_vault), expected_fee);
    assert_eq!(
        token_client.balance(&depositor),
        DEPOSIT_AMOUNT * 2 - expected_fee
    );
}

// ── 7. Cancel After Timelock No Approvals ─────────────────────

#[test]
fn test_cancel_after_timelock_no_approvals() {
    let (e, client, _, _, _, _, _) = setup();
    client.deposit(&DEPOSIT_AMOUNT);

    advance_ledger(&e, LOCK_LEDGERS + 1);

    // No approvals, so cancel should succeed
    client.cancel();

    let config = client.get_config();
    assert!(config.is_cancelled);
}

// ── 8. Cancel After Timelock With Approvals Fails ─────────────

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_cancel_after_timelock_with_approvals_fails() {
    let (e, client, _, _, _, guardians, _) = setup();
    client.deposit(&DEPOSIT_AMOUNT);

    client.approve(&guardians.get(0).unwrap());

    advance_ledger(&e, LOCK_LEDGERS + 1);
    client.cancel();
}

// ── 9. Release After Cancel Fails ─────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_release_after_cancel_fails() {
    let (e, client, _, _, _, guardians, _) = setup();
    client.deposit(&DEPOSIT_AMOUNT);
    client.cancel();

    client.approve(&guardians.get(0).unwrap());
    client.approve(&guardians.get(1).unwrap());
    client.approve(&guardians.get(2).unwrap());
    advance_ledger(&e, LOCK_LEDGERS + 1);
    client.release();
}

// ── 10. Guardian Rotation ─────────────────────────────────────

#[test]
fn test_guardian_rotation() {
    let (e, client, _, _, _, guardians, _) = setup();
    client.deposit(&DEPOSIT_AMOUNT);

    // Guardian 0 approves before rotation
    client.approve(&guardians.get(0).unwrap());
    assert_eq!(client.get_approval_count(), 1);

    // Rotate with 3 current guardians authorizing
    let mut new_guardians = Vec::new(&e);
    for _ in 0..5 {
        new_guardians.push_back(Address::generate(&e));
    }

    let mut approving = Vec::new(&e);
    approving.push_back(guardians.get(0).unwrap());
    approving.push_back(guardians.get(1).unwrap());
    approving.push_back(guardians.get(2).unwrap());

    client.rotate_guardians(&new_guardians, &approving);

    // Approvals should be reset
    assert_eq!(client.get_approval_count(), 0);
    assert_eq!(client.get_approval_bitmap(), 0);

    // New guardians can approve
    client.approve(&new_guardians.get(0).unwrap());
    assert_eq!(client.get_approval_count(), 1);

    // Verify new guardian list
    let stored = client.get_guardians();
    assert_eq!(stored.len(), 5);
    assert_eq!(stored.get(0).unwrap(), new_guardians.get(0).unwrap());
}

// ── 11. Guardian Rotation Insufficient Sigs ───────────────────

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_guardian_rotation_insufficient_sigs() {
    let (e, client, _, _, _, guardians, _) = setup();

    let mut new_guardians = Vec::new(&e);
    for _ in 0..5 {
        new_guardians.push_back(Address::generate(&e));
    }

    // Only 2 approving guardians
    let mut approving = Vec::new(&e);
    approving.push_back(guardians.get(0).unwrap());
    approving.push_back(guardians.get(1).unwrap());

    client.rotate_guardians(&new_guardians, &approving);
}

// ── 12. Duplicate Guardians Rejected ──────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_duplicate_guardians_rejected() {
    let e = Env::default();
    e.mock_all_auths();

    let depositor = Address::generate(&e);
    let beneficiary = Address::generate(&e);
    let protocol_vault = Address::generate(&e);
    let token_addr = e
        .register_stellar_asset_contract_v2(Address::generate(&e))
        .address();

    let g = Address::generate(&e);
    let mut guardians = Vec::new(&e);
    guardians.push_back(g.clone());
    guardians.push_back(g.clone()); // duplicate
    guardians.push_back(Address::generate(&e));
    guardians.push_back(Address::generate(&e));
    guardians.push_back(Address::generate(&e));

    let contract_id = e.register(TimelockEscrow, ());
    let client = TimelockEscrowClient::new(&e, &contract_id);

    client.initialize(
        &depositor,
        &beneficiary,
        &token_addr,
        &guardians,
        &LOCK_LEDGERS,
        &FEE_BPS,
        &protocol_vault,
    );
}

// ── 13. Double Initialize Fails ───────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_double_initialize_fails() {
    let (_e, client, token_addr, depositor, beneficiary, guardians, _vault) = setup();

    client.initialize(
        &depositor,
        &beneficiary,
        &token_addr,
        &guardians,
        &LOCK_LEDGERS,
        &FEE_BPS,
        &_vault,
    );
}

// ── 14. Deposit After Release Fails ───────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_deposit_after_release_fails() {
    let (e, client, _, _, _, guardians, _) = setup();
    client.deposit(&DEPOSIT_AMOUNT);

    client.approve(&guardians.get(0).unwrap());
    client.approve(&guardians.get(1).unwrap());
    client.approve(&guardians.get(2).unwrap());

    advance_ledger(&e, LOCK_LEDGERS + 1);
    client.release();

    client.deposit(&DEPOSIT_AMOUNT);
}

// ── 15. Bitmap Unit Tests ─────────────────────────────────────

#[test]
fn test_bitmap_operations() {
    assert_eq!(approval_count(0b00000), 0);
    assert_eq!(approval_count(0b00001), 1);
    assert_eq!(approval_count(0b10101), 3);
    assert_eq!(approval_count(0b11111), 5);

    assert!(!has_quorum(0b00000));
    assert!(!has_quorum(0b00011));
    assert!(has_quorum(0b00111));
    assert!(has_quorum(0b11111));

    // set_approval
    let bm = set_approval(0, 0).unwrap();
    assert_eq!(bm, 0b00001);
    let bm = set_approval(bm, 2).unwrap();
    assert_eq!(bm, 0b00101);
    let bm = set_approval(bm, 4).unwrap();
    assert_eq!(bm, 0b10101);

    // double-set fails
    assert!(set_approval(bm, 0).is_err());
    assert!(set_approval(bm, 2).is_err());
}

// ── 16. View Functions ────────────────────────────────────────

#[test]
fn test_view_functions() {
    let (e, client, _, _, _, guardians, _) = setup();
    client.deposit(&DEPOSIT_AMOUNT);

    // Initial state
    assert_eq!(client.get_approval_count(), 0);
    assert_eq!(client.get_approval_bitmap(), 0);
    assert!(!client.is_releasable());

    let stored_guardians = client.get_guardians();
    assert_eq!(stored_guardians.len(), 5);

    // After approvals
    client.approve(&guardians.get(0).unwrap());
    client.approve(&guardians.get(3).unwrap());
    assert_eq!(client.get_approval_count(), 2);
    assert_eq!(client.get_approval_bitmap(), 0b01001);

    client.approve(&guardians.get(4).unwrap());
    assert_eq!(client.get_approval_count(), 3);

    // Still not releasable (timelock)
    assert!(!client.is_releasable());

    advance_ledger(&e, LOCK_LEDGERS + 1);
    assert!(client.is_releasable());
}

// ── 17. Mutual Cancel Deducts Fee ─────────────────────────────

#[test]
fn test_mutual_cancel_deducts_fee() {
    let (e, client, token_addr, depositor, beneficiary, _guardians, protocol_vault) = setup();
    let token_client = token::Client::new(&e, &token_addr);

    client.deposit(&DEPOSIT_AMOUNT);

    client.mutual_cancel();

    let config = client.get_config();
    assert!(config.is_cancelled);
    assert_eq!(config.amount, 0);

    let expected_fee: i128 = DEPOSIT_AMOUNT * 100 / 10_000; // 100

    assert_eq!(token_client.balance(&protocol_vault), expected_fee);
    assert_eq!(
        token_client.balance(&depositor),
        DEPOSIT_AMOUNT * 2 - expected_fee
    );
    assert_eq!(token_client.balance(&beneficiary), 0);
}

// ── 18. Mutual Cancel After Timelock ──────────────────────────

#[test]
fn test_mutual_cancel_after_timelock() {
    let (e, client, token_addr, depositor, _beneficiary, guardians, protocol_vault) = setup();
    let token_client = token::Client::new(&e, &token_addr);

    client.deposit(&DEPOSIT_AMOUNT);

    // Get some approvals
    client.approve(&guardians.get(0).unwrap());
    client.approve(&guardians.get(1).unwrap());

    advance_ledger(&e, LOCK_LEDGERS + 1);

    // Even after timelock with approvals, mutual cancel works
    client.mutual_cancel();

    let config = client.get_config();
    assert!(config.is_cancelled);

    let expected_fee: i128 = DEPOSIT_AMOUNT * 100 / 10_000; // 100
    assert_eq!(token_client.balance(&protocol_vault), expected_fee);
    assert_eq!(
        token_client.balance(&depositor),
        DEPOSIT_AMOUNT * 2 - expected_fee
    );
}

// ── 19. Mutual Cancel After Cancel Fails ──────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_mutual_cancel_after_cancel_fails() {
    let (_, client, _, _, _, _, _) = setup();
    client.deposit(&DEPOSIT_AMOUNT);
    client.cancel();
    client.mutual_cancel();
}

// ── 20. Initialize With Max Fee Succeeds ──────────────────────

#[test]
fn test_initialize_with_max_fee() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let depositor = Address::generate(&e);
    let beneficiary = Address::generate(&e);
    let protocol_vault = Address::generate(&e);
    let token_addr = e
        .register_stellar_asset_contract_v2(Address::generate(&e))
        .address();

    let mut guardians = Vec::new(&e);
    for _ in 0..5 {
        guardians.push_back(Address::generate(&e));
    }

    let contract_id = e.register(TimelockEscrow, ());
    let client = TimelockEscrowClient::new(&e, &contract_id);

    // Max fee = 10_000 bps = 100%
    client.initialize(
        &depositor,
        &beneficiary,
        &token_addr,
        &guardians,
        &LOCK_LEDGERS,
        &10_000u32,
        &protocol_vault,
    );

    let config = client.get_config();
    assert_eq!(config.cancellation_fee_bps, 10_000);
    assert_eq!(config.protocol_vault, protocol_vault);
}

// ── 21. Initialize With Excessive Fee Fails ───────────────────

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn test_initialize_excessive_fee_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let depositor = Address::generate(&e);
    let beneficiary = Address::generate(&e);
    let protocol_vault = Address::generate(&e);
    let token_addr = e
        .register_stellar_asset_contract_v2(Address::generate(&e))
        .address();

    let mut guardians = Vec::new(&e);
    for _ in 0..5 {
        guardians.push_back(Address::generate(&e));
    }

    let contract_id = e.register(TimelockEscrow, ());
    let client = TimelockEscrowClient::new(&e, &contract_id);

    // Fee > 10_000 bps
    client.initialize(
        &depositor,
        &beneficiary,
        &token_addr,
        &guardians,
        &LOCK_LEDGERS,
        &10_001u32,
        &protocol_vault,
    );
}

// ── 22. Cancel With Zero Fee Sends All To Depositor ───────────

#[test]
fn test_cancel_with_zero_fee() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let depositor = Address::generate(&e);
    let beneficiary = Address::generate(&e);
    let protocol_vault = Address::generate(&e);
    let token_admin = Address::generate(&e);
    let token_addr = e
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let sac = token::StellarAssetClient::new(&e, &token_addr);
    sac.mint(&depositor, &(DEPOSIT_AMOUNT * 2));

    let mut guardians = Vec::new(&e);
    for _ in 0..5 {
        guardians.push_back(Address::generate(&e));
    }

    let contract_id = e.register(TimelockEscrow, ());
    let client = TimelockEscrowClient::new(&e, &contract_id);

    client.initialize(
        &depositor,
        &beneficiary,
        &token_addr,
        &guardians,
        &LOCK_LEDGERS,
        &0u32,
        &protocol_vault,
    );

    let token_client = token::Client::new(&e, &token_addr);
    let depositor_initial = token_client.balance(&depositor);

    client.deposit(&DEPOSIT_AMOUNT);
    client.cancel();

    // No fee deducted — depositor gets full refund
    assert_eq!(token_client.balance(&depositor), depositor_initial);
    assert_eq!(token_client.balance(&protocol_vault), 0);
}

// ── 23. Mutual Cancel With Zero Fee ───────────────────────────

#[test]
fn test_mutual_cancel_with_zero_fee() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let depositor = Address::generate(&e);
    let beneficiary = Address::generate(&e);
    let protocol_vault = Address::generate(&e);
    let token_admin = Address::generate(&e);
    let token_addr = e
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let sac = token::StellarAssetClient::new(&e, &token_addr);
    sac.mint(&depositor, &(DEPOSIT_AMOUNT * 2));

    let mut guardians = Vec::new(&e);
    for _ in 0..5 {
        guardians.push_back(Address::generate(&e));
    }

    let contract_id = e.register(TimelockEscrow, ());
    let client = TimelockEscrowClient::new(&e, &contract_id);

    client.initialize(
        &depositor,
        &beneficiary,
        &token_addr,
        &guardians,
        &LOCK_LEDGERS,
        &0u32,
        &protocol_vault,
    );

    let token_client = token::Client::new(&e, &token_addr);
    let depositor_initial = token_client.balance(&depositor);

    client.deposit(&DEPOSIT_AMOUNT);
    client.mutual_cancel();

    assert_eq!(token_client.balance(&depositor), depositor_initial);
    assert_eq!(token_client.balance(&protocol_vault), 0);
}

// ── 24. Release After Mutual Cancel Fails ─────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_release_after_mutual_cancel_fails() {
    let (e, client, _, _, _, guardians, _) = setup();
    client.deposit(&DEPOSIT_AMOUNT);
    client.mutual_cancel();

    client.approve(&guardians.get(0).unwrap());
    client.approve(&guardians.get(1).unwrap());
    client.approve(&guardians.get(2).unwrap());
    advance_ledger(&e, LOCK_LEDGERS + 1);
    client.release();
}
