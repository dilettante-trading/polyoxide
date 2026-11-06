# CLOB live fills Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A hand-run live test that buys about $3 of one CLOB outcome token (a partial FAK and a FOK through `Trading::place`), sells it all back with a native FAK, and checks every fill path, kept out of the nightly by a new `manual` registry key.

**Architecture:** `scripts/gen_registry.py` learns `manual = "<why>"` on a live entry and leaves such entries out of `Registry.jobs()`, the one seam every nightly output reads; `claude_nightly` lists them as manual. The test, `polyoxide-polymarket/tests/live_clob_fills.rs`, picks a qualifying token before spending, runs the three orders inside `catch_unwind`, and a guard sells whatever is held on any failure.

**Tech Stack:** Python 3 + pytest (registry), Rust 2021 + tokio + `polyoxide-test-support` (live test), `rust_decimal`.

**Spec:** `docs/superpowers/specs/2026-11-06-clob-live-fills-design.md`.

**Running things:** every build, test run and lint goes through loom's queue:
`loom job run -d "<what>" -- <cmd>` (blocks, prints output) with Bash's `run_in_background` for long ones. CI tools come from the dev shell: prefix with `direnv exec .`. Stage and commit by path only.

---

## File map

| File | Change |
|---|---|
| `scripts/gen_registry.py` | `manual` key: docstring, `LIVE_KEYS`, `Live.manual`, `_live`, `Registry.jobs`, `claude_nightly` |
| `.github/scripts/tests/test_gen_registry.py` | synthetic tests for the key |
| `.github/scripts/tests/test_live_registry.py` | real-tree test for the manual entry; the jobs test compares non-manual entries |
| `polyoxide-polymarket/Cargo.toml` | `[[test]] live_clob_fills` and its `manual` live entry |
| `polyoxide-polymarket/tests/live_clob_fills.rs` | the test (new) |
| `CLAUDE.md` | regenerated `claude-nightly` region (never edited by hand) |
| `.claude/rules/live-tests.md`, `.claude/rules/polymarket-clob.md` | rule text (AD-21) |

---

### Task 1: the `manual` key in the registry

**Files:**
- Modify: `scripts/gen_registry.py` (docstring live block ~line 46-55, `LIVE_KEYS` line 129, `Live` ~line 169, `Registry.jobs` ~line 292, `_live` ~line 491, `claude_nightly` ~line 1113)
- Test: `.github/scripts/tests/test_gen_registry.py` (after `test_two_crates_may_not_share_a_runner_job`)

- [ ] **Step 1: Write the failing tests**

Append to `.github/scripts/tests/test_gen_registry.py`:

```python
# --- manual targets ------------------------------------------------------------

MANUAL = "spends money on every run"


def _with_manual_target(metadata, workspace, manifests):
    table = manifests["polyoxide-omega"]["package"]["metadata"]["polyoxide"]["live"]
    table["live_fills"] = {**live(features=["ws"], secrets=["A_KEY"]), "manual": MANUAL}


def test_a_manual_target_makes_no_job(tmp_path: Path) -> None:
    registry = build(tmp_path, _with_manual_target)
    jobs = registry.jobs()
    assert [job.id for job in jobs] == [
        "live-polyoxide-alpha-live", "live-polyoxide-alpha-slow", "live-polyoxide-omega-live"]
    omega = next(job for job in jobs if job.id == "live-polyoxide-omega-live")
    assert [t.target for t in omega.targets] == ["live_api"]
    assert omega.secrets == ()
    assert "live_fills" not in "\n".join(gen_registry.nightly_live_jobs(registry))
    assert "live_fills" not in "\n".join(gen_registry.selfheal_behavioral(registry))


def test_a_manual_target_is_listed_as_manual(tmp_path: Path) -> None:
    registry = build(tmp_path, _with_manual_target)
    assert gen_registry.claude_nightly(registry)[-1] == (
        "- Manual, never in the nightly: `live_fills` in `polyoxide-omega` with `--features ws`: "
        f"{MANUAL}. Run it by hand with `-- --ignored`.")


@pytest.mark.parametrize(
    ("change", "message"),
    [
        ({"manual": ""}, "manual must say why"),
        ({"manual": "   "}, "manual must say why"),
        ({"manual": True}, "manual must say why"),
        ({"runner": "wasm-bindgen", "secrets": []}, "is manual and names a runner"),
    ],
)
def test_a_bad_manual_entry_fails(tmp_path: Path, change: dict, message: str) -> None:
    def bad(metadata, workspace, manifests):
        _with_manual_target(metadata, workspace, manifests)
        table = manifests["polyoxide-omega"]["package"]["metadata"]["polyoxide"]["live"]
        table["live_fills"].update(change)

    with pytest.raises(RegistryError, match=message):
        build(tmp_path, bad)
```

- [ ] **Step 2: Run them to see them fail**

Run: `loom job run -d "registry manual tests (red)" -- direnv exec . sh -c 'cd .github/scripts && uv run pytest tests/test_gen_registry.py -q -k manual'`
Expected: 6 FAIL, each with `RegistryError: ... unknown key manual` (the key is not in `LIVE_KEYS` yet).

- [ ] **Step 3: Implement**

In the module docstring's live block, after the three `runner` lines, add:

```
    manual = "..."            # optional: why the target runs only by hand. It is registered
                              # and checked like any other, but makes no nightly job, and
                              # CLAUDE.md's nightly list names it as manual; no runner
```

`LIVE_KEYS` becomes:

```python
LIVE_KEYS = {"suite", "timeout", "features", "secrets", "note", "runner", "manual"}
```

`Live` gains a last field:

```python
    runner: str | None = None
    # Why the target runs only by hand; such a target makes no nightly job.
    manual: str | None = None
```

In `Registry.jobs`, skip manual targets:

```python
            for live in crate.live:
                if live.manual is None:
                    suites.setdefault(live.suite, []).append(live)
```

In `_live`, before the `return`:

```python
    manual = entry.get("manual")
    if manual is not None and (not isinstance(manual, str) or not manual.strip()):
        raise RegistryError(f"{where} manual must say why the target is run only by hand, "
                            f"not {manual!r}")
    if manual is not None and runner is not None:
        raise RegistryError(f"{where} is manual and names a runner; a runner exists only to run "
                            f"a target in the nightly")
    return Live(target, _id(f"{where} suite", entry["suite"]), timeout,
                _strings(f"{where} features", entry["features"]), secrets, note, runner, manual)
```

`claude_nightly` appends the manual targets after the job lines:

