"""Live integration tests for polyoxide Python bindings.

These tests hit real Polymarket APIs. They mirror the Rust-side live_api.rs
tests in each crate but exercise the full Python binding round-trip.

Run with:
    cd polyoxide-py && uv run maturin develop && uv run pytest tests/ -v
"""

import asyncio

import pytest

import polyoxide

# ── Helpers ───────────────────────────────────────────────────────


def run_async(coro):
    """Run an async coroutine in a fresh event loop."""
    return asyncio.run(coro)


# ══════════════════════════════════════════════════════════════════
# Gamma — Sync
# ══════════════════════════════════════════════════════════════════


class TestGammaSyncHealth:
    def test_ping(self):
        gamma = polyoxide.GammaSync()
        latency = gamma.health().ping()
        assert isinstance(latency, float)
        assert 0 < latency < 10


class TestGammaSyncMarkets:
    def test_list(self):
        gamma = polyoxide.GammaSync()
        markets = gamma.markets().list(limit=3, open=True)
        assert len(markets) > 0

    def test_list_returns_market_objects(self):
        gamma = polyoxide.GammaSync()
        markets = gamma.markets().list(limit=1)
        m = markets[0]
        assert m.id is not None
        assert m.question is not None

    def test_to_dict(self):
        gamma = polyoxide.GammaSync()
        markets = gamma.markets().list(limit=1)
        d = markets[0].to_dict()
        assert isinstance(d, dict)
        assert "id" in d

    def test_repr_and_str(self):
        gamma = polyoxide.GammaSync()
        markets = gamma.markets().list(limit=1)
        m = markets[0]
        assert "Market(" in repr(m)
        assert isinstance(str(m), str)

    def test_get_by_id(self):
        gamma = polyoxide.GammaSync()
        markets = gamma.markets().list(limit=1)
        market_id = markets[0].id
        market = gamma.markets().get(market_id)
        assert market.id == market_id

    def test_get_by_slug(self):
        gamma = polyoxide.GammaSync()
        markets = gamma.markets().list(limit=10)
        market_with_slug = next((m for m in markets if m.slug is not None), None)
        if market_with_slug is None:
            return  # no slugs found, skip
        slug = market_with_slug.slug
        market = gamma.markets().get_by_slug(slug)
        assert market.slug == slug

    def test_list_closed(self):
        gamma = polyoxide.GammaSync()
        markets = gamma.markets().list(limit=3, closed=True)
        assert len(markets) > 0
        for m in markets:
            assert m.closed is True

    def test_tags(self):
        gamma = polyoxide.GammaSync()
        markets = gamma.markets().list(limit=1)
        _tags = gamma.markets().tags(markets[0].id)


class TestGammaSyncEvents:
    def test_list(self):
        gamma = polyoxide.GammaSync()
        events = gamma.events().list(limit=3)
        assert len(events) > 0
        e = events[0]
        assert e.id is not None

    def test_get_by_id(self):
        gamma = polyoxide.GammaSync()
        events = gamma.events().list(limit=1)
        event_id = events[0].id
        event = gamma.events().get(event_id)
        assert event.id == event_id

    def test_get_by_slug(self):
        gamma = polyoxide.GammaSync()
        events = gamma.events().list(limit=10)
        event_with_slug = next((e for e in events if e.slug is not None), None)
        if event_with_slug is None:
            return
        slug = event_with_slug.slug
        event = gamma.events().get_by_slug(slug)
        assert event.slug == slug


class TestGammaSyncTags:
    def test_list(self):
        gamma = polyoxide.GammaSync()
        tags = gamma.tags().list(limit=3)
        assert len(tags) > 0

    def test_get_by_id(self):
        gamma = polyoxide.GammaSync()
        tags = gamma.tags().list(limit=1)
        tag_id = tags[0].id
        tag = gamma.tags().get(tag_id)
        assert tag.id == tag_id


class TestGammaSyncSeries:
    def test_list(self):
        gamma = polyoxide.GammaSync()
        series = gamma.series().list(limit=3)
        assert len(series) > 0


class TestGammaSyncSports:
    def test_list(self):
        gamma = polyoxide.GammaSync()
        sports = gamma.sports().list()
        assert len(sports) > 0

    def test_list_teams(self):
        gamma = polyoxide.GammaSync()
        teams = gamma.sports().list_teams(limit=3)
        assert len(teams) > 0

    def test_market_types(self):
        gamma = polyoxide.GammaSync()
        types = gamma.sports().market_types()
        assert types is not None


