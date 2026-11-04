//! The Data API's records are built by struct literal outside the crate, as a
//! consumer that synthesizes rows or returns its own pages does (prader-rs
//! #179): none of the structs under `data::types`, nor `Page` or `Pagination`,
//! is `#[non_exhaustive]`. The enums stay `#[non_exhaustive]`, since their
//! `Other(raw)` arm already takes what this version does not know.

use polyoxide_polymarket::data::types::*;
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

// One literal per record struct under `data::types`, written out field by field, so
// a struct made `#[non_exhaustive]`, a field made private and a field added (each a
// breaking change now) all fail to build. Generated from `src/data/types/*.rs`.

fn activity() -> Activity {
    Activity {
        bio: String::new(),
        condition_id: String::new(),
        event_slug: String::new(),
        icon: String::new(),
        is_combo: None,
        name: String::new(),
        outcome: String::new(),
        outcome_index: OutcomeIndex::new(0),
        price: 0.0,
        profile_image: String::new(),
        profile_image_optimized: String::new(),
        proxy_wallet: String::new(),
        pseudonym: String::new(),
        side: ActivitySide::Other(String::new()),
        size: 0.0,
        slug: String::new(),
        timestamp: 0,
        title: String::new(),
        token_id: String::new(),
        transaction_hash: String::new(),
        activity_type: ActivityType::Other(String::new()),
        usdc_size: 0.0,
    }
}

fn approval_contract() -> ApprovalContract {
    ApprovalContract {
        amount: None,
        approved: false,
        feature: String::new(),
        id: String::new(),
        spender: String::new(),
        standard: String::new(),
        token: String::new(),
    }
}

fn approvals() -> Approvals {
    Approvals {
        address: String::new(),
        chain_id: 0,
        checked_at: String::new(),
        contracts: Vec::new(),
    }
}

fn biggest_winner() -> BiggestWinner {
    BiggestWinner {
        condition_id: String::new(),
        event_id: 0,
        event_slug: String::new(),
        event_title: String::new(),
        final_value: 0.0,
        initial_value: 0.0,
        kind: String::new(),
        pnl: 0.0,
        position_id: String::new(),
        profile_image: String::new(),
        resolved_at: 0,
        user_id: String::new(),
        user_name: String::new(),
        win_rank: 0,
    }
}

fn builder_standing() -> BuilderStanding {
    BuilderStanding {
        active_users: 0,
        builder_code: String::new(),
        builder_name: String::new(),
        profile_image: String::new(),
        rank: 0,
        verified: false,
        volume: 0.0,
    }
}

fn builder_volume_point() -> BuilderVolumePoint {
    BuilderVolumePoint {
        active_users: 0,
        builder_code: String::new(),
        builder_name: String::new(),
        date: String::new(),
        profile_image: String::new(),
        rank: 0,
        verified: false,
        volume: 0.0,
    }
}

fn combo_activity() -> ComboActivity {
    ComboActivity {
        amount_usdc: None,
        block_number: 0,
        combo_condition_id: String::new(),
        combo_position_id: String::new(),
        id: String::new(),
        legs: Vec::new(),
        payout_usdc: None,
        proxy_wallet: String::new(),
        timestamp: 0,
        transaction_hash: String::new(),
        combo_activity_type: String::new(),
    }
}

fn combo_leg() -> ComboLeg {
    ComboLeg {
        leg_condition_id: String::new(),
        leg_current_price: 0.0,
        leg_index: 0,
        leg_outcome_index: 0,
        leg_outcome_label: String::new(),
        leg_position_id: String::new(),
        leg_resolved_at: None,
        leg_status: String::new(),
        market: combo_leg_market(),
    }
}

fn combo_leg_event() -> ComboLegEvent {
    ComboLegEvent {
        event_id: String::new(),
        event_image: String::new(),
        event_slug: String::new(),
        event_title: String::new(),
    }
}

fn combo_leg_market() -> ComboLegMarket {
    ComboLegMarket {
        category: String::new(),
        end_date: String::new(),
        event: combo_leg_event(),
        group_item_title: None,
        icon_url: String::new(),
        image_url: String::new(),
        line: None,
        market_id: String::new(),
        outcome: String::new(),
        outcomes: None,
        question: None,
        slug: String::new(),
        sports_market_type: None,
        subcategory: String::new(),
        tags: Vec::new(),
        title: String::new(),
    }
}

