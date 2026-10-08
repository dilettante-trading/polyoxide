---
reviewer: adversary lens (update gate)
target: ARCHITECTURE-SPINE.md after the epics-validation update (AD-10, AD-13, AD-15, AD-16, AD-22, AD-25 amended in place; memlog lines 130-136)
units: stories in _bmad-output/planning-artifacts/epics.md
binds: ARCHITECTURE-GUIDE.md, CLAUDE.md, workspace at e3d8c3e, live GitHub repo settings (read-only)
lens: "construct two units one level down that each obey every AD to the letter yet still build incompatibly"
date: 2026-10-08
---

# Adversary review of the spine update: incompatible story pairs

## Verdict

**Not ready.** The six amendments close the four questions they were written for, but each
one names a new shared artefact (a `secrets` field, a generated workflow region, an
exception list, a transport-error table, a new-crate count, a provisional bucket size)
without fixing that artefact's shape or owner. Eight story pairs can each obey every AD and
still not fit together. Five are **High**: either a gate goes red on a PR that is not
allowed to fix it, or behaviour changes silently (auth-gated tests filed as real faults
every night, a breaking removal shipped in a semver-compatible patch).

Every pair closes with a sentence or two of AD text. None needs a new user decision except
P1's secret-scope rule, which is a security policy.

One finding outside the lens is **Critical** and predates the update. It is in "Also noted"
because AD-25 governs `release.yml` and the amendment did not close it.

## Method

The units are the stories in `epics.md`. For each pair: (1) units A and B; (2) why each
obeys every AD that touches the point; (3) the concrete clash, with evidence from the tree
or the live repo; (4) severity; (5) the smallest AD text that closes it. **High** means a
gate cannot go green, or protected behaviour changes silently. **Medium** means the units
merge but a later story has to rework one of them.

## Pairs at a glance

| # | Unit A | Unit B | Clash | Severity | Closes with |
|---|---|---|---|---|---|
| P1 | 1.5 generated secret wiring | 2.3 loaders; 8.2/8.4 "auth-gated when absent" | An unset repo secret becomes an empty string, which today's loaders read as present. The `secrets` schema and its scope are unowned | **High** | AD-13, AD-14 |
| P2 | 4.3 one WsError table | 2.2 per-crate tables; 4.10 "same class except R1"; 4.6/4.7 unchanged behaviour | The table covers 6 of tungstenite's 12 variants. Perps and Binance disagree on `AlreadyClosed`. A TLS EOF arrives as `Io`, not `Tls`. Two owners decide whether to reconnect | **High** | AD-15, AD-11 |
| P3 | 5.2 CAP-7 gate and exception list | 8.1 registers `kalshi`; 3.2/3.7 S1 residue | "Whole identifier" cannot see `clob_default` (26 uses in core). The id set grows at 8.1, which may edit neither the foundation nor the list. The AD-2 allowlist and `capture_common.py` sit outside the touch set too | **High** | AD-16, AD-13, AD-2 |
| P4 | 1.7 gate, baselined on the last release tag | AD-25 bumps on `main`; 4.11 final list | A mid-S1 patch release ships listed removals as 0.38.x and moves the baseline. The gate cannot see doc-hidden removals | **High** | AD-22, AD-25 |
| P5 | 8.2 provisional Basic-tier sizing | 3.3 core capacity bucket; guide step 3; 8.5 audit | The guide says "never copy" and 8.2 copies. Resizing at runtime needs a resizable bucket with a shared hold, or a core edit. `Refused` means "never", but a provisional size is not final | **High** | AD-10, AD-23 |
| P6 | 1.2 `publish_order.py` ("publish only what it lists") | 5.12 dedicated tombstone step; `publish = false` members | Two publish paths, and the tombstone path cannot resume. The five-name count includes crates that are never published | Medium | AD-25, AD-16 |
| P7 | 4.11 and 6.6 hand-edit `docs/ARCHITECTURE.md` | 5.11 and 8.5 regenerate it from the spine; 7.5 ∥ 8.5 amend the spine | Two writers of the guide. Parallel S3 epics both allocate AD-26 | Medium | AD-21, AD-13 |
| P8 | 2.10 `capture_common.py` WS client | 8.3/8.4 Kalshi capture (signed handshake) | The shared client takes no handshake headers, so Kalshi must edit it (outside its touch set) or copy one (FR9) | Medium | AD-13 |

