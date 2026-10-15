# Proposed spine amendments: Epic 5

Recorded per AD-21: an epic records each proposed amendment here, and one session later merges this file into the spine and regenerates the guide.

## A5-1: Python stays off the venue axis in S2 (drops Story 5.10; amends the S2 stage, the Python convention and CAP-7's bindings)

- **Proposed by:** the user, 2026-10-15, after Stories 5.8 and 5.9 landed on the S2 hub, `aidanb/restructure-3`. The user dropped the story; it was not deferred.
- **Current rule:**
  - The spine's S2 stage lists "the umbrella, the CLI tree and Python modules".
  - Its conventions table puts Python at `polyoxide.polymarket.{clob,gamma,data}`, with data v2 at `polyoxide.polymarket.data.v2`.
  - Its layout gives `polyoxide-py/` the comment `# polyoxide.polymarket.*`.
  - CAP-7 binds the CLI and Python.
  - The guide's S2 line says "the CLI and Python move to the venue layout". Its S2 row lists "Python modules", and its "Standing rules this replaces" table retires the `polyoxide.v2` namespace for `polyoxide.polymarket.data.v2`.
- **Proposed rule:**
  - S2 does not move the Python package. The bindings keep v0.39.0's module names:
    - `polyoxide`, with its `Clob*` classes (`ClobClient`, `ClobMarket` and the rest);
    - `polyoxide.v2`, for data v2's rows, `Page` and the page iterators.
  - `polyoxide-py` stays a facade over `polyoxide-polymarket` and `polyoxide-venue` (AD-1), and it maps errors by class (C4).
  - CAP-7 binds the venue crates, the umbrella and the CLI.
  - The S2 stage drops "Python modules". The conventions row, the layout comment and the guide's lines follow.
  - The `polyoxide.v2` rule in CLAUDE.md stands, so the guide's table no longer retires it.
- **Why:**
  - **No consumer.** No one has asked for per-venue Python modules, and the spine already says of new Python coverage: "Revisit when a consumer asks." prader-rs, the consumer S2 is for, uses the Rust crates.
  - **Nothing in S2 waits on it.** The bindings already build on `polyoxide-polymarket` (Story 5.3), and their CI job and guards pass at their v0.39.0 paths. Stories 5.11 and 5.12 need nothing from 5.10.
  - **Less to move.** Without it, S2 changes no Python name. Two Story 5.1 deferrals become moot: extending the body diff to Python tests, and giving the rename manifest a Python half.
- **Cost:** if Python ever moves to the venue axis, the move is a breaking release of its own, outside S2's single migration.
- **Follow-up in the code:** none in S2.
  - `epics.md` drops Story 5.10, keeps a note where it stood, and removes `polyoxide.v2` from Story 5.11's list of superseded rules.
  - `sprint-status.yaml` drops its key.
  - The session that merges this file edits the spine's S2 stage, the Python convention row, the layout comment and CAP-7's bindings, then regenerates `docs/ARCHITECTURE.md` from the guide.