```python
def claude_nightly(r: Registry) -> list[str]:
    lines = []
    for job in r.jobs():
        targets = ", ".join(
            f"`{t.target}`" + (f" with `--features {','.join(t.features)}`" if t.features else "")
            for t in sorted(job.targets, key=lambda t: t.target))
        line = f"- `{job.id}` ({job.timeout} min): {targets}"
        lines.append(line + (f"; secrets {_code_list(job.secrets)}" if job.secrets else ""))
    for crate in r.crates:
        for t in sorted((t for t in crate.live if t.manual), key=lambda t: t.target):
            features = f" with `--features {','.join(t.features)}`" if t.features else ""
            lines.append(f"- Manual, never in the nightly: `{t.target}` in `{crate.name}`"
                         f"{features}: {t.manual}. Run it by hand with `-- --ignored`.")
    return lines
```

- [ ] **Step 4: Run the whole file**

Run: `loom job run -d "registry tests (green)" -- direnv exec . sh -c 'cd .github/scripts && uv run pytest tests/test_gen_registry.py tests/test_live_registry.py -q'`
Expected: all pass (the real tree has no manual entry yet, so its generated regions are unchanged).

- [ ] **Step 5: Commit**

```bash
git add scripts/gen_registry.py .github/scripts/tests/test_gen_registry.py
git commit -m "ci(registry): a live entry may be manual, registered and checked but never a nightly job" -- scripts/gen_registry.py .github/scripts/tests/test_gen_registry.py
```

(end the message with the session's `Co-Authored-By` line)

---

### Task 2: `live_clob_fills`

**Files:**
- Create: `polyoxide-polymarket/tests/live_clob_fills.rs`
- Modify: `polyoxide-polymarket/Cargo.toml` (after the `live_clob_trading` `[[test]]` at ~line 330, and after its live entry at ~line 34)
- Modify: `.github/scripts/tests/test_live_registry.py` (`test_the_generated_jobs_equal_the_registered_entries`; one new test after it)
- Regenerate: `CLAUDE.md`
- Modify: `.claude/rules/live-tests.md`, `.claude/rules/polymarket-clob.md`

- [ ] **Step 1: Register the target**

In `polyoxide-polymarket/Cargo.toml`, after the `live_clob_trading` `[[test]]` stanza:

```toml
[[test]]
name = "live_clob_fills"
required-features = ["clob-ws", "data", "gamma"]
```

After the `[package.metadata.polyoxide.live.live_clob_trading]` table:

```toml
[package.metadata.polyoxide.live.live_clob_fills]
suite = "clob"
timeout = 15
features = ["clob-ws", "data", "gamma"]
secrets = [
  "POLYMARKET_API_KEY",
  "POLYMARKET_API_PASSPHRASE",
  "POLYMARKET_API_SECRET",
  "POLYMARKET_PRIVATE_KEY",
]
manual = "spends real money: buys about $3 of one outcome token in two marketable orders and sells it back, at a cost of about the spread"
```

- [ ] **Step 2: Write the test file**

Create `polyoxide-polymarket/tests/live_clob_fills.rs`:

```rust
//! Live fills on the CLOB: one test buys about $3 of one outcome token in two
//! marketable orders, checks every fill path the SDK has, and sells it all
//! back. **It spends real money**, about the spread, so its registry entry is
//! `manual` and the nightly never runs it. Run it by hand, with the account in
//! the OS keychain:
//!
//! ```text
//! cargo test -p polyoxide-polymarket --no-default-features \
//!   --features clob-ws,data,gamma,keychain --test live_clob_fills -- --ignored --nocapture
//! ```
//!
//! Without credentials it fails as `auth-gated` and sends nothing. The design
//! is `docs/superpowers/specs/2026-11-06-clob-live-fills-design.md`.

use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{FutureExt, StreamExt};
use polyoxide_polymarket::clob::venue::FillExt;
use polyoxide_polymarket::clob::{
    Account, Clob, ClobBuilder, CreateOrderParams, Credentials, OrderBook, OrderKind, OrderLevel,
    OrderSide, SignatureType, Trade,
};
use polyoxide_polymarket::gamma::Gamma;
use polyoxide_polymarket::shared::keys::{self, TokenId};
#[cfg(feature = "keychain")]
use polyoxide_test_support::keychain;
use polyoxide_test_support::{
    environmental, fail, load_env, transient, ClassifiedResultExt, ResultExt,
};
use polyoxide_venue::{
    ClientOrderId, DynTrading, Fill, KillReason, Liquidity, MarketKey, Order, OrderId,
    OrderRequest, OrderStatus, Side, TimeInForce, Trading, TradingEvent, TradingEvents,
};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use tokio::time::Instant;

/// `units` at `scale` decimal places.
const fn dec(units: u32, scale: u32) -> Decimal {
    Decimal::from_parts(units, 0, 0, false, scale)
}

/// The least pUSD the wallet must hold before the test spends anything.
const LEAST_FUNDS: Decimal = dec(5, 0);
/// The cheapest best ask the test buys at.
const LOWEST_ASK: Decimal = dec(10, 2);
/// The dearest best ask the test buys at.
const HIGHEST_ASK: Decimal = dec(40, 2);
/// The one tick the test trades on, so a whole-share BUY pays in whole cents.
const TICK: Decimal = dec(1, 2);
/// The largest minimum order size the test accepts, and the one a book that
/// omits it is taken to have.
const LARGEST_MINIMUM: Decimal = dec(5, 0);
/// The widest spread the test accepts.
const WIDEST_SPREAD: Decimal = dec(2, 2);
/// The partial FAK takes the whole top ask level, which must cost less than this.
const THIN_LEVEL: Decimal = dec(150, 2);
/// The bids this close to the best bid must absorb everything the test buys.
const BID_BAND: Decimal = dec(3, 2);
/// How far above the best ask the FOK's limit may reach.
const FOK_REACH: Decimal = dec(2, 2);
/// How far under the best bid the sale is limited.
const SALE_REACH: Decimal = dec(3, 2);
/// A marketable BUY's least notional.
const MARKETABLE_MINIMUM: Decimal = dec(1, 0);
/// How far the pUSD balance may differ from the trades' cost.
const COST_TOLERANCE: Decimal = dec(1, 2);
/// How far a position may differ from the credited balance.
const POSITION_TOLERANCE: Decimal = dec(1, 2);
/// The CLOB's raw units per share and per pUSD.
const RAW_PER_UNIT: Decimal = dec(1_000_000, 0);
/// How many gamma-listed candidates are read on the CLOB before giving up.
const MAX_BOOK_PROBES: usize = 15;
/// How long an order's fills may take to arrive on the user channel.
const EVENT_WAIT: Duration = Duration::from_secs(30);
/// How long the account's trades may take to list an order.
const TRADES_WAIT: Duration = Duration::from_secs(30);
/// How long a balance may take to settle.
const CREDIT_WAIT: Duration = Duration::from_secs(90);
/// How long the positions may take to show a change.
const POSITIONS_WAIT: Duration = Duration::from_secs(60);
/// The pause between two reads of a balance or the trades.
const SHORT_POLL: Duration = Duration::from_secs(3);
/// The pause between two reads of the positions (Data v2 caches them 5 s).
const POSITIONS_POLL: Duration = Duration::from_secs(5);

/// The account the test trades as, and its API key, from `POLYMARKET_*` and
/// then the OS keychain under `polyoxide-clob` when the `keychain` feature is
/// on. Without all four the test fails as `auth-gated` before any request.
fn load_account() -> (Account, String) {
    let creds = load_env(&[
        "POLYMARKET_PRIVATE_KEY",
        "POLYMARKET_API_KEY",
        "POLYMARKET_API_SECRET",
        "POLYMARKET_API_PASSPHRASE",
    ]);
    #[cfg(feature = "keychain")]
    let creds = creds.or_else(|_| {
        keychain(
            "polyoxide-clob",
            &[
                ("POLYMARKET_PRIVATE_KEY", "private_key"),
                ("POLYMARKET_API_KEY", "api_key"),
                ("POLYMARKET_API_SECRET", "api_secret"),
                ("POLYMARKET_API_PASSPHRASE", "api_passphrase"),
            ],
        )
    });
    let creds = creds.unwrap_or_else(|missing| missing.or_auth_gated());
    let api_key = creds.get("POLYMARKET_API_KEY").to_owned();
    let account = Account::new(
        creds.get("POLYMARKET_PRIVATE_KEY"),
        Credentials {
            key: api_key.clone(),
            secret: creds.get("POLYMARKET_API_SECRET").to_owned(),
            passphrase: creds.get("POLYMARKET_API_PASSPHRASE").to_owned(),
        },
    )
    .or_fail("build the account from POLYMARKET_*");
    (account, api_key)
}

/// The account's CLOB client. The test account is a Polymarket proxy wallet.
fn client(account: Account) -> Clob {
    ClobBuilder::new()
        .with_account(account)
        .signature_type(SignatureType::PolyProxy)
        .build()
        .or_fail("authenticated clob client")
}

/// The test's clock, for unique client order ids.
fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis())
}

