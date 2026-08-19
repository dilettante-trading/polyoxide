# Captured Gamma payloads

These are real responses from `gamma-api.polymarket.com`, used by
`tests/wire_agreement.rs` to assert that the comment, profile and user types
agree with what the server sends. They are the oracle, deliberately in
preference to `docs/specs/gamma/openapi.yaml` — see
`docs/superpowers/specs/2026-08-19-gamma-comments-and-mirror-syncs-design.md`
decision 4, and `docs/specs/gamma/OBSERVED.md` for a case where the published
spec is wrong.

| File | Source | Fetched | Notes |
|---|---|---|---|
| `comment_full.json` | `GET /comments?parent_entity_type=Event&parent_entity_id=45915&limit=64&get_positions=true` | 2026-08-19 | One comment (id `3218542`) selected as the only one of 159 carrying every optional top-level key. Its `profile` is nevertheless not fully populated: it lacks `pseudonym`, which 164 of the 166 sampled comments carry, so `CommentProfile::pseudonym` is not exercised by this fixture. Verbatim except `profile.positions` truncated 6→2 and `reactions` truncated 3→1. **No keys added or removed.** |
| `comment_sparse.json` | `GET /comments/user_address/0x8ef197aa37f6d0ec4c5802b40c34ec358e4da6ab?limit=1` | 2026-08-19 | Verbatim. Carries no `profile` and no `reactions` key. |
| `profile_full.json` | `GET /profiles/user_address/0x9b74f592ae5a27b4a2660b93f76136b5de60dcf6` | 2026-08-19 | Verbatim (re-indented to 2 spaces; **no keys added or removed**). The only capture in a 65-address sweep carrying all nine optional keys at once, including a non-default `takerTier`/`takerTierName`. |
| `profile_sparse.json` | `GET /profiles/user_address/0x69463a2ab818b0453e8380ae2b5dd5d33b2625f2` | 2026-08-19 | Verbatim (re-indented to 2 spaces; **no keys added or removed**). Lacks `profileImage` and `bio`, the two keys the server omits most often — see the sample below. |
| `user_response_full.json` | `GET /public-profile?address=0xc7e53ac4a7c76d6df8b794de2e7d0794265d2d3a` | 2026-08-19 | Verbatim (re-indented to 2 spaces; **no keys added or removed**). The richest capture in a 39-address sweep: carries `profileImage`, `bio`, `pseudonym`, `xUsername` and a non-default `takerTier`/`takerTierName` together, plus a `users[]` entry with an explicit `communityMod: false`. Only `discordUsername` is missing — never observed in the sweep at all (see below). |
| `user_response_sparse.json` | `GET /public-profile?address=0x226b48c1ab114eb890dfae18ea6eb3304c92df8b` | 2026-08-19 | Verbatim (re-indented to 2 spaces; **no keys added or removed**). Lacks `profileImage`, `bio` and `xUsername`. Its `users[]` entry carries `communityMod: true`, the rarer of the two observed boolean values (2/38 nested entries where the key was present at all). |

Across the 39-address sweep backing the `user_response_*` pair, top-level key
frequency was: `$schema`, `createdAt`, `proxyWallet`, `displayUsernamePublic`,
`name`, `users`, `verifiedBadge`, `takerTier`, `takerTierName`,
`weightedVolume` in 39/39; `pseudonym` in 38/39 (the one miss is again
`0x95bac246a983529e6a57feae41ecc028357d3a5c`, `h0ip0ll0i`); `profileImage` in
12/39; `bio` in 5/39; `xUsername` in 5/39; `discordUsername` in **0/39** —
declared optional by the schema but not observed on the wire at all, so
`UserResponse::discord_username` is untested by these fixtures beyond
`tests/wire_agreement.rs`'s `EXPECTED_ABSENT` entry and the hand-written unit
test in `polyoxide-gamma/src/types.rs`. Nested `users[]` entries (39 total,
one per response): `id`, `creator`, `mod` in 39/39; `communityMod` in 38/39 —
the one miss (`0x0cb10c40b0776e9ee8cef970af85724654dda76c`, a `creator: true`
account) confirms it is genuinely optional on `PublicProfileUser.json` rather
than always sent as `false`. No capture in the sweep sent `users` as an
explicit JSON `null`, though the schema types it `["array","null"]`;
`user_response_tolerates_null_users` in `tests/wire_agreement.rs` covers that
case with a hand-written payload instead. `$schema` is response metadata, not
data — see `tests/wire_agreement.rs`'s `IGNORED` list.

## Recapturing

When `wire_agreement.rs` fails because upstream added a field, refetch with the
command above and re-trim. Do not hand-edit a fixture to make a test pass —
that reintroduces the self-referential fixture that caused issue #28.
