use crate::{Domain, Transfer, TypedDataAuth};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, BytesN, Env, String};

#[test]
fn test_domain_separator_hash() {
    let env = Env::default();
    let contract_address = Address::generate(&env);
    let domain = Domain {
        name: String::from_str(&env, "TestContract"),
        version: String::from_str(&env, "1.0"),
        chain_id: 1,
        verifying_contract: contract_address,
    };
    let hash = TypedDataAuth::domain_separator_hash(&env, &domain);
    // Hash should be non-zero (32 bytes, not all zeros)
    let zero = BytesN::from_array(&env, &[0u8; 32]);
    assert_ne!(hash, zero);
}

#[test]
fn test_struct_hash() {
    let env = Env::default();
    let from = Address::generate(&env);
    let to = Address::generate(&env);
    let transfer = Transfer {
        from: from.clone(),
        to: to.clone(),
        amount: 1000,
    };

    let hash = TypedDataAuth::struct_hash(&env, &transfer);
    let zero = BytesN::from_array(&env, &[0u8; 32]);
    assert_ne!(hash, zero);
}

#[test]
fn test_message_hash() {
    let env = Env::default();
    let domain_hash = BytesN::from_array(&env, &[1u8; 32]);
    let struct_hash = BytesN::from_array(&env, &[2u8; 32]);

    let message_hash = TypedDataAuth::message_hash(&env, &domain_hash, &struct_hash);
    let zero = BytesN::from_array(&env, &[0u8; 32]);
    assert_ne!(message_hash, zero);
}

#[test]
fn test_domain_separator_consistency() {
    let env = Env::default();
    let contract_address = Address::generate(&env);
    let domain1 = Domain {
        name: String::from_str(&env, "TestContract"),
        version: String::from_str(&env, "1.0"),
        chain_id: 1,
        verifying_contract: contract_address.clone(),
    };
    let domain2 = Domain {
        name: String::from_str(&env, "TestContract"),
        version: String::from_str(&env, "1.0"),
        chain_id: 1,
        verifying_contract: contract_address,
    };
    assert_eq!(
        TypedDataAuth::domain_separator_hash(&env, &domain1),
        TypedDataAuth::domain_separator_hash(&env, &domain2),
    );
}

#[test]
fn test_different_domains_produce_different_hashes() {
    let env = Env::default();
    let contract_address = Address::generate(&env);
    let domain1 = Domain {
        name: String::from_str(&env, "TestContract"),
        version: String::from_str(&env, "1.0"),
        chain_id: 1,
        verifying_contract: contract_address.clone(),
    };
    // Different chain_id should produce a different hash
    let domain2 = Domain {
        name: String::from_str(&env, "TestContract"),
        version: String::from_str(&env, "1.0"),
        chain_id: 2,
        verifying_contract: contract_address,
    };

    let hash1 = TypedDataAuth::domain_separator_hash(&env, &domain1);
    let hash2 = TypedDataAuth::domain_separator_hash(&env, &domain2);

    assert_ne!(hash1, hash2);
}