---

## P1. Story 1.5 (generated secret wiring) vs Stories 2.3 and 8.2/8.4 (auth-gated when absent)

**Unit A, Story 1.5.** It obeys AD-13 as amended: `live.<target>` gains `secrets = [..]`,
and "the nightly-behavioral secret wiring is a generated region built from each target's
declared secrets" (spine l.296, l.307). It wires each declared name the natural way:
`NAME: ${{ secrets.NAME }}` in the test step's `env:`, inside a
`generated:begin nightly-secrets` region.

**Unit B, Story 2.3 loaders, then 8.2/8.4.** These obey AD-14: loaders "print `auth-gated`"
when "credentials are absent", using env names the test supplies (l.333). 8.2 and 8.4
require the Kalshi live tests to be `auth-gated` "when its declared secrets are absent"
(epics l.1669, l.1718).

**The clash.**
- **Absent vs empty.** GitHub documents that a reference to a secret that is not set
  evaluates to an empty string. The step's env var is therefore *set and empty*, never
  absent.
  - Today's credential code treats empty as present.
    `polyoxide-clob/tests/live_ws.rs:40` takes `if let Ok(private_key) = std::env::var("POLYMARKET_PRIVATE_KEY")`
    and then `Account::new("", …).expect("build account from private key")`.
    `Account::from_env` (`polyoxide-clob/src/account/mod.rs:185-205`) also maps only `Err`
    to "missing".
  - The panic text no longer matches `AUTH_GATED_RE`. After Story 2.7 there is no regex at
    all, so the failure is tagged `real`.
  - `nightly-behavioral.yml` wires no secrets today (l.49-59), and CLAUDE.md says the
    `POLYMARKET_*`/`BUILDER_*` repo secrets are not set. So the day 1.5 declares clob's and
    relay's secrets, about 33 auth-gated tests start filing `real` issues every night.
  - The Kalshi tests do the same from 8.2 until demo credentials are provisioned.
- **Schema.** AD-13 does not type `secrets`. Two readings both obey it:
  - a list of env names, each equal to a repo-secret name;
  - a map from env name to secret name.

  8.2 also lets the Kalshi key come "from a file or an environment variable" (l.1655), and
  neither reading can say "materialise this secret to a file".
- **Scope.** Rows are grouped by (crate, suite) (l.300), but secrets are declared per
  target. The simplest generated region lists the union under the one `test` job's step
  `env:`. That obeys AD-13 to the letter and hands every row every secret: the funded
  `POLYMARKET_PRIVATE_KEY` reaches the Kalshi skeleton's process, and Kalshi's key reaches
  every Polymarket row.
- **Ownership.** No rule ties a declared name to the declaring venue. A new venue's PR is
  allowed to regenerate the region (AD-13 touch set), so it can wire `POLYMARKET_PRIVATE_KEY`
  or `CARGO_REGISTRY_TOKEN` into its own test. Review is then the only control.

**Severity: High.** Protected nightly behaviour (CLAUDE.md's four verdicts) changes
silently, and secrets cross venues.

**Close with**, added to AD-14:

> A credential loader treats an unset **or empty** variable as absent and prints `auth-gated`.

And added to AD-13:

> `secrets` is a list of env names, each equal to its repo-secret name. A target may declare
> only names with its venue's prefix (`<VENUE>_`; for Polymarket the existing `POLYMARKET_`,
> `BUILDER_` and `RELAYER_`), and CI fails on any other name. The generated region scopes
> each name to the rows whose targets declare it (`${{ contains(matrix.secrets, 'X') && secrets.X || '' }}`
> or one step per row), never to workflow-level or job-level env. A file-held credential is
> read from its env var by the test, never from a path.

---

## P2. Story 4.3 (one WsError table) vs Stories 2.2/4.10 (table tests) and 4.6/4.7 (behaviour unchanged)

**Unit A, Story 4.3.** It obeys the amended AD-15 (l.380-382):

