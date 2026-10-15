# polyoxide-data

This crate is retired. It was the Polymarket Data API client. Since 0.40 its code is the
`data` module of [`polyoxide-polymarket`](https://crates.io/crates/polyoxide-polymarket),
behind that crate's `data` feature, and every path that was `polyoxide_data::…` is
`polyoxide_polymarket::data::…`. Its `specta` feature is `polyoxide-polymarket`'s `specta`,
beside `data`.

```toml
[dependencies]
polyoxide-polymarket = { version = "0.40", default-features = false, features = ["data"] }
```

The [rename manifest][manifest] maps every moved or removed path to where it went, and the
[migration guide][guide] explains the moves.

To stay on the old crate, pin `polyoxide-data = "0.39"`: no release is yanked, and this one
only stops the build, with an error that says the same.

## License

Licensed under either of [MIT][mit] or [Apache-2.0][apache] at your option.

[manifest]: https://github.com/dilettante-trading/polyoxide/blob/main/docs/rename-manifest.md
[guide]: https://github.com/dilettante-trading/polyoxide/blob/main/docs/s2-migration.md
[mit]: https://github.com/dilettante-trading/polyoxide/blob/main/LICENSE-MIT
[apache]: https://github.com/dilettante-trading/polyoxide/blob/main/LICENSE-APACHE