/// `text` as a decimal; text that is not one fails the test.
fn decimal(text: &str, what: &str) -> Decimal {
    let parsed = text.parse::<Decimal>();
    assert!(parsed.is_ok(), "{what} {text:?} is not a decimal");
    parsed.unwrap_or_default()
}

/// A raw CLOB amount (six decimals) in shares or pUSD.
fn units(raw: &str, what: &str) -> Decimal {
    decimal(raw, what) / RAW_PER_UNIT
}

/// `value` as the `f64` the native order parameters take.
fn float(value: Decimal) -> f64 {
    let float = value.to_f64();
    assert!(float.is_some(), "{value} has no f64");
    float.unwrap_or_default()
}

fn best_ask(book: &OrderBook) -> Option<Decimal> {
    book.asks.iter().map(|level| level.price).min()
}

fn best_bid(book: &OrderBook) -> Option<Decimal> {
    book.bids.iter().map(|level| level.price).max()
}

/// The shares the levels hold at `price`.
fn depth_at(levels: &[OrderLevel], price: Decimal) -> Decimal {
    levels
        .iter()
        .filter(|level| level.price == price)
        .map(|level| level.size)
        .sum()
}

/// The shares the bids hold at `floor` or above.
fn bids_from(book: &OrderBook, floor: Decimal) -> Decimal {
    book.bids
        .iter()
        .filter(|level| level.price >= floor)
        .map(|level| level.size)
        .sum()
}

/// The whole shares a marketable BUY at `ask` needs to reach its $1 minimum.
fn shares_for_a_dollar(ask: Decimal) -> Decimal {
    (MARKETABLE_MINIMUM / ask).ceil()
}

/// The partial FAK's size: more than the top level's `depth`, and enough for
/// the marketable minimum.
fn partial_size(depth: Decimal, ask: Decimal) -> Decimal {
    (depth.floor() + Decimal::ONE).max(shares_for_a_dollar(ask))
}

/// The FOK's size: the book's minimum, and enough for the marketable minimum.
fn fok_size(least: Decimal, ask: Decimal) -> Decimal {
    least.ceil().max(shares_for_a_dollar(ask))
}

/// The lowest ask price at which the asks up to it hold `size` shares.
fn fok_limit(book: &OrderBook, size: Decimal) -> Option<Decimal> {
    let mut asks: Vec<&OrderLevel> = book.asks.iter().collect();
    asks.sort_by(|a, b| a.price.cmp(&b.price));
    let mut held = Decimal::ZERO;
    for level in asks {
        held += level.size;
        if held >= size {
            return Some(level.price);
        }
    }
    None
}

/// The book's minimum order size, the largest one when it omits it.
fn least_size(book: &OrderBook) -> Option<Decimal> {
    match book.min_order_size.as_deref() {
        None => Some(LARGEST_MINIMUM),
        Some(text) => text.parse().ok(),
    }
}

/// The book's minimum order size, when the book admits the run: a best ask in
/// [`LOWEST_ASK`, `HIGHEST_ASK`] on a [`TICK`] tick, a small enough minimum, a
/// spread of at most [`WIDEST_SPREAD`], a top ask level cheaper than
/// [`THIN_LEVEL`], and enough bids within [`BID_BAND`] for both buys.
fn admits_the_run(book: &OrderBook) -> Option<Decimal> {
    let ask = best_ask(book)?;
    let bid = best_bid(book)?;
    let tick = book.tick_size.as_deref()?.parse::<Decimal>().ok()?;
    let least = least_size(book)?;
    let depth = depth_at(&book.asks, ask);
    let planned = partial_size(depth, ask) + LARGEST_MINIMUM.max(shares_for_a_dollar(ask));
    let fits = (LOWEST_ASK..=HIGHEST_ASK).contains(&ask)
        && tick == TICK
        && least <= LARGEST_MINIMUM
        && ask - bid <= WIDEST_SPREAD
        && depth * ask < THIN_LEVEL
        && bids_from(book, bid - BID_BAND) >= planned;
    fits.then_some(least)
}