| Error | Class |
| --- | --- |
| close codes 1001/1011/1012/1013, a TLS EOF | `Network` |
| `Url`, `HttpFormat`, `AttackAttempt`, non-EOF `Tls` | `InvalidRequest`, "never retried" |
| handshake statuses | the status rule |

Its "table test … covers every row" (l.969) covers exactly these rows.

**Unit B, Story 2.2.** It classifies today's `PerpsWsError` and `UsdmWsError` with "no
variant change". The most faithful reading derives each class from the crate's current
`recovery()`, and each crate gets its own table test. Story 4.10 then requires that "the
Story 2.2 table tests still give every case the same class, except where DRIFT R1" (l.1125).
Stories 4.6 and 4.7 move the perps and Binance suites with behaviour unchanged.

**The clash.**
- **Incompleteness.** tungstenite 0.26.2 `Error` has 12 variants and is `non_exhaustive`:
  `ConnectionClosed`, `AlreadyClosed`, `Io`, `Tls`, `Capacity`, `Protocol`,
  `WriteBufferFull`, `Utf8`, `AttackAttempt`, `Url`, `Http` and `HttpFormat`. The amended
  table classes four of them, plus close codes and EOF. `Class` has no "unknown" variant, so
  every macro author picks a default for the rest, and two picks are both legal.
- **A real divergence the single table cannot carry.** Perps maps `AlreadyClosed` to `Fatal`
  (`polyoxide-perps/src/ws/error.rs:121-125`). Binance maps it to `Reconnect` through its `_`
  arm (`polyoxide-binance/src/usdm/ws/error.rs:129-133`). Story 2.2 pins two different
  classes, the macro must emit one, and no DFR or DRIFT row covers the case. 4.10's
  acceptance criterion cannot pass.
- **"Non-EOF `Tls`" names the wrong variant.**
  - With rustls, a TLS EOF without `close_notify` surfaces as `Io(UnexpectedEof)`. A
    rejected certificate also surfaces as `Io`: sports says so at `src/error.rs:89-90`, and
    retries it today.
  - In this stack `Tls(_)` carries only `InvalidDnsName`.
  - One implementer matches "TLS" by variant, so certificate rejection is `Network` and is
    retried. Another downcasts `Io` for rustls errors, so a venue's expired certificate is
    `InvalidRequest` and the feed ends for good.
- **`AttackAttempt` is server-originated.** tungstenite raises it when the *server's*
  handshake response exceeds 64 KiB, exceeds 512 reads, or trickles in small packets
  (`handshake/machine.rs:162-187`). Calling it `InvalidRequest` contradicts the class's own
  definition ("a client-side refusal … never from a status").
- **Two owners of "reconnect or stop".** AD-11 gives the `Protocol` "venue-specific
  recovery". AD-15 now states "never retried" for the four variants. One Supervisor author
  ends the connection when `!class().is_retriable()`. Another asks `P::recovery`. For the
  four variants named they agree; for `AlreadyClosed`, `Capacity` and `Protocol(_)` they
  disagree.

**Severity: High.** Story 4.10 cannot be satisfied as written, and the nightly tag for a
socket failure depends on which default each author picked.

**Close with**, added to AD-15:

> `impl_ws_classification!` classes every `tungstenite::Error` variant explicitly:
>
> | Variant | Class |
> | --- | --- |
> | `Io(UnexpectedEof \| ConnectionReset \| ConnectionAborted \| TimedOut)`, `ConnectionClosed`, `Protocol(ResetWithoutClosingHandshake)` | `Network` |
> | `Io` carrying a rustls certificate error | `Network` |
> | `Url`, `HttpFormat`, `Tls(InvalidDnsName)`, `AlreadyClosed`, `WriteBufferFull` | `InvalidRequest` |
> | `AttackAttempt`, `Capacity`, `Protocol(_)`, `Utf8` | `VenueRefusal { code: None }` |
> | unknown (`non_exhaustive`) | `Network` |
>
> CI fails on an unlisted variant through a test that matches each variant without a
> wildcard arm. Perps' `AlreadyClosed → Fatal` becomes a DRIFT row, or a DFR row if it is
> kept.

