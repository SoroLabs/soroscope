use super::*;
use soroban_sdk::Env;

#[test]
fn test_ttl_extension() {
    let env = Env::default();
    let contract_id = env.register_contract(None, TtlExtension);
    let client = TtlExtensionClient::new(&env, &contract_id);

    client.write(&1);
    let init_p = client.persistent_ttl();
    let init_i = client.instance_ttl();

    client.extend_ttl(&0, &(init_p + 1000));
    assert_eq!(client.persistent_ttl(), init_p);
    assert_eq (client.instance_ttl(), init_i);

    client.extend_ttl(&(init_p + 1), &(init_p + 2000));
    assert!(client.persistent_ttl() > init_p);
    assert!(client.instance_ttl() > init_i);
}