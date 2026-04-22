//! Live integration tests against the Polymarket Gamma API.
//!
//! These tests hit the real API and require network access.
//! They are gated behind `#[ignore]` so they don't run in CI.
//!
//! Run manually with:
//! ```sh
//! cargo test -p polyoxide-gamma --test live_api -- --ignored
//! ```

use polyoxide_gamma::Gamma;
use std::time::Duration;

fn client() -> Gamma {
    Gamma::builder().build().expect("gamma client")
}

// ── Health ───────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_ping() {
    let gamma = client();
    let latency = gamma.health().ping().await.expect("ping should succeed");
    assert!(
        latency < Duration::from_secs(10),
        "latency too high: {latency:?}"
    );
}

// ── Markets ──────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_list_markets() {
    let gamma = client();
    let markets = gamma
        .markets()
        .list()
        .limit(5)
        .send()
        .await
        .expect("list markets");
    assert!(!markets.is_empty(), "should return at least one market");
}

#[tokio::test]
#[ignore]
async fn live_get_market_by_id() {
    let gamma = client();
    let markets = gamma
        .markets()
        .list()
        .limit(1)
        .send()
        .await
        .expect("list markets to discover id");
    let first = markets.first().expect("need at least one market");
    let id = first.id.clone();

    let market = gamma
        .markets()
        .get(&id)
        .send()
        .await
        .expect("get market by id");
    assert_eq!(market.id, id);
}

#[tokio::test]
#[ignore]
async fn live_get_market_by_slug() {
    let gamma = client();
    let markets = gamma
        .markets()
        .list()
        .limit(10)
        .send()
        .await
        .expect("list markets to discover slug");
    let market_with_slug = markets
        .iter()
        .find(|m| m.slug.is_some())
        .expect("need at least one market with a slug");
    let slug = market_with_slug.slug.as_ref().unwrap().clone();

    let market = gamma
        .markets()
        .get_by_slug(&slug)
        .send()
        .await
        .expect("get market by slug");
    assert_eq!(market.slug.as_deref(), Some(slug.as_str()));
}

#[tokio::test]
#[ignore]
async fn live_list_markets_closed_true() {
    let gamma = client();
    let markets = gamma
        .markets()
        .list()
        .closed(true)
        .limit(5)
        .send()
        .await
        .expect("list closed markets");
    assert!(
        !markets.is_empty(),
        "should return at least one closed market"
    );
    for m in &markets {
        assert_eq!(m.closed, Some(true), "market {} should be closed", m.id);
    }
}

#[tokio::test]
#[ignore]
async fn live_list_markets_closed_false() {
    let gamma = client();
    let markets = gamma
        .markets()
        .list()
        .closed(false)
        .limit(5)
        .send()
        .await
        .expect("list open markets");
    assert!(
        !markets.is_empty(),
        "should return at least one open market"
    );
    for m in &markets {
        assert_ne!(m.closed, Some(true), "market {} should not be closed", m.id);
    }
}

#[tokio::test]
#[ignore]
async fn live_get_many_returns_both_open_and_closed() {
    let gamma = client();

    // Discover one open and one closed market ID.
    let open = gamma
        .markets()
        .list()
        .closed(false)
        .limit(1)
        .send()
        .await
        .expect("list open markets");
    let closed = gamma
        .markets()
        .list()
        .closed(true)
        .limit(1)
        .send()
        .await
        .expect("list closed markets");

    let open_id: i64 = open
        .first()
        .expect("need an open market")
        .id
        .parse()
        .expect("open market id should be numeric");
    let closed_id: i64 = closed
        .first()
        .expect("need a closed market")
        .id
        .parse()
        .expect("closed market id should be numeric");

    let markets = gamma
        .markets()
        .get_many([open_id, closed_id])
        .send()
        .await
        .expect("get_many should succeed");

    let open_str = open_id.to_string();
    let closed_str = closed_id.to_string();
    let returned: Vec<&str> = markets.iter().map(|m| m.id.as_str()).collect();
    assert!(
        returned.contains(&open_str.as_str()),
        "open market {open_id} missing from get_many result: {returned:?}"
    );
    assert!(
        returned.contains(&closed_str.as_str()),
        "closed market {closed_id} missing from get_many result: {returned:?}"
    );
    assert!(
        markets.iter().any(|m| m.closed == Some(true)),
        "get_many result should include the closed market with closed=true"
    );
}

