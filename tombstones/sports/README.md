# polyoxide-sports

This crate is retired. It was the client for Polymarket's live sports feed. Since 0.40 its
code is the `sports` module of
[`polyoxide-polymarket`](https://crates.io/crates/polyoxide-polymarket), behind that crate's
`sports` feature, and every path that was `polyoxide_sports::…` is
`polyoxide_polymarket::sports::…`. Its `test-server` feature is `polyoxide-polymarket`'s
`test-server`, beside `sports`.

```toml
[dependencies]
polyoxide-polymarket = { version = "0.40", default-features = false, features = ["sports"] }
```

The [rename manifest][manifest] maps every moved or removed path to where it went, and the
[migration guide][guide] explains the moves.

To stay on the old crate, pin `polyoxide-sports = "0.39"`: no release is yanked, and this
one only stops the build, with an error that says the same.

## License

Licensed under either of [MIT][mit] or [Apache-2.0][apache] at your option.

[manifest]: https://github.com/dilettante-trading/polyoxide/blob/main/docs/rename-manifest.md
[guide]: https://github.com/dilettante-trading/polyoxide/blob/main/docs/s2-migration.md
[mit]: https://github.com/dilettante-trading/polyoxide/blob/main/LICENSE-MIT
[apache]: https://github.com/dilettante-trading/polyoxide/blob/main/LICENSE-APACHE
