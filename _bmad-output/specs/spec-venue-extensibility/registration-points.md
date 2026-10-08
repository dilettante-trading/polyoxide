# Registration points

These are the places that must change today when a crate or venue is added. The list is
reconstructed from the Binance design's wiring table, its plans, and a repo-wide grep; it is
not a verified diff. CAP-8 makes each one derive from, or be checked against, one source.

| Place | What it lists | Known drift |
|---|---|---|
| `Cargo.toml` `members` and `[workspace.dependencies]` | crates and version pins | — |
| `.github/workflows/release.yml:83` `CRATES` (plus the comment at `:79`) | publish order | omits `polyoxide-cli` (verified). `polyoxide-cli` has never been on crates.io ("crate does not exist", 2026-10-08), although `README.md:54` says `cargo install polyoxide-cli`. Its trigger also accepts a fork PR from a branch named `main`: the `workflow_run` branch filter matches the fork's `head_branch`, nothing checks the event or `head_repository`, and it checks out `head_sha` before `cargo login` (verified 2026-10-08; spine AD-25 guards it) |
| `scripts/finish_release.sh` | publish order, hand-expanded | publishes `polyoxide-cli` (verified), unlike `release.yml` |
| `.github/workflows/nightly-behavioral.yml:49-59` | live suites, with flags per row | nothing checks that every `tests/live_*.rs` has a row |
| `.github/workflows/nightly-schema.yml:52-63` and its exclusion comment `:32-50` | watched specs and deliberate exclusions | the exclusions are copied three times (here, `SELF-HEALING.md:112-123`, CLAUDE.md) |
| `.github/scripts/classify_failures.py` | `AUTH_GATED_RE` (`POLYMARKET_*` only); `ENVIRONMENTAL_RE` and `TRANSIENT_RES` hold each crate's error Display text | every venue's error text is copied in by hand |
| `CLAUDE.md` | dependency graph, crate count, publish order, nightly list, exclusions | three separate copies |
| `README.md` crate table | crates | once lacked perps and rtds |
| `docs/specs/INDEX.md` | spec mirrors | titled "Polymarket API Specs" |
| `SELF-HEALING.md` | live suites and exclusions | — |
| `CHANGELOG.md` | release notes | — |
| Umbrella `polyoxide/Cargo.toml` features | which crates the facade re-exports | Binance is excluded today. That decision is superseded: the umbrella becomes multi-venue (CAP-7). |
| `polyoxide-cli/Cargo.toml` and `src/commands/*` enum arms | CLI dependencies and commands | — |

Side effects in the same change (CAP-10): adding Binance turned on `reqwest`'s `gzip` for
every crate, which needed `HttpClientBuilder::gzip` in core and `.gzip(false)` pins in four
examples, and shipped a regression in 0.37.0 that 0.37.1 fixed.

Each design re-derives the wiring checklist (sports `:279`, perps `:279`, binance `:463`;
rtds plan Task 17, perps plan Task 14). No written onboarding guide exists. The only partial
recipe is `SELF-HEALING.md:170-175`, and it covers CI only.