#[tokio::test]
#[ignore]
async fn live_get_market_description() {
    let gamma = client();

    // Discover a market id.
    let markets = gamma
        .markets()
        .list()
        .limit(1)
        .send()
        .await
        .expect("list markets to discover id");
    let first = markets.first().expect("need at least one market");

    let desc = gamma
        .markets()
        .get_description(&first.id)
        .send()
        .await
        .expect("get market description");
    // Deserialization succeeded; description may be None/empty on some markets.
    let _ = desc;
}

#[tokio::test]
#[ignore]
async fn live_query_markets_by_information() {
    use polyoxide_gamma::types::MarketsInformationBody;

    let gamma = client();

    // Discover a market id so we have something concrete to query.
    let markets = gamma
        .markets()
        .list()
        .limit(1)
        .send()
        .await
        .expect("list markets");
    let first = markets.first().expect("need at least one market");
    let id: i64 = first.id.parse().expect("market id should be numeric");

    let body = MarketsInformationBody {
        id: vec![id],
        ..Default::default()
    };
    let found = gamma
        .markets()
        .query_by_information(&body)
        .await
        .expect("POST /markets/information");
    assert!(
        found.iter().any(|m| m.id == first.id),
        "expected market {} in response",
        first.id
    );
}

#[tokio::test]
#[ignore]
async fn live_query_abridged_markets() {
    use polyoxide_gamma::types::MarketsInformationBody;

    let gamma = client();
    let body = MarketsInformationBody {
        closed: Some(false),
        ..Default::default()
    };
    let found = gamma
        .markets()
        .query_abridged(&body)
        .await
        .expect("POST /markets/abridged");
    // Deserialization is the primary assertion; the array may be empty.
    let _ = found;
}

#[tokio::test]
#[ignore]
async fn live_list_markets_keyset() {
    let gamma = client();
    let resp = gamma
        .markets()
        .list_keyset()
        .limit(5)
        .send()
        .await
        .expect("list markets (keyset)");
    // Deserialization is the assertion; upstream may page differently.
    let _ = resp;
}

// ── Events ──────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_list_events() {
    let gamma = client();
    let events = gamma
        .events()
        .list()
        .limit(5)
        .send()
        .await
        .expect("list events");
    assert!(!events.is_empty(), "should return at least one event");
}

#[tokio::test]
#[ignore]
async fn live_get_event_by_id() {
    let gamma = client();
    let events = gamma
        .events()
        .list()
        .limit(1)
        .send()
        .await
        .expect("list events to discover id");
    let first = events.first().expect("need at least one event");
    let id = first.id.clone();

    let event = gamma
        .events()
        .get(&id)
        .send()
        .await
        .expect("get event by id");
    assert_eq!(event.id, id);
}

#[tokio::test]
#[ignore]
async fn live_get_event_by_slug() {
    let gamma = client();
    let events = gamma
        .events()
        .list()
        .limit(10)
        .send()
        .await
        .expect("list events to discover slug");
    let event_with_slug = events
        .iter()
        .find(|e| e.slug.is_some())
        .expect("need at least one event with a slug");
    let slug = event_with_slug.slug.as_ref().unwrap().clone();

    let event = gamma
        .events()
        .get_by_slug(&slug)
        .send()
        .await
        .expect("get event by slug");
    assert_eq!(event.slug.as_deref(), Some(slug.as_str()));
}

