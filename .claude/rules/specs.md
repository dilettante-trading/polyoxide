---
paths:
  - "docs/specs/**"
  - ".github/workflows/nightly-schema.yml"
  - ".github/scripts/diff_openapi.py"
  - ".github/scripts/tests/test_diff_openapi.py"
---

# API spec mirrors

Upstream API documentation lives in `docs/specs/`, one directory per host, indexed by
`docs/specs/INDEX.md`. These are the source of truth for endpoint contracts, rate limits and
response schemas. Polymarket's are mirrored from https://docs.polymarket.com and the official
OpenAPI specs; Kalshi's OpenAPI and AsyncAPI are mirrored in `docs/specs/kalshi/`; Binance
publishes none for USDⓈ-M, so `docs/specs/binance/` holds what its hosts were observed to do.

- **A mirror stays byte-faithful.** Never edit a vendored OpenAPI or AsyncAPI file that
  `nightly-schema.yml` watches, or the drift check alarms forever.
- **A mirror can match upstream and still be wrong.** The drift check compares the mirror with the
  published document, never with the live host, so a host's disagreements with its own spec are
  recorded beside the mirror, in the directory's `OBSERVED.md`.
- The WebSocket contracts are AsyncAPI, not OpenAPI (`docs/specs/clob/asyncapi-{market,user}.json`,
  `docs/specs/sports/asyncapi.json`): a parity audit that diffs only the OpenAPI files misses that
  whole surface.
- Data v2's spec is the one mirror served by the API host (`/v2/openapi.json`) rather than
  docs.polymarket.com, so a parity audit that walks only `/api-spec/` misses it.
- `docs/specs/polymarket-llms.txt` is a snapshot of Polymarket's own documentation index
  (`https://docs.polymarket.com/llms.txt`): a flat list of every doc page, with `.md` URLs,
  covering CLOB/auth/orders, builder attribution and the CLOB V2 migration. Use it to find the
  authoritative upstream page when the local copies are insufficient.

## The drift check (`nightly-schema.yml`)

It fetches each published upstream spec and compares it with its mirror; on drift it files a
tracking issue labelled `schema-drift` **and** `spec:<id>`. The watched specs and the deliberate
exclusions are generated into CLAUDE.md.

- It creates no branches and opens no PRs: Actions cannot open PRs here (org policy: 12 refusals,
  0 PRs in run 31811673456), and adopting a drift is one `curl`, which the issue body spells out.
  The workflow holds `contents: read` and `issues: write` only.
- The issue is found by label intersection, never by title: `gh issue list --search "<title> in:title"`
  is a tokenized full-text search, so `perps` also matches `perps-ws`. That collision let one
  spec's job edit and close another's issue for eleven days, and made `combos-rfq-ws` look like it
  was flapping.
- A spec deliberately not synced is recorded in `docs/specs/.drift-acknowledged.json`, keyed by the
  SHA-256 of the canonical diff, which makes the check exit 3 and close the issue. Fingerprinting
  the disagreement rather than upstream means the acknowledgement expires the moment either side
  moves, so it is never permanent blindness. `clob` is acknowledged because upstream's own
  re-serialization made `example: 'Yes'` parse as boolean `true` on a `type: string` field.
- The issue body carries a key-path summary (changed JSON pointers with before → after, so drift
  inside `components.schemas` is named rather than merely counted) plus the canonicalized diff,
  composed in Python under GitHub's 65,536-character cap.
