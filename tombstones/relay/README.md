# polyoxide-relay

This crate is retired. It was the client for Polymarket's gasless relayer. Since 0.40 its
code is the `relay` module of
[`polyoxide-polymarket`](https://crates.io/crates/polyoxide-polymarket), behind that crate's
`relay` feature, and every path that was `polyoxide_relay::…` is
`polyoxide_polymarket::relay::…`. Its `keychain` feature is `polyoxide-polymarket`'s
`keychain`, beside `relay`.

```toml
[dependencies]
polyoxide-polymarket = { version = "0.40", default-features = false, features = ["relay"] }
```

The [rename manifest][manifest] maps every moved or removed path to where it went, and the
[migration guide][guide] explains the moves.

To stay on the old crate, pin `polyoxide-relay = "0.39"`: no release is yanked, and this one
only stops the build, with an error that says the same.

## License

Licensed under either of [MIT][mit] or [Apache-2.0][apache] at your option.

[manifest]: https://github.com/dilettante-trading/polyoxide/blob/main/docs/rename-manifest.md
[guide]: https://github.com/dilettante-trading/polyoxide/blob/main/docs/s2-migration.md
[mit]: https://github.com/dilettante-trading/polyoxide/blob/main/LICENSE-MIT
[apache]: https://github.com/dilettante-trading/polyoxide/blob/main/LICENSE-APACHE
