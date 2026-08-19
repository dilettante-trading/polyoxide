//! Live integration tests against the Polymarket Gamma API.
//!
//! These tests hit the real API and require network access.
//! They are gated behind `#[ignore]` so they don't run in CI.
//!
//! Run manually with:
//! ```sh
//! cargo test -p polyoxide-gamma --test live_api -- --ignored
//! ```

use polyoxide_core::ApiError;
use polyoxide_gamma::types::ParentEntityType;
use polyoxide_gamma::{Gamma, GammaError};
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
        .query_by_information(body)
        .send()
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
        .query_abridged(body)
        .send()
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

    // Use a tag known to have relations. The previous version of this test took
    // whatever `list()` returned first, which is almost always a long-tail tag
    // with zero relations — an empty array parses as *any* element type, so the
    // test passed for years while the response was typed as `Vec<Tag>` instead
    // of `Vec<RelatedTag>`. Asserting on a populated response is the point.
    let related = gamma
        .tags()
        .get_related_by_slug("politics")
        .send()
        .await
        .expect("get related tags");

    assert!(
        !related.is_empty(),
        "expected the 'politics' tag to have relations; if upstream changed, \
         pick another well-connected tag rather than relaxing this assertion"
    );

    for row in &related {
        assert!(!row.id.is_empty(), "relationship row needs its own id");
        // The type allows these to be null because the upstream schema says
        // nullable, but nothing observed live has been. Asserting they are
        // populated makes this test a canary: if it starts failing, the venue
        // really does emit nulls and the Option typing is earning its keep.
        assert!(
            row.related_tag_id.is_some_and(|id| id > 0),
            "row must name the tag it relates to, got {:?}",
            row.related_tag_id
        );
    }

    // The by-ID route must return the same rows as the by-slug route.
    let tag_id = related[0]
        .tag_id
        .expect("the queried tag's own id should be populated");
    let by_id = gamma
        .tags()
        .get_related(tag_id.to_string())
        .send()
        .await
        .expect("get related tags by id");
    assert_eq!(
        by_id.len(),
        related.len(),
        "by-id and by-slug routes disagree"
    );

    // `/related-tags/tags` is the sibling that really does return tags; the two
    // shapes must not be conflated again.
    let detailed = gamma
        .tags()
        .get_related_detailed_by_slug("politics")
        .send()
        .await
        .expect("get detailed related tags");
    assert!(
        detailed.iter().all(|t| !t.slug.is_empty()),
        "/related-tags/tags must return Tag objects with slugs"
    );
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

#[tokio::test]
#[ignore]
async fn live_get_series_summary() {
    let gamma = client();
    let series = gamma
        .series()
        .list()
        .limit(1)
        .send()
        .await
        .expect("list series to discover id");
    let first = series.first().expect("need at least one series");

    let summary = gamma
        .series()
        .get_summary(&first.id)
        .send()
        .await
        .expect("get series summary by id");
    assert_eq!(summary.id, first.id);
}

#[tokio::test]
#[ignore]
async fn live_get_series_summary_by_slug() {
    let gamma = client();
    let series = gamma
        .series()
        .list()
        .limit(1)
        .send()
        .await
        .expect("list series to discover slug");
    let first = series.first().expect("need at least one series");
    let slug = first.slug.clone();

    let summary = gamma
        .series()
        .get_summary_by_slug(&slug)
        .send()
        .await
        .expect("get series summary by slug");
    // The upstream may map slug to a different summary id, but deserialization
    // is the primary assertion here.
    let _ = summary;
}

