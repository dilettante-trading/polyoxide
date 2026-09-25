//! Live round trip for a Deposit Wallet and one session key.
//!
//! authorize (relay, owner) → derive session credentials (clob L1, session key)
//! → place a resting GTC (clob, session key) → list from the session key and from
//! the owner → cancel (session key) → revoke (relay, owner) → confirm the key is
//! gone from `GET /v1/user/session-signers`.
//!
//! Gated behind `#[ignore]` and a Deposit Wallet fixture account that does not
//! exist yet: session-key management is enabled per Builder API key by the venue
//! (prader-rs #125). Until it exists this file only has to compile. Run with
//!
//! ```sh
//! cargo test -p polyoxide-clob --test live_session_keys -- --ignored
//! ```
//!
//! Environment (a `.env` is picked up by dotenvy):
//!
//! | Variable | Meaning |
//! |---|---|
//! | `POLYMARKET_DW_OWNER_PRIVATE_KEY` | The owner EOA's hex key. |
//! | `POLYMARKET_DW_WALLET` | The Deposit Wallet address (resolve it with `RelayClient::resolve_wallet`). |
//! | `POLYMARKET_DW_SESSION_PRIVATE_KEY` | A fresh EOA the test authorizes and then revokes. |
//! | `BUILDER_API_KEY`, `BUILDER_SECRET`, `BUILDER_PASS_PHRASE` (optional) | Builder HMAC credentials, enabled for session-key management. |
//!
//! When any is missing the test panics with the wording the nightly classifier
//! treats as auth-gated (`AUTH_GATED_RE` in `.github/scripts/classify_failures.py`),
//! so the nightly logs and skips it instead of filing an issue.

use std::time::{Duration, Instant};

use alloy::primitives::Address;
use alloy::signers::local::PrivateKeySigner;
use polyoxide_clob::{
    Account, Clob, ClobBuilder, CreateOrderParams, Credentials, DepositWalletRole, OrderKind,
    OrderSide, SessionSignerScope, SigningTarget,
};
use polyoxide_gamma::Gamma;
use polyoxide_relay::{BuilderAccount, BuilderConfig, RelayClient, WalletType};
use rust_decimal::Decimal;

/// How long to wait for the relayer to report a session-signer batch terminal
/// and for the registry to reflect it. The venue allows five minutes.
const REGISTRY_TIMEOUT: Duration = Duration::from_secs(6 * 60);
const POLL_INTERVAL: Duration = Duration::from_secs(5);
const MAX_BOOK_PROBES: usize = 10;

struct Fixture {
    owner: PrivateKeySigner,
    wallet: Address,
    session: PrivateKeySigner,
    builder: BuilderConfig,
}

/// Load the fixture account, or panic in the auth-gated wording. Never
/// soft-skip: a test that asserted nothing must not report `ok`.
fn load_fixture() -> Fixture {
    dotenvy::dotenv().ok();
    let var = |name: &str| {
        std::env::var(name).unwrap_or_else(|_| {
            panic!(
                "POLYMARKET_* env vars required for the Deposit Wallet round trip \
                 ({name} unset; see docs/specs/session-keys/README.md, Live verification)"
            )
        })
    };
    let owner: PrivateKeySigner = var("POLYMARKET_DW_OWNER_PRIVATE_KEY")
        .parse()
        .expect("POLYMARKET_DW_OWNER_PRIVATE_KEY is a hex private key");
    let wallet: Address = var("POLYMARKET_DW_WALLET")
        .parse()
        .expect("POLYMARKET_DW_WALLET is an address");
    let session: PrivateKeySigner = var("POLYMARKET_DW_SESSION_PRIVATE_KEY")
        .parse()
        .expect("POLYMARKET_DW_SESSION_PRIVATE_KEY is a hex private key");
    let builder = BuilderConfig::new(
        var("BUILDER_API_KEY"),
        var("BUILDER_SECRET"),
        std::env::var("BUILDER_PASS_PHRASE").ok(),
    );
    Fixture {
        owner,
        wallet,
        session,
        builder,
    }
}