fn combo_position() -> ComboPosition {
    ComboPosition {
        combo_condition_id: String::new(),
        combo_position_id: String::new(),
        current_size: 0.0,
        entry_avg_price_usdc: 0.0,
        entry_cost_usdc: 0.0,
        entry_fees_usdc: 0.0,
        first_entry_at: String::new(),
        first_entry_at_micros: None,
        gross_entry_cost_usdc: 0.0,
        legs: Vec::new(),
        legs_pending: 0,
        legs_resolved: 0,
        legs_total: 0,
        outcome_index: OutcomeIndex::new(0),
        outcome_label: String::new(),
        proxy_wallet: String::new(),
        realized_payout_usdc: 0.0,
        redeemable: false,
        resolved_at: None,
        status: String::new(),
        updated_at: String::new(),
        updated_at_micros: 0,
    }
}

fn condition_volume() -> ConditionVolume {
    ConditionVolume {
        condition_id: String::new(),
        taker_volume: 0.0,
    }
}

fn cursor_lag() -> CursorLag {
    CursorLag {
        behind_max: 0,
        block: 0,
        source: String::new(),
    }
}

fn holder() -> Holder {
    Holder {
        amount: 0.0,
        avg_price: None,
        bio: String::new(),
        current_price: None,
        current_value: None,
        display_username_public: false,
        entry_cost_usdc: None,
        name: String::new(),
        outcome_index: OutcomeIndex::new(0),
        profile_image: String::new(),
        profile_image_optimized: String::new(),
        proxy_wallet: String::new(),
        pseudonym: String::new(),
        realized_pnl: None,
        token_id: String::new(),
        total_pnl: None,
        unrealized_pnl: None,
        verified: false,
    }
}

fn ingestion_freshness() -> IngestionFreshness {
    IngestionFreshness {
        chain_id: 0,
        cursors: 0,
        lagging: Vec::new(),
        max_synced_block: None,
        min_synced_block: None,
        most_lagged: None,
        network: None,
    }
}

fn leaderboard_entry() -> LeaderboardEntry {
    LeaderboardEntry {
        pnl: 0.0,
        profile_image: String::new(),
        rank: 0,
        user_id: String::new(),
        user_name: String::new(),
        verified: false,
        volume: 0.0,
        x_username: String::new(),
    }
}

fn leaderboard_user_entry() -> LeaderboardUserEntry {
    LeaderboardUserEntry {
        pnl: 0.0,
        profile_image: String::new(),
        rank_pnl: None,
        rank_volume: None,
        user_id: String::new(),
        user_name: String::new(),
        verified: false,
        volume: 0.0,
        x_username: String::new(),
    }
}

fn live_volume() -> LiveVolume {
    LiveVolume {
        conditions: Vec::new(),
        taker_volume_total: 0.0,
    }
}

fn meta_holder() -> MetaHolder {
    MetaHolder {
        holders: Vec::new(),
        token_id: String::new(),
    }
}

fn open_interest() -> OpenInterest {
    OpenInterest {
        condition_id: String::new(),
        value: 0.0,
    }
}

fn portfolio_value() -> PortfolioValue {
    PortfolioValue {
        proxy_wallet: String::new(),
        value: 0.0,
    }
}

fn position() -> Position {
    Position {
        archived: false,
        avg_price: 0.0,
        condition_id: String::new(),
        current_price: 0.0,
        current_size: 0.0,
        current_value: 0.0,
        end_date: String::new(),
        entry_cost_usdc: 0.0,
        entry_fees_usdc: 0.0,
        event_id: String::new(),
        event_slug: String::new(),
        first_entry_at: 0,
        icon: String::new(),
        last_event_at: 0,
        mergeable: false,
        name: String::new(),
        negative_risk: false,
        opposite_outcome: String::new(),
        opposite_token_id: String::new(),
        outcome: String::new(),
        outcome_index: OutcomeIndex::new(0),
        percent_pnl: 0.0,
        percent_realized_pnl: 0.0,
        profile_image: String::new(),
        proxy_wallet: String::new(),
        realized_pnl: 0.0,
        redeemable: false,
        slug: String::new(),
        status: PositionStatus::Other(String::new()),
        title: String::new(),
        token_id: String::new(),
        total_cost_usdc: 0.0,
        total_pnl: 0.0,
        total_size: 0.0,
        unrealized_pnl: 0.0,
        verified: false,
    }
}

fn price_point() -> PricePoint {
    PricePoint {
        price: 0.0,
        resolution_seconds: 0,
        timestamp: 0,
    }
}

