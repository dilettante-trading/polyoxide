//! Enums and newtypes shared across the v2 response and request types.

/// Declares a string enum that also appears in **responses**: unknown wire
/// values are kept verbatim in `Other(String)` instead of failing the page.
macro_rules! open_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident => $wire:literal, )+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )+
            /// A value this version of the SDK does not recognise, kept verbatim.
            /// Sending it in a request passes the string through unchanged.
            Other(String),
        }

        impl $name {
            /// Every variant this SDK knows, in declaration order. `Other` is not
            /// among them: it holds whatever else the server sends.
            pub const ALL: &'static [Self] = &[$( Self::$variant ),+];

            /// The wire spelling.
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $wire, )+
                    Self::Other(raw) => raw,
                }
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = ::std::convert::Infallible;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(match s {
                    $( $wire => Self::$variant, )+
                    other => Self::Other(other.to_owned()),
                })
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = <String as ::serde::Deserialize>::deserialize(deserializer)?;
                let Ok(value) = raw.parse();
                Ok(value)
            }
        }

        #[cfg(feature = "specta")]
        impl ::specta::Type for $name {
            fn inline(type_map: &mut ::specta::TypeMap, generics: ::specta::Generics) -> ::specta::DataType {
                <String as ::specta::Type>::inline(type_map, generics)
            }
        }
    };
}

/// Declares a request-only string enum. The server rejects unknown values with
/// a `400`, so there is no escape hatch.
macro_rules! closed_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident => $wire:literal, )+
        }
    ) => {
        $(#[$meta])*
        #[cfg_attr(feature = "specta", derive(specta::Type))]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ::serde::Serialize, ::serde::Deserialize)]
        #[non_exhaustive]
        pub enum $name {
            $( $(#[$vmeta])* #[serde(rename = $wire)] $variant, )+
        }

        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: &'static [Self] = &[$( Self::$variant ),+];

            /// The wire spelling.
            pub fn as_str(&self) -> &'static str {
                match self {
                    $( Self::$variant => $wire, )+
                }
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $( $wire => Ok(Self::$variant), )+
                    other => Err(format!(concat!("unknown ", stringify!($name), ": {}"), other)),
                }
            }
        }
    };
}

/// Index of an outcome within its market.
///
/// Upstream sends `999` when it could not label the outcome, and that happens
/// on ordinary rows. [`get`](Self::get) returns `None` for it, so the sentinel
/// cannot be used as an index by accident.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct OutcomeIndex(u32);

impl OutcomeIndex {
    /// The wire value meaning "the outcome could not be labeled".
    pub const UNLABELED: u32 = 999;

    /// The index, or `None` when upstream could not label the outcome.
    pub fn get(self) -> Option<u32> {
        (self.0 != Self::UNLABELED).then_some(self.0)
    }

    /// The value exactly as sent, including the `999` sentinel.
    pub fn raw(self) -> u32 {
        self.0
    }
}

open_enum! {
    /// Side of a fill, from the row's wallet's perspective (`BUY` / `SELL`).
    pub enum TradeSide {
        /// `BUY`
        Buy => "BUY",
        /// `SELL`
        Sell => "SELL",
    }
}

open_enum! {
    /// `side` on an activity row. Trades carry `BUY`/`SELL`, tips carry
    /// `IN`/`OUT`, and rows where no side applies carry an empty string.
    pub enum ActivitySide {
        /// `BUY`
        Buy => "BUY",
        /// `SELL`
        Sell => "SELL",
        /// `IN`: a tip received.
        In => "IN",
        /// `OUT`: a tip sent.
        Out => "OUT",
        /// `""`: no side applies to this row.
        Unspecified => "",
    }
}

