//! Every enum value a v2 request can send, pinned to the values the server
//! itself accepts.
//!
//! `v2_spec_agreement.rs` checks query parameter *names*. It cannot see a
//! misspelled value, because the schema types every enum-like parameter as a
//! bare `string`. The oracle here is the server: most parameters' 400 messages
//! list the accepted values (recorded in `docs/specs/data-v2/OBSERVED.md` on
//! 2026-09-14), and the rest come from the mirror's parameter descriptions or a
//! live probe. Each list names its source.
//!
//! Each `ALL` is compared as a set, so a misspelled variant, a variant added
//! without updating its list, and a documented value no variant covers all fail.

use std::{collections::BTreeSet, fmt::Debug, fmt::Display, str::FromStr};

use polyoxide_data::v2::types::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

fn wire<T: Display>(all: &[T]) -> BTreeSet<String> {
    all.iter().map(ToString::to_string).collect()
}

fn set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

/// Display, FromStr and serde all agree on each variant's spelling.
fn round_trips<T>(all: &[T])
where
    T: Display + FromStr + PartialEq + Debug + Serialize + DeserializeOwned,
    <T as FromStr>::Err: Debug,
{
    for variant in all {
        let spelled = variant.to_string();
        assert_eq!(
            &spelled.parse::<T>().unwrap(),
            variant,
            "FromStr({spelled:?})"
        );
        assert_eq!(
            serde_json::to_value(variant).unwrap(),
            Value::String(spelled.clone()),
            "serialize {variant:?}"
        );
        assert_eq!(
            &serde_json::from_value::<T>(Value::String(spelled.clone())).unwrap(),
            variant,
            "deserialize {spelled:?}"
        );
    }
}

#[test]
fn request_enums_spell_what_the_server_accepts() {
    // Server: "filterType must be CASH or TOKENS"
    assert_eq!(
        wire(FilterType::ALL),
        set(&["CASH", "TOKENS"]),
        "FilterType"
    );
    // Server: "time_period must be one of day, week, month, all"
    assert_eq!(
        wire(TimePeriod::ALL),
        set(&["day", "week", "month", "all"]),
        "TimePeriod"
    );
    // Server: "sort_by must be PNL or VOLUME"
    assert_eq!(
        wire(LeaderboardBoard::ALL),
        set(&["PNL", "VOLUME"]),
        "LeaderboardBoard"
    );
    // Server: "interval must be one of max, all, 1m, 1w, 1d, 12h, 6h" (/v2/user-pnl)
    assert_eq!(
        wire(PnlInterval::ALL),
        set(&["max", "all", "1m", "1w", "1d", "12h", "6h"]),
        "PnlInterval"
    );
    // Server: "fidelity must be one of 1d, 18h, 12h, 3h, 1h"
    assert_eq!(
        wire(PnlFidelity::ALL),
        set(&["1d", "18h", "12h", "3h", "1h"]),
        "PnlFidelity"
    );
    // Server: "interval must be one of max, all, 1m, 1w, 1d, 6h, 1h" (/v2/prices-history)
    assert_eq!(
        wire(PricesInterval::ALL),
        set(&["max", "all", "1m", "1w", "1d", "6h", "1h"]),
        "PricesInterval"
    );
    // Server: "sortBy must be one of FIRST_ENTRY, ENTRY_COST, CURRENT_VALUE, UPDATED"
    assert_eq!(
        wire(ComboPositionSortBy::ALL),
        set(&["FIRST_ENTRY", "ENTRY_COST", "CURRENT_VALUE", "UPDATED"]),
        "ComboPositionSortBy"
    );
    // Mirror, /v2/positions sort_by: "One of CURRENT_VALUE, PRICE, TOKENS,
    // UNREALIZED_PNL, REALIZED_PNL, TOTAL_PNL, or TIMESTAMP". PRICE was added
    // upstream in September 2026; a live probe on 2026-09-23 accepted it.
    assert_eq!(
        wire(PositionSortBy::ALL),
        set(&[
            "CURRENT_VALUE",
            "PRICE",
            "TOKENS",
            "UNREALIZED_PNL",
            "REALIZED_PNL",
            "TOTAL_PNL",
            "TIMESTAMP"
        ]),
        "PositionSortBy"
    );
    // Mirror, /v2/positions/combos status: "One of OPEN, REDEEMABLE, PARTIAL,
    // RESOLVED_WIN, RESOLVED_LOSS, RESOLVED_PARTIAL"
    assert_eq!(
        wire(ComboPositionStatus::ALL),
        set(&[
            "OPEN",
            "REDEEMABLE",
            "PARTIAL",
            "RESOLVED_WIN",
            "RESOLVED_LOSS",
            "RESOLVED_PARTIAL"
        ]),
        "ComboPositionStatus"
    );
    // Mirror, /v2/activity sort_by: "Only TIMESTAMP is supported"
    assert_eq!(
        wire(ActivitySortBy::ALL),
        set(&["TIMESTAMP"]),
        "ActivitySortBy"
    );
}

