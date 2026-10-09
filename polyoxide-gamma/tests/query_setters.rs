//! Every query setter, called with a typed value, sends its key and value in
//! the order it was called: the golden test that holds the setters in place
//! while they move onto `polyoxide_core::query_setters!` (Story 3.8). A
//! changed name or argument type fails to compile; a changed key, value or
//! order fails the test.

use std::{future::Future, pin::Pin};

use polyoxide_gamma::{types::ParentEntityType, Gamma};

fn client(base: &str) -> Gamma {
    Gamma::builder().base_url(base).build().unwrap()
}

type Fire = fn(String) -> Pin<Box<dyn Future<Output = ()> + Send>>;

/// A builder, the path it sends to, a call of every setter it has, and the
/// pairs that call sends.
struct Case {
    builder: &'static str,
    path: &'static str,
    fire: Fire,
    sends: &'static [(&'static str, &'static str)],
}

const CASES: &[Case] = &[
    Case {
        builder: "ListComments",
        path: "/comments",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .comments()
                    .list()
                    .limit(5u32)
                    .offset(5u32)
                    .order("order-v")
                    .ascending(true)
                    .parent_entity_type(ParentEntityType::Series)
                    .parent_entity_id(-7i64)
                    .get_positions(true)
                    .holders_only(true)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("limit", "5"),
            ("offset", "5"),
            ("order", "order-v"),
            ("ascending", "true"),
            ("parent_entity_type", "Series"),
            ("parent_entity_id", "-7"),
            ("get_positions", "true"),
            ("holders_only", "true"),
        ],
    },
    Case {
        builder: "ListEventCreators",
        path: "/events/creators",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .events()
                    .list_creators()
                    .limit(5u32)
                    .offset(5u32)
                    .order("order-v")
                    .ascending(true)
                    .creator_name("creator_name-v")
                    .creator_handle("creator_handle-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("limit", "5"),
            ("offset", "5"),
            ("order", "order-v"),
            ("ascending", "true"),
            ("creator_name", "creator_name-v"),
            ("creator_handle", "creator_handle-v"),
        ],
    },
    Case {
        builder: "ListPaginatedEvents",
        path: "/events/pagination",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .events()
                    .list_paginated()
                    .limit(5u32)
                    .offset(5u32)
                    .order("order-v")
                    .ascending(true)
                    .include_chat(true)
                    .include_template(true)
                    .recurrence("recurrence-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("limit", "5"),
            ("offset", "5"),
            ("order", "order-v"),
            ("ascending", "true"),
            ("include_chat", "true"),
            ("include_template", "true"),
            ("recurrence", "recurrence-v"),
        ],
    },
    Case {
        builder: "ListEventResults",
        path: "/events/results",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .events()
                    .list_results()
                    .limit(5u32)
                    .offset(5u32)
                    .order("order-v")
                    .ascending(true)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("limit", "5"),
            ("offset", "5"),
            ("order", "order-v"),
            ("ascending", "true"),
        ],
    },
    Case {
        builder: "ListKeysetEvents",
        path: "/events/keyset",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .events()
                    .list_keyset()
                    .limit(5u32)
                    .order("order-v")
                    .ascending(true)
                    .after_cursor("after_cursor-v")
                    .id([11i64, 12i64])
                    .slug(["slug-1", "slug-2"])
                    .closed(true)
                    .live(true)
                    .featured(true)
                    .title_search("title_search-v")
                    .tag_id([11i64, 12i64])
                    .tag_slug("tag_slug-v")
                    .liquidity_min(1.5f64)
                    .liquidity_max(1.5f64)
                    .volume_min(1.5f64)
                    .volume_max(1.5f64)
                    .cyom(true)
                    .start_date_min("start_date_min-v")
                    .start_date_max("start_date_max-v")
                    .end_date_min("end_date_min-v")
                    .end_date_max("end_date_max-v")
                    .start_time_min("start_time_min-v")
                    .start_time_max("start_time_max-v")
                    .exclude_tag_id([11i64, 12i64])
                    .related_tags(true)
                    .tag_match("tag_match-v")
                    .series_id([11i64, 12i64])
                    .game_id([11i64, 12i64])
                    .event_date("event_date-v")
                    .event_week(-7i64)
                    .featured_order(true)
                    .recurrence("recurrence-v")
                    .created_by(["created_by-1", "created_by-2"])
                    .parent_event_id(-7i64)
                    .include_children(true)
                    .partner_slug("partner_slug-v")
                    .include_chat(true)
                    .include_template(true)
                    .include_best_lines(true)
                    .include_markets(true)
                    .locale("locale-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("limit", "5"),
            ("order", "order-v"),
            ("ascending", "true"),
            ("after_cursor", "after_cursor-v"),
            ("id", "11"),
            ("id", "12"),
            ("slug", "slug-1"),
            ("slug", "slug-2"),
            ("closed", "true"),
            ("live", "true"),
            ("featured", "true"),
            ("title_search", "title_search-v"),
            ("tag_id", "11"),
            ("tag_id", "12"),
            ("tag_slug", "tag_slug-v"),
            ("liquidity_min", "1.5"),
            ("liquidity_max", "1.5"),
            ("volume_min", "1.5"),
            ("volume_max", "1.5"),
            ("cyom", "true"),
            ("start_date_min", "start_date_min-v"),
            ("start_date_max", "start_date_max-v"),
            ("end_date_min", "end_date_min-v"),
            ("end_date_max", "end_date_max-v"),
            ("start_time_min", "start_time_min-v"),
            ("start_time_max", "start_time_max-v"),
            ("exclude_tag_id", "11"),
            ("exclude_tag_id", "12"),
            ("related_tags", "true"),
            ("tag_match", "tag_match-v"),
            ("series_id", "11"),
            ("series_id", "12"),
            ("game_id", "11"),
            ("game_id", "12"),
            ("event_date", "event_date-v"),
            ("event_week", "-7"),
            ("featured_order", "true"),
            ("recurrence", "recurrence-v"),
            ("created_by", "created_by-1"),
            ("created_by", "created_by-2"),
            ("parent_event_id", "-7"),
            ("include_children", "true"),
            ("partner_slug", "partner_slug-v"),
            ("include_chat", "true"),
            ("include_template", "true"),
            ("include_best_lines", "true"),
            ("include_markets", "true"),
            ("locale", "locale-v"),
        ],
    },
    Case {
        builder: "GetEvent",
        path: "/events/1",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .events()
                    .get("1")
                    .include_chat(true)
                    .include_template(true)
                    .send()
                    .await;
            })
        },
        sends: &[("include_chat", "true"), ("include_template", "true")],
    },
    Case {
        builder: "ListEvents",
        path: "/events",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .events()
                    .list()
                    .limit(5u32)
                    .offset(5u32)
                    .order("order-v")
                    .ascending(true)
                    .id([11i64, 12i64])
                    .game_id([11i64, 12i64])
                    .tag_id(-7i64)
                    .exclude_tag_id([11i64, 12i64])
                    .slug(["slug-1", "slug-2"])
                    .tag_slug("tag_slug-v")
                    .related_tags(true)
                    .active(true)
                    .archived(true)
                    .featured(true)
                    .cyom(true)
                    .include_chat(true)
                    .include_template(true)
                    .include_markets(true)
                    .recurrence("recurrence-v")
                    .closed(true)
                    .liquidity_min(1.5f64)
                    .liquidity_max(1.5f64)
                    .volume_min(1.5f64)
                    .volume_max(1.5f64)
                    .start_date_min("start_date_min-v")
                    .start_date_max("start_date_max-v")
                    .end_date_min("end_date_min-v")
                    .end_date_max("end_date_max-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("limit", "5"),
            ("offset", "5"),
            ("order", "order-v"),
            ("ascending", "true"),
            ("id", "11"),
            ("id", "12"),
            ("game_id", "11"),
            ("game_id", "12"),
            ("tag_id", "-7"),
            ("exclude_tag_id", "11"),
            ("exclude_tag_id", "12"),
            ("slug", "slug-1"),
            ("slug", "slug-2"),
            ("tag_slug", "tag_slug-v"),
            ("related_tags", "true"),
            ("active", "true"),
            ("archived", "true"),
            ("featured", "true"),
            ("cyom", "true"),
            ("include_chat", "true"),
            ("include_template", "true"),
            ("include_markets", "true"),
            ("recurrence", "recurrence-v"),
            ("closed", "true"),
            ("liquidity_min", "1.5"),
            ("liquidity_max", "1.5"),
            ("volume_min", "1.5"),
            ("volume_max", "1.5"),
            ("start_date_min", "start_date_min-v"),
            ("start_date_max", "start_date_max-v"),
            ("end_date_min", "end_date_min-v"),
            ("end_date_max", "end_date_max-v"),
        ],
    },
    Case {
        builder: "GetMarket",
        path: "/markets/1",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .get("1")
                    .include_tag(true)
                    .send()
                    .await;
            })
        },
        sends: &[("include_tag", "true")],
    },
    Case {
        builder: "ListMarkets",
        path: "/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .list()
                    .limit(5u32)
                    .offset(5u32)
                    .order("order-v")
                    .ascending(true)
                    .id([11i64, 12i64])
                    .slug(["slug-1", "slug-2"])
                    .clob_token_ids(["clob_token_ids-1", "clob_token_ids-2"])
                    .condition_ids(["condition_ids-1", "condition_ids-2"])
                    .market_maker_address(["market_maker_address-1", "market_maker_address-2"])
                    .liquidity_num_min(1.5f64)
                    .liquidity_num_max(1.5f64)
                    .volume_num_min(1.5f64)
                    .volume_num_max(1.5f64)
                    .start_date_min("start_date_min-v")
                    .start_date_max("start_date_max-v")
                    .end_date_min("end_date_min-v")
                    .end_date_max("end_date_max-v")
                    .tag_id(-7i64)
                    .related_tags(true)
                    .cyom(true)
                    .uma_resolution_status("uma_resolution_status-v")
                    .game_id("game_id-v")
                    .sports_market_types(["sports_market_types-1", "sports_market_types-2"])
                    .rewards_min_size(1.5f64)
                    .question_ids(["question_ids-1", "question_ids-2"])
                    .include_tag(true)
                    .closed(true)
                    .open(true)
                    .archived(true)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("limit", "5"),
            ("offset", "5"),
            ("order", "order-v"),
            ("ascending", "true"),
            ("id", "11"),
            ("id", "12"),
            ("slug", "slug-1"),
            ("slug", "slug-2"),
            ("clob_token_ids", "clob_token_ids-1"),
            ("clob_token_ids", "clob_token_ids-2"),
            ("condition_ids", "condition_ids-1"),
            ("condition_ids", "condition_ids-2"),
            ("market_maker_address", "market_maker_address-1"),
            ("market_maker_address", "market_maker_address-2"),
            ("liquidity_num_min", "1.5"),
            ("liquidity_num_max", "1.5"),
            ("volume_num_min", "1.5"),
            ("volume_num_max", "1.5"),
            ("start_date_min", "start_date_min-v"),
            ("start_date_max", "start_date_max-v"),
            ("end_date_min", "end_date_min-v"),
            ("end_date_max", "end_date_max-v"),
            ("tag_id", "-7"),
            ("related_tags", "true"),
            ("cyom", "true"),
            ("uma_resolution_status", "uma_resolution_status-v"),
            ("game_id", "game_id-v"),
            ("sports_market_types", "sports_market_types-1"),
            ("sports_market_types", "sports_market_types-2"),
            ("rewards_min_size", "1.5"),
            ("question_ids", "question_ids-1"),
            ("question_ids", "question_ids-2"),
            ("include_tag", "true"),
            ("closed", "true"),
            ("closed", "false"),
            ("archived", "true"),
        ],
    },
    Case {
        builder: "ListKeysetMarkets",
        path: "/markets/keyset",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .list_keyset()
                    .limit(5u32)
                    .order("order-v")
                    .ascending(true)
                    .after_cursor("after_cursor-v")
                    .id([11i64, 12i64])
                    .slug(["slug-1", "slug-2"])
                    .closed(true)
                    .clob_token_ids(["clob_token_ids-1", "clob_token_ids-2"])
                    .condition_ids(["condition_ids-1", "condition_ids-2"])
                    .question_ids(["question_ids-1", "question_ids-2"])
                    .market_maker_address(["market_maker_address-1", "market_maker_address-2"])
                    .liquidity_num_min(1.5f64)
                    .liquidity_num_max(1.5f64)
                    .volume_num_min(1.5f64)
                    .volume_num_max(1.5f64)
                    .start_date_min("start_date_min-v")
                    .start_date_max("start_date_max-v")
                    .end_date_min("end_date_min-v")
                    .end_date_max("end_date_max-v")
                    .tag_id([11i64, 12i64])
                    .related_tags(true)
                    .cyom(true)
                    .rfq_enabled(true)
                    .uma_resolution_status("uma_resolution_status-v")
                    .game_id("game_id-v")
                    .sports_market_types(["sports_market_types-1", "sports_market_types-2"])
                    .include_tag(true)
                    .decimalized(true)
                    .tag_match("tag_match-v")
                    .locale("locale-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("limit", "5"),
            ("order", "order-v"),
            ("ascending", "true"),
            ("after_cursor", "after_cursor-v"),
            ("id", "11"),
            ("id", "12"),
            ("slug", "slug-1"),
            ("slug", "slug-2"),
            ("closed", "true"),
            ("clob_token_ids", "clob_token_ids-1"),
            ("clob_token_ids", "clob_token_ids-2"),
            ("condition_ids", "condition_ids-1"),
            ("condition_ids", "condition_ids-2"),
            ("question_ids", "question_ids-1"),
            ("question_ids", "question_ids-2"),
            ("market_maker_address", "market_maker_address-1"),
            ("market_maker_address", "market_maker_address-2"),
            ("liquidity_num_min", "1.5"),
            ("liquidity_num_max", "1.5"),
            ("volume_num_min", "1.5"),
            ("volume_num_max", "1.5"),
            ("start_date_min", "start_date_min-v"),
            ("start_date_max", "start_date_max-v"),
            ("end_date_min", "end_date_min-v"),
            ("end_date_max", "end_date_max-v"),
            ("tag_id", "11"),
            ("tag_id", "12"),
            ("related_tags", "true"),
            ("cyom", "true"),
            ("rfq_enabled", "true"),
            ("uma_resolution_status", "uma_resolution_status-v"),
            ("game_id", "game_id-v"),
            ("sports_market_types", "sports_market_types-1"),
            ("sports_market_types", "sports_market_types-2"),
            ("include_tag", "true"),
            ("decimalized", "true"),
            ("tag_match", "tag_match-v"),
            ("locale", "locale-v"),
        ],
    },
    Case {
        builder: "PublicSearch",
        path: "/public-search",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .search()
                    .public_search("q-v")
                    .search_profiles(true)
                    .limit_per_type(5u32)
                    .page(5u32)
                    .cache(true)
                    .events_status("events_status-v")
                    .events_tag(["events_tag-1", "events_tag-2"])
                    .keep_closed_markets(8i32)
                    .sort("sort-v")
                    .ascending(true)
                    .search_tags(true)
                    .recurrence("recurrence-v")
                    .exclude_tag_id([11i64, 12i64])
                    .optimized(true)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("q", "q-v"),
            ("search_profiles", "true"),
            ("limit_per_type", "5"),
            ("page", "5"),
            ("cache", "true"),
            ("events_status", "events_status-v"),
            ("events_tag", "events_tag-1"),
            ("events_tag", "events_tag-2"),
            ("keep_closed_markets", "8"),
            ("sort", "sort-v"),
            ("ascending", "true"),
            ("search_tags", "true"),
            ("recurrence", "recurrence-v"),
            ("exclude_tag_id", "11"),
            ("exclude_tag_id", "12"),
            ("optimized", "true"),
        ],
    },
    Case {
        builder: "GetSeries",
        path: "/series/1",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .series()
                    .get("1")
                    .include_chat(true)
                    .send()
                    .await;
            })
        },
        sends: &[("include_chat", "true")],
    },
    Case {
        builder: "ListSeries",
        path: "/series",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .series()
                    .list()
                    .limit(5u32)
                    .offset(5u32)
                    .ascending(true)
                    .closed(true)
                    .slug(["slug-1", "slug-2"])
                    .categories_ids(["categories_ids-1", "categories_ids-2"])
                    .categories_labels(["categories_labels-1", "categories_labels-2"])
                    .include_chat(true)
                    .recurrence("recurrence-v")
                    .order("order-v")
                    .exclude_events(true)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("limit", "5"),
            ("offset", "5"),
            ("ascending", "true"),
            ("closed", "true"),
            ("slug", "slug-1"),
            ("slug", "slug-2"),
            ("categories_ids", "categories_ids-1"),
            ("categories_ids", "categories_ids-2"),
            ("categories_labels", "categories_labels-1"),
            ("categories_labels", "categories_labels-2"),
            ("include_chat", "true"),
            ("recurrence", "recurrence-v"),
            ("order", "order-v"),
            ("exclude_events", "true"),
        ],
    },
    Case {
        builder: "ListTeams",
        path: "/teams",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .sports()
                    .list_teams()
                    .limit(5u32)
                    .offset(5u32)
                    .order("order-v")
                    .ascending(true)
                    .league(["league-1", "league-2"])
                    .name(["name-1", "name-2"])
                    .abbreviation(["abbreviation-1", "abbreviation-2"])
                    .send()
                    .await;
            })
        },
        sends: &[
            ("limit", "5"),
            ("offset", "5"),
            ("order", "order-v"),
            ("ascending", "true"),
            ("league", "league-1"),
            ("league", "league-2"),
            ("name", "name-1"),
            ("name", "name-2"),
            ("abbreviation", "abbreviation-1"),
            ("abbreviation", "abbreviation-2"),
        ],
    },
    Case {
        builder: "RelatedTags",
        path: "/tags/1/related-tags",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .tags()
                    .get_related("1")
                    .omit_empty(true)
                    .status("status-v")
                    .send()
                    .await;
            })
        },
        sends: &[("omit_empty", "true"), ("status", "status-v")],
    },
    Case {
        builder: "ListTags",
        path: "/tags",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .tags()
                    .list()
                    .limit(5u32)
                    .offset(5u32)
                    .order("order-v")
                    .ascending(true)
                    .include_template(true)
                    .is_carousel(true)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("limit", "5"),
            ("offset", "5"),
            ("order", "order-v"),
            ("ascending", "true"),
            ("include_template", "true"),
            ("is_carousel", "true"),
        ],
    },
    // an unknown parent entity type sends nothing.
    Case {
        builder: "ListComments",
        path: "/comments",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .comments()
                    .list()
                    .parent_entity_type(ParentEntityType::Unknown)
                    .send()
                    .await;
            })
        },
        sends: &[],
    },
    // open(false) asks for closed markets.
    Case {
        builder: "ListMarkets",
        path: "/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base).markets().list().open(false).send().await;
            })
        },
        sends: &[("closed", "true")],
    },
];

#[tokio::test]
async fn every_setter_sends_its_key_and_value() {
    for case in CASES {
        let pairs = polyoxide_test_support::query::pairs_sent(case.path, case.fire).await;
        let sent: Vec<(&str, &str)> = pairs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        assert_eq!(sent, case.sends, "{} on {}", case.builder, case.path);
    }
}
