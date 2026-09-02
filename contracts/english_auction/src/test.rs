#![cfg(test)]
use super::*;
use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{Address, Env};

// ---------------------------------------------------------------------------
// Minimal token used for both the NFT (amount = 1) and the bid currency.
// ---------------------------------------------------------------------------

mod mock_token {
    use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

    #[contracttype]
    enum DataKey {
        Balance(Address),
    }

    #[contract]
    pub struct MockToken;

    #[contractimpl]
    impl MockToken {
        pub fn mint(env: Env, to: Address, amount: i128) {
            let mut bal: i128 = env
                .storage()
                .instance()
                .get(&DataKey::Balance(to.clone()))
                .unwrap_or(0);
            bal += amount;
            env.storage()
                .instance()
                .set(&DataKey::Balance(to), &bal);
        }

        pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
            from.require_auth();
            let mut from_bal: i128 = env
                .storage()
                .instance()
                .get(&DataKey::Balance(from.clone()))
                .unwrap_or(0);
            if from_bal < amount {
                panic!("insufficient balance");
            }
            from_bal -= amount;
            env.storage()
                .instance()
                .set(&DataKey::Balance(from), &from_bal);

            let mut to_bal: i128 = env
                .storage()
                .instance()
                .get(&DataKey::Balance(to.clone()))
                .unwrap_or(0);
            to_bal += amount;
            env.storage()
                .instance()
                .set(&DataKey::Balance(to), &to_bal);
        }

        pub fn balance(env: Env, id: Address) -> i128 {
            env.storage()
                .instance()
                .get(&DataKey::Balance(id))
                .unwrap_or(0)
        }
    }

    pub use MockToken as Contract;
    pub use MockTokenClient as Client;
}

fn set_ledger(env: &Env, sequence: u32, timestamp: u64) {
    env.ledger().set(LedgerInfo {
        timestamp,
        protocol_version: 22,
        sequence_number: sequence,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 3_110_400,
    });
}

struct AuctionFixture {
    env: Env,
    seller: Address,
    bidder1: Address,
    bidder2: Address,
    nft_id: Address,
    payment_id: Address,
    auction_id: Address,
}

impl AuctionFixture {
    fn nft(&self) -> mock_token::Client<'_> {
        mock_token::Client::new(&self.env, &self.nft_id)
    }

    fn payment(&self) -> mock_token::Client<'_> {
        mock_token::Client::new(&self.env, &self.payment_id)
    }

    fn auction(&self) -> EnglishAuctionClient<'_> {
        EnglishAuctionClient::new(&self.env, &self.auction_id)
    }
}

fn setup(duration_ledgers: u32) -> AuctionFixture {
    let env = Env::default();
    env.mock_all_auths();

    let seller = Address::generate(&env);
    let bidder1 = Address::generate(&env);
    let bidder2 = Address::generate(&env);

    let nft_id = env.register(mock_token::Contract, ());
    let nft = mock_token::Client::new(&env, &nft_id);
    nft.mint(&seller, &1);

    let payment_id = env.register(mock_token::Contract, ());
    let payment = mock_token::Client::new(&env, &payment_id);
    payment.mint(&bidder1, &1_000);
    payment.mint(&bidder2, &1_000);

    let auction_id = env.register(EnglishAuction, ());
    let auction = EnglishAuctionClient::new(&env, &auction_id);

    auction.initialize(
        &seller,
        &nft_id,
        &1,
        &payment_id,
        &100,
        &200,
        &duration_ledgers,
    );

    AuctionFixture {
        env,
        seller,
        bidder1,
        bidder2,
        nft_id,
        payment_id,
        auction_id,
    }
}