#[test]
fn enums_that_also_appear_in_responses_spell_what_the_server_sends() {
    // Server: "side must be BUY or SELL"
    assert_eq!(wire(TradeSide::ALL), set(&["BUY", "SELL"]), "TradeSide");
    // Mirror, /v2/positions status: "One of OPEN, REDEEMABLE, REDEEMABLE_LOST,
    // MERGEABLE, or CLOSED". The two filters were added upstream in September
    // 2026 and a live probe on 2026-09-23 accepted both. Rows only ever carry
    // the other three: REDEEMABLE_LOST rows say REDEEMABLE, MERGEABLE rows OPEN.
    assert_eq!(
        wire(PositionStatus::ALL),
        set(&[
            "OPEN",
            "REDEEMABLE",
            "REDEEMABLE_LOST",
            "MERGEABLE",
            "CLOSED"
        ]),
        "PositionStatus"
    );
    // Live probe: every one of these was accepted by /v2/activity?type=
    assert_eq!(
        wire(ActivityType::ALL),
        set(&[
            "TRADE",
            "SPLIT",
            "MERGE",
            "REDEEM",
            "REWARD",
            "CONVERSION",
            "DEPOSIT",
            "WITHDRAWAL",
            "YIELD",
            "MAKER_REBATE",
            "REFERRAL_REWARD",
            "TAKER_REBATE",
            "TIP",
        ]),
        "ActivityType"
    );
    // Mirror, Activity.side: BUY/SELL on trades, empty where no side applies;
    // /v2/activity type: tips carry IN/OUT
    assert_eq!(
        wire(ActivitySide::ALL),
        set(&["BUY", "SELL", "IN", "OUT", ""]),
        "ActivitySide"
    );
}

#[test]
fn every_variant_round_trips_through_display_fromstr_and_serde() {
    round_trips(FilterType::ALL);
    round_trips(TimePeriod::ALL);
    round_trips(LeaderboardBoard::ALL);
    round_trips(PnlInterval::ALL);
    round_trips(PnlFidelity::ALL);
    round_trips(PricesInterval::ALL);
    round_trips(ComboPositionSortBy::ALL);
    round_trips(PositionSortBy::ALL);
    round_trips(ComboPositionStatus::ALL);
    round_trips(ActivitySortBy::ALL);
    round_trips(TradeSide::ALL);
    round_trips(PositionStatus::ALL);
    round_trips(ActivityType::ALL);
    round_trips(ActivitySide::ALL);
}

#[test]
fn a_request_only_enum_refuses_an_unknown_value() {
    assert!("NOPE".parse::<FilterType>().is_err());
    assert!(serde_json::from_value::<FilterType>(Value::String("NOPE".into())).is_err());
}

#[test]
fn a_response_enum_keeps_an_unknown_value_verbatim() {
    let future =
        serde_json::from_value::<ActivityType>(Value::String("FUTURE_TYPE".into())).unwrap();
    assert_eq!(future, ActivityType::Other("FUTURE_TYPE".into()));
    assert_eq!(
        serde_json::to_value(&future).unwrap(),
        Value::String("FUTURE_TYPE".into())
    );
    assert_eq!("FUTURE_TYPE".parse::<ActivityType>(), Ok(future));
    assert!(!ActivityType::ALL
        .iter()
        .any(|t| matches!(t, ActivityType::Other(_))));
}