open_enum! {
    /// Activity row type. `TIP` is never in the default set and is only
    /// returned when requested explicitly.
    pub enum ActivityType {
        /// `TRADE`
        Trade => "TRADE",
        /// `SPLIT`
        Split => "SPLIT",
        /// `MERGE`
        Merge => "MERGE",
        /// `REDEEM`
        Redeem => "REDEEM",
        /// `REWARD`
        Reward => "REWARD",
        /// `CONVERSION`
        Conversion => "CONVERSION",
        /// `DEPOSIT`. Excluded unless `exclude_deposits_withdrawals(false)` is sent.
        Deposit => "DEPOSIT",
        /// `WITHDRAWAL`. Excluded unless `exclude_deposits_withdrawals(false)` is sent.
        Withdrawal => "WITHDRAWAL",
        /// `YIELD`
        Yield => "YIELD",
        /// `MAKER_REBATE`
        MakerRebate => "MAKER_REBATE",
        /// `REFERRAL_REWARD`
        ReferralReward => "REFERRAL_REWARD",
        /// `TAKER_REBATE`
        TakerRebate => "TAKER_REBATE",
        /// `TIP`: a user-to-user pUSD transfer. Opt-in.
        Tip => "TIP",
    }
}

open_enum! {
    /// Position lifecycle state (`OPEN` / `REDEEMABLE` / `CLOSED`), plus two
    /// request-only filters (`REDEEMABLE_LOST` / `MERGEABLE`).
    ///
    /// `OPEN` is the superset: an `OPEN` request also returns rows whose own
    /// status is `REDEEMABLE`. A `/v2/positions` row's status is only ever
    /// `OPEN`, `REDEEMABLE` or `CLOSED`: a `REDEEMABLE_LOST` request returns
    /// rows labelled `REDEEMABLE`, and a `MERGEABLE` request rows labelled
    /// `OPEN`.
    pub enum PositionStatus {
        /// `OPEN`
        Open => "OPEN",
        /// `REDEEMABLE`
        Redeemable => "REDEEMABLE",
        /// `REDEEMABLE_LOST`: settled positions on the losing side, which
        /// redeem for nothing. Requires `user`; defaults to sorting by
        /// `CURRENT_VALUE`. Its rows keep the status `REDEEMABLE`.
        RedeemableLost => "REDEEMABLE_LOST",
        /// `MERGEABLE`: `OPEN` narrowed to conditions where the wallet holds
        /// two or more live outcome tokens, i.e. a complementary set it can
        /// merge. Defaults to sorting by `TOKENS`. Wallet-scoped: a
        /// market-anchored request is served as `OPEN`. Its rows keep the
        /// status `OPEN`.
        Mergeable => "MERGEABLE",
        /// `CLOSED`
        Closed => "CLOSED",
    }
}

closed_enum! {
    /// Ranking window for boards (`day` / `week` / `month` / `all`).
    pub enum TimePeriod {
        /// `day` (the upstream default)
        Day => "day",
        /// `week`
        Week => "week",
        /// `month`
        Month => "month",
        /// `all`
        All => "all",
    }
}

closed_enum! {
    /// Combo position status filter. Values other than `REDEEMABLE` may be
    /// combined; `REDEEMABLE` must be sent alone.
    pub enum ComboPositionStatus {
        /// `OPEN`: the superset, including still-held redeemable positions.
        Open => "OPEN",
        /// `REDEEMABLE`: exactly the rows whose `redeemable` flag is `true`.
        Redeemable => "REDEEMABLE",
        /// `PARTIAL`
        Partial => "PARTIAL",
        /// `RESOLVED_WIN`
        ResolvedWin => "RESOLVED_WIN",
        /// `RESOLVED_LOSS`
        ResolvedLoss => "RESOLVED_LOSS",
        /// `RESOLVED_PARTIAL`
        ResolvedPartial => "RESOLVED_PARTIAL",
    }
}

closed_enum! {
    /// Sort key for `/v2/positions`. The upstream default depends on the
    /// status: `CURRENT_VALUE` for `OPEN`/`REDEEMABLE`/`REDEEMABLE_LOST`,
    /// `TOKENS` for `MERGEABLE`, `REALIZED_PNL` for `CLOSED`.
    pub enum PositionSortBy {
        /// `CURRENT_VALUE`
        CurrentValue => "CURRENT_VALUE",
        /// `PRICE`: the row's `current_price`. Under `REDEEMABLE`, winners
        /// still sort ahead of losers, and price orders each group.
        Price => "PRICE",
        /// `TOKENS`
        Tokens => "TOKENS",
        /// `UNREALIZED_PNL`
        UnrealizedPnl => "UNREALIZED_PNL",
        /// `REALIZED_PNL`
        RealizedPnl => "REALIZED_PNL",
        /// `TOTAL_PNL`
        TotalPnl => "TOTAL_PNL",
        /// `TIMESTAMP`: the row's `last_event_at`.
        Timestamp => "TIMESTAMP",
    }
}