#[test]
fn test_english_auction() {
    // 200 ledgers ≈ 1000s, well outside the 5-minute anti-snipe window.
    let fx = setup(200);

    fx.auction().bid(&fx.bidder1, &150);
    assert_eq!(fx.auction().get_highest_bid(), 150);
    assert_eq!(fx.auction().get_highest_bidder(), Some(fx.bidder1.clone()));

    fx.auction().bid(&fx.bidder2, &220);
    assert_eq!(fx.auction().get_highest_bid(), 220);
    assert_eq!(fx.auction().get_highest_bidder(), Some(fx.bidder2.clone()));

    set_ledger(&fx.env, 200, 10_000);
    fx.auction().end_auction();

    assert_eq!(fx.payment().balance(&fx.seller), 220);
    assert_eq!(fx.nft().balance(&fx.bidder2), 1);
}

#[test]
fn test_outbid_refunds_previous_bidder_immediately() {
    let fx = setup(200);

    fx.auction().bid(&fx.bidder1, &150);
    assert_eq!(fx.payment().balance(&fx.bidder1), 850);
    assert_eq!(fx.payment().balance(&fx.auction_id), 150);

    fx.auction().bid(&fx.bidder2, &220);
    // bidder1 is refunded the moment they are outbid.
    assert_eq!(fx.payment().balance(&fx.bidder1), 1_000);
    assert_eq!(fx.payment().balance(&fx.bidder2), 780);
    assert_eq!(fx.payment().balance(&fx.auction_id), 220);
    assert_eq!(fx.auction().get_highest_bidder(), Some(fx.bidder2.clone()));
    assert_eq!(fx.auction().get_highest_bid(), 220);
}

#[test]
fn test_deadline_extends_when_bid_in_final_sixty_ledgers() {
    let fx = setup(80);
    let original_end = fx.auction().get_end_ledger();
    assert_eq!(original_end, 80);

    // Remaining ledgers = 20 ≤ 60 → anti-snipe extension of 60 ledgers.
    set_ledger(&fx.env, 60, 300);
    fx.auction().bid(&fx.bidder1, &150);

    assert_eq!(fx.auction().get_end_ledger(), original_end + 60);
    assert_eq!(fx.auction().get_highest_bid(), 150);
}

#[test]
fn test_deadline_extends_when_bid_in_final_five_minutes() {
    let fx = setup(80);
    // duration 80 ledgers → end_timestamp = 400s. Jump to t=150 so 250s remain
    // (≤ 300s) while remaining ledgers (80) are still above the 60-ledger
    // window — this isolates the time-based rule.
    let original_end_ts = fx.auction().get_end_timestamp();
    let original_end_ledger = fx.auction().get_end_ledger();
    assert_eq!(original_end_ts, 400);

    set_ledger(&fx.env, 0, 150);
    fx.auction().bid(&fx.bidder1, &150);

    assert_eq!(fx.auction().get_end_timestamp(), original_end_ts + 300);
    assert_eq!(fx.auction().get_end_ledger(), original_end_ledger + 60);
}

#[test]
fn test_early_bid_does_not_extend_deadline() {
    let fx = setup(200);
    let original_end = fx.auction().get_end_ledger();
    let original_ts = fx.auction().get_end_timestamp();

    fx.auction().bid(&fx.bidder1, &150);

    assert_eq!(fx.auction().get_end_ledger(), original_end);
    assert_eq!(fx.auction().get_end_timestamp(), original_ts);
}

#[test]
#[should_panic(expected = "Auction ended")]
fn test_bid_after_deadline_is_rejected() {
    let fx = setup(200);
    set_ledger(&fx.env, 200, 10_000);
    fx.auction().bid(&fx.bidder1, &150);
}

#[test]
#[should_panic(expected = "Bid not higher than current highest")]
fn test_bid_must_exceed_current_highest() {
    let fx = setup(200);
    fx.auction().bid(&fx.bidder1, &150);
    fx.auction().bid(&fx.bidder2, &150);
}

#[test]
fn test_nft_is_escrowed_on_initialize() {
    let fx = setup(200);
    assert_eq!(fx.nft().balance(&fx.seller), 0);
    assert_eq!(fx.nft().balance(&fx.auction_id), 1);
}