/// The token the test trades, chosen before anything is spent.
struct Pick {
    token: String,
    key: MarketKey,
    slug: String,
    least: Decimal,
}

/// The first open market's token whose book admits the run and whose market
/// takes no taker fee and holds no taker delay. Fails as `environmental` when
/// none does, and with the last error when every book probe failed.
async fn pick_a_token(clob: &Clob) -> Pick {
    let gamma = Gamma::builder().build().or_fail("gamma client");
    let markets = gamma
        .markets()
        .list()
        .closed(false)
        .limit(100)
        .send()
        .await
        .or_fail("gamma list markets");
    let mut probed = 0usize;
    let mut errored = 0usize;
    let mut last_error = None;
    for market in markets.iter() {
        let gamma_ask = market.best_ask.and_then(|ask| Decimal::try_from(ask).ok());
        if !gamma_ask.is_some_and(|ask| (LOWEST_ASK..=HIGHEST_ASK).contains(&ask)) {
            continue;
        }
        // clob_token_ids is a JSON-encoded array string: '["id1", "id2"]'
        let token = market.clob_token_ids.as_ref().and_then(|ids| {
            serde_json::from_str::<Vec<String>>(ids)
                .ok()
                .and_then(|ids| ids.into_iter().next())
        });
        let Some(token) = token else {
            continue;
        };
        if probed >= MAX_BOOK_PROBES {
            break;
        }
        probed += 1;
        let book = match clob.markets().order_book(&token).send().await {
            Ok(book) => book,
            Err(probe_error) => {
                errored += 1;
                last_error = Some(probe_error);
                continue;
            }
        };
        let Some(least) = admits_the_run(&book) else {
            continue;
        };
        let details = clob
            .markets()
            .clob_market_details(market.condition_id.as_str())
            .send()
            .await
            .or_fail("read the market's CLOB details");
        if details.tbf.unwrap_or(0) != 0 || details.itode == Some(true) {
            continue;
        }
        let key = keys::clob(&TokenId::new(token.clone()).or_fail("name the token"));
        return Pick {
            token,
            key,
            slug: market.slug.clone().unwrap_or_default(),
            least,
        };
    }
    if let Some(probe_error) = last_error.filter(|_| errored == probed) {
        fail(
            &format!("every one of {probed} book probe(s) failed"),
            &probe_error,
        );
    }
    environmental(&format!(
        "no open market admits the run ({probed} book(s) probed): a best ask in \
         [{LOWEST_ASK}, {HIGHEST_ASK}] on a {TICK} tick, a spread of at most {WIDEST_SPREAD}, \
         a top ask level under ${THIN_LEVEL}, enough bids, no taker fee or delay; market \
         conditions rather than a defect, so re-run"
    ));
}

/// The account's pUSD balance. The CLOB reports no `available`.
async fn pusd(trading: &Arc<DynTrading<'static>>) -> Decimal {
    let balances = trading.balances().await.or_fail("read the pUSD balance");
    let total = balances
        .iter()
        .find(|balance| balance.asset == "pUSD")
        .and_then(|balance| balance.total);
    assert!(total.is_some(), "no pUSD total among {balances:?}");
    total.unwrap_or_default()
}

/// Fails as `environmental` unless the wallet lets every exchange the CLOB
/// reports move its outcome tokens, which the sale needs.
async fn require_approval(clob: &Clob, token: &str) {
    let held = clob
        .account_api()
        .or_fail("account_api")
        .balance_allowance(token)
        .send()
        .await
        .or_fail("read the token's balance and allowances");
    eprintln!("the outcome token's allowances: {:?}", held.allowances);
    let unapproved: Vec<&String> = held
        .allowances
        .iter()
        .filter(|(_, amount)| amount.trim_start_matches('0').is_empty())
        .map(|(spender, _)| spender)
        .collect();
    if !unapproved.is_empty() {
        environmental(&format!(
            "the wallet has not approved {unapproved:?} to move its outcome tokens, so it \
             could not sell them back; approve them once on polymarket.com"
        ));
    }
}

async fn read_book(clob: &Clob, token: &str) -> OrderBook {
    clob.markets()
        .order_book(token)
        .send()
        .await
        .or_fail("read the book")
}

/// What has matched of order `id`, and the address that trades for the
/// account (the proxy wallet), from `GET /data/order/{id}`.
async fn read_back(clob: &Clob, id: &OrderId) -> (Decimal, String) {
    let row = clob
        .orders()
        .or_fail("orders")
        .get(id.as_str())
        .send()
        .await
        .or_fail("read the order back");
    (
        decimal(&row.size_matched, "the order's size_matched"),
        row.maker_address,
    )
}

/// A buy of `size` shares at `price` through `Trading::place`, with a client
/// order id unique to the run.
async fn place(
    trading: &Arc<DynTrading<'static>>,
    key: &MarketKey,
    price: Decimal,
    size: Decimal,
    time_in_force: TimeInForce,
    what: &str,
) -> (Order, ClientOrderId) {
    let mut request = OrderRequest::new(key.clone(), Side::Buy, price, size);
    request.time_in_force = time_in_force;
    let client = ClientOrderId::new(format!("polyoxide-fills-{what}-{}", now_millis()));
    request.client_order_id = Some(client.clone());
    let order = trading
        .place(request)
        .await
        .or_fail(&format!("place the {what} buy"));
    eprintln!(
        "{what}: {size} at {price} -> {:?}, {} filled",
        order.status, order.filled
    );
    (order, client)
}

/// A native FAK SELL of `size` shares of `token`, limited at `price`.
fn sale(token: &str, price: Decimal, size: Decimal) -> CreateOrderParams {
    CreateOrderParams {
        token_id: token.to_owned(),
        price: float(price),
        size: float(size),
        side: OrderSide::Sell,
        order_type: OrderKind::Fak,
        post_only: false,
        expiration: None,
        funder: None,
        signature_type: Some(SignatureType::PolyProxy),
    }
}

/// The events stream, and the fills it has yielded, by order.
struct FillLog {
    events: TradingEvents,
    fills: HashMap<OrderId, Vec<Fill>>,
}

