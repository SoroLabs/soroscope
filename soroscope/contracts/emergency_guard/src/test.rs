#![cfg(test)]

use crate::PauseType;

#[test]
fn test_granular_pause_types() {
    let mut pause = PauseType::new(0);

    pause.set_paused(PauseType::SWAP, true);
    assert!(pause.is_paused(PauseType::SWAP));
    assert!(!pause.is_paused(PauseType::DEPOSIT));

    pause.set_paused(PauseType::DEPOSIT, true);
    assert!(pause.is_paused(PauseType::SWAP));
    assert!(pause.is_paused(PauseType::DEPOSIT));

    pause.set_paused(PauseType::WITHDRAW, true);
    assert!(pause.is_paused(PauseType::WITHDRAW));

    pause.set_paused(PauseType::SWAP, false);
    assert!(!pause.is_paused(PauseType::SWAP));
    assert!(pause.is_paused(PauseType::DEPOSIT));
    assert!(pause.is_paused(PauseType::WITHDRAW));
}

#[test]
fn test_bitwise_pause_logic() {
    let mut pause = PauseType::new(0);

    // Test setting operations
    pause.set_paused(PauseType::SWAP, true);
    assert!(pause.is_paused(PauseType::SWAP));
    assert!(!pause.is_paused(PauseType::DEPOSIT));

    // Test checking multiple operations
    pause.set_paused(PauseType::MINT, true);
    assert!(pause.is_paused(PauseType::SWAP));
    assert!(pause.is_paused(PauseType::MINT));

    // Test clearing operations
    pause.set_paused(PauseType::SWAP, false);
    assert!(!pause.is_paused(PauseType::SWAP));
    assert!(pause.is_paused(PauseType::MINT));

    // Test clearing all manually
    pause.set_paused(PauseType::MINT, false);
    assert_eq!(pause.as_u32(), 0);
}

#[test]
fn test_pause_all_and_unpause_all() {
    let mut pause = PauseType::new(0);
    pause.pause_all();
    for op in [
        PauseType::SWAP,
        PauseType::DEPOSIT,
        PauseType::WITHDRAW,
        PauseType::TRANSFER,
        PauseType::MINT,
        PauseType::BURN,
    ] {
        assert!(pause.is_paused(op));
    }
    pause.unpause_all();
    for op in [
        PauseType::SWAP,
        PauseType::DEPOSIT,
        PauseType::WITHDRAW,
        PauseType::TRANSFER,
        PauseType::MINT,
        PauseType::BURN,
    ] {
        assert!(!pause.is_paused(op));
    }
}

#[test]
fn test_multiple_pause_types() {
    let mut pause = PauseType::new(0);
    let combined = PauseType::SWAP | PauseType::DEPOSIT | PauseType::MINT;
    pause.set_paused(combined, true);
    assert!(pause.is_paused(PauseType::SWAP));
    assert!(pause.is_paused(PauseType::DEPOSIT));
    assert!(!pause.is_paused(PauseType::WITHDRAW));
    assert!(pause.is_paused(PauseType::MINT));
    assert!(!pause.is_paused(PauseType::BURN));
}

#[test]
fn test_pause_type_as_u32_bitmask() {
    let mut pause = crate::PauseType::new(0);
    pause.set_paused(crate::PauseType::SWAP, true);
    pause.set_paused(crate::PauseType::DEPOSIT, true);
    assert_eq!(
        pause.as_u32(),
        crate::PauseType::SWAP | crate::PauseType::DEPOSIT
    );
}