/// Derive (or create) L2 credentials for `signer` under plain L1 auth, then build
/// a clob client whose orders are signed for the Deposit Wallet in `role`.
async fn clob_for(signer: &PrivateKeySigner, wallet: Address, role: DepositWalletRole) -> Clob {
    // L1 needs a signer and no credentials; the placeholder is never sent.
    let placeholder = Credentials {
        key: String::new(),
        secret: String::new(),
        passphrase: String::new(),
    };
    let l1 = ClobBuilder::new()
        .with_account(Account::with_signer(signer.clone(), placeholder))
        .build()
        .expect("L1 client");
    let derived = match l1.auth().expect("auth").derive_api_key(0).send().await {
        Ok(creds) => creds,
        Err(derive_err) => l1
            .auth()
            .expect("auth")
            .create_api_key(0)
            .send()
            .await
            .unwrap_or_else(|create_err| {
                panic!("derive api key failed ({derive_err}); create api key failed ({create_err})")
            }),
    };
    let credentials = Credentials {
        key: derived.api_key,
        secret: derived.secret,
        passphrase: derived.passphrase,
    };
    ClobBuilder::new()
        .with_account(
            Account::with_signer(signer.clone(), credentials)
                .with_target(SigningTarget::DepositWallet { wallet, role }),
        )
        .build()
        .expect("deposit wallet clob client")
}

fn relay_for_owner(fx: &Fixture) -> RelayClient {
    let account = BuilderAccount::with_signer(
        fx.owner.clone(),
        Some(polyoxide_relay::AuthConfig::Builder(fx.builder.clone())),
    );
    RelayClient::builder()
        .expect("relay builder")
        .with_account(account)
        .wallet_type(WalletType::DepositWallet)
        .deposit_wallet(fx.wallet)
        .build()
        .expect("relay client")
}