impl FillLog {
    /// Opens the account's events stream, whose first item must be `Resync`.
    async fn open(trading: &Arc<DynTrading<'static>>) -> Self {
        let mut events = trading.events();
        let first = match tokio::time::timeout(EVENT_WAIT, events.next()).await {
            Err(_) => transient("the events stream yielded nothing in its first 30 s"),
            Ok(None) => transient("the events stream ended before its first event"),
            Ok(Some(item)) => item.or_fail("the events stream's first item"),
        };
        assert!(
            matches!(first, TradingEvent::Resync),
            "an events stream begins with Resync, not {first:?}"
        );
        Self {
            events,
            fills: HashMap::new(),
        }
    }

    /// The fills of order `id`, once they sum to `filled`. Every other fill is
    /// kept for its own order; acks are skipped. A resync may have lost fills,
    /// so it fails as `transient`, as does [`EVENT_WAIT`] passing first.
    async fn fills_of(&mut self, id: &OrderId, filled: Decimal) -> Vec<Fill> {
        let deadline = Instant::now() + EVENT_WAIT;
        loop {
            let seen: Decimal = self.fills.get(id).map_or(Decimal::ZERO, |fills| {
                fills.iter().map(|fill| fill.size).sum()
            });
            if seen >= filled {
                return self.fills.remove(id).unwrap_or_default();
            }
            match tokio::time::timeout_at(deadline, self.events.next()).await {
                Err(_) => transient(&format!(
                    "the events stream yielded fills of {seen} of the {filled} order {id} \
                     filled within 30 s"
                )),
                Ok(None) => transient("the events stream ended before the fills"),
                Ok(Some(item)) => match item.or_fail("the events stream yields an item") {
                    TradingEvent::Fill(fill) => {
                        self.fills.entry(fill.order_id.clone()).or_default().push(fill)
                    }
                    TradingEvent::Resync => {
                        transient("the events stream resynced, so it may have lost fills")
                    }
                    _ => {}
                },
            }
        }
    }
}

/// The fills of order `id` are the account's taker fills of `filled` shares,
/// each on `side` at or better than `limit`, carrying `client`.
fn check_fills(
    fills: &[Fill],
    id: &OrderId,
    side: Side,
    limit: Decimal,
    filled: Decimal,
    client: Option<&ClientOrderId>,
) {
    let total: Decimal = fills.iter().map(|fill| fill.size).sum();
    assert_eq!(
        total, filled,
        "the fills of order {id} sum to {total}, not {filled}: {fills:?}"
    );
    for fill in fills {
        assert_eq!(fill.side, side, "a fill of order {id} is on the wrong side: {fill:?}");
        let within = match side {
            Side::Buy => fill.price <= limit,
            Side::Sell => fill.price >= limit,
        };
        assert!(within, "a fill of order {id} is beyond its limit {limit}: {fill:?}");
        assert_eq!(
            fill.liquidity,
            Some(Liquidity::Taker),
            "a fill of order {id} is not the taker's: {fill:?}"
        );
        assert_eq!(
            fill.client_order_id.as_ref(),
            client,
            "a fill of order {id} carries the wrong client id: {fill:?}"
        );
        assert!(
            fill.fee.is_none_or(|fee| fee.is_zero()),
            "a fill of order {id} charged a fee on a fee-free market: {fill:?}"
        );
        let taker = fill
            .extensions
            .get::<FillExt>()
            .and_then(|ext| ext.taker_order_id.as_deref());
        assert_eq!(
            taker,
            Some(id.as_str()),
            "a fill of order {id} names another taker order: {fill:?}"
        );
    }
}

/// The token's credited balance in shares, once it reaches `bought`.
async fn wait_for_credit(clob: &Clob, token: &str, bought: Decimal) -> Decimal {
    let deadline = Instant::now() + CREDIT_WAIT;
    loop {
        let account = clob.account_api().or_fail("account_api");
        account
            .update_balance_allowance(
                "CONDITIONAL",
                Some(token.to_owned()),
                Some(SignatureType::PolyProxy as u8),
            )
            .await
            .or_fail("refresh the token's balance");
        let held = account
            .balance_allowance(token)
            .send()
            .await
            .or_fail("read the token's balance");
        let credited = units(&held.balance, "the token's balance");
        if credited >= bought {
            return credited;
        }
        if Instant::now() >= deadline {
            transient(&format!(
                "the CLOB credited {credited} of the {bought} shares bought within {} s",
                CREDIT_WAIT.as_secs()
            ));
        }
        tokio::time::sleep(SHORT_POLL).await;
    }
}

/// Waits until the positions hold `want` of `key` (within
/// [`POSITION_TOLERANCE`]), or nothing of it when `want` is `None`.
async fn wait_for_position(
    trading: &Arc<DynTrading<'static>>,
    key: &MarketKey,
    want: Option<Decimal>,
) {
    let deadline = Instant::now() + POSITIONS_WAIT;
    loop {
        let positions = trading.positions().await.or_fail("read the positions");
        let held = positions
            .iter()
            .find(|position| position.key == *key)
            .map(|position| position.size);
        let settled = match want {
            Some(size) => held.is_some_and(|held| (held - size).abs() <= POSITION_TOLERANCE),
            None => held.is_none(),
        };
        if settled || Instant::now() >= deadline {
            assert!(
                settled,
                "after {} s the positions hold {held:?} of {key:?}, not {want:?}",
                POSITIONS_WAIT.as_secs()
            );
            return;
        }
        tokio::time::sleep(POSITIONS_POLL).await;
    }
}

/// Whether `text` is shaped as a UUID, as an API key is.
fn uuid_shaped(text: &str) -> bool {
    text.len() == 36
        && text.chars().filter(|c| *c == '-').count() == 4
        && text.chars().all(|c| c == '-' || c.is_ascii_hexdigit())
}