class TestGammaSyncSearch:
    def test_public_search(self):
        gamma = polyoxide.GammaSync()
        result = gamma.search().public_search("bitcoin", limit_per_type=3)
        # search result should have to_dict
        d = result.to_dict()
        assert isinstance(d, dict)


class TestGammaSyncComments:
    def test_list(self):
        gamma = polyoxide.GammaSync()
        events = gamma.events().list(limit=1)
        event_id = int(events[0].id)
        _comments = gamma.comments().list(
            parent_entity_type="Event", parent_entity_id=event_id, limit=3
        )


# ══════════════════════════════════════════════════════════════════
# Gamma — Async
# ══════════════════════════════════════════════════════════════════


class TestGammaAsync:
    def test_list_markets(self):
        async def go():
            gamma = polyoxide.Gamma()
            markets = await gamma.markets().list(limit=2)
            assert len(markets) > 0
            m = markets[0]
            assert m.id is not None
            return markets

        run_async(go())

    def test_ping(self):
        async def go():
            gamma = polyoxide.Gamma()
            latency = await gamma.health().ping()
            assert isinstance(latency, float)
            assert 0 < latency < 10

        run_async(go())

    def test_to_dict(self):
        async def go():
            gamma = polyoxide.Gamma()
            markets = await gamma.markets().list(limit=1)
            d = markets[0].to_dict()
            assert isinstance(d, dict)
            assert "id" in d

        run_async(go())


# ══════════════════════════════════════════════════════════════════
# CLOB — Sync
# ══════════════════════════════════════════════════════════════════


class TestClobSyncHealth:
    def test_ping(self):
        clob = polyoxide.ClobClientSync()
        latency = clob.health().ping()
        assert isinstance(latency, float)
        assert 0 < latency < 10

    def test_server_time(self):
        clob = polyoxide.ClobClientSync()
        st = clob.health().server_time()
        d = st.to_dict()
        assert isinstance(d, dict)


class TestClobSyncMarkets:
    def test_list(self):
        clob = polyoxide.ClobClientSync()
        resp = clob.markets().list()
        assert resp.data is not None

    @pytest.mark.xfail(reason="upstream CLOB schema drift: missing field `question_id`")
    def test_simplified(self):
        clob = polyoxide.ClobClientSync()
        resp = clob.markets().simplified()
        assert resp.data is not None

    def test_sampling(self):
        clob = polyoxide.ClobClientSync()
        resp = clob.markets().sampling()
        assert resp is not None

    @pytest.mark.xfail(reason="upstream CLOB schema drift: missing field `question_id`")
    def test_sampling_simplified(self):
        clob = polyoxide.ClobClientSync()
        resp = clob.markets().sampling_simplified()
        assert resp is not None

    def test_order_book(self):
        """Discover an active token_id via Gamma, then fetch its order book."""
        gamma = polyoxide.GammaSync()
        markets = gamma.markets().list(limit=20, closed=False)
        token_id = None
        for m in markets:
            d = m.to_dict()
            ids = d.get("clobTokenIds")
            if ids:
                import json

                try:
                    parsed = json.loads(ids)
                    if parsed:
                        token_id = parsed[0]
                        break
                except (json.JSONDecodeError, TypeError):
                    continue
        if token_id is None:
            return  # no active token found, skip

        clob = polyoxide.ClobClientSync()
        book = clob.markets().order_book(token_id)
        assert book.market is not None

    def test_midpoint(self):
        gamma = polyoxide.GammaSync()
        markets = gamma.markets().list(limit=20, closed=False)
        token_id = _find_active_token_id(markets)
        if token_id is None:
            return
        clob = polyoxide.ClobClientSync()
        resp = clob.markets().midpoint(token_id)
        assert resp is not None

    def test_price(self):
        gamma = polyoxide.GammaSync()
        markets = gamma.markets().list(limit=20, closed=False)
        token_id = _find_active_token_id(markets)
        if token_id is None:
            return
        clob = polyoxide.ClobClientSync()
        resp = clob.markets().price(token_id, "BUY")
        assert resp is not None

    @pytest.mark.xfail(reason="upstream CLOB schema drift: missing field `token_id`")
    def test_spread(self):
        gamma = polyoxide.GammaSync()
        markets = gamma.markets().list(limit=20, closed=False)
        token_id = _find_active_token_id(markets)
        if token_id is None:
            return
        clob = polyoxide.ClobClientSync()
        resp = clob.markets().spread(token_id)
        assert resp is not None

    def test_neg_risk(self):
        gamma = polyoxide.GammaSync()
        markets = gamma.markets().list(limit=20, closed=False)
        token_id = _find_active_token_id(markets)
        if token_id is None:
            return
        clob = polyoxide.ClobClientSync()
        resp = clob.markets().neg_risk(token_id)
        assert resp is not None


