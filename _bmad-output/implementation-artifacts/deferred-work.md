# Deferred work

- source_spec: `_bmad-output/implementation-artifacts/spec-1-1-re-baseline-on-current-main.md`
  summary: Gamma's generic `null_as_empty<T>` (`polyoxide-gamma/src/types.rs:119`, v0.38.0) duplicates the older `deserialize_users` (`polyoxide-gamma/src/api/user.rs:100`); one should go.
  evidence: Both bodies are `Option<Vec<_>>::deserialize(..)?.unwrap_or_default()`; `null_as_empty` arrived in 63ddbb1. No duplication-inventory row covers gamma-internal serde helpers, so no restructure story removes it.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-1-re-baseline-on-current-main.md`
  summary: The gamma–sports `game_id` join uses three types (`i64` setters, `u64` on `Event` and sports `MatchUpdate`, `String` on `Market`), so a caller casts `u64` to `i64` to filter `/events` by the id sports sends.
  evidence: `polyoxide-gamma/src/api/events.rs:421,589` take `i64`; `polyoxide-gamma/src/types.rs:479` and `polyoxide-sports/src/update.rs:57` are `Option<u64>`; `types.rs:333` is `Option<String>`. Shipped in v0.38.0; S2 puts gamma and sports in one crate, a natural point to align them (a breaking change, so S2's rename stage).