#[tokio::test]
#[ignore]
async fn live_list_event_creators() {
    let gamma = client();
    let creators = gamma
        .events()
        .list_creators()
        .limit(5)
        .send()
        .await
        .expect("list event creators");
    let _ = creators; // may be empty; deserialization is the assertion
}

#[tokio::test]
#[ignore]
async fn live_list_events_pagination() {
    let gamma = client();
    let resp = gamma
        .events()
        .list_paginated()
        .limit(3)
        .send()
        .await
        .expect("list paginated events");
    // Data may be empty when no matching events; struct must deserialize.
    let _ = resp;
}

#[tokio::test]
#[ignore]
async fn live_list_events_results() {
    let gamma = client();
    let _events = gamma
        .events()
        .list_results()
        .limit(3)
        .send()
        .await
        .expect("list event results");
}

#[tokio::test]
#[ignore]
async fn live_list_events_keyset() {
    let gamma = client();
    let resp = gamma
        .events()
        .list_keyset()
        .limit(5)
        .send()
        .await
        .expect("list events (keyset)");
    let _ = resp; // events may be empty on some configurations; deserialization is the assertion
}

// ── Tags ────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_list_tags() {
    let gamma = client();
    let tags = gamma
        .tags()
        .list()
        .limit(5)
        .send()
        .await
        .expect("list tags");
    assert!(!tags.is_empty(), "should return at least one tag");
}

#[tokio::test]
#[ignore]
async fn live_get_tag_by_id() {
    let gamma = client();
    let tags = gamma
        .tags()
        .list()
        .limit(1)
        .send()
        .await
        .expect("list tags to discover id");
    let first = tags.first().expect("need at least one tag");
    let id = first.id.clone();

    let tag = gamma.tags().get(&id).send().await.expect("get tag by id");
    assert_eq!(tag.id, id);
}

#[tokio::test]
#[ignore]
async fn live_get_tag_by_slug() {
    let gamma = client();
    let tags = gamma
        .tags()
        .list()
        .limit(10)
        .send()
        .await
        .expect("list tags to discover slug");
    let first = tags.first().expect("need at least one tag");
    let slug = first.slug.clone();

    let tag = gamma
        .tags()
        .get_by_slug(&slug)
        .send()
        .await
        .expect("get tag by slug");
    assert_eq!(tag.slug, slug);
}

#[tokio::test]
#[ignore]
async fn live_get_related_tags() {
    let gamma = client();
    let tags = gamma
        .tags()
        .list()
        .limit(10)
        .send()
        .await
        .expect("list tags to discover id");
    let first = tags.first().expect("need at least one tag");
    let id = first.id.clone();

    // Related tags may be empty for some tags, but the call should succeed.
    let _related = gamma
        .tags()
        .get_related(&id)
        .send()
        .await
        .expect("get related tags");
}

// ── Series ──────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_list_series() {
    let gamma = client();
    let series = gamma
        .series()
        .list()
        .limit(5)
        .send()
        .await
        .expect("list series");
    assert!(!series.is_empty(), "should return at least one series");
}

#[tokio::test]
#[ignore]
async fn live_get_series_by_id() {
    let gamma = client();
    let series = gamma
        .series()
        .list()
        .limit(1)
        .send()
        .await
        .expect("list series to discover id");
    let first = series.first().expect("need at least one series");
    let id = first.id.clone();

    let s = gamma
        .series()
        .get(&id)
        .send()
        .await
        .expect("get series by id");
    assert_eq!(s.id, id);
}

// ── Sports ──────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_list_sports() {
    let gamma = client();
    let sports = gamma.sports().list().send().await.expect("list sports");
    assert!(
        !sports.is_empty(),
        "should return at least one sport metadata entry"
    );
}

#[tokio::test]
#[ignore]
async fn live_list_teams() {
    let gamma = client();
    let teams = gamma
        .sports()
        .list_teams()
        .limit(5)
        .send()
        .await
        .expect("list teams");
    assert!(!teams.is_empty(), "should return at least one team");
}