closed_enum! {
    /// Sort key for `/v2/positions/combos`.
    pub enum ComboPositionSortBy {
        /// `FIRST_ENTRY` (the default, except under `status=REDEEMABLE`)
        FirstEntry => "FIRST_ENTRY",
        /// `ENTRY_COST` (the default under `status=REDEEMABLE`)
        EntryCost => "ENTRY_COST",
        /// `CURRENT_VALUE`, an alias of `ENTRY_COST` upstream
        CurrentValue => "CURRENT_VALUE",
        /// `UPDATED`
        Updated => "UPDATED",
    }
}

closed_enum! {
    /// Sort key for `/v2/activity`. Only `TIMESTAMP` is supported.
    pub enum ActivitySortBy {
        /// `TIMESTAMP`
        Timestamp => "TIMESTAMP",
    }
}

closed_enum! {
    /// Unit of `filter_amount` on `/v2/trades` and `/v2/positions`.
    pub enum FilterType {
        /// `CASH`: USDC.
        Cash => "CASH",
        /// `TOKENS`: shares (the upstream default).
        Tokens => "TOKENS",
    }
}

closed_enum! {
    /// Which trader board `/v2/leaderboard` reads.
    pub enum LeaderboardBoard {
        /// `PNL` (the upstream default)
        Pnl => "PNL",
        /// `VOLUME`: ranked by both-sides volume in shares.
        Volume => "VOLUME",
    }
}

closed_enum! {
    /// Window for `/v2/user-pnl`. The upstream default is `1d`.
    pub enum PnlInterval {
        /// `max`
        Max => "max",
        /// `all`
        All => "all",
        /// `1m`
        OneMonth => "1m",
        /// `1w`
        OneWeek => "1w",
        /// `1d`
        OneDay => "1d",
        /// `12h`
        TwelveHours => "12h",
        /// `6h`
        SixHours => "6h",
    }
}

closed_enum! {
    /// Output grid for `/v2/user-pnl`. The upstream default is `1h`.
    pub enum PnlFidelity {
        /// `1d`
        OneDay => "1d",
        /// `18h`
        EighteenHours => "18h",
        /// `12h`
        TwelveHours => "12h",
        /// `3h`
        ThreeHours => "3h",
        /// `1h`
        OneHour => "1h",
    }
}

closed_enum! {
    /// Relative window for `/v2/prices-history`, used instead of `start`/`end`.
    pub enum PricesInterval {
        /// `max`: the market's whole life.
        Max => "max",
        /// `all`: the market's whole life.
        All => "all",
        /// `1m`
        OneMonth => "1m",
        /// `1w`
        OneWeek => "1w",
        /// `1d`
        OneDay => "1d",
        /// `6h`
        SixHours => "6h",
        /// `1h`
        OneHour => "1h",
    }
}

/// Which positions `/v2/positions` lists. Upstream requires at least one of a
/// wallet and a market.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PositionAnchor {
    /// Every position this proxy wallet holds or held.
    User(String),
    /// A market's holders, by condition id. Upstream accepts exactly one id
    /// here and rejects a list rather than truncating it.
    Condition(String),
    /// This wallet's positions, narrowed to these markets (at most 20).
    UserInConditions {
        /// Proxy wallet.
        user: String,
        /// Condition ids.
        conditions: Vec<String>,
    },
}

impl From<&str> for PositionAnchor {
    fn from(user: &str) -> Self {
        Self::User(user.to_owned())
    }
}

impl From<String> for PositionAnchor {
    fn from(user: String) -> Self {
        Self::User(user)
    }
}

/// Which resolutions `/v2/resolutions` returns. Upstream takes exactly one
/// selector family per request.
///
/// There is deliberately no `From<&str>`: a UMA question id and a condition id
/// are both `0x` plus 64 hex characters, so a bare string does not say which.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResolutionKey {
    /// One UMA question id.
    Question(String),
    /// Condition ids (at most 20).
    Conditions(Vec<String>),
    /// Gamma event ids (at most 20).
    Events(Vec<String>),
}