/// Checks each order's trades on `/data/trades` and returns what the run
/// cost: the buys' matched amounts less the sale's, each at the maker's price
/// for the test's token, `1 - price` for a maker on the complementary outcome.
async fn trades_cost(
    clob: &Clob,
    maker: &str,
    api_key: &str,
    token: &str,
    orders: &[(OrderId, OrderSide)],
) -> Decimal {
    let deadline = Instant::now() + TRADES_WAIT;
    let trades: Vec<Trade> = loop {
        let page = clob
            .account_api()
            .or_fail("account_api")
            .trades(maker)
            .send()
            .await
            .or_fail("read the account's trades")
            .data;
        let listed = orders.iter().all(|(id, _)| {
            page.iter()
                .any(|trade| trade.taker_order_id == id.as_str())
        });
        if listed || Instant::now() >= deadline {
            assert!(
                listed,
                "/data/trades lists no trade for one of {orders:?} within {} s",
                TRADES_WAIT.as_secs()
            );
            break page;
        }
        tokio::time::sleep(SHORT_POLL).await;
    };
    let mut cost = Decimal::ZERO;
    for (id, side) in orders {
        for trade in trades
            .iter()
            .filter(|trade| trade.taker_order_id == id.as_str())
        {
            assert_eq!(
                trade.owner, api_key,
                "trade {} of order {id} is owned by another key",
                trade.id
            );
            assert!(
                uuid_shaped(&trade.owner),
                "trade {}'s owner {} is not a UUID",
                trade.id,
                trade.owner
            );
            assert_eq!(
                trade.side, *side,
                "trade {} of order {id} is on the wrong side",
                trade.id
            );
            let stage = trade.status.as_str();
            assert!(
                ["MATCHED", "MINED", "CONFIRMED"].contains(&stage),
                "trade {} of order {id} is {stage}",
                trade.id
            );
            for entry in &trade.maker_orders {
                let amount = decimal(&entry.matched_amount, "a maker's matched_amount");
                let price = decimal(&entry.price, "a maker's price");
                let per_share = if entry.asset_id == token {
                    price
                } else {
                    Decimal::ONE - price
                };
                match side {
                    OrderSide::Buy => cost += amount * per_share,
                    OrderSide::Sell => cost -= amount * per_share,
                }
            }
        }
    }
    cost
}

/// What the run spent, once the pUSD balance has fallen from `baseline` by
/// `cost` (within [`COST_TOLERANCE`]).
async fn wait_for_cost(
    clob: &Clob,
    trading: &Arc<DynTrading<'static>>,
    baseline: Decimal,
    cost: Decimal,
) -> Decimal {
    let deadline = Instant::now() + CREDIT_WAIT;
    loop {
        clob.account_api()
            .or_fail("account_api")
            .update_balance_allowance("COLLATERAL", None, Some(SignatureType::PolyProxy as u8))
            .await
            .or_fail("refresh the pUSD balance");
        let spent = baseline - pusd(trading).await;
        let agrees = (spent - cost).abs() <= COST_TOLERANCE;
        if agrees || Instant::now() >= deadline {
            assert!(agrees, "pUSD fell by {spent}, but the trades cost {cost}");
            return spent;
        }
        tokio::time::sleep(SHORT_POLL).await;
    }
}

/// Sells what the account holds of the token, best effort, for a run that
/// failed after it bought. It never fails the test itself: what it could not
/// sell it prints, so it can be sold by hand.
async fn sell_what_is_held(clob: &Clob, pick: &Pick) {
    let stranded = |held: &str, why: &str| {
        eprintln!(
            "STRANDED: {held} shares of token {} ({}) are still held: {why}; sell them by \
             hand on polymarket.com",
            pick.token, pick.slug
        )
    };
    let held = match clob.account_api() {
        Ok(account) => account.balance_allowance(&pick.token).send().await,
        Err(cause) => Err(cause),
    };
    let held = match held
        .ok()
        .and_then(|held| held.balance.parse::<Decimal>().ok())
    {
        Some(raw) => raw / RAW_PER_UNIT,
        None => return stranded("an unknown number of", "the guard could not read the balance"),
    };
    let size = held.round_dp_with_strategy(2, RoundingStrategy::ToZero);
    if size < TICK {
        return;
    }
    let bid = clob
        .markets()
        .order_book(&pick.token)
        .send()
        .await
        .ok()
        .and_then(|book| best_bid(&book));
    let Some(bid) = bid else {
        return stranded(&size.to_string(), "the guard found no bid");
    };
    let price = (bid - SALE_REACH).max(TICK);
    match clob.place_order(&sale(&pick.token, price, size), None).await {
        Ok(response) if response.success => {
            let matched = match (&response.order_id, clob.orders()) {
                (Some(id), Ok(orders)) => orders
                    .get(id.as_str())
                    .send()
                    .await
                    .ok()
                    .and_then(|row| row.size_matched.parse::<Decimal>().ok()),
                _ => None,
            };
            match matched {
                Some(sold) if sold >= size => {
                    eprintln!("the guard sold the {size} shares held, at {price} or better")
                }
                Some(sold) => stranded(&(size - sold).to_string(), "the guard's sale fell short"),
                None => stranded(
                    &format!("up to {size}"),
                    "the guard could not read its sale back",
                ),
            }
        }
        Ok(response) => stranded(
            &size.to_string(),
            &format!("the venue refused the guard's sale: {:?}", response.error_msg),
        ),
        Err(cause) => stranded(
            &size.to_string(),
            &format!("the guard's sale failed: {cause}"),
        ),
    }
}

