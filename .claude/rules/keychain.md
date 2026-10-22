---
paths:
  - "polyoxide-core/src/keychain.rs"
  - "polyoxide-venue/src/secret.rs"
  - "polyoxide-cli/src/commands/credentials.rs"
  - "polyoxide-cli/src/commands/polymarket/credentials.rs"
  - "polyoxide-polymarket/src/clob/account/**"
  - "polyoxide-polymarket/src/relay/account.rs"
---

# The keychain and stored credentials (feature `keychain`)

With the `keychain` feature, credentials can be stored in and loaded from the OS keychain instead
of environment variables: `Account::from_keychain()` (CLOB, the `polyoxide-clob` service),
`BuilderAccount::from_keychain()` (relay, `polyoxide-relay`), or the CLI's
`polyoxide polymarket credentials <store|show|delete> <clob|builder>`. The feature is optional and
off by default.

- A kind is a `polyoxide_core::keychain::StoredCredential` its library defines: `clob::Account`
  for the `polyoxide-clob` service and `relay::BuilderAccount` for `polyoxide-relay`, with
  v0.39.0's field names, so stored values load as before. Each account's fields are listed once,
  in its `StoredCredential` impl, which its `delete_from_keychain` and the CLI both use.
- The trait gives the service, the `FIELDS` (each a `Field { name, secret }`) and a `validate`.
  Core's generic `store`, `show` and `delete_all` carry values as `polyoxide_venue::Secret`
  (`delete` was taken by the per-key function).
- The CLI's command is `commands::credentials::Credentials` over Polymarket's kinds, `clob` and
  `builder` (v0.39.0's `relay`), which `commands::polymarket::credentials` lists and nothing more.
  Its shared module generates a flag per field (`api_passphrase` is `--api-passphrase`), redacts,
  and calls the keychain.
- `store` writes only the fields given, and only after every check passes: core's (a name that is
  not a field, one given twice, an empty or blank value) and the kind's (a private key that
  parses; the relayer API key and its address given together, the address parsing). A refusal is
  `KeychainError::Invalid { service, field, reason }`, classed `InvalidRequest`, whose reason never
  quotes the value.
- `show` prints each field as `present`, `not found` or `error: …`, and the one plain field, the
  relayer API key's address, as its value when present; it never prints a secret.
- The public credential types (`Credentials`, `BuilderConfig`, `ApiCredentials`) keep their
  `String` fields; `Secret` on them is S3's.
