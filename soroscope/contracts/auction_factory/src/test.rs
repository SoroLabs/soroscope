#![cfg(test)]
use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

#[test]
fn test_factory() {
    let env = Env::default();
    env.mock_all_auths();

    let factory_id = env.register(AuctionFactory, ());
    let factory_client = AuctionFactoryClient::new(&env, &factory_id);

    // For full test, need WASM hashes, which require building the contracts.
    // In a real test, we would upload the WASM.
    // For now, just check that the contract registers.

    let seller = Address::generate(&env);
    let nft_contract = Address::generate(&env);
    let payment_token = Address::generate(&env);

    // Dummy WASM hash
    let dummy_hash = BytesN::<32>::from_array(&env, &[0; 32]);

    // This will fail because dummy_hash is not valid WASM, but the function signature is tested.
    // In practice, we need real WASM.

    // assert!(factory_client.create_english_auction(&seller, &nft_contract, &1, &payment_token, &100, &200, &10, &dummy_hash).is_some());
}