/// The three orders and every check on them. The caller catches a panic
/// here and sells what is held before raising it again.
async fn round_trip(
    clob: &Clob,
    trading: &Arc<DynTrading<'static>>,
    log: &mut FillLog,
    pick: &Pick,
    api_key: &str,
    baseline: Decimal,
) {
    // 1. The partial FAK: the whole top ask level, and a share more.
    let book = read_book(clob, &pick.token).await;
    let ask = best_ask(&book).unwrap_or_else(|| environmental("the book has no asks"));
    let size = partial_size(depth_at(&book.asks, ask), ask);
    let (partial, partial_client) = place(
        trading,
        &pick.key,
        ask,
        size,
        TimeInForce::ImmediateOrCancel,
        "partial FAK",
    )
    .await;
    if partial.filled.is_zero() {
        environmental(
            "the partial FAK matched nothing: the top ask level was taken first, and nothing \
             was spent",
        );
    }
    let reached_partial = matches!(
        partial.status,
        OrderStatus::Killed {
            reason: KillReason::Unmatched
        }
    );
    assert!(
        reached_partial || matches!(partial.status, OrderStatus::Filled),
        "the partial FAK ended neither killed nor filled: {partial:?}"
    );
    if reached_partial {
        assert!(
            partial.filled < size,
            "a killed FAK filled its whole size: {partial:?}"
        );
    }
    let fills = log.fills_of(&partial.id, partial.filled).await;
    check_fills(
        &fills,
        &partial.id,
        Side::Buy,
        ask,
        partial.filled,
        Some(&partial_client),
    );
    let (matched, maker) = read_back(clob, &partial.id).await;
    assert_eq!(
        matched, partial.filled,
        "GET /data/order disagrees with the partial FAK: {partial:?}"
    );

    // 2. The FOK: whole shares worth at least $1, limited to the first price
    // that covers them.
    let book = read_book(clob, &pick.token).await;
    let ask = best_ask(&book).unwrap_or_else(|| environmental("the book has no asks"));
    let size = fok_size(pick.least, ask);
    let Some(limit) = fok_limit(&book, size).filter(|limit| *limit <= ask + FOK_REACH) else {
        environmental(&format!(
            "the asks within {FOK_REACH} of {ask} hold fewer than {size} shares; nothing was \
             spent on the FOK"
        ));
    };
    let (fok, fok_client) = place(
        trading,
        &pick.key,
        limit,
        size,
        TimeInForce::FillOrKill,
        "FOK",
    )
    .await;
    if matches!(
        fok.status,
        OrderStatus::Killed {
            reason: KillReason::Unfillable
        }
    ) {
        environmental("the book moved: the FOK could not fill, and nothing was spent on it");
    }
    assert!(
        matches!(fok.status, OrderStatus::Filled) && fok.filled == size,
        "the FOK did not fill in full: {fok:?}"
    );
    let fills = log.fills_of(&fok.id, fok.filled).await;
    check_fills(&fills, &fok.id, Side::Buy, limit, fok.filled, Some(&fok_client));
    let (matched, _) = read_back(clob, &fok.id).await;
    assert_eq!(
        matched, fok.filled,
        "GET /data/order disagrees with the FOK: {fok:?}"
    );

    // 3. The buys are credited and listed.
    let credited = wait_for_credit(clob, &pick.token, partial.filled + fok.filled).await;
    wait_for_position(trading, &pick.key, Some(credited)).await;

    // 4. The sale of everything held, through the native client.
    let book = read_book(clob, &pick.token).await;
    let bid = best_bid(&book).unwrap_or_else(|| environmental("the book has no bids to sell to"));
    let price = (bid - SALE_REACH).max(TICK);
    let size = credited.round_dp_with_strategy(2, RoundingStrategy::ToZero);
    let response = clob
        .place_order(&sale(&pick.token, price, size), None)
        .await
        .or_fail("place the sale");
    eprintln!("sale: {size} at {price} or better -> {response:?}");
    let refusal = response.error_msg.clone();
    assert!(response.success, "the venue refused the sale: {refusal:?}");
    let word = response.status.clone().map(|word| word.to_ascii_lowercase());
    assert_eq!(
        word.as_deref(),
        Some("matched"),
        "the sale did not match at once: {response:?}"
    );
    let id = response.order_id.clone().map(OrderId::new);
    assert!(id.is_some(), "a matched sale names no order: {response:?}");
    let id = id.unwrap_or_else(|| OrderId::new(""));
    let (sold, _) = read_back(clob, &id).await;
    if sold < size {
        sell_what_is_held(clob, pick).await;
        let book = read_book(clob, &pick.token).await;
        if bids_from(&book, bid - SALE_REACH) < size - sold {
            environmental(&format!(
                "the sale filled {sold} of {size}: the bids thinned before it, and the guard \
                 sold the rest"
            ));
        }
    }
    assert_eq!(
        sold, size,
        "the sale filled {sold} of {size} though the bids held the rest"
    );
    let making = decimal(
        response.making_amount.as_deref().unwrap_or_default(),
        "the sale's making_amount",
    );
    let taking = decimal(
        response.taking_amount.as_deref().unwrap_or_default(),
        "the sale's taking_amount",
    );
    assert_eq!(
        making, sold,
        "the sale's making_amount is not the shares sold: {response:?}"
    );
    assert!(
        taking >= sold * price && taking <= sold * bid,
        "the sale's taking_amount {taking} is outside [{}, {}]: {response:?}",
        sold * price,
        sold * bid
    );
    let fills = log.fills_of(&id, sold).await;
    check_fills(&fills, &id, Side::Sell, price, sold, None);

    // 5. The trades, and what they cost.
    let orders = [
        (partial.id.clone(), OrderSide::Buy),
        (fok.id.clone(), OrderSide::Buy),
        (id.clone(), OrderSide::Sell),
    ];
    let cost = trades_cost(clob, &maker, api_key, &pick.token, &orders).await;

    // 6. The sale empties the position, and the balance moves by the cost.
    wait_for_position(trading, &pick.key, None).await;
    let spent = wait_for_cost(clob, trading, baseline, cost).await;
    eprintln!("the round trip cost {spent} pUSD");
    if !reached_partial {
        environmental(
            "the top ask level refilled before the partial FAK, so it filled in full and the \
             partial branch was not reached; everything else was checked",
        );
    }
}

/// A partial FAK buy and a FOK buy through `Trading::place`, then a native FAK
/// sale of everything bought, each checked against its read-back, its fills on
/// the user channel, its trade, the positions and the pUSD balance.
#[tokio::test]
#[ignore]
async fn live_clob_fills_buy_partially_and_in_full_then_sell_back() {
    let (account, api_key) = load_account();
    let clob = client(account);
    let trading = DynTrading::new_arc(clob.clone());

    let funds = pusd(&trading).await;
    if funds < LEAST_FUNDS {
        environmental(&format!(
            "the proxy wallet holds {funds} pUSD; fund it with at least {LEAST_FUNDS} pUSD"
        ));
    }
    let pick = pick_a_token(&clob).await;
    eprintln!("trading token {} ({})", pick.token, pick.slug);
    require_approval(&clob, &pick.token).await;
    let mut log = FillLog::open(&trading).await;
    let baseline = pusd(&trading).await;

    let run = AssertUnwindSafe(round_trip(
        &clob, &trading, &mut log, &pick, &api_key, baseline,
    ))
    .catch_unwind()
    .await;
    if let Err(failure) = run {
        sell_what_is_held(&clob, &pick).await;
        std::panic::resume_unwind(failure);
    }
}
```

- [ ] **Step 3: Build and lint it, without and with `keychain`**

Run: `loom job run -d "clippy live_clob_fills" -- direnv exec . sh -c 'cargo clippy -p polyoxide-polymarket --no-default-features --features clob-ws,data,gamma --test live_clob_fills -- -D warnings && cargo clippy -p polyoxide-polymarket --no-default-features --features clob-ws,data,gamma,keychain --test live_clob_fills -- -D warnings'`
Expected: both finish with no warnings. Fix anything clippy names before moving on (the code above is the target shape; a lint fix that changes behaviour is a deviation to note).

- [ ] **Step 4: Prove the credential-free run sends nothing**

Run: `loom job run -d "live_clob_fills without credentials" -- direnv exec . cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data,gamma --test live_clob_fills -- --ignored`
Expected: FAIL, the output holding `polyoxide-class=auth-gated` and `POLYMARKET_PRIVATE_KEY` (or another `POLYMARKET_*` name) absent; no other request line. (No `keychain` feature, and no `POLYMARKET_*` in the environment or a `.env`: check `ls .env 2>/dev/null` is empty first.)

- [ ] **Step 5: The real-tree registry tests**

In `.github/scripts/tests/test_live_registry.py`, the last line of `test_the_generated_jobs_equal_the_registered_entries` becomes:

```python
    assert selected == {key for key, live in registered(REGISTRY).items() if live.manual is None}
