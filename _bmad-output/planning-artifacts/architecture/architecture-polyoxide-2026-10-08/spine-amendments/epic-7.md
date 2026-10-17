# Proposed spine amendments: Epic 7

Recorded per AD-21: an epic records each proposed amendment here, and one session later merges this file into the spine and regenerates the guide.

## A7-1: `Trading::events()` returns the `TradingEvents` newtype (amends AD-5's stream rule for dyn-held traits)

- **Proposed by:** the user, 2026-10-12, during bundle N's Story 7.3, as bundle N's decision N-D3, made when implementation found the conflict. Epic 7's design of 2026-10-17 adopts it (spec D2, in `docs/superpowers/specs/2026-10-17-epic-7-supervised-clob-sockets-and-trading-design.md`) and Story 4 ships it.
- **Current rule (AD-5):** "Streams are written `-> impl Stream<Item = …> + Send + Unpin + 'static` (futures-core). Implementations return `Box::pin(..)` or another `Unpin` stream." The rule applies to every dyn-held trait, each carrying `#[dynosaur::dynosaur(pub Dyn<Trait> = dyn(box) <Trait>)]`.
- **Proposed rule:**
  - **The return type.** A dyn-held trait's stream method returns a named, concrete stream type, not `impl Stream`. For `Trading` this is `pub struct TradingEvents(Pin<Box<dyn Stream<Item = Result<TradingEvent, ClassifiedError>> + Send + 'static>>)`.
  - **Its API.** It is built with `TradingEvents::new(stream)` and implements `Stream` by delegation. It is `Send`, `Unpin` and `'static`.
  - **Its documentation.** The type documents the replay-nothing and `Resync` contract (AD-24, as A7-2 amends it).
  - **Futures are unchanged.** Every future-returning method keeps `impl Future<Output = …> + Send`, which dynosaur handles.
- **Why:**
  - **dynosaur cannot keep `'static` on a returned `impl Trait`.** dynosaur 0.3.1, the newest release, removes every lifetime bound from a returned `impl Trait` and substitutes the method's own borrow lifetime (`dynosaur_derive` 0.3.1, `src/expand.rs`: `expand_sig_ret_ty` boxes the returned `impl Trait` as a `dyn` with the method's lifetime, and `expand_bounds`, at line 247, drops every lifetime bound). Through `Arc<DynTrading<'static>>`, `events()` would therefore return a stream that borrows the `Arc`. Calling it fails with E0597 ("argument requires that `x` is borrowed for `'static`"), so a consumer that picks its venue at run time could not move its event stream onto a task. Static dispatch was unaffected, which is why only a dyn test shows the problem.
  - **A concrete type passes through.** dynosaur leaves a return type that is not `impl Trait` as written, so the stream keeps `'static` on both paths. Implementations box the stream either way, as AD-5 already said.
  - **A newtype, not an alias.** The representation can change later without a break, and the type has a page of its own on which to document the stream's contract.
- **Follow-up in the code:** Story 4 ships it (`polyoxide-venue/src/trading.rs`, feature `trading`). `polyoxide-venue/tests/trading.rs` proves the claim rather than asserting it: it takes the stream from a temporary `Arc<DynTrading<'static>>` clone, drops every `Arc`, and drains the stream on a `std::thread::spawn`ed thread, which accepts only `Send + 'static`. A later dyn-held stream trait follows the same pattern, for example a streaming market-data trait, which Epic 6 deferred.

## A7-2: every events stream begins with `Resync`, and a voided fill yields `Resync` (amends AD-24)

- **Proposed by:** the user, 2026-10-17, during Epic 7's design (spec D3 and D4). Spec D3 is bundle N's review finding E15.
- **Current rule (AD-24):**
  - A `Trading` implementation feeds `events()` from an unbounded queue. Its fills channel's `Protocol` declares `Unbounded`.
  - After every `Reconnected` on a fills channel, it yields `TradingEvent::Resync` before any later fill. The consumer reconciles through `open_orders()` and trade history.
  - The trait documents that the venue replays nothing.
