//! The Data API's records are built by struct literal outside the crate, as a
//! consumer that synthesizes rows or returns its own pages does (prader-rs
//! #179): none of the structs under `data::types`, nor `Page` or `Pagination`,
//! is `#[non_exhaustive]`. The enums stay `#[non_exhaustive]`, since their
//! `Other(raw)` arm already takes what this version does not know.

use polyoxide_polymarket::data::types::{Activity, ActivitySide, ActivityType, OutcomeIndex};
use polyoxide_polymarket::data::{Page, Pagination};

/// A paper fill, as a row of the activity feed.
fn paper_fill(price: f64, size: f64) -> Activity {
    Activity {
        bio: String::new(),
        condition_id: "0xcondition".into(),
        event_slug: String::new(),
        icon: String::new(),
        is_combo: Some(false),
        name: String::new(),
        outcome: "Yes".into(),
        outcome_index: OutcomeIndex::new(0),
        price,
        profile_image: String::new(),
        profile_image_optimized: String::new(),
        proxy_wallet: "0xwallet".into(),
        pseudonym: String::new(),
        side: ActivitySide::Buy,
        size,
        slug: String::new(),
        timestamp: 1_792_000_000,
        title: "A market".into(),
        token_id: "1".into(),
        transaction_hash: String::new(),
        activity_type: ActivityType::Trade,
        usdc_size: price * size,
    }
}

#[test]
fn a_consumer_builds_its_own_page_of_rows() {
    let page = Page {
        data: vec![paper_fill(0.5, 10.0)],
        pagination: Pagination {
            limit: 1,
            offset: 0,
            has_more: true,
            next_cursor: Some("next".into()),
        },
    };

    // A page built here serializes as one the server sent decodes.
    let wire = serde_json::to_value(&page).unwrap();
    assert_eq!(wire["data"][0]["usdc_size"], 5.0);
    assert_eq!(wire["data"][0]["outcome_index"], 0);
    assert_eq!(wire["pagination"]["next_cursor"], "next");
    let round_trip: Page<Activity> = serde_json::from_value(wire).unwrap();
    assert_eq!(round_trip, page);
}

#[test]
fn an_unlabeled_outcome_index_is_built_from_its_sentinel() {
    let row = Activity {
        outcome_index: OutcomeIndex::new(OutcomeIndex::UNLABELED),
        ..paper_fill(0.5, 1.0)
    };
    assert_eq!(row.outcome_index.get(), None);
    assert_eq!(serde_json::to_value(&row).unwrap()["outcome_index"], 999);
}

/// Every source file holding a record struct, which the guard below reads.
const SOURCES: &[(&str, &str)] = &[
    (
        "types/boards.rs",
        include_str!("../src/data/types/boards.rs"),
    ),
    (
        "types/common.rs",
        include_str!("../src/data/types/common.rs"),
    ),
    ("types/feeds.rs", include_str!("../src/data/types/feeds.rs")),
    (
        "types/markets.rs",
        include_str!("../src/data/types/markets.rs"),
    ),
    (
        "types/service.rs",
        include_str!("../src/data/types/service.rs"),
    ),
    (
        "types/wallet.rs",
        include_str!("../src/data/types/wallet.rs"),
    ),
    ("envelope.rs", include_str!("../src/data/envelope.rs")),
];

#[test]
fn no_record_struct_is_non_exhaustive() {
    let mut structs = 0;
    for (file, source) in SOURCES {
        let lines: Vec<&str> = source.lines().collect();
        for (n, line) in lines.iter().enumerate() {
            if !line.starts_with("pub struct ") {
                continue;
            }
            structs += 1;
            // The attributes and doc lines directly above the struct.
            let above = lines[..n]
                .iter()
                .rev()
                .take_while(|l| l.starts_with("#[") || l.starts_with("///"));
            for attribute in above {
                assert!(
                    !attribute.contains("non_exhaustive"),
                    "src/data/{file}:{}: `{line}` is #[non_exhaustive], so a consumer \
                     cannot build it",
                    n + 1
                );
            }
        }
    }
    // Every record the module serves, so a file left out of SOURCES shows.
    assert_eq!(structs, 36, "the record structs read");
}