And added to AD-11:

> For transport errors the Supervisor reconnects exactly when `is_retriable()` holds.
> `Protocol` recovery applies only to protocol-level errors (refusals, frames).

---

## P3. Story 5.2 (CAP-7 gate) vs Story 8.1 (registers `kalshi`) and the S1 residue

**Unit A, Story 5.2.** It obeys the amended AD-16 (l.428). It greps
`polyoxide-{venue,core,ws,test-support}` case-insensitively for each venue id "as a whole
identifier" and for each `venue.product:` prefix, "never for bare product ids", against a
checked-in exception list. It reads "whole identifier" as the regex `\b<id>\b`.

**Unit B, Story 8.1.** It obeys AD-13: it declares venue `kalshi` and product `events` in
metadata (l.1631), and its touch-list check fails on any path outside the amended set
(l.1638-1640). Story 8.5 forbids any edit to the four foundation crates (l.1733).

**The clash.**
- **The id set grows after the exception list is written.** The gate's input is "every venue
  id in the metadata". `kalshi` enters only at 8.1, in S3. Any foundation text written in S1
  to S3 that names Kalshi was not a hit when 5.2 built the list. Examples:
  - Story 3.3's "Kalshi-style throttle" test, which must sit "outside polyoxide-core"; the
    obvious non-venue home is `polyoxide-test-support`, a grepped crate;
  - capacity-bucket docs that copy AD-10's "Kalshi's `/account/limits`".

  Each turns red in 8.1's own PR. 8.1 may not edit the foundation (8.5), and the exception
  list is not in AD-13's touch set. Its location is not specified anywhere, so neither
  story owns it.
- **"Whole identifier" is three different gates.** The readings are:
  1. regex `\b..\b`, where `_` is a word character;
  2. a Rust identifier token equal to the id;
  3. any snake-case or camel-case component.

  Readings 1 and 2 miss `PolymarketRetryPolicy`, `polymarket_default` and
  `POLYMARKET_TABLE`. Reading 2 also misses the strings `"clob.polymarket.com"` and
  `"data-api.polymarket.com"` in `polyoxide-core/src/client.rs:836-839`.
- **Excluding product ids blinds the gate to the residue S1 leaves.** Polymarket identifiers
  in core today are product-named, not venue-named:

  | Identifier | Uses |
  | --- | --- |
  | `clob_default` | 26 |
  | `gamma_default` | 8 |
  | `perps_default` | 6 |
  | `relay_default` | 3 |

  `documented_perps_limits`, `SessionSignerScope::Clob` and `"CLOB"` are there too. AD-16
  keeps public paths stable through S1 except consolidations, so Story 3.2 may legally keep
  `RateLimiter::clob_default()` as the public entry and move only the table into
  `core::polymarket`. Story 5.2 then moves `core::polymarket`. The gate finds nothing and
  passes, with Polymarket constructors still in core.
- **The same failure mode on a second central file.** AD-2 requires "a committed per-module
  allowlist" that CI diffs against `cargo tree`, at no stated location. If Stories 4.1 and
  4.8 put the allowlists in a central `ci/` directory, Kalshi's module needs a new entry
  there, outside its touch set.

**Severity: High.** Either FR13's success signal (no foundation edits, touch set only) is
unreachable, or the CAP-7 gate passes with Polymarket code still in core.

**Close with**, replacing AD-16's CAP-7 sentence:

> The gate splits every identifier, string and comment in the four foundation crates
> (`src/`, `tests/`, `examples/`) into tokens at `_`, `-`, `.`, `:` and case boundaries.
> - A venue id matches any token.
> - A product id matches only as one token of a multi-token identifier (`clob_default`
>   yes; `fn events` no).
> - A `venue.product:` prefix matches as text.
>
> Exceptions are keyed by id and live in the declaring crate's
> `[package.metadata.polyoxide.cap7-exceptions]`, so a new venue owns the exceptions for
> its own ids inside its touch set.

And added to AD-2:

> A module's allowlist lives in its crate directory (`<crate>/deps/<module>.allow`).

---

## P4. Story 1.7 (removal gate baselined on the last release tag) vs AD-25 bumps on `main` and Story 4.11

