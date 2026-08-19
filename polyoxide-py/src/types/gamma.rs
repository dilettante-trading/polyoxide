use pyo3::types::PyModuleMethods;

py_type!(PyMarket, "Market", polyoxide_gamma::types::Market,
    id, condition_id, slug, question, description, category, active, closed, archived,
    question_id => "questionID",
    tokens, tags, neg_risk, volume, liquidity,
    volume_num, liquidity_num, end_date_iso, start_date_iso,
    volume_24hr => "volume24hr",
    volume_1wk => "volume1wk",
    volume_1mo => "volume1mo",
    volume_1yr => "volume1yr",
    submitted_by => "submitted_by",
    denomination_token => "denomationToken",
    image, icon, outcomes, outcome_prices,
    last_trade_price, best_bid, best_ask, spread,
    one_day_price_change, one_week_price_change,
    created_at, updated_at, closed_time,
    market_maker_address, clob_token_ids,
    enable_order_book, accepting_orders,
    comment_count, featured, restricted,
);

py_type!(
    PyMarketToken,
    "MarketToken",
    polyoxide_gamma::types::MarketToken,
    token_id,
    outcome,
    price,
    winner,
);

py_type!(PyEvent, "Event", polyoxide_gamma::types::Event,
    id, ticker, slug, title, subtitle, description,
    start_date, end_date, image, icon,
    active, closed, archived, featured, restricted,
    liquidity, open_interest,
    volume_24hr => "volume24hr",
    volume_1wk => "volume1wk",
    volume_1mo => "volume1mo",
    markets, tags, series,
    neg_risk, neg_risk_market_id,
    created_at, updated_at, category,
    comments_enabled, competitive,
);

py_type!(PySeriesInfo, "SeriesInfo", polyoxide_gamma::types::SeriesInfo,
    id, slug, title, ticker, series_type, recurrence,
    image, icon, active, closed, archived,
    volume_24hr => "volume24hr",
    comment_count,
);

py_type!(
    PySeriesData,
    "SeriesData",
    polyoxide_gamma::types::SeriesData,
    id,
    slug,
    title,
    description,
    image,
    icon,
    active,
    closed,
    archived,
    tags,
    volume,
    liquidity,
    events,
    competitive,
);

py_type!(
    PyTag,
    "Tag",
    polyoxide_gamma::types::Tag,
    id,
    slug,
    label,
    force_show,
    force_hide,
    is_carousel,
    created_at,
    updated_at,
);

py_type!(
    PyRelatedTag,
    "RelatedTag",
    polyoxide_gamma::types::RelatedTag,
    id,
    tag_id,
    related_tag_id,
    rank,
);

py_type!(
    PySportMetadata,
    "SportMetadata",
    polyoxide_gamma::types::SportMetadata,
    id,
    sport,
    image,
    resolution,
    ordering,
    tags,
    series,
);

py_type!(
    PyTeam,
    "Team",
    polyoxide_gamma::types::Team,
    id,
    name,
    league,
    record,
    logo,
    abbreviation,
    alias,
    created_at,
    updated_at,
);

py_type!(
    PyComment,
    "Comment",
    polyoxide_gamma::types::Comment,
    id,
    body,
    parent_entity_type,
    parent_entity_id => "parentEntityID",
    parent_comment_id => "parentCommentID",
    user_address,
    reply_address,
    created_at,
    updated_at,
    profile,
    reactions,
    report_count,
    reaction_count,
);

py_type!(
    PyCommentProfile,
    "CommentProfile",
    polyoxide_gamma::types::CommentProfile,
    name,
    pseudonym,
    display_username_public,
    bio,
    is_mod,
    is_creator,
    proxy_wallet,
    base_address,
    profile_image,
    profile_image_optimized,
    positions,
);

py_type!(
    PyCommentReaction,
    "CommentReaction",
    polyoxide_gamma::types::CommentReaction,
    id,
    comment_id => "commentID",
    reaction_type,
    icon,
    user_address,
    created_at,
    profile,
);

py_type!(
    PyCommentPosition,
    "CommentPosition",
    polyoxide_gamma::types::CommentPosition,
    token_id,
    position_size,
);

py_type!(
    PyCountResponse,
    "CountResponse",
    polyoxide_gamma::types::CountResponse,
    count,
);

py_type!(
    PyCursor,
    "Cursor",
    polyoxide_gamma::types::Cursor,
    next_cursor,
);

py_type!(
    PySearchResponse,
    "SearchResponse",
    polyoxide_gamma::api::search::SearchResponse,
    profiles,
    events,
    tags,
);

py_type!(
    PySearchProfile,
    "SearchProfile",
    polyoxide_gamma::api::search::SearchProfile,
    address,
    name,
    profile_image,
    pseudonym,
    bio,
    proxy_wallet,
);