- **Proposed rule:**
  - **Unchanged:** the unbounded queue, and the documentation that the venue replays nothing.
  - **The stream begins with `Resync` (spec D3).** It yields `TradingEvent::Resync` as its first item, once its fills connection's first connect attempt has ended, whether the connection went live or began in an outage.
  - **After every reconnect.** `Resync` still comes after every reconnect of the fills connection, before any later event.
  - **A voided fill yields `Resync` (spec D4).** A trade yields a `Fill` when the venue reports it matched, and a trade the venue later reports as failed yields `Resync`, because the trait has no event that takes a fill back. The venue's intermediate states yield nothing, so a trade is reported once.
  - **The meaning of `Resync`.** It tells the consumer to reconcile through `open_orders()` and the trade history: the stream cannot vouch for what it reported or missed. It comes at the start, after every reconnect of the fills connection, and when the venue voids a fill.
- **Why:**
  - **The window before the first subscription.** A fill between `events()` returning and the first subscription is neither replayed nor preceded by a `Reconnected`, so AD-24's rule alone leaves it unflagged. A leading `Resync` covers it with one reconciliation.
  - **The trait cannot take a fill back.** A fill reported on a match and later failed on chain would leave a consumer's books wrong with no signal. `Resync` is the one signal the trait has.
- **Follow-up in the code:** Story 4 documents the contract on `Trading::events` and `TradingEvent::Resync`, and Story 5's events adapter implements it for the CLOB (`MATCHED` yields a `Fill`, `FAILED` a `Resync`). The Kalshi mapping applies the same two rules to Kalshi's streams.

## A7-3: the CLOB's `Trading` is gated `all(clob-ws, data)` (amends AD-3's last bullet)

- **Proposed by:** the user, 2026-10-17, during Epic 7's design (spec D6).
- **Current rule (AD-3, last bullet):** "Polymarket's `Trading::positions` reads the data API, so the clob `Trading` impl is gated `all(clob, data)` and declared in Cargo metadata."
- **Proposed rule:**
  - The clob `Trading` impl is gated `all(feature = "clob-ws", feature = "data")`, and the gate is declared in `polyoxide-polymarket`'s `[package.metadata.polyoxide] notes`.
  - `clob-ws` implies `clob` and turns on `polyoxide-venue/trading`; the umbrella's `polymarket-clob-ws` turns it on too.
- **Why:**
  - **`events()` needs the user socket.** The supervised user socket is `clob-ws`'s, so an impl gated on `clob` alone would have to omit `events()` or build without the socket it reads.
  - **`positions` needs the Data API.** That half of AD-3's reasoning stands, so `data` stays in the gate.
- **Follow-up in the code:** Story 5 adds the impl under that gate, the feature wiring in `polyoxide-polymarket/Cargo.toml` and `polyoxide/Cargo.toml`, and the `notes` entry that the generated crate lists in CLAUDE.md and README.md show.

## A7-4: a `Protocol` may dial its own connection (amends AD-11's "What a venue implements")

- **Proposed by:** the user, 2026-10-17, during Epic 7's design (spec D7).
- **Current rule (AD-11, "What a venue implements"):** only a `Protocol`: the handshake request per attempt; decode; the liveness and delivered predicates; the ping schedule; `P::Membership` with membership frames and paced `replay`; `wanted(&Membership)`; optionally `max_connection_age`; venue-specific recovery and protocol-originated events; whether it declares `Disconnected`; and its queue.
- **Proposed rule:** the list gains one item, "optionally how it dials: `Protocol::connect`, a provided method the `Supervisor` calls for every attempt. Its default builds the handshake's request and dials it with the kit's single-address `connect`, so a `Protocol` that does not override it behaves as before."
- **Why:**
  - **The CLOB dials every address.** It resolves each address of its host, interleaves the families and dials each in turn, bounding every step (`divergences.md` D20). The kit's `connect` dials only the first address, so a blackholed route, such as a broken IPv6 one, would spend the whole timeout there.
  - **Without the hook the supervised sockets lose that.** A `Supervisor` that always dials with the kit's `connect` would take the per-address behaviour away from the CLOB the moment it is supervised.
  - **Additive.** The method is provided, so perps' and Binance's protocols compile and behave unchanged.
- **Follow-up in the code:** Story 1 adds the method and the `Supervisor`'s call, and Stories 2 and 3 override it in `ClobMarketProtocol` and `ClobUserProtocol` with the CLOB's `dial`.

## 7.5: no amendment

Story 7.5's Kalshi mapping (`_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`) found no Kalshi order or fill field that fits neither the `Trading` trait nor a deferral, so it proposes no amendment. The same is true of bundle N's reading of the mapping: Epic 7's changes to the trait (the `trading` feature, `Extensions::ensure_declared`, and A7-2's two rules) moved no field of it.
