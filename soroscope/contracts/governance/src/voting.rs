use crate::storage_types::{DataKey, GovernanceConfig};
use soroban_sdk::{Address, Env};

pub fn read_voting_power(e: &Env, addr: Address) -> i128 {
    let key = DataKey::VotingPower(addr);
    e.storage().persistent().get::<DataKey, i128>(&key).unwrap_or(0)
}

pub fn write_voting_power(e: &Env, addr: Address, amount: i128) {
    let key = DataKey::VotingPower(addr);
    e.storage().persistent().set(&key, &amount);
}

pub fn read_delegate(e: &Env, delegator: Address) -> Option<Address> {
    let key = DataKey::DelegatedPower(delegator);
    e.storage().persistent().get(&key)
}

pub fn write_delegate(e: &Env, delegator: Address, delegate: Address) {
    let key = DataKey::DelegatedPower(delegator);
    e.storage().persistent().set(&key, &delegate);
}

pub fn get_effective_voting_power(e: &Env, addr: Address) -> i128 {
    let mut power = read_voting_power(e, addr.clone());

    // Add delegated power
    let delegate_key = DataKey::Delegate(addr);
    power += e.storage().persistent().get::<DataKey, i128>(&delegate_key).unwrap_or(0);

    power
}

pub fn delegate_voting_power(e: &Env, delegator: Address, delegate: Address) {
    // Remove from old delegate if any
    if let Some(old_delegate) = read_delegate(e, delegator.clone()) {
        let old_delegate_key = DataKey::Delegate(old_delegate);
        let current_delegated = e.storage().persistent().get::<DataKey, i128>(&old_delegate_key).unwrap_or(0);
        e.storage().persistent().set(&old_delegate_key, &(current_delegated - read_voting_power(e, delegator.clone())));
    }

    // Set new delegate
    write_delegate(e, delegator.clone(), delegate.clone());

    // Add to new delegate
    let delegate_key = DataKey::Delegate(delegate);
    let current_delegated = e.storage().persistent().get::<DataKey, i128>(&delegate_key).unwrap_or(0);
    e.storage().persistent().set(&delegate_key, &(current_delegated + read_voting_power(e, delegator)));
}

pub fn has_voted(e: &Env, proposal_id: u32, voter: Address) -> bool {
    let key = DataKey::HasVoted(proposal_id, voter);
    e.storage().temporary().has(&key)
}

pub fn set_voted(e: &Env, proposal_id: u32, voter: Address) {
    let key = DataKey::HasVoted(proposal_id, voter);
    e.storage().temporary().set(&key, &true);
}

pub fn calculate_quorum(e: &Env, config: &GovernanceConfig) -> i128 {
    // This is simplified - in real implementation, you'd calculate total voting power
    // For now, assume total voting power is stored or calculated
    1000 * (config.quorum_percentage as i128) / 100
}