**Unit A, Story 1.7.** It obeys the amended AD-22 (l.506-509):
- it runs `semver-checks --baseline-rev <last release tag> --release-type patch`;
- removals listed in `docs/s1-removals.md` pass;
- the gate "lands before any story that removes a public item".

**Unit B, a maintenance release in mid-S1.** It obeys AD-25: `main` stays releasable after
every merge, and a bump is a separate commit after `git fetch` and a crates.io check
(l.538-540). It also obeys AD-16, because "S1 ships as one release, or as few as
practical" (l.412) permits more than one release. A gamma fix lands, which is how 0.38.1
happened, and the maintainer bumps to 0.38.2.

**The clash.**
- **The patch ships breaking changes.** 0.38.2 contains every S1 removal merged so far. The
  removal gate passed, because they are listed, and nothing ties the bump kind to the list.
  Under Cargo's caret rule 0.38.1 → 0.38.2 is compatible, so prader's `cargo update`
  silently pulls in the removals. That is the NFR1 failure the stage plan exists to prevent.
- **The baseline moves.** After any S1 release, the "last release tag" moves to it. Removals
  shipped in that release stop being reported, so `docs/s1-removals.md` has rows the gate
  can no longer verify. Story 4.11 needs that list to be "final" and cumulative for prader
  (l.1146). If `api_removals.py` also rejects stale rows, as this repo's allow-lists do,
  those rows must be deleted at every release.
- **The gate cannot see the removals prader depends on most.** It is blind to doc-hidden
  removals, which are "listed by hand", and those are exactly the `test_server` and
  `fixtures` paths prader imports (Story 4.4, l.997). Their listing depends on the
  author's discipline.

**Severity: High.** prader breaks without warning, which AD-16 exists to prevent.

**Close with**, added to AD-22:

> Until the S1 release, the removal gate's baseline is the S1 start tag that Story 1.1
> records, not the last release tag, and `docs/s1-removals.md` is cumulative.

And added to AD-25:

> `release.yml` runs `cargo semver-checks` against the previous tag. When it reports any
> removal, the release fails unless the bump raises the 0.x minor. A doc-hidden module that
> a consumer imports gets a `#[doc(hidden)]`-aware check: a test that `use`s each listed
> path, compiled against the baseline.

---

## P5. Story 8.2 (provisional Basic-tier sizing) vs Story 3.3 (core capacity bucket), guide step 3 and Story 8.5

**Unit A, Story 8.2.** It obeys the amended AD-10 (l.231):
- buckets are sized from `/account/limits`;
- when unauthenticated, from "the published Basic tier", recorded as provisional
  (l.1660-1661).

**Unit B, Story 3.3.** It obeys AD-10 and AD-23. Its capacity bucket is
`(capacity, refill, refusal)`, "shares the hold", and refuses "a cost it can never hold" as
non-retriable `InvalidRequest` (l.694-700). It builds the bucket immutably on governor,
which cannot change a quota after construction, with the hold created inside the composed
throttle.

**The clash.**
- **The guide contradicts the spine.** Story 8.1 builds Kalshi "by following the guide's
  steps alone". Guide step 3 says "Measure the limits; never copy them from the venue's
  published docs" (`ARCHITECTURE-GUIDE.md:182`), and the update did not touch it (memlog 136
  synced step 8 only).
  - If 8.2 copies the Basic tier, the 8.5 audit against the guide fails.
  - If 8.2 refuses to copy, it needs a soak, which needs the demo credentials that P1 may
    not deliver.
- **Runtime sizing needs something core does not promise.** "Sizes its buckets from that
  endpoint" happens after an authenticated call through the very client the throttle
  guards. With 3.3's immutable bucket, Kalshi has two options, and both break a rule:
  - build replacement buckets inside its own `Throttle`, which drops an active hold unless
    the hold is a separate handle (AD-23: "a composed hold stops every layer");
  - add a `resize` to core, which is a foundation edit (FR13, 8.5).
