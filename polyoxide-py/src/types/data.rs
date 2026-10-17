use pyo3::types::PyModuleMethods;

py_type!(
    PyPosition,
    "Position",
    polyoxide_polymarket::data::types::Position,
    proxy_wallet,
    asset,
    condition_id,
    size,
    avg_price,
    initial_value,
    gross_initial_value,
    entry_fees_usdc,
    current_value,
    cash_pnl,
    percent_pnl,
    total_bought,
    realized_pnl,
    percent_realized_pnl,
    cur_price,
    redeemable,
    mergeable,
    title,
    slug,
    icon,
    event_slug,
    outcome,
    outcome_index,
    opposite_outcome,
    opposite_asset,
    end_date,
    negative_risk,
);

py_type!(
    PyClosedPosition,
    "ClosedPosition",
    polyoxide_polymarket::data::types::ClosedPosition,
    proxy_wallet,
    asset,
    condition_id,
    avg_price,
    total_bought,
    realized_pnl,
    cur_price,
    timestamp,
    title,
    slug,
    icon,
    event_slug,
    outcome,
    outcome_index,
    opposite_outcome,
    opposite_asset,
    end_date,
);

py_type!(
    PyTrade,
    "Trade",
    polyoxide_polymarket::data::types::Trade,
    proxy_wallet,
    side,
    asset,
    condition_id,
    size,
    price,
    timestamp,
    title,
    slug,
    icon,
    event_slug,
    outcome,
    outcome_index,
    name,
    pseudonym,
    transaction_hash,
);

py_type!(PyActivity, "Activity", polyoxide_polymarket::data::types::Activity,
    proxy_wallet, timestamp, condition_id,
    activity_type => "type",
    size, usdc_size, transaction_hash, price, asset, side,
    outcome_index, title, slug, icon, outcome,
);

py_type!(
    PyUserValue,
    "UserValue",
    polyoxide_polymarket::data::types::UserValue,
    user,
    value,
);

py_type!(
    PyOpenInterest,
    "OpenInterest",
    polyoxide_polymarket::data::types::OpenInterest,
    market,
    value,
);

py_type!(
    PyUserTraded,
    "UserTraded",
    polyoxide_polymarket::data::api::users::UserTraded,
    user,
    traded,
);

py_type!(
    PyMarketHolders,
    "MarketHolders",
    polyoxide_polymarket::data::api::holders::MarketHolders,
    token,
    holders,
);

py_type!(
    PyHolder,
    "Holder",
    polyoxide_polymarket::data::api::holders::Holder,
    proxy_wallet,
    bio,
    asset,
    pseudonym,
    amount,
    display_username_public,
    outcome_index,
    name,
    profile_image,
    verified,
);

py_type!(
    PyTraderRanking,
    "TraderRanking",
    polyoxide_polymarket::data::api::leaderboard::TraderRanking,
    rank,
    proxy_wallet,
    user_name,
    vol,
    pnl,
    profile_image,
    x_username,
    verified_badge,
);

py_type!(
    PyBuilderRanking,
    "BuilderRanking",
    polyoxide_polymarket::data::api::builders::BuilderRanking,
    rank,
    builder,
    volume,
    active_users,
    verified,
    builder_logo,
);

py_type!(
    PyBuilderVolume,
    "BuilderVolume",
    polyoxide_polymarket::data::api::builders::BuilderVolume,
    dt,
    builder,
    builder_logo,
    verified,
    volume,
    active_users,
    rank,
);

py_type!(
    PyLiveVolume,
    "LiveVolume",
    polyoxide_polymarket::data::api::live_volume::LiveVolume,
    total,
    markets,
);

py_type!(
    PyMarketVolume,
    "MarketVolume",
    polyoxide_polymarket::data::api::live_volume::MarketVolume,
    market,
    value,
);

py_type!(
    PyHealthResponse,
    "HealthResponse",
    polyoxide_polymarket::data::api::health::HealthResponse,
    data,
);

pub fn register(m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
    m.add_class::<PyPosition>()?;
    m.add_class::<PyClosedPosition>()?;
    m.add_class::<PyTrade>()?;
    m.add_class::<PyActivity>()?;
    m.add_class::<PyUserValue>()?;
    m.add_class::<PyOpenInterest>()?;
    m.add_class::<PyUserTraded>()?;
    m.add_class::<PyMarketHolders>()?;
    m.add_class::<PyHolder>()?;
    m.add_class::<PyTraderRanking>()?;
    m.add_class::<PyBuilderRanking>()?;
    m.add_class::<PyBuilderVolume>()?;
    m.add_class::<PyLiveVolume>()?;
    m.add_class::<PyMarketVolume>()?;
    m.add_class::<PyHealthResponse>()?;
    Ok(())
}
