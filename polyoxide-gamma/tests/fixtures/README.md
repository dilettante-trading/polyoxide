# Captured Gamma payloads

These are real responses from `gamma-api.polymarket.com`, used by
`tests/wire_agreement.rs` to assert that the comment and profile types agree
with what the server sends. They are the oracle, deliberately in preference to
`docs/specs/gamma/openapi.yaml` — see
`docs/superpowers/specs/2026-08-19-gamma-comments-and-mirror-syncs-design.md`
decision 4, and `docs/specs/gamma/OBSERVED.md` for a case where the published
spec is wrong.

| File | Source | Fetched | Notes |
|---|---|---|---|
| `comment_full.json` | `GET /comments?parent_entity_type=Event&parent_entity_id=45915&limit=64&get_positions=true` | 2026-08-19 | One comment (id `3218542`) selected as the only one of 159 carrying every optional top-level key. Its `profile` is nevertheless not fully populated: it lacks `pseudonym`, which 164 of the 166 sampled comments carry, so `CommentProfile::pseudonym` is not exercised by this fixture. Verbatim except `profile.positions` truncated 6→2 and `reactions` truncated 3→1. **No keys added or removed.** |
| `comment_sparse.json` | `GET /comments/user_address/0x8ef197aa37f6d0ec4c5802b40c34ec358e4da6ab?limit=1` | 2026-08-19 | Verbatim. Carries no `profile` and no `reactions` key. |
| `profile_full.json` | `GET /profiles/user_address/0x9b74f592ae5a27b4a2660b93f76136b5de60dcf6` | 2026-08-19 | Verbatim (re-indented to 2 spaces; **no keys added or removed**). The only capture in a 65-address sweep carrying all nine optional keys at once, including a non-default `takerTier`/`takerTierName`. |
| `profile_sparse.json` | `GET /profiles/user_address/0x69463a2ab818b0453e8380ae2b5dd5d33b2625f2` | 2026-08-19 | Verbatim (re-indented to 2 spaces; **no keys added or removed**). Lacks `profileImage` and `bio`, the two keys the server omits most often — see the sample below. |

Across the 65-address sweep backing these two captures, key frequency was:
`$schema`, `name`, `proxyWallet`, `createdAt`, `takerTier`, `takerTierName`,
`weightedVolume` in 65/65; `pseudonym` in 64/65 (the one miss is
`0x8ef197aa37f6d0ec4c5802b40c34ec358e4da6ab`, `h0ip0ll0i` — the address used
as the worked example when this fix was scoped); `profileImage` in 31/65;
`bio` in 16/65. `$schema` is response metadata, not data — see
`tests/wire_agreement.rs`'s `IGNORED` list for `Profile`.

## Recapturing

When `wire_agreement.rs` fails because upstream added a field, refetch with the
command above and re-trim. Do not hand-edit a fixture to make a test pass —
that reintroduces the self-referential fixture that caused issue #28.
