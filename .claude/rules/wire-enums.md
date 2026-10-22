---
paths:
  - "polyoxide-venue/src/wire.rs"
  - "**/src/**/types.rs"
  - "**/src/**/types/**"
  - "polyoxide-polymarket/src/relay/session_signers.rs"
  - "polyoxide-polymarket/src/clob/api/rewards.rs"
  - "polyoxide-binance/src/usdm/ws/mod.rs"
  - "**/tests/enum_wire_*.rs"
---

# Wire enums

`polyoxide-venue`'s macros declare the wire enums of gamma (`src/gamma/types.rs`), data v2
(`src/data/v2/types/common.rs`), perps (`src/perps/types.rs`) and relay (`src/relay/types.rs`,
`session_signers.rs`), all in `polyoxide-polymarket`, and of Binance (`usdm/types.rs`):

- `wire_enum!` for a closed set the client sends: serde renames, `ALL`,
  `as_str(self) -> &'static str`, `Display`, and a `FromStr` whose error is
  `polyoxide_venue::UnknownVariant` (`"v" is not a valid X`, classed `InvalidRequest`).
- `open_enum!` for a set the venue extends: `#[non_exhaustive]`, an `Other(String)` that keeps an
  unknown spelling verbatim, `ALL`, `as_str`, `from_wire`, an infallible `FromStr`, and string
  serde.

Rules for using them:

- Both take `$vis enum Name { Variant => "wire", .. }` with attributes on the enum and each
  variant. Neither adds `#[non_exhaustive]` to a closed set, so data's add it themselves.
- The expansions name `::serde`, so the calling crate depends on serde; `wire_enum!`, which
  derives `::serde::Serialize` and `::serde::Deserialize`, also needs serde's `derive` feature,
  while `open_enum!` implements both by hand.
- They emit no specta code, since a `cfg(feature = "specta")` in an exported macro is evaluated in
  every calling crate: data's closed enums pass
  `#[cfg_attr(feature = "specta", derive(specta::Type))]`, and gamma and data give their open
  enums a `String` shape with one `polyoxide_venue::specta_as_string!(..)`.
- Perps and Binance keep separate `Interval` enums (D16).

Still written by hand, one spelling per variant: clob's `OrderSide` (`src/clob/types.rs`) and
`SortPosition`, `MultiMarketOrderBy` and `UserRewardMarketOrderBy` (`src/clob/api/rewards.rs`);
relay's `WalletType`; Binance's `StreamPath` (`usdm/ws/mod.rs`); gamma's `ParentEntityType`; and
data's v1 enums in `data/types.rs`, among them the `#[serde(other)] Unknown` ones (`TradeSide`,
`ActivityType`, `ApprovalFeature`, `ApprovalStandard`, `ComboStatus`, `ComboLegStatus`).