fn resolution() -> Resolution {
    Resolution {
        condition_id: None,
        expected_settlement_time: None,
        extended_review: false,
        last_update_timestamp: String::new(),
        log_index: String::new(),
        market_type: None,
        new_version_q: false,
        payouts: None,
        price: None,
        proposed_price: None,
        question_id: None,
        reporter: None,
        reproposed_price: None,
        resolution_source: None,
        resolved_at: None,
        resolved_block: None,
        settlement_time_basis: None,
        status: String::new(),
        transaction_hash: String::new(),
        was_arbitrated: None,
        was_disputed: false,
    }
}

fn service_status() -> ServiceStatus {
    ServiceStatus {
        age_seconds: 0,
        computed_at: String::new(),
        ingestion: ingestion_freshness(),
        serving: serving_freshness(),
    }
}

fn serving_freshness() -> ServingFreshness {
    ServingFreshness {
        lag_seconds: None,
        mechanisms: Vec::new(),
        worst: None,
    }
}

fn serving_mechanism() -> ServingMechanism {
    ServingMechanism {
        age_seconds: 0,
        blocks_behind: None,
        name: String::new(),
    }
}

fn token_ref() -> TokenRef {
    TokenRef {
        clob_index: None,
        closed: None,
        condition_id: String::new(),
        event_id: None,
        event_slug: None,
        final_price: None,
        market_slug: None,
        module: TokenModule::Other(String::new()),
        neg_risk: None,
        neg_risk_market_id: None,
        opposite_token_id: None,
        outcome: None,
        outcome_index: 0,
        question_index: None,
        resolved: false,
        structural_condition_id: None,
        title: None,
        token_id: String::new(),
    }
}

fn trade() -> Trade {
    Trade {
        bio: String::new(),
        condition_id: String::new(),
        event_slug: String::new(),
        icon: String::new(),
        name: String::new(),
        outcome: String::new(),
        outcome_index: OutcomeIndex::new(0),
        price: 0.0,
        profile_image: String::new(),
        profile_image_optimized: String::new(),
        proxy_wallet: String::new(),
        pseudonym: String::new(),
        side: TradeSide::Other(String::new()),
        size: 0.0,
        slug: String::new(),
        timestamp: 0,
        title: String::new(),
        token_id: String::new(),
        transaction_hash: String::new(),
    }
}

fn user_pnl_point() -> UserPnlPoint {
    UserPnlPoint {
        cashflow_net: None,
        deposits: None,
        economic_pnl: None,
        fees: None,
        fees_paid: None,
        fees_refunded: None,
        maker_rebate: None,
        position_pnl: None,
        realized_combo_pnl: 0.0,
        realized_lp_pnl: 0.0,
        realized_market_pnl: 0.0,
        realized_pnl: 0.0,
        referral_income: None,
        reward_income: None,
        settled_pnl: None,
        source_block: 0,
        sponsored_income: None,
        taker_rebate: None,
        timestamp: 0,
        trade_count: 0,
        trade_pnl: None,
        unrealized_pnl: None,
        volume: 0.0,
        volume_usdc: 0.0,
        wallet_income: None,
        withdrawals: None,
        yield_income: None,
    }
}

fn user_pnl_series() -> UserPnlSeries {
    UserPnlSeries {
        fidelity: String::new(),
        interval: String::new(),
        points: Vec::new(),
        proxy_wallet: String::new(),
        source_fidelity: String::new(),
    }
}

fn user_stats() -> UserStats {
    UserStats {
        all_time_pnl: None,
        biggest_win: 0.0,
        join_date: None,
        proxy_wallet: String::new(),
        trades: 0,
        views: 0,
    }
}

fn user_volume() -> UserVolume {
    UserVolume {
        trade_count: 0,
        volume: 0.0,
        volume_usdc: 0.0,
    }
}

#[test]
fn every_record_is_built_by_literal_outside_the_crate() {
    let _ = activity();
    let _ = approval_contract();
    let _ = approvals();
    let _ = biggest_winner();
    let _ = builder_standing();
    let _ = builder_volume_point();
    let _ = combo_activity();
    let _ = combo_leg();
    let _ = combo_leg_event();
    let _ = combo_leg_market();
    let _ = combo_position();
    let _ = condition_volume();
    let _ = cursor_lag();
    let _ = holder();
    let _ = ingestion_freshness();
    let _ = leaderboard_entry();
    let _ = leaderboard_user_entry();
    let _ = live_volume();
    let _ = meta_holder();
    let _ = open_interest();
    let _ = portfolio_value();
    let _ = position();
    let _ = price_point();
    let _ = resolution();
    let _ = service_status();
    let _ = serving_freshness();
    let _ = serving_mechanism();
    let _ = token_ref();
    let _ = trade();
    let _ = user_pnl_point();
    let _ = user_pnl_series();
    let _ = user_stats();
    let _ = user_volume();
}