py_type!(PyUserResponse, "UserResponse", polyoxide_gamma::api::user::UserResponse,
    proxy => "proxyWallet",
    address, id, name, created_at, profile_image,
    display_username_public, bio, pseudonym, x_username,
    verified_badge, users,
);

py_type!(PyUserInfo, "UserInfo", polyoxide_gamma::api::user::UserInfo,
    id, creator,
    moderator => "mod",
);

pub fn register(m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
    m.add_class::<PyMarket>()?;
    m.add_class::<PyMarketToken>()?;
    m.add_class::<PyEvent>()?;
    m.add_class::<PySeriesInfo>()?;
    m.add_class::<PySeriesData>()?;
    m.add_class::<PyTag>()?;
    m.add_class::<PyRelatedTag>()?;
    m.add_class::<PySportMetadata>()?;
    m.add_class::<PyTeam>()?;
    m.add_class::<PyComment>()?;
    m.add_class::<PyCommentProfile>()?;
    m.add_class::<PyCommentReaction>()?;
    m.add_class::<PyCommentPosition>()?;
    m.add_class::<PyCountResponse>()?;
    m.add_class::<PyCursor>()?;
    m.add_class::<PySearchResponse>()?;
    m.add_class::<PySearchProfile>()?;
    m.add_class::<PyUserResponse>()?;
    m.add_class::<PyUserInfo>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use pyo3::{Py, PyAny, PyResult, Python};

    use super::PyComment;

    /// The shared fixture `test_comment_types.py` also reads, so both guards
    /// stay pinned to the same captured payload.
    const COMMENT_FULL: &str =
        include_str!("../../../polyoxide-gamma/tests/fixtures/comment_full.json");

    fn assert_present(py: Python<'_>, name: &str, value: PyResult<Py<PyAny>>) {
        let value = value.unwrap_or_else(|e| panic!("Comment.{name}() errored: {e}"));
        assert!(
            !value.is_none(py),
            "Comment.{name}() resolved to None against a fixture that carries the key — \
             py_type!'s field list has drifted from the wire"
        );
    }

    /// `polyoxide-py/tests/test_comment_types.py` calls `hasattr(polyoxide.Comment, attr)`
    /// on the class object, which is always true: PyO3 registers a property
    /// descriptor for every entry in the `py_type!` list regardless of which
    /// JSON key it resolves. `get_field`/`get_field_exact`
    /// (`polyoxide-py/src/convert.rs`) return `py.None()` for a missing key,
    /// so a stale rename is invisible to a class-level `hasattr` check.
    ///
    /// This test builds a real `PyComment` from the shared fixture and reads
    /// every getter, so a drifted key actually fails. Concrete case it
    /// catches: delete `parent_entity_id => "parentEntityID"`'s rename from
    /// the `py_type!` list and the getter falls back to `parentEntityId` then
    /// `parent_entity_id`, neither of which the server sends —
    /// `parent_entity_id` silently becomes `None` in production, and only an
    /// instance-level check like this one notices.
    #[test]
    fn comment_getters_resolve_against_the_shared_fixture() {
        let comment: polyoxide_gamma::types::Comment =
            serde_json::from_str(COMMENT_FULL).expect("shared fixture deserializes into Comment");
        let py_comment = PyComment::from(comment);

        Python::attach(|py| {
            assert_present(py, "id", py_comment.id(py));
            assert_present(py, "body", py_comment.body(py));
            assert_present(py, "parent_entity_type", py_comment.parent_entity_type(py));
            assert_present(py, "parent_entity_id", py_comment.parent_entity_id(py));
            assert_present(py, "parent_comment_id", py_comment.parent_comment_id(py));
            assert_present(py, "user_address", py_comment.user_address(py));
            assert_present(py, "reply_address", py_comment.reply_address(py));
            assert_present(py, "created_at", py_comment.created_at(py));
            assert_present(py, "updated_at", py_comment.updated_at(py));
            assert_present(py, "profile", py_comment.profile(py));
            assert_present(py, "reactions", py_comment.reactions(py));
            assert_present(py, "report_count", py_comment.report_count(py));
            assert_present(py, "reaction_count", py_comment.reaction_count(py));

            // The two `ID`-suffixed renames specifically: pin the values, not
            // just presence, since these are exactly what the mistake above
            // would silently null out.
            let parent_entity_id: i64 = py_comment
                .parent_entity_id(py)
                .unwrap()
                .extract(py)
                .expect("parent_entity_id is a number");
            assert_eq!(parent_entity_id, 45915);

            let parent_comment_id: String = py_comment
                .parent_comment_id(py)
                .unwrap()
                .extract(py)
                .expect("parent_comment_id is a string");
            assert_eq!(parent_comment_id, "3218360");
        });
    }
}