- **`Refused` assumes final sizing.** A batch that costs more than the Basic capacity but
  less than the account's real tier is refused permanently, as `InvalidRequest`, before
  the limits are read. Polymarket has the same shape, but its tier is adopted from every
  response header. Kalshi's sizing comes from one explicit call that nothing schedules.
- **Wrong layer.** The Basic tier describes per-account buckets. An unauthenticated request
  has no account, and AD-10 does not say which layer such a request charges.

**Severity: High.** The Kalshi success signal is at stake through either the guide audit or
a forced core edit.

**Close with**, added to AD-10:

> Core's capacity bucket takes its hold as a shared handle and offers
> `resize(capacity, refill)`, which keeps tokens (clamped) and the hold. A throttle sized
> provisionally never returns `Refused`. Until its sizing is confirmed it waits, and it
> confirms by reading the limits endpoint on first use. The guide's measurement step reads:
> "measure window quotas; a published capacity may be copied, provisional in `OBSERVED.md`
> until a soak confirms it."

And added to AD-23:

> A hold is a handle shared by every layer of a throttle and survives the replacement or
> resizing of any layer.

---

## P6. Story 1.2 (`publish_order.py`, "publish only what it lists") vs Story 5.12 (tombstone step) and `publish = false` members

**Unit A, Story 1.2.** It obeys AD-25 (l.542-543):
- the script reads `cargo metadata` and lists the (crate, version) pairs absent from
  crates.io;
- `release.yml` and `finish_release.sh` "publish only those", so both can resume;
- the run fails if more than five crate names are absent.

It adds a CI-scripts test that `release.yml` contains no `cargo publish` outside the
script's loop.

**Unit B, Story 5.12.** It obeys AD-16 (l.420-426): tombstones live under `tombstones/`,
outside the workspace (so `cargo metadata` never sees them), and a dedicated
`cargo publish --manifest-path … --no-verify` step publishes them.

**The clash.**
- **The spine contradicts itself.** "Publish only those" (AD-25) and a "dedicated release
  step" (AD-16) cannot both hold, so 5.12 breaks 1.2's guard test.
- **The tombstone step cannot resume.** Re-running the release after a partial failure
  re-publishes tombstones that already exist, gets "crate version already exists", and
  stops the resume. `finish_release.sh` does not know tombstones exist. (Memlog 118 said
  `publish_order.py` lists them separately. The spine and the stories dropped that.)
- **The count includes crates that are never published.** "Crate names absent from
  crates.io" counts publish-false members unless scoped otherwise. Checked today:
  `polyoxide-py` and `polyoxide-test-support` both return 404.
  - S1 counts cli, venue, ws, py and test-support: exactly 5, with no headroom.
  - From S3, `polyoxide-kalshi` (publish false) counts on every release.
  - The script would also list publish-false pairs to publish unless it filters them.
    Story 1.3 filters them for the dry run, but 1.2 does not.

**Severity: Medium.** The S2 release cannot be resumed, and S1 has no room for one more
crate.

**Close with**, in AD-25:

> The unpublished set and the five-name count cover only members whose `publish` is not
> `false`. `publish_order.py` also lists `tombstones/*/Cargo.toml` pairs absent from
> crates.io, after every workspace crate. The same resumable loop publishes them, with
> `--no-verify`.

Then delete "by a dedicated release step" from AD-16.

---

## P7. Stories 4.11 and 6.6 (hand edits) vs Stories 5.11 and 8.5 (regeneration), and 7.5 ∥ 8.5 (spine amendments)

**Unit A, Stories 4.11 and 6.6.** "`docs/ARCHITECTURE.md` records that S1 has shipped"
(l.1154) and "links to the example" (l.1503). Both are hand edits. AD-21 forbids hand
edits only inside generated regions, and the guide has none.

**Unit B, Stories 5.11 and 8.5.** They obey the amended AD-13 (l.323): `docs/ARCHITECTURE.md`
is "regenerated from the spine" (l.1347, l.1735). A regeneration drops A's text, because it
is not in the spine.

**The clash.**
- The memlog 135 amendment made regeneration the single writer of the guide. Two stories
  still write it by hand, so the guide has two writers.
