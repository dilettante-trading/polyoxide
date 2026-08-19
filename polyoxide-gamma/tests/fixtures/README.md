# Captured Gamma payloads

These are real responses from `gamma-api.polymarket.com`, used by
`tests/wire_agreement.rs` to assert that the comment types agree with what the
server sends. They are the oracle, deliberately in preference to
`docs/specs/gamma/openapi.yaml` — see
`docs/superpowers/specs/2026-08-19-gamma-comments-and-mirror-syncs-design.md`
decision 4, and `docs/specs/gamma/OBSERVED.md` for a case where the published
spec is wrong.

| File | Source | Fetched | Notes |
|---|---|---|---|
| `comment_full.json` | `GET /comments?parent_entity_type=Event&parent_entity_id=45915&limit=64&get_positions=true` | 2026-08-19 | One comment (id `3218542`) selected as the only one of 159 carrying every optional top-level key. Its `profile` is nevertheless not fully populated: it lacks `pseudonym`, which 164 of the 166 sampled comments carry, so `CommentProfile::pseudonym` is not exercised by this fixture. Verbatim except `profile.positions` truncated 6→2 and `reactions` truncated 3→1. **No keys added or removed.** |
| `comment_sparse.json` | `GET /comments/user_address/0x8ef197aa37f6d0ec4c5802b40c34ec358e4da6ab?limit=1` | 2026-08-19 | Verbatim. Carries no `profile` and no `reactions` key. |

## Recapturing

When `wire_agreement.rs` fails because upstream added a field, refetch with the
command above and re-trim. Do not hand-edit a fixture to make a test pass —
that reintroduces the self-referential fixture that caused issue #28.