# ══════════════════════════════════════════════════════════════════
# CLOB — Async
# ══════════════════════════════════════════════════════════════════


class TestClobAsync:
    def test_list_markets(self):
        async def go():
            clob = polyoxide.ClobClient()
            resp = await clob.markets().list()
            assert resp.data is not None

        run_async(go())

    def test_ping(self):
        async def go():
            clob = polyoxide.ClobClient()
            latency = await clob.health().ping()
            assert isinstance(latency, float)

        run_async(go())


# ══════════════════════════════════════════════════════════════════
# Data API — Sync
# ══════════════════════════════════════════════════════════════════

TEST_USER = "0x0000000000000000000000000000000000000001"


class TestDataSyncHealth:
    def test_ping(self):
        data = polyoxide.DataApiSync()
        latency = data.health().ping()
        assert isinstance(latency, float)
        assert 0 < latency < 10


class TestDataSyncTrades:
    def test_list(self):
        data = polyoxide.DataApiSync()
        trades = data.trades().list(limit=3)
        assert len(trades) > 0
        t = trades[0]
        d = t.to_dict()
        assert isinstance(d, dict)


class TestDataSyncOpenInterest:
    def test_get(self):
        data = polyoxide.DataApiSync()
        oi = data.open_interest().get()
        assert len(oi) > 0


class TestDataSyncLeaderboard:
    def test_get(self):
        data = polyoxide.DataApiSync()
        rankings = data.leaderboard().get(limit=3)
        assert len(rankings) > 0
        r = rankings[0]
        d = r.to_dict()
        assert isinstance(d, dict)


class TestDataSyncBuilders:
    def test_leaderboard(self):
        data = polyoxide.DataApiSync()
        rankings = data.builders().leaderboard(limit=3)
        assert len(rankings) > 0


class TestDataSyncUser:
    def test_list_positions(self):
        """Positions for a dummy address should return empty list, not error."""
        async def go():
            data = polyoxide.DataApi()
            positions = await data.user(TEST_USER).list_positions()
            assert isinstance(positions, list)

        run_async(go())

    def test_trades(self):
        async def go():
            data = polyoxide.DataApi()
            trades = await data.user(TEST_USER).trades()
            assert isinstance(trades, list)

        run_async(go())


# ══════════════════════════════════════════════════════════════════
# Data API — Async
# ══════════════════════════════════════════════════════════════════


class TestDataAsync:
    def test_list_trades(self):
        async def go():
            data = polyoxide.DataApi()
            trades = await data.trades().list(limit=2)
            assert len(trades) > 0

        run_async(go())

    def test_ping(self):
        async def go():
            data = polyoxide.DataApi()
            latency = await data.health().ping()
            assert isinstance(latency, float)

        run_async(go())

    def test_open_interest(self):
        async def go():
            data = polyoxide.DataApi()
            oi = await data.open_interest().get()
            assert len(oi) > 0

        run_async(go())


# ══════════════════════════════════════════════════════════════════
# Error Hierarchy
# ══════════════════════════════════════════════════════════════════


class TestErrors:
    def test_exception_hierarchy(self):
        assert issubclass(polyoxide.ApiError, polyoxide.PolyoxideError)
        assert issubclass(polyoxide.AuthenticationError, polyoxide.PolyoxideError)
        assert issubclass(polyoxide.ValidationError, polyoxide.PolyoxideError)
        assert issubclass(polyoxide.RateLimitError, polyoxide.PolyoxideError)
        assert issubclass(polyoxide.NetworkError, polyoxide.PolyoxideError)
        assert issubclass(polyoxide.TimeoutError, polyoxide.PolyoxideError)

    def test_polyoxide_error_is_exception(self):
        assert issubclass(polyoxide.PolyoxideError, Exception)


# ══════════════════════════════════════════════════════════════════
# Helpers
# ══════════════════════════════════════════════════════════════════


def _find_active_token_id(gamma_markets):
    """Extract first token_id from a list of Gamma markets."""
    import json

    for m in gamma_markets:
        d = m.to_dict()
        ids = d.get("clobTokenIds")
        if ids:
            try:
                parsed = json.loads(ids)
                if parsed:
                    return parsed[0]
            except (json.JSONDecodeError, TypeError):
                continue
    return None
