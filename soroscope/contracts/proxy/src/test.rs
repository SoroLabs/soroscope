extern crate std;

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol, TryIntoVal, Val, Vec};

#[test]
fn test_proxy_upgrade_keeps_state_and_changes_logic() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let proxy_id = env.register(Proxy, ());
    let impl_v1_id = env.register(ProxyLogicV1, ());
    let impl_v2_id = env.register(ProxyLogicV2, ());

    let proxy = ProxyClient::new(&env, &proxy_id);

    proxy.initialize(&admin, &impl_v1_id);
    assert_eq!(proxy.get_admin(), admin);
    assert_eq!(proxy.get_implementation(), impl_v1_id);
    assert_eq!(proxy.get_value(), 0);

    let first = proxy.increment(&7);
    assert_eq!(first, 7);
    assert_eq!(proxy.get_value(), 7);

    proxy.upgrade_to(&impl_v2_id);
    assert_eq!(proxy.get_implementation(), impl_v2_id);

    let second = proxy.increment(&3);
    assert_eq!(second, 20);
    assert_eq!(proxy.get_value(), 20);

    let version_symbol = Symbol::new(&env, "version");
    let version_val: Val = proxy.delegate_call(&version_symbol, &Vec::new(&env));
    let version: u32 = version_val.try_into_val(&env).unwrap();
    assert_eq!(version, 2);
}

#[test]
fn test_upgrade_to_and_call_executes_new_implementation_method() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let proxy_id = env.register(Proxy, ());
    let impl_v2_id = env.register(ProxyLogicV2, ());

    let proxy = ProxyClient::new(&env, &proxy_id);
    proxy.initialize(&admin, &impl_v2_id);

    let call_result: Val =
        proxy.upgrade_to_and_call(&impl_v2_id, &Symbol::new(&env, "version"), &Vec::new(&env));
    let version: u32 = call_result.try_into_val(&env).unwrap();
    assert_eq!(version, 2);
}