- **A second clash in S3.** Epics 7 and 8 both run after Epic 6, in parallel. Story 7.5
  raises spine amendments (l.1612) and Story 8.5 records them (l.1735). Two loom sessions
  edit `ARCHITECTURE-SPINE.md` and `.memlog.md` at once. Each allocates the next AD id
  (AD-26), and each regenerates the guide from a different spine.

**Severity: Medium.** Text is lost at the next regeneration, and AD ids collide.

**Close with**, in AD-21:

> `docs/ARCHITECTURE.md` is written only by regeneration from the spine. Stage status and
> example links are spine content, or a `gen_registry.py` region fed from metadata.
> Concurrent epics record proposed amendments in their own `spine-amendments/<epic>.md`.
> One session merges them into the spine, assigns new AD ids at merge, and regenerates the
> guide once.

---

## P8. Story 2.10 (`capture_common.py` WS client) vs Stories 8.3/8.4 (Kalshi capture)

**Unit A, Story 2.10.** `capture_common.py` provides "one WebSocket client" (l.609). It is
stdlib-only, like today's scripts ("Stdlib only, no credentials":
`scripts/capture_binance_fixtures.py`), and opens an unauthenticated connection.

**Unit B, Stories 8.3 and 8.4.** Kalshi's socket requires a signed handshake "even for
market data" (`venue-landscape.md` l.13). Fixtures must be captured "through
`capture_common.py`" (l.1690). The amended touch set admits `scripts/capture_<venue>_*.py`
but not `capture_common.py`.

**The clash.** Kalshi can do one of two things, and both fail an audit:
- add a headers parameter to `capture_common.py`, which is outside the touch set (8.1, 8.5);
- open its own socket in `capture_kalshi_*.py`, which is a copy (FR9; 8.5 checks for copied
  helpers).

**Severity: Medium.**

**Close with**, in AD-13 or the shared-code homes row for T9:

> `capture_common.py`'s HTTP `get` and WebSocket client accept per-request headers computed
> by a caller-supplied function (per-attempt signing), so a venue's signing lives in its own
> capture script.

---

## Also noted

- **Critical, predates the update; AD-25 does not close it: fork PRs can trigger
  `release.yml`.**
  - `release.yml` fires on `workflow_run` of CI with `branches: [main]` (l.3-8).
    `workflow_run`'s branch filter matches the triggering run's `head_branch`. For a
    `pull_request` run from a fork, that is the fork's branch name, so a fork PR from its own
    `main` matches.
  - Nothing else stops it:
    - the job never checks `github.event.workflow_run.event == 'push'` or
      `head_repository.full_name`;
    - it checks out `workflow_run.head_sha`, which is the fork's commit;
    - `publish` runs `cargo login ${{ secrets.CARGO_REGISTRY_TOKEN }}` in environment `cargo`.
  - Read through the GitHub API today: the `cargo` and `pypi` environments have
    `protection_rules: []` and no deployment branch policy, and the fork-PR approval policy
    is `first_time_contributors`.
  - Any returning contributor's fork PR from a branch named `main`, carrying a bumped
    version, is one green CI run away from publishing to crates.io and PyPI.
  - Verify with a throwaway fork before acting on this; the path is consistent with GitHub's
    documented `workflow_run` semantics.
  - Close with, in AD-25: "`release.yml` proceeds only when
    `workflow_run.event == 'push'`, `head_branch == 'main'` and
    `head_repository.full_name == github.repository`, and the `cargo` and `pypi`
    environments restrict deployments to `main`."
- **Generated-region markers in YAML** (low). AD-13's markers are HTML comments. Inside
  `nightly-behavioral.yml` they must be YAML comments (`# <!-- generated:begin … -->`), and
  the generated lines must match the surrounding indentation. If Story 1.4's generator
  anchors on `^<!-- generated:`, it cannot find Story 1.5's region. Close with: "markers
  may be prefixed by the host file's comment leader; the generator preserves indentation."
- **The ordering is not a declared dependency** (low). "The gate lands before any story that
  removes a public item" is a precondition that the removing story's author judges. Epic 2
  is not declared after Story 1.7. Nothing in Epic 2 appears to remove a public item today,
  so this is friction only. Make Story 1.7 a declared predecessor of Epics 2 to 4.