// ── Comments ────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_list_comments() {
    let gamma = client();

    // The comments endpoint requires parent_entity_type and parent_entity_id.
    // Discover an event ID first.
    let events = gamma
        .events()
        .list()
        .limit(5)
        .send()
        .await
        .expect("list events to discover id for comments");
    let first = events.first().expect("need at least one event");
    let event_id: i64 = first.id.parse().expect("event id should be numeric");

    let comments = gamma
        .comments()
        .list()
        .parent_entity_type("Event")
        .parent_entity_id(event_id)
        .limit(5)
        .send()
        .await
        .expect("list comments");
    // Some events may have no comments, but deserialization must succeed.
    let _ = comments;
}

// ── Comments: get and by_user ───────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_get_comment_by_id() {
    let gamma = client();

    // Discover a comment ID from listing
    let events = gamma
        .events()
        .list()
        .active(true)
        .limit(5)
        .send()
        .await
        .expect("list events");
    let first = events.first().expect("need at least one event");
    let event_id: i64 = first.id.parse().expect("event id should be numeric");

    let comments = gamma
        .comments()
        .list()
        .parent_entity_type("Event")
        .parent_entity_id(event_id)
        .limit(1)
        .send()
        .await
        .expect("list comments");

    if let Some(comment) = comments.first() {
        let fetched = gamma
            .comments()
            .get(&comment.id)
            .send()
            .await
            .expect("get comment by id");
        assert_eq!(fetched.id, comment.id);
    }
}

// ── Events: related by ID, tags, counts ────────────────────────

#[tokio::test]
#[ignore]
async fn live_event_tags() {
    let gamma = client();
    let events = gamma
        .events()
        .list()
        .limit(1)
        .send()
        .await
        .expect("list events");
    let first = events.first().expect("need at least one event");

    let _tags = gamma
        .events()
        .tags(&first.id)
        .send()
        .await
        .expect("get event tags");
}

// ── Markets: tags ──────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_market_tags() {
    let gamma = client();
    let markets = gamma
        .markets()
        .list()
        .limit(1)
        .send()
        .await
        .expect("list markets");
    let first = markets.first().expect("need at least one market");

    let _tags = gamma
        .markets()
        .tags(&first.id)
        .send()
        .await
        .expect("get market tags");
}

// ── Sports: market types ───────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_sports_market_types() {
    let gamma = client();
    let _types = gamma
        .sports()
        .market_types()
        .send()
        .await
        .expect("sports market types should deserialize");
}

// ── Search ──────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_public_search() {
    let gamma = client();
    let results = gamma
        .search()
        .public_search("bitcoin")
        .search_profiles(true)
        .limit_per_type(5)
        .send()
        .await
        .expect("public search");
    // Search may return empty results for some queries,
    // but deserialization must succeed.
    let _ = results;
}

// ── User ────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_get_user() {
    let gamma = client();

    // Discover a real user address from comments.
    let events = gamma
        .events()
        .list()
        .active(true)
        .limit(5)
        .send()
        .await
        .expect("list events");
    let first = events.first().expect("need at least one event");
    let event_id: i64 = first.id.parse().expect("event id should be numeric");

    let comments = gamma
        .comments()
        .list()
        .parent_entity_type("Event")
        .parent_entity_id(event_id)
        .limit(20)
        .send()
        .await
        .expect("list comments to find a user");

    if let Some(comment) = comments.first() {
        let user_id = &comment.user.id;
        let user = gamma
            .user()
            .get(user_id)
            .send()
            .await
            .expect("get user profile");
        // Deserialization succeeded; the profile may have sparse fields.
        let _ = user;
    }
    // If no comments found, skip silently -- the endpoint itself is
    // exercised in the request path even when no suitable address exists.
}