```

and after that test add:

```python
def test_a_manual_target_makes_no_job_and_is_named_in_claude_md() -> None:
    manual = {key: live for key, live in registered(REGISTRY).items() if live.manual}
    assert set(manual) == {("polyoxide-polymarket", "live_clob_fills")}
    workflow = (REPO / ".github/workflows/nightly-behavioral.yml").read_text()
    claude = (REPO / "CLAUDE.md").read_text()
    for crate, target in manual:
        assert target not in workflow, f"{target} is manual, but the nightly names it"
        assert f"`{target}` in `{crate}`" in claude, f"CLAUDE.md does not name manual {target}"
```

- [ ] **Step 6: Regenerate the generated regions**

Run: `direnv exec . python3 scripts/gen_registry.py --write`
Expected: it rewrites only CLAUDE.md's `claude-nightly` region, adding one line:
``- Manual, never in the nightly: `live_clob_fills` in `polyoxide-polymarket` with `--features clob-ws,data,gamma`: spends real money: buys about $3 of one outcome token in two marketable orders and sells it back, at a cost of about the spread. Run it by hand with `-- --ignored`.``
Check with `git diff --stat` that no workflow or SELF-HEALING.md changed.

- [ ] **Step 7: The rule text (AD-21)**

In `.claude/rules/live-tests.md`, after the first paragraph (ending "run with `-- --ignored`."), add:

```markdown
A live entry with `manual = "<why>"` is registered and checked like any other (its features, its
secrets, its `#[ignore]`s, `live_unwraps.py`), but `gen_registry.py` makes no nightly job of it,
and CLAUDE.md's generated nightly list names it as manual; it may not name a `runner`. Today's one
is `polyoxide-polymarket`'s `live_clob_fills`, which spends real money (`polymarket-clob.md`).
```

In `.claude/rules/polymarket-clob.md`, after the "Heartbeats arm a dead man's switch" section, add:

```markdown
## The manual fill test

`tests/live_clob_fills.rs` buys about $3 of one outcome token (a partial FAK and a FOK through
`Trading::place`), sells everything back with a native FAK, and checks each order against
`GET /data/order`, `/data/trades`, the user channel's `Fill`s, the positions and the pUSD balance,
the cost read from the trades' maker entries (`1 - price` for a maker on the complementary
outcome). It spends about the spread, so its live entry is `manual` and the nightly never runs it;
on any failure a guard sells what is held, and prints `STRANDED:` with the token for what it could
not sell. None of its orders rest, so it and `live_heartbeat` cannot cancel each other's orders.
Spec: `docs/superpowers/specs/2026-11-06-clob-live-fills-design.md`.
```

- [ ] **Step 8: Run the scripts that read the new file**

Run: `loom job run -d "registry + live_unwraps" -- direnv exec . sh -c 'cd .github/scripts && uv run pytest tests/test_gen_registry.py tests/test_live_registry.py tests/test_claude_rules.py -q && cd ../.. && python3 scripts/live_unwraps.py && python3 scripts/gen_registry.py --check'`
Expected: all pass; `live_unwraps.py` reports no problem (the file has no `.unwrap()`, `.expect(`, `panic!` or error- or status-named assertion message, so it needs no baseline entry).

- [ ] **Step 9: Commit**

```bash
git add polyoxide-polymarket/Cargo.toml polyoxide-polymarket/tests/live_clob_fills.rs .github/scripts/tests/test_live_registry.py CLAUDE.md .claude/rules/live-tests.md .claude/rules/polymarket-clob.md
git commit -m "test(clob): live_clob_fills, a manual live test that buys, fills and sells back" -- polyoxide-polymarket/Cargo.toml polyoxide-polymarket/tests/live_clob_fills.rs .github/scripts/tests/test_live_registry.py CLAUDE.md .claude/rules/live-tests.md .claude/rules/polymarket-clob.md
```

(end the message with the session's `Co-Authored-By` line)

---

### Task 3: the CI gates

- [ ] **Step 1: format**

Run: `direnv exec . cargo fmt --all -- --check`
Expected: no output. If it reports a diff, run `direnv exec . cargo fmt --all`, and amend nothing: commit the formatting as `style: rustfmt` by path.

- [ ] **Step 2: lint (the whole workspace, as CI)**

Run (background): `loom job run -d "clippy all" -- direnv exec . cargo clippy --all-targets --all-features -- -D warnings`
Expected: no warnings.

- [ ] **Step 3: the CI scripts job**

Run: `loom job run -d "CI scripts job" -- direnv exec . sh -c 'cd .github/scripts && uv sync && uv run pytest tests/ -q && cd ../.. && python3 scripts/runtime_gate.py'`
Expected: all pass.

- [ ] **Step 4: test bodies**

Run: `loom job run -d "test bodies" -- direnv exec . python3 scripts/test_body_diff.py --baseline v0.40.0`
Expected: pass (the new file and stanza are additions; nothing existing moved).

---

### Task 4: the live run (owner's go-ahead first)

- [ ] **Step 1: Ask the owner** before running: the test spends real money (about the spread on up to ~$3.60 of one outcome).

- [ ] **Step 2: Run it**

Run (background): `loom job run -d "live_clob_fills (real money)" -- direnv exec . cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data,gamma,keychain --test live_clob_fills -- --ignored --nocapture`
Expected: PASS, printing the token, each order's status and fills, and `the round trip cost <x> pUSD`. An `environmental` result names the market condition and spends nothing (or, after money moved, the guard's sale). Any `STRANDED:` line is reported to the owner at once, with the token and size.

- [ ] **Step 3: Record the result**

Update the memory note `polyoxide-live-order-safety.md` with what the run proved (or what it found), and tell the owner. A wire fact the run surfaced (the units of `making_amount`/`taking_amount`, the trade `price` of a multi-maker taker trade, what `/data/trades?maker_address=` lists for a taker) goes into `docs/specs/clob/OBSERVED.md` in a follow-up commit.