#[tokio::test]
#[ignore]
async fn live_series_comment_count() {
    let gamma = client();
    let series = gamma
        .series()
        .list()
        .limit(1)
        .send()
        .await
        .expect("list series to discover id");
    let first = series.first().expect("need at least one series");
    let count = gamma
        .series()
        .comment_count(&first.id)
        .send()
        .await
        .expect("get series comment count");
    // Deserialization is the primary assertion; count is u64 so any value is
    // valid.
    let _ = count;
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

#[tokio::test]
#[ignore]
async fn live_get_team_by_id() {
    let gamma = client();
    let teams = gamma
        .sports()
        .list_teams()
        .limit(1)
        .send()
        .await
        .expect("list teams to discover id");
    let first = teams.first().expect("need at least one team");
    let id = first.id.to_string();

    let team = gamma
        .sports()
        .get_team(&id)
        .send()
        .await
        .expect("get team by id");
    assert_eq!(team.id, first.id);
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
        .parent_entity_type(ParentEntityType::Event)
        .parent_entity_id(event_id)
        .limit(5)
        .send()
        .await
        .expect("list comments");
    // An empty result is not signal: the discovered event may simply have no
    // comments, which is exactly the luck that let issue #28 hide for months.
    // Say so out loud rather than passing silently.
    if comments.is_empty() {
        eprintln!(
            "SKIPPED: no comments on event {event_id}; this run did not exercise \
             comment deserialization"
        );
        return;
    }
    assert!(
        comments.iter().all(|c| !c.id.is_empty()),
        "every comment must carry an id"
    );
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
        .parent_entity_type(ParentEntityType::Event)
        .parent_entity_id(event_id)
        .limit(1)
        .send()
        .await
        .expect("list comments");

    let Some(comment) = comments.first() else {
        eprintln!("SKIPPED: no comments on event {event_id}; nothing to fetch by id");
        return;
    };
    let thread = gamma
        .comments()
        .get(&comment.id)
        .send()
        .await
        .expect("get comment thread by id");
    // Upstream returns the whole thread, with the requested id somewhere
    // inside it — not necessarily first.
    assert!(
        thread.iter().any(|c| c.id == comment.id),
        "the requested comment must appear in the returned thread"
    );
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

/// Regression test for the `SearchResponse::profiles` null-element failure
/// (see `docs/specs/gamma/OBSERVED.md` and `docs/plans/2026-08-19-gamma-type-parity-worklist.md`,
/// follow-up 17). `q=sports` at this `limit_per_type` reliably returns a JSON
/// `null` entry in `profiles`; the old `Vec<SearchProfile>` errored the whole
/// call on it instead of tolerating it.
#[tokio::test]
#[ignore]
async fn live_public_search_sports_profiles() {
    let gamma = client();
    let results = gamma
        .search()
        .public_search("sports")
        .search_profiles(true)
        .limit_per_type(20)
        .send()
        .await
        .expect("public search must deserialize even when profiles contains a null entry");
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
        .parent_entity_type(ParentEntityType::Event)
        .parent_entity_id(event_id)
        .limit(20)
        .send()
        .await
        .expect("list comments to find a user");

    let Some(comment) = comments.first() else {
        eprintln!("SKIPPED: no comments on event {event_id}; no address to resolve");
        return;
    };
    // `/public-profile` wants an address. The old code passed `comment.user.id`,
    // which was an id-shaped field that never existed on the wire.
    let Some(address) = comment.user_address.as_deref() else {
        eprintln!("SKIPPED: comment {} carries no userAddress", comment.id);
        return;
    };
    let user = gamma
        .user()
        .get(address)
        .send()
        .await
        .expect("get user profile");
    let _ = user;
}

#[tokio::test]
#[ignore]
async fn live_get_profile_by_address() {
    let gamma = client();

    // Discover a real user proxy-wallet address via the public-profile
    // endpoint; fall back to exercising the endpoint with a burner address if
    // no real user is found.
    let events = gamma
        .events()
        .list()
        .active(true)
        .limit(5)
        .send()
        .await
        .expect("list events");
    let Some(first) = events.first() else { return };
    let event_id: i64 = first.id.parse().expect("event id should be numeric");

    let comments = gamma
        .comments()
        .list()
        .parent_entity_type(ParentEntityType::Event)
        .parent_entity_id(event_id)
        .limit(20)
        .send()
        .await
        .expect("list comments to find an address");

    let Some(comment) = comments.first() else {
        return;
    };
    let Some(user_address) = comment.user_address.as_deref() else {
        return;
    };
    let user = gamma
        .user()
        .get(user_address)
        .send()
        .await
        .expect("resolve user to proxy wallet");
    let Some(address) = user.proxy.clone() else {
        return;
    };

    // The endpoint returns 200 for almost any address that has ever touched
    // the platform; a 404 here means `address` genuinely has no profile,
    // which is a legitimate skip. Anything else — in particular a
    // deserialization error — is a real failure and must not be swallowed.
    // See `docs/plans/2026-08-19-gamma-type-parity-worklist.md` finding #1,
    // fixed by modelling `Profile` against the endpoint's published schema.
    match gamma.user().get_by_address(&address).send().await {
        Ok(profile) => {
            // Every capture behind tests/wire_agreement.rs carried all three;
            // taker_tier_name in particular must never be empty.
            assert!(
                !profile.taker_tier_name.is_empty(),
                "takerTierName must not be empty"
            );
        }
        Err(GammaError::Api(ApiError::Api { status: 404, .. })) => {
            eprintln!("SKIPPED: {address} has no profile (404)");
        }
        Err(e) => panic!("get_by_address({address}) failed: {e}"),
    }
}

#[tokio::test]
#[ignore]
async fn live_get_event_creator_by_id() {
    let gamma = client();
    let creators = gamma
        .events()
        .list_creators()
        .limit(1)
        .send()
        .await
        .expect("list event creators to discover id");
    let Some(first) = creators.first() else {
        return; // No creators available; treat as skip.
    };
    let _ = gamma
        .events()
        .get_creator(&first.id)
        .send()
        .await
        .expect("get event creator by id");
}

#[tokio::test]
#[ignore]
async fn live_get_related_detailed_by_id() {
    let gamma = client();
    let tags = gamma
        .tags()
        .list()
        .limit(1)
        .send()
        .await
        .expect("list tags to discover id");
    let first = tags.first().expect("need at least one tag");
    let _ = gamma
        .tags()
        .get_related_detailed(&first.id)
        .send()
        .await
        .expect("get related detailed by id");
}

#[tokio::test]
#[ignore]
async fn live_get_related_detailed_by_slug() {
    let gamma = client();
    let tags = gamma
        .tags()
        .list()
        .limit(1)
        .send()
        .await
        .expect("list tags to discover slug");
    let first = tags.first().expect("need at least one tag");
    let _ = gamma
        .tags()
        .get_related_detailed_by_slug(&first.slug)
        .send()
        .await
        .expect("get related detailed by slug");
}
