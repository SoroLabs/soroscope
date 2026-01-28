use super::*;
use soroban_sdk::{symbol_short, Bytes, Env, Vec};

#[test]
fn test_storage() {
    let env = Env::default();
    let contract_id = env.register(StorageHeavyContract, ());
    let client = StorageHeavyContractClient::new(&env, &contract_id);

    let key = symbol_short!("data");
    let mut data = [0u8; 1000];
    for (i, item) in data.iter_mut().enumerate() {
        *item = (i % 256) as u8;
    }
    let data_bytes = Bytes::from_slice(&env, &data);

    // Test Persistent
    client.write_persistent(&key, &data_bytes);
    assert_eq!(client.read_persistent(&key), data_bytes);

    // Test Temporary
    client.write_temporary(&key, &data_bytes);
    assert_eq!(client.read_temporary(&key), data_bytes);
}

#[test]
fn test_batch_storage() {
    let env = Env::default();
    let contract_id = env.register(StorageHeavyContract, ());
    let client = StorageHeavyContractClient::new(&env, &contract_id);

    let mut keys = Vec::new(&env);
    let mut data_points = Vec::new(&env);

    keys.push_back(symbol_short!("k1"));
    keys.push_back(symbol_short!("k2"));
    keys.push_back(symbol_short!("k3"));

    data_points.push_back(Bytes::from_slice(&env, &[1u8; 100]));
    data_points.push_back(Bytes::from_slice(&env, &[2u8; 100]));
    data_points.push_back(Bytes::from_slice(&env, &[3u8; 100]));

    client.batch_write_persistent(&keys, &data_points);

    assert_eq!(
        client.read_persistent(&symbol_short!("k1")),
        Bytes::from_slice(&env, &[1u8; 100])
    );
    assert_eq!(
        client.read_persistent(&symbol_short!("k2")),
        Bytes::from_slice(&env, &[2u8; 100])
    );
    assert_eq!(
        client.read_persistent(&symbol_short!("k3")),
        Bytes::from_slice(&env, &[3u8; 100])
    );
}