/// Wait until the owner's session-signer list does (`present == true`) or does
/// not contain `session`, or fail after `REGISTRY_TIMEOUT`.
async fn wait_for_registry(owner_clob: &Clob, session: Address, present: bool) {
    let started = Instant::now();
    loop {
        let listed = owner_clob
            .account_api()
            .expect("account api")
            .list_session_signers()
            .await
            .expect("list session signers");
        let found = listed.signers.iter().any(|s| s.address == session);
        if found == present {
            return;
        }
        assert!(
            started.elapsed() < REGISTRY_TIMEOUT,
            "session signer {session} {} in the registry after {:?}: {listed:?}",
            if present {
                "never appeared"
            } else {
                "is still"
            },
            started.elapsed()
        );
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Wait for a relayer transaction to reach a terminal state and assert success.
async fn wait_for_transaction(relay: &RelayClient, id: &str) {
    let started = Instant::now();
    loop {
        let tx = relay
            .get_gasless_transaction(id)
            .await
            .expect("get gasless transaction");
        if tx.state.is_terminal() {
            assert!(tx.state.is_success(), "transaction {id} failed: {tx:?}");
            return;
        }
        assert!(
            started.elapsed() < REGISTRY_TIMEOUT,
            "transaction {id} not terminal after {:?}: {tx:?}",
            started.elapsed()
        );
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// An open market whose CLOB best ask sits strictly above `min_ask`, so a bid at
/// `min_ask` rests. Same selection as `live_api.rs`; selected on the URL it is
/// asserted on, never on gamma's cached figure alone.
async fn find_token_id_with_min_ask(min_ask: Decimal) -> String {
    let gamma = Gamma::builder().build().expect("gamma client");
    let markets = gamma
        .markets()
        .list()
        .closed(false)
        .limit(100)
        .send()
        .await
        .expect("gamma list markets");
    let clob = Clob::public();
    let mut probed = 0usize;
    for market in markets.iter() {
        let gamma_ask = market.best_ask.and_then(|ask| Decimal::try_from(ask).ok());
        if gamma_ask.is_none_or(|ask| ask <= min_ask) {
            continue;
        }
        let token_id = market.clob_token_ids.as_ref().and_then(|ids| {
            serde_json::from_str::<Vec<String>>(ids)
                .ok()
                .and_then(|v| v.into_iter().next())
        });
        let Some(token_id) = token_id else {
            continue;
        };
        probed += 1;
        if let Ok(book) = clob.markets().order_book(&token_id).send().await {
            let best_ask = book.asks.iter().map(|level| level.price).min();
            if best_ask.is_some_and(|ask| ask > min_ask) {
                return token_id;
            }
        }
        if probed >= MAX_BOOK_PROBES {
            break;
        }
    }
    panic!("no suitable market: no open market with a best ask above {min_ask} in {probed} books");
}

#[tokio::test]
#[ignore] // live; needs a Deposit Wallet fixture account and an enabled Builder key
async fn live_session_key_round_trip() {
    let fx = load_fixture();
    // Select the market before touching the registry, so a "no suitable market"
    // environmental panic leaves nothing authorized behind it.
    let min_ask = Decimal::new(1, 2);
    let token_id = find_token_id_with_min_ask(min_ask).await;

    let session_address = fx.session.address();
    let relay = relay_for_owner(&fx);
    let owner_clob = clob_for(&fx.owner, fx.wallet, DepositWalletRole::Owner).await;

    // 0. Self-heal: an earlier aborted run may have left the key authorized, and
    //    perhaps an order resting. Revocation also sweeps the key's open orders.
    let listed = owner_clob
        .account_api()
        .expect("account api")
        .list_session_signers()
        .await
        .expect("list session signers");
    if listed.signers.iter().any(|s| s.address == session_address) {
        eprintln!(
            "cleanup: session signer {session_address} is still authorized from an earlier \
             run; revoking it first"
        );
        let revoked = relay
            .revoke_session_signer(session_address)
            .await
            .expect("revoke leftover session signer");
        assert!(
            !revoked.status.is_terminal_failure(),
            "revocation of the leftover session signer refused: {revoked:?}"
        );
        wait_for_registry(&owner_clob, session_address, false).await;
    }

    // 1. Authorize the session key (owner signs the batch; Builder HMAC submits it).
    let submitted = relay
        .authorize_session_signer(session_address, vec![SessionSignerScope::Clob])
        .await
        .expect("authorize session signer");
    assert!(
        !submitted.status.is_terminal_failure(),
        "authorization refused: {submitted:?}"
    );
    wait_for_transaction(&relay, &submitted.transaction_id).await;
    wait_for_registry(&owner_clob, session_address, true).await;

    // 2. The session key derives its own credentials and places a resting order.
    let session_clob = clob_for(&fx.session, fx.wallet, DepositWalletRole::SessionKey).await;
    const RESTING_PRICE: f64 = 0.01;
    let params = CreateOrderParams {
        token_id,
        price: RESTING_PRICE,
        size: 5.0,
        side: OrderSide::Buy,
        order_type: OrderKind::Gtc,
        post_only: true,
        expiration: None,
        funder: None,
        // Defaults to type 3 from the account's target.
        signature_type: None,
    };
    let resp = session_clob
        .place_order(&params, None)
        .await
        .expect("place order as session key");
    assert!(
        resp.success,
        "session-key order rejected: {:?}",
        resp.error_msg
    );
    let order_id = resp.order_id.expect("accepted order must return an id");

    // 3. Visibility, both directions. The page says the owner cannot see it; the
    //    assertion is one-directional and the owner's answer is only recorded.
    //    Nothing is asserted until the cancel below has run, so a failed
    //    visibility claim never strands the order.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let from_session = session_clob
        .orders()
        .expect("orders")
        .list()
        .send()
        .await
        .expect("list as session key");
    let session_view = from_session
        .data
        .iter()
        .find(|o| o.id == order_id)
        .map(|o| o.size_matched.clone());
    let from_owner = owner_clob
        .orders()
        .expect("orders")
        .list()
        .send()
        .await
        .expect("list as owner");
    eprintln!(
        "visibility: owner {} order {order_id} placed by the session key",
        if from_owner.data.iter().any(|o| o.id == order_id) {
            "SEES"
        } else {
            "does not see"
        }
    );

    // 4. Cancel with the key that placed it.
    let cancelled = session_clob
        .orders()
        .expect("orders")
        .cancel(order_id.clone())
        .send()
        .await
        .expect("cancel as session key");
    assert!(
        cancelled.canceled.contains(&order_id),
        "cancel did not report {order_id}: {cancelled:?}"
    );

    // The deferred visibility assertions, now that the order is off the book.
    let session_sees = session_view.is_some();
    assert!(
        session_sees,
        "session key cannot see its own order {order_id}: {from_session:?}"
    );
    assert_eq!(
        session_view.as_deref(),
        Some("0"),
        "session-key order {order_id} must rest unfilled, not execute"
    );

    // 5. Revoke and confirm the key leaves the registry.
    let revoked = relay
        .revoke_session_signer(session_address)
        .await
        .expect("revoke session signer");
    assert!(
        !revoked.status.is_terminal_failure(),
        "revocation refused: {revoked:?}"
    );
    wait_for_registry(&owner_clob, session_address, false).await;
}
