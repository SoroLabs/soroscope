extern crate std;

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env};

#[test]
fn test_proxy_upgrade_with_timelock() {
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

    // 1. Propose upgrade
    proxy.propose_upgrade(&impl_v2_id);

    let pending = proxy.get_pending_upgrade().unwrap();
    assert_eq!(pending.new_implementation, impl_v2_id);
    assert_eq!(pending.eta, env.ledger().timestamp() + 172_800);

    // 2. Fast-forward time by 48 hours (172,800 seconds)
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 172_800);

    // 3. Execute upgrade
    proxy.execute_upgrade();
    assert_eq!(proxy.get_implementation(), impl_v2_id);
    assert_eq!(proxy.get_pending_upgrade(), None);
}

#[test]
#[should_panic(expected = "Timelock delay of 48 hours has not elapsed")]
fn test_execute_upgrade_before_timelock_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let proxy_id = env.register(Proxy, ());
    let impl_v1_id = env.register(ProxyLogicV1, ());
    let impl_v2_id = env.register(ProxyLogicV2, ());

    let proxy = ProxyClient::new(&env, &proxy_id);
    proxy.initialize(&admin, &impl_v1_id);

    proxy.propose_upgrade(&impl_v2_id);

    // Try executing immediately without advancing time -> should panic
    proxy.execute_upgrade();
}

#[test]
fn test_cancel_upgrade() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let proxy_id = env.register(Proxy, ());
    let impl_v1_id = env.register(ProxyLogicV1, ());
    let impl_v2_id = env.register(ProxyLogicV2, ());

    let proxy = ProxyClient::new(&env, &proxy_id);
    proxy.initialize(&admin, &impl_v1_id);

    proxy.propose_upgrade(&impl_v2_id);
    assert!(proxy.get_pending_upgrade().is_some());

    proxy.cancel_upgrade();
    assert_eq!(proxy.get_pending_upgrade(), None);
}
