# polyoxide-clob

This crate is retired. It was the Polymarket CLOB client. Since 0.40 its code is the `clob`
module of [`polyoxide-polymarket`](https://crates.io/crates/polyoxide-polymarket), behind
that crate's `clob` feature, and every path that was `polyoxide_clob::…` is
`polyoxide_polymarket::clob::…`. Its `ws` feature is `clob-ws`, which turns `clob` on, so
`polyoxide_clob::ws` is `polyoxide_polymarket::clob::ws` under `clob-ws`.

```toml
[dependencies]
polyoxide-polymarket = { version = "0.40", default-features = false, features = ["clob", "gamma"] }
```

`gamma` builds the proxy-wallet lookup through Gamma, which `polyoxide-clob`'s defaults
turned on and `clob` alone no longer implies.

The [rename manifest][manifest] maps every moved or removed path to where it went, and the
[migration guide][guide] explains the moves.

To stay on the old crate, pin `polyoxide-clob = "0.39"`: no release is yanked, and this one
only stops the build, with an error that says the same.

## License

Licensed under either of [MIT][mit] or [Apache-2.0][apache] at your option.

[manifest]: https://github.com/dilettante-trading/polyoxide/blob/main/docs/rename-manifest.md
[guide]: https://github.com/dilettante-trading/polyoxide/blob/main/docs/s2-migration.md
[mit]: https://github.com/dilettante-trading/polyoxide/blob/main/LICENSE-MIT
[apache]: https://github.com/dilettante-trading/polyoxide/blob/main/LICENSE-APACHE
