from __future__ import annotations

from collections.abc import Coroutine
from typing import Any, Generic, TypeVar

T = TypeVar("T")

# =============================================================================
# Errors
# =============================================================================

class PolyoxideError(Exception):
    """Base exception for all polyoxide errors.

    The attributes below carry a Data API v2 error body's fields. Each is set on
    every exception the SDK raises, and is None unless the error came from a v2
    route.
    """
    status: int | None
    """HTTP status."""
    code: str | None
    """Stable classification, e.g. `invalid_request`, `rate_limited`, `dependency_unavailable`."""
    retryable: bool | None
    """Whether the server says the request may be retried unchanged."""
    trace_id: str | None
    """Id to quote when reporting a failure."""
    parameter: str | None
    """The query parameter a validation failure concerns, when the server names one."""
    retry_after: float | None
    """The `Retry-After` delay in seconds, when the response carried one."""

class ApiError(PolyoxideError):
    """The venue refused the request (a 4xx not named below, such as a 400 or a 404).

    A Data API v2 error maps by its `code` instead, and every code but
    `invalid_request`, `rate_limited` and `request_timeout` raises this one.
    """
    ...

class AuthenticationError(PolyoxideError):
    """Authentication credentials are invalid or missing (401, 403)."""
    ...

class ValidationError(PolyoxideError):
    """The request was refused before it was sent, or could not be built or signed.

    A Data API v2 error maps by its `code` instead, and `invalid_request` raises this one.
    """
    ...

class RateLimitError(PolyoxideError):
    """API rate limit was exceeded (429)."""
    ...

class NetworkError(PolyoxideError):
    """No response arrived: a failed connect, a timeout, or a dropped connection."""
    ...

class UnavailableError(PolyoxideError):
    """The venue is unavailable for now (408, 425, 5xx); retrying may succeed."""
    ...

class RestrictedError(PolyoxideError):
    """The venue will not serve this caller (418, 451)."""
    ...

class DecodeError(PolyoxideError):
    """A response arrived but did not decode, or a page walk could not continue."""
    ...

class TimeoutError(UnavailableError):
    """A Data API v2 `request_timeout`: the request deadline or the datastore's statement timeout."""
    ...

# =============================================================================
# Gamma type classes
# =============================================================================

class Market:
    """A Polymarket prediction market."""
    @property
    def id(self) -> Any: ...
    @property
    def condition_id(self) -> Any: ...
    @property
    def slug(self) -> Any: ...
    @property
    def question(self) -> Any: ...
    @property
    def description(self) -> Any: ...
    @property
    def category(self) -> Any: ...
    @property
    def active(self) -> Any: ...
    @property
    def closed(self) -> Any: ...
    @property
    def archived(self) -> Any: ...
    @property
    def question_id(self) -> Any: ...
    @property
    def tokens(self) -> Any: ...
    @property
    def tags(self) -> Any: ...
    @property
    def neg_risk(self) -> Any: ...
    @property
    def volume(self) -> Any: ...
    @property
    def liquidity(self) -> Any: ...
    @property
    def volume_num(self) -> Any: ...
    @property
    def liquidity_num(self) -> Any: ...
    @property
    def end_date_iso(self) -> Any: ...
    @property
    def start_date_iso(self) -> Any: ...
    @property
    def volume_24hr(self) -> Any: ...
    @property
    def volume_1wk(self) -> Any: ...
    @property
    def volume_1mo(self) -> Any: ...
    @property
    def volume_1yr(self) -> Any: ...
    @property
    def submitted_by(self) -> Any: ...
    @property
    def denomination_token(self) -> Any: ...
    @property
    def image(self) -> Any: ...
    @property
    def icon(self) -> Any: ...
    @property
    def outcomes(self) -> Any: ...
    @property
    def outcome_prices(self) -> Any: ...
    @property
    def last_trade_price(self) -> Any: ...
    @property
    def best_bid(self) -> Any: ...
    @property
    def best_ask(self) -> Any: ...
    @property
    def spread(self) -> Any: ...
    @property
    def one_day_price_change(self) -> Any: ...
    @property
    def one_week_price_change(self) -> Any: ...
    @property
    def created_at(self) -> Any: ...
    @property
    def updated_at(self) -> Any: ...
    @property
    def closed_time(self) -> Any: ...
    @property
    def market_maker_address(self) -> Any: ...
    @property
    def clob_token_ids(self) -> Any: ...
    @property
    def enable_order_book(self) -> Any: ...
    @property
    def accepting_orders(self) -> Any: ...
    @property
    def comment_count(self) -> Any: ...
    @property
    def featured(self) -> Any: ...
    @property
    def restricted(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class MarketToken:
    """A token within a prediction market."""
    @property
    def token_id(self) -> Any: ...
    @property
    def outcome(self) -> Any: ...
    @property
    def price(self) -> Any: ...
    @property
    def winner(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class Event:
    """A Polymarket event grouping related markets."""
    @property
    def id(self) -> Any: ...
    @property
    def ticker(self) -> Any: ...
    @property
    def slug(self) -> Any: ...
    @property
    def title(self) -> Any: ...
    @property
    def subtitle(self) -> Any: ...
    @property
    def description(self) -> Any: ...
    @property
    def start_date(self) -> Any: ...
    @property
    def end_date(self) -> Any: ...
    @property
    def image(self) -> Any: ...
    @property
    def icon(self) -> Any: ...
    @property
    def active(self) -> Any: ...
    @property
    def closed(self) -> Any: ...
    @property
    def archived(self) -> Any: ...
    @property
    def featured(self) -> Any: ...
    @property
    def restricted(self) -> Any: ...
    @property
    def liquidity(self) -> Any: ...
    @property
    def open_interest(self) -> Any: ...
    @property
    def volume_24hr(self) -> Any: ...
    @property
    def volume_1wk(self) -> Any: ...
    @property
    def volume_1mo(self) -> Any: ...
    @property
    def markets(self) -> Any: ...
    @property
    def tags(self) -> Any: ...
    @property
    def series(self) -> Any: ...
    @property
    def neg_risk(self) -> Any: ...
    @property
    def neg_risk_market_id(self) -> Any: ...
    @property
    def created_at(self) -> Any: ...
    @property
    def updated_at(self) -> Any: ...
    @property
    def category(self) -> Any: ...
    @property
    def comments_enabled(self) -> Any: ...
    @property
    def competitive(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class SeriesInfo:
    """Summary information about a market series."""
    @property
    def id(self) -> Any: ...
    @property
    def slug(self) -> Any: ...
    @property
    def title(self) -> Any: ...
    @property
    def ticker(self) -> Any: ...
    @property
    def series_type(self) -> Any: ...
    @property
    def recurrence(self) -> Any: ...
    @property
    def image(self) -> Any: ...
    @property
    def icon(self) -> Any: ...
    @property
    def active(self) -> Any: ...
    @property
    def closed(self) -> Any: ...
    @property
    def archived(self) -> Any: ...
    @property
    def volume_24hr(self) -> Any: ...
    @property
    def comment_count(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class SeriesData:
    """Full data for a market series including events."""
    @property
    def id(self) -> Any: ...
    @property
    def slug(self) -> Any: ...
    @property
    def title(self) -> Any: ...
    @property
    def description(self) -> Any: ...
    @property
    def image(self) -> Any: ...
    @property
    def icon(self) -> Any: ...
    @property
    def active(self) -> Any: ...
    @property
    def closed(self) -> Any: ...
    @property
    def archived(self) -> Any: ...
    @property
    def tags(self) -> Any: ...
    @property
    def volume(self) -> Any: ...
    @property
    def liquidity(self) -> Any: ...
    @property
    def events(self) -> Any: ...
    @property
    def competitive(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class RelatedTag:
    """An edge in the tag graph, as returned by the related-tags endpoints.

    This is not a Tag: it names the two tags a relationship connects and its
    ordering rank, and carries no slug or label. Resolve related_tag_id via
    tags.get() to obtain the tag itself.
    """
    @property
    def id(self) -> Any: ...
    @property
    def tag_id(self) -> Any: ...
    @property
    def related_tag_id(self) -> Any: ...
    @property
    def rank(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class Tag:
    """A tag used to categorize markets and events."""
    @property
    def id(self) -> Any: ...
    @property
    def slug(self) -> Any: ...
    @property
    def label(self) -> Any: ...
    @property
    def force_show(self) -> Any: ...
    @property
    def force_hide(self) -> Any: ...
    @property
    def is_carousel(self) -> Any: ...
    @property
    def created_at(self) -> Any: ...
    @property
    def updated_at(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class SportMetadata:
    """Metadata for a sport category."""
    @property
    def id(self) -> Any: ...
    @property
    def sport(self) -> Any: ...
    @property
    def image(self) -> Any: ...
    @property
    def resolution(self) -> Any: ...
    @property
    def ordering(self) -> Any: ...
    @property
    def tags(self) -> Any: ...
    @property
    def series(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class Team:
    """A sports team."""
    @property
    def id(self) -> Any: ...
    @property
    def name(self) -> Any: ...
    @property
    def league(self) -> Any: ...
    @property
    def record(self) -> Any: ...
    @property
    def logo(self) -> Any: ...
    @property
    def abbreviation(self) -> Any: ...
    @property
    def alias(self) -> Any: ...
    @property
    def created_at(self) -> Any: ...
    @property
    def updated_at(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class Comment:
    """A comment on an event, series, or perps asset."""
    @property
    def id(self) -> Any: ...
    @property
    def body(self) -> Any: ...
    @property
    def parent_entity_type(self) -> Any: ...
    @property
    def parent_entity_id(self) -> Any: ...
    @property
    def parent_comment_id(self) -> Any: ...
    @property
    def user_address(self) -> Any: ...
    @property
    def reply_address(self) -> Any: ...
    @property
    def created_at(self) -> Any: ...
    @property
    def updated_at(self) -> Any: ...
    @property
    def profile(self) -> Any: ...
    @property
    def reactions(self) -> Any: ...
    @property
    def report_count(self) -> Any: ...
    @property
    def reaction_count(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class CommentProfile:
    """The author profile embedded in a comment."""
    @property
    def name(self) -> Any: ...
    @property
    def pseudonym(self) -> Any: ...
    @property
    def display_username_public(self) -> Any: ...
    @property
    def bio(self) -> Any: ...
    @property
    def is_mod(self) -> Any: ...
    @property
    def is_creator(self) -> Any: ...
    @property
    def proxy_wallet(self) -> Any: ...
    @property
    def base_address(self) -> Any: ...
    @property
    def profile_image(self) -> Any: ...
    @property
    def profile_image_optimized(self) -> Any: ...
    @property
    def positions(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class CommentReaction:
    """A reaction on a comment."""
    @property
    def id(self) -> Any: ...
    @property
    def comment_id(self) -> Any: ...
    @property
    def reaction_type(self) -> Any: ...
    @property
    def icon(self) -> Any: ...
    @property
    def user_address(self) -> Any: ...
    @property
    def created_at(self) -> Any: ...
    @property
    def profile(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class CommentPosition:
    """A position held by a comment author."""
    @property
    def token_id(self) -> Any: ...
    @property
    def position_size(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class CountResponse:
    """A response containing a count value."""
    @property
    def count(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class Cursor:
    """A pagination cursor."""
    @property
    def next_cursor(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class SearchResponse:
    """Search results containing profiles, events, and tags."""
    @property
    def profiles(self) -> Any: ...
    @property
    def events(self) -> Any: ...
    @property
    def tags(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class SearchProfile:
    """A user profile returned from search."""
    @property
    def name(self) -> Any: ...
    @property
    def display_username_public(self) -> Any: ...
    @property
    def profile_image(self) -> Any: ...
    @property
    def pseudonym(self) -> Any: ...
    @property
    def bio(self) -> Any: ...
    @property
    def proxy_wallet(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class UserResponse:
    """A Polymarket user profile response (`GET /public-profile`)."""
    @property
    def proxy(self) -> Any: ...
    @property
    def name(self) -> Any: ...
    @property
    def created_at(self) -> Any: ...
    @property
    def profile_image(self) -> Any: ...
    @property
    def display_username_public(self) -> Any: ...
    @property
    def bio(self) -> Any: ...
    @property
    def pseudonym(self) -> Any: ...
    @property
    def x_username(self) -> Any: ...
    @property
    def discord_username(self) -> Any: ...
    @property
    def verified_badge(self) -> Any: ...
    @property
    def taker_tier(self) -> Any: ...
    @property
    def taker_tier_name(self) -> Any: ...
    @property
    def weighted_volume(self) -> Any: ...
    @property
    def users(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class UserInfo:
    """User info with role flags."""
    @property
    def id(self) -> Any: ...
    @property
    def creator(self) -> Any: ...
    @property
    def moderator(self) -> Any: ...
    @property
    def community_mod(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

# =============================================================================
# CLOB type classes
# =============================================================================

class ClobMarket:
    """A CLOB order book market."""
    @property
    def condition_id(self) -> Any: ...
    @property
    def question_id(self) -> Any: ...
    @property
    def tokens(self) -> Any: ...
    @property
    def rewards(self) -> Any: ...
    @property
    def minimum_order_size(self) -> Any: ...
    @property
    def minimum_tick_size(self) -> Any: ...
    @property
    def description(self) -> Any: ...
    @property
    def category(self) -> Any: ...
    @property
    def end_date_iso(self) -> Any: ...
    @property
    def question(self) -> Any: ...
    @property
    def active(self) -> Any: ...
    @property
    def closed(self) -> Any: ...
    @property
    def archived(self) -> Any: ...
    @property
    def neg_risk(self) -> Any: ...
    @property
    def neg_risk_market_id(self) -> Any: ...
    @property
    def enable_order_book(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class ClobMarketToken:
    """A token within a CLOB market."""
    @property
    def token_id(self) -> Any: ...
    @property
    def outcome(self) -> Any: ...
    @property
    def price(self) -> Any: ...
    @property
    def winner(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class ListMarketsResponse:
    """Paginated list of CLOB markets."""
    @property
    def data(self) -> Any: ...
    @property
    def next_cursor(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class OrderBook:
    """Order book for a market token."""
    @property
    def market(self) -> Any: ...
    @property
    def asset_id(self) -> Any: ...
    @property
    def bids(self) -> Any: ...
    @property
    def asks(self) -> Any: ...
    @property
    def timestamp(self) -> Any: ...
    @property
    def hash(self) -> Any: ...
    @property
    def min_order_size(self) -> Any: ...
    @property
    def tick_size(self) -> Any: ...
    @property
    def neg_risk(self) -> Any: ...
    @property
    def last_trade_price(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class OrderLevel:
    """A price level in the order book."""
    @property
    def price(self) -> Any: ...
    @property
    def size(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class PriceResponse:
    """A price response for a token."""
    @property
    def price(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class MidpointResponse:
    """The midpoint price for a token."""
    @property
    def mid(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class PriceHistoryPoint:
    """A single point in a price history series."""
    @property
    def timestamp(self) -> Any: ...
    @property
    def price(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class PricesHistoryResponse:
    """Price history for a token."""
    @property
    def history(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class NegRiskResponse:
    """Whether a token uses negative risk."""
    @property
    def neg_risk(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class FeeRateResponse:
    """Fee rate for a token."""
    @property
    def base_fee(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class TickSizeResponse:
    """Minimum tick size for a token."""
    @property
    def minimum_tick_size(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class SpreadResponse:
    """Bid-ask spread for a token."""
    @property
    def token_id(self) -> Any: ...
    @property
    def spread(self) -> Any: ...
    @property
    def bid(self) -> Any: ...
    @property
    def ask(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class LastTradePriceResponse:
    """Last trade price for a token."""
    @property
    def token_id(self) -> Any: ...
    @property
    def last_trade_price(self) -> Any: ...
    @property
    def timestamp(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class CalculatePriceResponse:
    """Calculated price for a given amount."""
    @property
    def price(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class ServerTimeResponse:
    """Server time from the CLOB API."""
    @property
    def time(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

# =============================================================================
# Data type classes
# =============================================================================

class Page(Generic[T]):
    """One page of a paged route. Follow `pagination.next_cursor` until it is None."""
    @property
    def data(self) -> list[T]: ...
    @property
    def pagination(self) -> Pagination: ...
    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...

class PageIterator(Generic[T]):
    """Async iterator over every page of a route, from `DataApi.iter_*`."""
    def __aiter__(self) -> PageIterator[T]: ...
    def __anext__(self) -> Coroutine[Any, Any, Page[T]]: ...

class PageIteratorSync(Generic[T]):
    """Iterator over every page of a route, from `DataApiSync.iter_*`."""
    def __iter__(self) -> PageIteratorSync[T]: ...
    def __next__(self) -> Page[T]: ...

class Approvals:
    """Non-paginated approval snapshot for one Polygon proxy wallet."""
    @property
    def address(self) -> str: ...
    @property
    def chain_id(self) -> int: ...
    @property
    def checked_at(self) -> str: ...
    @property
    def contracts(self) -> list[dict[str, Any]]: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class Position:
    """One position (`/v2/positions`)."""
    @property
    def archived(self) -> bool: ...
    @property
    def avg_price(self) -> float: ...
    @property
    def condition_id(self) -> str: ...
    @property
    def current_price(self) -> float: ...
    @property
    def current_size(self) -> float: ...
    @property
    def current_value(self) -> float: ...
    @property
    def end_date(self) -> str: ...
    @property
    def entry_cost_usdc(self) -> float: ...
    @property
    def entry_fees_usdc(self) -> float: ...
    @property
    def event_id(self) -> str: ...
    @property
    def event_slug(self) -> str: ...
    @property
    def first_entry_at(self) -> int: ...
    @property
    def icon(self) -> str: ...
    @property
    def last_event_at(self) -> int: ...
    @property
    def mergeable(self) -> bool: ...
    @property
    def name(self) -> str: ...
    @property
    def negative_risk(self) -> bool: ...
    @property
    def opposite_outcome(self) -> str: ...
    @property
    def opposite_token_id(self) -> str: ...
    @property
    def outcome(self) -> str: ...
    @property
    def outcome_index(self) -> int: ...
    @property
    def percent_pnl(self) -> float: ...
    @property
    def percent_realized_pnl(self) -> float: ...
    @property
    def profile_image(self) -> str: ...
    @property
    def proxy_wallet(self) -> str: ...
    @property
    def realized_pnl(self) -> float: ...
    @property
    def redeemable(self) -> bool: ...
    @property
    def slug(self) -> str: ...
    @property
    def status(self) -> str: ...
    @property
    def title(self) -> str: ...
    @property
    def token_id(self) -> str: ...
    @property
    def total_cost_usdc(self) -> float: ...
    @property
    def total_pnl(self) -> float: ...
    @property
    def total_size(self) -> float: ...
    @property
    def unrealized_pnl(self) -> float: ...
    @property
    def verified(self) -> bool: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class ComboPosition:
    """One combo position (`/v2/positions/combos`)."""
    @property
    def combo_condition_id(self) -> str: ...
    @property
    def combo_position_id(self) -> str: ...
    @property
    def current_size(self) -> float: ...
    @property
    def entry_avg_price_usdc(self) -> float: ...
    @property
    def entry_cost_usdc(self) -> float: ...
    @property
    def entry_fees_usdc(self) -> float: ...
    @property
    def first_entry_at(self) -> str: ...
    @property
    def first_entry_at_micros(self) -> int | None: ...
    @property
    def gross_entry_cost_usdc(self) -> float: ...
    @property
    def legs(self) -> list[dict[str, Any]]: ...
    @property
    def legs_pending(self) -> int: ...
    @property
    def legs_resolved(self) -> int: ...
    @property
    def legs_total(self) -> int: ...
    @property
    def outcome_index(self) -> int: ...
    @property
    def outcome_label(self) -> str: ...
    @property
    def proxy_wallet(self) -> str: ...
    @property
    def realized_payout_usdc(self) -> float: ...
    @property
    def redeemable(self) -> bool: ...
    @property
    def resolved_at(self) -> str | None: ...
    @property
    def status(self) -> str: ...
    @property
    def updated_at(self) -> str: ...
    @property
    def updated_at_micros(self) -> int: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class UserPnlSeries:
    """Complete v2 user-PnL response data."""
    @property
    def fidelity(self) -> str: ...
    @property
    def interval(self) -> str: ...
    @property
    def points(self) -> list[dict[str, Any]]: ...
    @property
    def proxy_wallet(self) -> str: ...
    @property
    def source_fidelity(self) -> str: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class UserStats:
    """The profile card for one wallet, plus its newest persisted all-time PnL observation."""
    @property
    def all_time_pnl(self) -> dict[str, Any] | None: ...
    @property
    def biggest_win(self) -> float: ...
    @property
    def join_date(self) -> int | None: ...
    @property
    def proxy_wallet(self) -> str: ...
    @property
    def trades(self) -> int: ...
    @property
    def views(self) -> int: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class UserVolume:
    """`/v2/user-volume`."""
    @property
    def trade_count(self) -> int: ...
    @property
    def volume(self) -> float: ...
    @property
    def volume_usdc(self) -> float: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class PortfolioValue:
    """`/v2/value`."""
    @property
    def proxy_wallet(self) -> str: ...
    @property
    def value(self) -> float: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class Activity:
    """One activity-feed event (`/v2/activity`)."""
    @property
    def activity_type(self) -> str: ...
    @property
    def bio(self) -> str: ...
    @property
    def condition_id(self) -> str: ...
    @property
    def event_slug(self) -> str: ...
    @property
    def icon(self) -> str: ...
    @property
    def is_combo(self) -> bool | None: ...
    @property
    def name(self) -> str: ...
    @property
    def outcome(self) -> str: ...
    @property
    def outcome_index(self) -> int: ...
    @property
    def price(self) -> float: ...
    @property
    def profile_image(self) -> str: ...
    @property
    def profile_image_optimized(self) -> str: ...
    @property
    def proxy_wallet(self) -> str: ...
    @property
    def pseudonym(self) -> str: ...
    @property
    def side(self) -> str: ...
    @property
    def size(self) -> float: ...
    @property
    def slug(self) -> str: ...
    @property
    def timestamp(self) -> int: ...
    @property
    def title(self) -> str: ...
    @property
    def token_id(self) -> str: ...
    @property
    def transaction_hash(self) -> str: ...
    @property
    def usdc_size(self) -> float: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class ComboActivity:
    """One combo lifecycle/redemption event (`/v2/activity/combos`)."""
    @property
    def amount_usdc(self) -> float | None: ...
    @property
    def block_number(self) -> int: ...
    @property
    def combo_activity_type(self) -> str: ...
    @property
    def combo_condition_id(self) -> str: ...
    @property
    def combo_position_id(self) -> str: ...
    @property
    def id(self) -> str: ...
    @property
    def legs(self) -> list[dict[str, Any]]: ...
    @property
    def payout_usdc(self) -> float | None: ...
    @property
    def proxy_wallet(self) -> str: ...
    @property
    def timestamp(self) -> int: ...
    @property
    def transaction_hash(self) -> str: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class Trade:
    """A trade (`/v2/trades`)."""
    @property
    def bio(self) -> str: ...
    @property
    def condition_id(self) -> str: ...
    @property
    def event_slug(self) -> str: ...
    @property
    def icon(self) -> str: ...
    @property
    def name(self) -> str: ...
    @property
    def outcome(self) -> str: ...
    @property
    def outcome_index(self) -> int: ...
    @property
    def price(self) -> float: ...
    @property
    def profile_image(self) -> str: ...
    @property
    def profile_image_optimized(self) -> str: ...
    @property
    def proxy_wallet(self) -> str: ...
    @property
    def pseudonym(self) -> str: ...
    @property
    def side(self) -> str: ...
    @property
    def size(self) -> float: ...
    @property
    def slug(self) -> str: ...
    @property
    def timestamp(self) -> int: ...
    @property
    def title(self) -> str: ...
    @property
    def token_id(self) -> str: ...
    @property
    def transaction_hash(self) -> str: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class MetaHolder:
    """One outcome token's holder group in `/v2/holders`."""
    @property
    def holders(self) -> list[dict[str, Any]]: ...
    @property
    def token_id(self) -> str: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class LiveVolume:
    """`/v2/live-volume`."""
    @property
    def conditions(self) -> list[dict[str, Any]]: ...
    @property
    def taker_volume_total(self) -> float: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class OpenInterest:
    """One `/v2/oi` row."""
    @property
    def condition_id(self) -> str: ...
    @property
    def value(self) -> float: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class PricePoint:
    """One served point."""
    @property
    def price(self) -> float: ...
    @property
    def resolution_seconds(self) -> int: ...
    @property
    def timestamp(self) -> int: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class Resolution:
    """One non-paginated `/v2/resolutions` row."""
    @property
    def condition_id(self) -> str | None: ...
    @property
    def expected_settlement_time(self) -> str | None: ...
    @property
    def extended_review(self) -> bool: ...
    @property
    def last_update_timestamp(self) -> str: ...
    @property
    def log_index(self) -> str: ...
    @property
    def market_type(self) -> str | None: ...
    @property
    def new_version_q(self) -> bool: ...
    @property
    def payouts(self) -> list[int] | None: ...
    @property
    def price(self) -> str | None: ...
    @property
    def proposed_price(self) -> str | None: ...
    @property
    def question_id(self) -> str | None: ...
    @property
    def reporter(self) -> str | None: ...
    @property
    def reproposed_price(self) -> str | None: ...
    @property
    def resolution_source(self) -> str | None: ...
    @property
    def resolved_at(self) -> str | None: ...
    @property
    def resolved_block(self) -> int | None: ...
    @property
    def settlement_time_basis(self) -> str | None: ...
    @property
    def status(self) -> str: ...
    @property
    def transaction_hash(self) -> str: ...
    @property
    def was_arbitrated(self) -> bool | None: ...
    @property
    def was_disputed(self) -> bool: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class TokenRef:
    """One `/v2/tokens` row: the reference facts for one ERC-1155 token id."""
    @property
    def clob_index(self) -> int | None: ...
    @property
    def closed(self) -> bool | None: ...
    @property
    def condition_id(self) -> str: ...
    @property
    def event_id(self) -> int | None: ...
    @property
    def event_slug(self) -> str | None: ...
    @property
    def final_price(self) -> float | None: ...
    @property
    def market_slug(self) -> str | None: ...
    @property
    def module(self) -> str: ...
    @property
    def neg_risk(self) -> bool | None: ...
    @property
    def neg_risk_market_id(self) -> str | None: ...
    @property
    def opposite_token_id(self) -> str | None: ...
    @property
    def outcome(self) -> str | None: ...
    @property
    def outcome_index(self) -> int: ...
    @property
    def question_index(self) -> int | None: ...
    @property
    def resolved(self) -> bool: ...
    @property
    def structural_condition_id(self) -> str | None: ...
    @property
    def title(self) -> str | None: ...
    @property
    def token_id(self) -> str: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class BiggestWinner:
    """One `/v2/biggest-winners` row."""
    @property
    def condition_id(self) -> str: ...
    @property
    def event_id(self) -> int: ...
    @property
    def event_slug(self) -> str: ...
    @property
    def event_title(self) -> str: ...
    @property
    def final_value(self) -> float: ...
    @property
    def initial_value(self) -> float: ...
    @property
    def kind(self) -> str: ...
    @property
    def pnl(self) -> float: ...
    @property
    def position_id(self) -> str: ...
    @property
    def profile_image(self) -> str: ...
    @property
    def resolved_at(self) -> int: ...
    @property
    def user_id(self) -> str: ...
    @property
    def user_name(self) -> str: ...
    @property
    def win_rank(self) -> int: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class BuilderStanding:
    """One `/v2/builders/leaderboard` row."""
    @property
    def active_users(self) -> int: ...
    @property
    def builder_code(self) -> str: ...
    @property
    def builder_name(self) -> str: ...
    @property
    def profile_image(self) -> str: ...
    @property
    def rank(self) -> int: ...
    @property
    def verified(self) -> bool: ...
    @property
    def volume(self) -> float: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class BuilderVolumePoint:
    """One `/v2/builders/volume` row."""
    @property
    def active_users(self) -> int: ...
    @property
    def builder_code(self) -> str: ...
    @property
    def builder_name(self) -> str: ...
    @property
    def date(self) -> str: ...
    @property
    def profile_image(self) -> str: ...
    @property
    def rank(self) -> int: ...
    @property
    def verified(self) -> bool: ...
    @property
    def volume(self) -> float: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class LeaderboardEntry:
    """One `/v2/leaderboard` row."""
    @property
    def pnl(self) -> float: ...
    @property
    def profile_image(self) -> str: ...
    @property
    def rank(self) -> int: ...
    @property
    def user_id(self) -> str: ...
    @property
    def user_name(self) -> str: ...
    @property
    def verified(self) -> bool: ...
    @property
    def volume(self) -> float: ...
    @property
    def x_username(self) -> str: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class LeaderboardUserEntry:
    """`/v2/leaderboard?user=`."""
    @property
    def pnl(self) -> float: ...
    @property
    def profile_image(self) -> str: ...
    @property
    def rank_pnl(self) -> int | None: ...
    @property
    def rank_volume(self) -> int | None: ...
    @property
    def user_id(self) -> str: ...
    @property
    def user_name(self) -> str: ...
    @property
    def verified(self) -> bool: ...
    @property
    def volume(self) -> float: ...
    @property
    def x_username(self) -> str: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class ServiceStatus:
    """`/v2/status` payload."""
    @property
    def age_seconds(self) -> int: ...
    @property
    def computed_at(self) -> str: ...
    @property
    def ingestion(self) -> dict[str, Any]: ...
    @property
    def serving(self) -> dict[str, Any]: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class Pagination:
    """Paging state returned with every page."""
    @property
    def has_more(self) -> bool: ...
    @property
    def limit(self) -> int: ...
    @property
    def next_cursor(self) -> str | None: ...
    @property
    def offset(self) -> int: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

# =============================================================================
# Gamma namespace classes
# =============================================================================

class GammaMarkets:
    """Async namespace for Gamma market endpoints."""
    def get(
        self, id: str, *, include_tag: bool | None = None
    ) -> Coroutine[Any, Any, Market]:
        """Get a market by ID."""
        ...
    def get_by_slug(
        self, slug: str, *, include_tag: bool | None = None
    ) -> Coroutine[Any, Any, Market]:
        """Get a market by slug."""
        ...
    def list(
        self,
        *,
        limit: int | None = None,
        offset: int | None = None,
        order: str | None = None,
        ascending: bool | None = None,
        id: list[int] | None = None,
        slug: list[str] | None = None,
        clob_token_ids: list[str] | None = None,
        condition_ids: list[str] | None = None,
        liquidity_num_min: float | None = None,
        liquidity_num_max: float | None = None,
        volume_num_min: float | None = None,
        volume_num_max: float | None = None,
        tag_id: int | None = None,
        related_tags: bool | None = None,
        closed: bool | None = None,
        open: bool | None = None,
        archived: bool | None = None,
    ) -> Coroutine[Any, Any, list[Market]]:
        """List markets with optional filters."""
        ...
    def tags(self, id: str) -> Coroutine[Any, Any, list[Tag]]:
        """Get tags for a market."""
        ...

class GammaMarketsSync:
    """Sync namespace for Gamma market endpoints."""
    def get(self, id: str, *, include_tag: bool | None = None) -> Market:
        """Get a market by ID."""
        ...
    def get_by_slug(self, slug: str, *, include_tag: bool | None = None) -> Market:
        """Get a market by slug."""
        ...
    def list(
        self,
        *,
        limit: int | None = None,
        offset: int | None = None,
        order: str | None = None,
        ascending: bool | None = None,
        id: list[int] | None = None,
        slug: list[str] | None = None,
        clob_token_ids: list[str] | None = None,
        condition_ids: list[str] | None = None,
        liquidity_num_min: float | None = None,
        liquidity_num_max: float | None = None,
        volume_num_min: float | None = None,
        volume_num_max: float | None = None,
        tag_id: int | None = None,
        related_tags: bool | None = None,
        closed: bool | None = None,
        open: bool | None = None,
        archived: bool | None = None,
    ) -> list[Market]:
        """List markets with optional filters."""
        ...
    def tags(self, id: str) -> list[Tag]:
        """Get tags for a market."""
        ...

class GammaEvents:
    """Async namespace for Gamma event endpoints."""
    def list(
        self,
        *,
        limit: int | None = None,
        offset: int | None = None,
        order: str | None = None,
        ascending: bool | None = None,
        id: list[int] | None = None,
        slug: list[str] | None = None,
        tag_id: int | None = None,
        tag_slug: str | None = None,
        active: bool | None = None,
        archived: bool | None = None,
        featured: bool | None = None,
        closed: bool | None = None,
        liquidity_min: float | None = None,
        liquidity_max: float | None = None,
        volume_min: float | None = None,
        volume_max: float | None = None,
    ) -> Coroutine[Any, Any, list[Event]]:
        """List events with optional filters."""
        ...
    def get(
        self,
        id: str,
        *,
        include_chat: bool | None = None,
        include_template: bool | None = None,
    ) -> Coroutine[Any, Any, Event]:
        """Get an event by ID."""
        ...
    def get_by_slug(
        self,
        slug: str,
        *,
        include_chat: bool | None = None,
        include_template: bool | None = None,
    ) -> Coroutine[Any, Any, Event]:
        """Get an event by slug."""
        ...
    def tags(self, id: str) -> Coroutine[Any, Any, list[Tag]]:
        """Get tags for an event."""
        ...
    def tweet_count(self, id: str) -> Coroutine[Any, Any, CountResponse]:
        """Get tweet count for an event."""
        ...
    def comment_count(self, id: str) -> Coroutine[Any, Any, CountResponse]:
        """Get comment count for an event."""
        ...

class GammaEventsSync:
    """Sync namespace for Gamma event endpoints."""
    def list(
        self,
        *,
        limit: int | None = None,
        offset: int | None = None,
        order: str | None = None,
        ascending: bool | None = None,
        id: list[int] | None = None,
        slug: list[str] | None = None,
        tag_id: int | None = None,
        tag_slug: str | None = None,
        active: bool | None = None,
        archived: bool | None = None,
        featured: bool | None = None,
        closed: bool | None = None,
        liquidity_min: float | None = None,
        liquidity_max: float | None = None,
        volume_min: float | None = None,
        volume_max: float | None = None,
    ) -> list[Event]:
        """List events with optional filters."""
        ...
    def get(
        self,
        id: str,
        *,
        include_chat: bool | None = None,
        include_template: bool | None = None,
    ) -> Event:
        """Get an event by ID."""
        ...
    def get_by_slug(
        self,
        slug: str,
        *,
        include_chat: bool | None = None,
        include_template: bool | None = None,
    ) -> Event:
        """Get an event by slug."""
        ...
    def tags(self, id: str) -> list[Tag]:
        """Get tags for an event."""
        ...
    def tweet_count(self, id: str) -> CountResponse:
        """Get tweet count for an event."""
        ...
    def comment_count(self, id: str) -> CountResponse:
        """Get comment count for an event."""
        ...

class GammaSeries:
    """Async namespace for Gamma series endpoints."""
    def list(
        self,
        *,
        limit: int | None = None,
        offset: int | None = None,
        ascending: bool | None = None,
        closed: bool | None = None,
        slug: list[str] | None = None,
        include_chat: bool | None = None,
        recurrence: str | None = None,
    ) -> Coroutine[Any, Any, list[SeriesData]]:
        """List series with optional filters."""
        ...
    def get(
        self, id: str, *, include_chat: bool | None = None
    ) -> Coroutine[Any, Any, SeriesData]:
        """Get a series by ID."""
        ...

class GammaSeriesSync:
    """Sync namespace for Gamma series endpoints."""
    def list(
        self,
        *,
        limit: int | None = None,
        offset: int | None = None,
        ascending: bool | None = None,
        closed: bool | None = None,
        slug: list[str] | None = None,
        include_chat: bool | None = None,
        recurrence: str | None = None,
    ) -> list[SeriesData]:
        """List series with optional filters."""
        ...
    def get(self, id: str, *, include_chat: bool | None = None) -> SeriesData:
        """Get a series by ID."""
        ...

class GammaTags:
    """Async namespace for Gamma tag endpoints."""
    def list(
        self,
        *,
        limit: int | None = None,
        offset: int | None = None,
        order: str | None = None,
        ascending: bool | None = None,
        include_template: bool | None = None,
        is_carousel: bool | None = None,
    ) -> Coroutine[Any, Any, list[Tag]]:
        """List tags with optional filters."""
        ...
    def get(self, id: str) -> Coroutine[Any, Any, Tag]:
        """Get a tag by ID."""
        ...
    def get_by_slug(self, slug: str) -> Coroutine[Any, Any, Tag]:
        """Get a tag by slug."""
        ...
    def get_related(
        self,
        id: str,
        *,
        omit_empty: bool | None = None,
        status: str | None = None,
    ) -> Coroutine[Any, Any, list[RelatedTag]]:
        """Get the tag-relationship rows for a tag, by ID.

        Returns relationship edges, not tags. Use get_related_detailed for tags.
        """
        ...
    def get_related_by_slug(
        self,
        slug: str,
        *,
        omit_empty: bool | None = None,
        status: str | None = None,
    ) -> Coroutine[Any, Any, list[RelatedTag]]:
        """Get the tag-relationship rows for a tag, by slug.

        Returns relationship edges, not tags. Use get_related_detailed_by_slug
        for tags.
        """
        ...

class GammaTagsSync:
    """Sync namespace for Gamma tag endpoints."""
    def list(
        self,
        *,
        limit: int | None = None,
        offset: int | None = None,
        order: str | None = None,
        ascending: bool | None = None,
        include_template: bool | None = None,
        is_carousel: bool | None = None,
    ) -> list[Tag]:
        """List tags with optional filters."""
        ...
    def get(self, id: str) -> Tag:
        """Get a tag by ID."""
        ...
    def get_by_slug(self, slug: str) -> Tag:
        """Get a tag by slug."""
        ...
    def get_related(
        self,
        id: str,
        *,
        omit_empty: bool | None = None,
        status: str | None = None,
    ) -> list[RelatedTag]:
        """Get the tag-relationship rows for a tag, by ID.

        Returns relationship edges, not tags. Use get_related_detailed for tags.
        """
        ...
    def get_related_by_slug(
        self,
        slug: str,
        *,
        omit_empty: bool | None = None,
        status: str | None = None,
    ) -> list[RelatedTag]:
        """Get the tag-relationship rows for a tag, by slug.

        Returns relationship edges, not tags. Use get_related_detailed_by_slug
        for tags.
        """
        ...

class GammaComments:
    """Async namespace for Gamma comment endpoints."""
    def list(
        self,
        *,
        limit: int | None = None,
        offset: int | None = None,
        order: str | None = None,
        ascending: bool | None = None,
        parent_entity_type: str | None = None,
        parent_entity_id: int | None = None,
        get_positions: bool | None = None,
        holders_only: bool | None = None,
    ) -> Coroutine[Any, Any, list[Comment]]:
        """List comments with optional filters."""
        ...
    def get(self, id: str) -> Coroutine[Any, Any, list[Comment]]:
        """Get the comment thread containing `id` (root comment plus every reply)."""
        ...
    def by_user(self, address: str) -> Coroutine[Any, Any, list[Comment]]:
        """Get comments by a user address."""
        ...

class GammaCommentsSync:
    """Sync namespace for Gamma comment endpoints."""
    def list(
        self,
        *,
        limit: int | None = None,
        offset: int | None = None,
        order: str | None = None,
        ascending: bool | None = None,
        parent_entity_type: str | None = None,
        parent_entity_id: int | None = None,
        get_positions: bool | None = None,
        holders_only: bool | None = None,
    ) -> list[Comment]:
        """List comments with optional filters."""
        ...
    def get(self, id: str) -> list[Comment]:
        """Get the comment thread containing `id` (root comment plus every reply)."""
        ...
    def by_user(self, address: str) -> list[Comment]:
        """Get comments by a user address."""
        ...

class GammaSports:
    """Async namespace for Gamma sports endpoints."""
    def list(self) -> Coroutine[Any, Any, list[SportMetadata]]:
        """List all sports."""
        ...
    def market_types(self) -> Coroutine[Any, Any, Any]:
        """Get available sport market types."""
        ...
    def list_teams(
        self,
        *,
        limit: int | None = None,
        offset: int | None = None,
        order: str | None = None,
        ascending: bool | None = None,
        league: list[str] | None = None,
        name: list[str] | None = None,
        abbreviation: list[str] | None = None,
    ) -> Coroutine[Any, Any, list[Team]]:
        """List teams with optional filters."""
        ...

class GammaSportsSync:
    """Sync namespace for Gamma sports endpoints."""
    def list(self) -> list[SportMetadata]:
        """List all sports."""
        ...
    def market_types(self) -> Any:
        """Get available sport market types."""
        ...
    def list_teams(
        self,
        *,
        limit: int | None = None,
        offset: int | None = None,
        order: str | None = None,
        ascending: bool | None = None,
        league: list[str] | None = None,
        name: list[str] | None = None,
        abbreviation: list[str] | None = None,
    ) -> list[Team]:
        """List teams with optional filters."""
        ...

class GammaSearch:
    """Async namespace for Gamma search endpoints."""
    def public_search(
        self,
        query: str,
        *,
        limit_per_type: int | None = None,
        page: int | None = None,
        cache: bool | None = None,
        events_status: str | None = None,
        events_tag: list[int] | None = None,
        keep_closed_markets: int | None = None,
        sort: str | None = None,
        search_tags: bool | None = None,
        search_profiles: bool | None = None,
    ) -> Coroutine[Any, Any, SearchResponse]:
        """Search markets, events, and profiles."""
        ...

class GammaSearchSync:
    """Sync namespace for Gamma search endpoints."""
    def public_search(
        self,
        query: str,
        *,
        limit_per_type: int | None = None,
        page: int | None = None,
        cache: bool | None = None,
        events_status: str | None = None,
        events_tag: list[int] | None = None,
        keep_closed_markets: int | None = None,
        sort: str | None = None,
        search_tags: bool | None = None,
        search_profiles: bool | None = None,
    ) -> SearchResponse:
        """Search markets, events, and profiles."""
        ...

class GammaUser:
    """Async namespace for Gamma user endpoints."""
    def get(self, address: str) -> Coroutine[Any, Any, UserResponse]:
        """Get a user by address."""
        ...

class GammaUserSync:
    """Sync namespace for Gamma user endpoints."""
    def get(self, address: str) -> UserResponse:
        """Get a user by address."""
        ...

class GammaHealth:
    """Async namespace for Gamma health endpoints."""
    def ping(self) -> Coroutine[Any, Any, float]:
        """Ping the Gamma API and return latency in seconds."""
        ...

class GammaHealthSync:
    """Sync namespace for Gamma health endpoints."""
    def ping(self) -> float:
        """Ping the Gamma API and return latency in seconds."""
        ...

# =============================================================================
# CLOB namespace classes
# =============================================================================

class ClobMarkets:
    """Async namespace for CLOB market endpoints."""
    def get(self, condition_id: str) -> Coroutine[Any, Any, ClobMarket]:
        """Get a CLOB market by condition ID."""
        ...
    def get_by_token_ids(
        self, token_ids: list[str]
    ) -> Coroutine[Any, Any, ListMarketsResponse]:
        """Get markets by token IDs."""
        ...
    def list(self) -> Coroutine[Any, Any, ListMarketsResponse]:
        """List all CLOB markets."""
        ...
    def order_book(self, token_id: str) -> Coroutine[Any, Any, OrderBook]:
        """Get the order book for a token."""
        ...
    def price(
        self, token_id: str, side: str
    ) -> Coroutine[Any, Any, PriceResponse]:
        """Get the price for a token and side."""
        ...
    def midpoint(
        self, token_id: str
    ) -> Coroutine[Any, Any, MidpointResponse]:
        """Get the midpoint price for a token."""
        ...
    def prices_history(
        self, token_id: str
    ) -> Coroutine[Any, Any, PricesHistoryResponse]:
        """Get the price history for a token."""
        ...
    def neg_risk(
        self, token_id: str
    ) -> Coroutine[Any, Any, NegRiskResponse]:
        """Check if a token uses negative risk."""
        ...
    def fee_rate(
        self, token_id: str
    ) -> Coroutine[Any, Any, FeeRateResponse]:
        """Get the fee rate for a token."""
        ...
    def tick_size(
        self, token_id: str
    ) -> Coroutine[Any, Any, TickSizeResponse]:
        """Get the minimum tick size for a token."""
        ...
    def spread(
        self, token_id: str
    ) -> Coroutine[Any, Any, SpreadResponse]:
        """Get the bid-ask spread for a token."""
        ...
    def last_trade_price(
        self, token_id: str
    ) -> Coroutine[Any, Any, LastTradePriceResponse]:
        """Get the last trade price for a token."""
        ...
    def simplified(self) -> Coroutine[Any, Any, ListMarketsResponse]:
        """List markets in simplified format."""
        ...
    def sampling(self) -> Coroutine[Any, Any, ListMarketsResponse]:
        """List sampling markets."""
        ...
    def sampling_simplified(self) -> Coroutine[Any, Any, ListMarketsResponse]:
        """List sampling markets in simplified format."""
        ...
    def calculate_price(
        self, token_id: str, side: str, amount: str
    ) -> Coroutine[Any, Any, CalculatePriceResponse]:
        """Calculate the price for a given amount."""
        ...

class ClobMarketsSync:
    """Sync namespace for CLOB market endpoints."""
    def get(self, condition_id: str) -> ClobMarket:
        """Get a CLOB market by condition ID."""
        ...
    def get_by_token_ids(self, token_ids: list[str]) -> ListMarketsResponse:
        """Get markets by token IDs."""
        ...
    def list(self) -> ListMarketsResponse:
        """List all CLOB markets."""
        ...
    def order_book(self, token_id: str) -> OrderBook:
        """Get the order book for a token."""
        ...
    def price(self, token_id: str, side: str) -> PriceResponse:
        """Get the price for a token and side."""
        ...
    def midpoint(self, token_id: str) -> MidpointResponse:
        """Get the midpoint price for a token."""
        ...
    def prices_history(self, token_id: str) -> PricesHistoryResponse:
        """Get the price history for a token."""
        ...
    def neg_risk(self, token_id: str) -> NegRiskResponse:
        """Check if a token uses negative risk."""
        ...
    def fee_rate(self, token_id: str) -> FeeRateResponse:
        """Get the fee rate for a token."""
        ...
    def tick_size(self, token_id: str) -> TickSizeResponse:
        """Get the minimum tick size for a token."""
        ...
    def spread(self, token_id: str) -> SpreadResponse:
        """Get the bid-ask spread for a token."""
        ...
    def last_trade_price(self, token_id: str) -> LastTradePriceResponse:
        """Get the last trade price for a token."""
        ...
    def simplified(self) -> ListMarketsResponse:
        """List markets in simplified format."""
        ...
    def sampling(self) -> ListMarketsResponse:
        """List sampling markets."""
        ...
    def sampling_simplified(self) -> ListMarketsResponse:
        """List sampling markets in simplified format."""
        ...
    def calculate_price(
        self, token_id: str, side: str, amount: str
    ) -> CalculatePriceResponse:
        """Calculate the price for a given amount."""
        ...

class ClobHealth:
    """Async namespace for CLOB health endpoints."""
    def ping(self) -> Coroutine[Any, Any, float]:
        """Ping the CLOB API and return latency in seconds."""
        ...
    def server_time(self) -> Coroutine[Any, Any, ServerTimeResponse]:
        """Get the server time."""
        ...

class ClobHealthSync:
    """Sync namespace for CLOB health endpoints."""
    def ping(self) -> float:
        """Ping the CLOB API and return latency in seconds."""
        ...
    def server_time(self) -> ServerTimeResponse:
        """Get the server time."""
        ...

# =============================================================================
# Gamma client classes
# =============================================================================

class Gamma:
    """Async client for the Gamma market data API."""
    def __init__(
        self,
        *,
        base_url: str | None = None,
        timeout_ms: int | None = None,
        pool_size: int | None = None,
    ) -> None: ...
    def markets(self) -> GammaMarkets:
        """Access market endpoints."""
        ...
    def events(self) -> GammaEvents:
        """Access event endpoints."""
        ...
    def series(self) -> GammaSeries:
        """Access series endpoints."""
        ...
    def tags(self) -> GammaTags:
        """Access tag endpoints."""
        ...
    def comments(self) -> GammaComments:
        """Access comment endpoints."""
        ...
    def sports(self) -> GammaSports:
        """Access sports endpoints."""
        ...
    def search(self) -> GammaSearch:
        """Access search endpoints."""
        ...
    def user(self) -> GammaUser:
        """Access user endpoints."""
        ...
    def health(self) -> GammaHealth:
        """Access health endpoints."""
        ...

class GammaSync:
    """Sync client for the Gamma market data API."""
    def __init__(
        self,
        *,
        base_url: str | None = None,
        timeout_ms: int | None = None,
        pool_size: int | None = None,
    ) -> None: ...
    def markets(self) -> GammaMarketsSync:
        """Access market endpoints."""
        ...
    def events(self) -> GammaEventsSync:
        """Access event endpoints."""
        ...
    def series(self) -> GammaSeriesSync:
        """Access series endpoints."""
        ...
    def tags(self) -> GammaTagsSync:
        """Access tag endpoints."""
        ...
    def comments(self) -> GammaCommentsSync:
        """Access comment endpoints."""
        ...
    def sports(self) -> GammaSportsSync:
        """Access sports endpoints."""
        ...
    def search(self) -> GammaSearchSync:
        """Access search endpoints."""
        ...
    def user(self) -> GammaUserSync:
        """Access user endpoints."""
        ...
    def health(self) -> GammaHealthSync:
        """Access health endpoints."""
        ...

# =============================================================================
# CLOB client classes
# =============================================================================

class ClobClient:
    """Async client for the CLOB trading API."""
    def __init__(self) -> None: ...
    def markets(self) -> ClobMarkets:
        """Access market endpoints."""
        ...
    def health(self) -> ClobHealth:
        """Access health endpoints."""
        ...

class ClobClientSync:
    """Sync client for the CLOB trading API."""
    def __init__(self) -> None: ...
    def markets(self) -> ClobMarketsSync:
        """Access market endpoints."""
        ...
    def health(self) -> ClobHealthSync:
        """Access health endpoints."""
        ...

# =============================================================================
# Data client classes
# =============================================================================

class DataApi:
    """Async client for the Polymarket Data API; each method is a route."""
    def __init__(
        self,
        *,
        base_url: str | None = None,
        timeout_ms: int | None = None,
        pool_size: int | None = None,
    ) -> None: ...
    def approvals(
        self,
        user: str,
    ) -> Coroutine[Any, Any, Approvals]:
        """A wallet's Polygon token and operator approvals."""
        ...
    def positions(
        self,
        *,
        user: str | None = None,
        conditions: list[str] | None = None,
        status: str | None = None,
        event_ids: list[str] | None = None,
        title: str | None = None,
        filter_type: str | None = None,
        filter_amount: float | None = None,
        include_archived: bool | None = None,
        sort_by: str | None = None,
        sort_direction: str | None = None,
        start: int | None = None,
        end: int | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Coroutine[Any, Any, Page[Position]]:
        """Positions for a wallet (`user`), one market (`conditions` of one id), or a wallet in some markets (both)."""
        ...
    def iter_positions(
        self,
        *,
        user: str | None = None,
        conditions: list[str] | None = None,
        status: str | None = None,
        event_ids: list[str] | None = None,
        title: str | None = None,
        filter_type: str | None = None,
        filter_amount: float | None = None,
        include_archived: bool | None = None,
        sort_by: str | None = None,
        sort_direction: str | None = None,
        start: int | None = None,
        end: int | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIterator[Position]:
        """Walk every page of `positions`, re-sending these filters on each one."""
        ...
    def combo_positions(
        self,
        user: str,
        *,
        conditions: list[str] | None = None,
        statuses: list[str] | None = None,
        sort_by: str | None = None,
        sort_direction: str | None = None,
        updated_after: int | None = None,
        updated_before: int | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Coroutine[Any, Any, Page[ComboPosition]]:
        """A wallet's combo positions."""
        ...
    def iter_combo_positions(
        self,
        user: str,
        *,
        conditions: list[str] | None = None,
        statuses: list[str] | None = None,
        sort_by: str | None = None,
        sort_direction: str | None = None,
        updated_after: int | None = None,
        updated_before: int | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIterator[ComboPosition]:
        """Walk every page of `combo_positions`, re-sending these filters on each one."""
        ...
    def user_pnl(
        self,
        user: str,
        *,
        interval: str | None = None,
        fidelity: str | None = None,
    ) -> Coroutine[Any, Any, UserPnlSeries]:
        """A wallet's cumulative PnL series. Not the same series as `data.pnl()`."""
        ...
    def user_stats(
        self,
        user: str,
    ) -> Coroutine[Any, Any, UserStats | None]:
        """A wallet's profile card, or None for a wallet the API does not know."""
        ...
    def user_volume(
        self,
        user: str,
        *,
        start: int | None = None,
        end: int | None = None,
    ) -> Coroutine[Any, Any, UserVolume]:
        """A wallet's traded volume over a whole-day window."""
        ...
    def value(
        self,
        user: str,
        *,
        conditions: list[str] | None = None,
    ) -> Coroutine[Any, Any, PortfolioValue]:
        """A wallet's portfolio value in USDC."""
        ...
    def activity(
        self,
        user: str,
        *,
        types: list[str] | None = None,
        conditions: list[str] | None = None,
        event_ids: list[str] | None = None,
        side: str | None = None,
        start: int | None = None,
        end: int | None = None,
        sort_by: str | None = None,
        sort_direction: str | None = None,
        exclude_deposits_withdrawals: bool | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Coroutine[Any, Any, Page[Activity]]:
        """A wallet's activity feed."""
        ...
    def iter_activity(
        self,
        user: str,
        *,
        types: list[str] | None = None,
        conditions: list[str] | None = None,
        event_ids: list[str] | None = None,
        side: str | None = None,
        start: int | None = None,
        end: int | None = None,
        sort_by: str | None = None,
        sort_direction: str | None = None,
        exclude_deposits_withdrawals: bool | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIterator[Activity]:
        """Walk every page of `activity`, re-sending these filters on each one."""
        ...
    def combo_activity(
        self,
        user: str,
        *,
        conditions: list[str] | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Coroutine[Any, Any, Page[ComboActivity]]:
        """A wallet's combo lifecycle events."""
        ...
    def iter_combo_activity(
        self,
        user: str,
        *,
        conditions: list[str] | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIterator[ComboActivity]:
        """Walk every page of `combo_activity`, re-sending these filters on each one."""
        ...
    def trades(
        self,
        *,
        user: str | None = None,
        conditions: list[str] | None = None,
        event_ids: list[str] | None = None,
        side: str | None = None,
        taker_only: bool | None = None,
        filter_type: str | None = None,
        filter_amount: float | None = None,
        start: int | None = None,
        end: int | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Coroutine[Any, Any, Page[Trade]]:
        """The trade feed, newest first."""
        ...
    def iter_trades(
        self,
        *,
        user: str | None = None,
        conditions: list[str] | None = None,
        event_ids: list[str] | None = None,
        side: str | None = None,
        taker_only: bool | None = None,
        filter_type: str | None = None,
        filter_amount: float | None = None,
        start: int | None = None,
        end: int | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIterator[Trade]:
        """Walk every page of `trades`, re-sending these filters on each one."""
        ...
    def holders(
        self,
        conditions: list[str],
        *,
        min_balance: float | None = None,
        include_pnl: bool | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Coroutine[Any, Any, Page[MetaHolder]]:
        """Top holders per outcome token, for these markets (at most 20)."""
        ...
    def iter_holders(
        self,
        conditions: list[str],
        *,
        min_balance: float | None = None,
        include_pnl: bool | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIterator[MetaHolder]:
        """Walk every page of `holders`, re-sending these filters on each one."""
        ...
    def live_volume(
        self,
        event_ids: list[str],
    ) -> Coroutine[Any, Any, LiveVolume]:
        """Taker volume per market under these Gamma events."""
        ...
    def open_interest(
        self,
        *,
        conditions: list[str] | None = None,
    ) -> Coroutine[Any, Any, list[OpenInterest]]:
        """Open interest per market; one GLOBAL row without `conditions`."""
        ...
    def prices_history(
        self,
        token_id: str,
        *,
        start: int | None = None,
        end: int | None = None,
        interval: str | None = None,
        bucket_seconds: int | None = None,
        as_of: int | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Coroutine[Any, Any, Page[PricePoint]]:
        """An outcome token's price series."""
        ...
    def iter_prices_history(
        self,
        token_id: str,
        *,
        start: int | None = None,
        end: int | None = None,
        interval: str | None = None,
        bucket_seconds: int | None = None,
        as_of: int | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIterator[PricePoint]:
        """Walk every page of `prices_history`, re-sending these filters on each one."""
        ...
    def resolutions(
        self,
        *,
        question_id: str | None = None,
        conditions: list[str] | None = None,
        event_ids: list[str] | None = None,
    ) -> Coroutine[Any, Any, list[Resolution]]:
        """Resolution state, by exactly one of `question_id`, `conditions` or `event_ids`."""
        ...
    def tokens(
        self,
        *,
        token_ids: list[str] | None = None,
        conditions: list[str] | None = None,
    ) -> Coroutine[Any, Any, list[TokenRef]]:
        """Token ids, or the tokens of conditions, resolved to their condition, module, outcome, market and event; by exactly one of `token_ids` or `conditions`."""
        ...
    def biggest_winners(
        self,
        *,
        time_period: str | None = None,
        category: str | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Coroutine[Any, Any, Page[BiggestWinner]]:
        """The largest single winning positions in a window."""
        ...
    def iter_biggest_winners(
        self,
        *,
        time_period: str | None = None,
        category: str | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIterator[BiggestWinner]:
        """Walk every page of `biggest_winners`, re-sending these filters on each one."""
        ...
    def builders_leaderboard(
        self,
        *,
        time_period: str | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Coroutine[Any, Any, Page[BuilderStanding]]:
        """Builders ranked by volume for a window."""
        ...
    def iter_builders_leaderboard(
        self,
        *,
        time_period: str | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIterator[BuilderStanding]:
        """Walk every page of `builders_leaderboard`, re-sending these filters on each one."""
        ...
    def builder_volume(
        self,
        *,
        interval: str | None = None,
        limit: int | None = None,
    ) -> Coroutine[Any, Any, list[BuilderVolumePoint]]:
        """Builder volume per time bucket."""
        ...
    def leaderboard(
        self,
        *,
        time_period: str | None = None,
        category: str | None = None,
        board: str | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Coroutine[Any, Any, Page[LeaderboardEntry]]:
        """The trader board. `board` picks PNL or VOLUME; volume is in shares, not USDC."""
        ...
    def iter_leaderboard(
        self,
        *,
        time_period: str | None = None,
        category: str | None = None,
        board: str | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIterator[LeaderboardEntry]:
        """Walk every page of `leaderboard`, re-sending these filters on each one."""
        ...
    def leaderboard_user(
        self,
        user: str,
        *,
        time_period: str | None = None,
        category: str | None = None,
    ) -> Coroutine[Any, Any, LeaderboardUserEntry | None]:
        """One wallet's standing on both boards, or None for a wallet the API does not know."""
        ...
    def status(
        self,
    ) -> Coroutine[Any, Any, ServiceStatus]:
        """How fresh the served data is. This is not a liveness check."""
        ...

class DataApiSync:
    """Sync client for the Polymarket Data API; each method is a route."""
    def __init__(
        self,
        *,
        base_url: str | None = None,
        timeout_ms: int | None = None,
        pool_size: int | None = None,
    ) -> None: ...
    def approvals(
        self,
        user: str,
    ) -> Approvals:
        """A wallet's Polygon token and operator approvals."""
        ...
    def positions(
        self,
        *,
        user: str | None = None,
        conditions: list[str] | None = None,
        status: str | None = None,
        event_ids: list[str] | None = None,
        title: str | None = None,
        filter_type: str | None = None,
        filter_amount: float | None = None,
        include_archived: bool | None = None,
        sort_by: str | None = None,
        sort_direction: str | None = None,
        start: int | None = None,
        end: int | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Page[Position]:
        """Positions for a wallet (`user`), one market (`conditions` of one id), or a wallet in some markets (both)."""
        ...
    def iter_positions(
        self,
        *,
        user: str | None = None,
        conditions: list[str] | None = None,
        status: str | None = None,
        event_ids: list[str] | None = None,
        title: str | None = None,
        filter_type: str | None = None,
        filter_amount: float | None = None,
        include_archived: bool | None = None,
        sort_by: str | None = None,
        sort_direction: str | None = None,
        start: int | None = None,
        end: int | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIteratorSync[Position]:
        """Walk every page of `positions`, re-sending these filters on each one."""
        ...
    def combo_positions(
        self,
        user: str,
        *,
        conditions: list[str] | None = None,
        statuses: list[str] | None = None,
        sort_by: str | None = None,
        sort_direction: str | None = None,
        updated_after: int | None = None,
        updated_before: int | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Page[ComboPosition]:
        """A wallet's combo positions."""
        ...
    def iter_combo_positions(
        self,
        user: str,
        *,
        conditions: list[str] | None = None,
        statuses: list[str] | None = None,
        sort_by: str | None = None,
        sort_direction: str | None = None,
        updated_after: int | None = None,
        updated_before: int | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIteratorSync[ComboPosition]:
        """Walk every page of `combo_positions`, re-sending these filters on each one."""
        ...
    def user_pnl(
        self,
        user: str,
        *,
        interval: str | None = None,
        fidelity: str | None = None,
    ) -> UserPnlSeries:
        """A wallet's cumulative PnL series. Not the same series as `data.pnl()`."""
        ...
    def user_stats(
        self,
        user: str,
    ) -> UserStats | None:
        """A wallet's profile card, or None for a wallet the API does not know."""
        ...
    def user_volume(
        self,
        user: str,
        *,
        start: int | None = None,
        end: int | None = None,
    ) -> UserVolume:
        """A wallet's traded volume over a whole-day window."""
        ...
    def value(
        self,
        user: str,
        *,
        conditions: list[str] | None = None,
    ) -> PortfolioValue:
        """A wallet's portfolio value in USDC."""
        ...
    def activity(
        self,
        user: str,
        *,
        types: list[str] | None = None,
        conditions: list[str] | None = None,
        event_ids: list[str] | None = None,
        side: str | None = None,
        start: int | None = None,
        end: int | None = None,
        sort_by: str | None = None,
        sort_direction: str | None = None,
        exclude_deposits_withdrawals: bool | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Page[Activity]:
        """A wallet's activity feed."""
        ...
    def iter_activity(
        self,
        user: str,
        *,
        types: list[str] | None = None,
        conditions: list[str] | None = None,
        event_ids: list[str] | None = None,
        side: str | None = None,
        start: int | None = None,
        end: int | None = None,
        sort_by: str | None = None,
        sort_direction: str | None = None,
        exclude_deposits_withdrawals: bool | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIteratorSync[Activity]:
        """Walk every page of `activity`, re-sending these filters on each one."""
        ...
    def combo_activity(
        self,
        user: str,
        *,
        conditions: list[str] | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Page[ComboActivity]:
        """A wallet's combo lifecycle events."""
        ...
    def iter_combo_activity(
        self,
        user: str,
        *,
        conditions: list[str] | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIteratorSync[ComboActivity]:
        """Walk every page of `combo_activity`, re-sending these filters on each one."""
        ...
    def trades(
        self,
        *,
        user: str | None = None,
        conditions: list[str] | None = None,
        event_ids: list[str] | None = None,
        side: str | None = None,
        taker_only: bool | None = None,
        filter_type: str | None = None,
        filter_amount: float | None = None,
        start: int | None = None,
        end: int | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Page[Trade]:
        """The trade feed, newest first."""
        ...
    def iter_trades(
        self,
        *,
        user: str | None = None,
        conditions: list[str] | None = None,
        event_ids: list[str] | None = None,
        side: str | None = None,
        taker_only: bool | None = None,
        filter_type: str | None = None,
        filter_amount: float | None = None,
        start: int | None = None,
        end: int | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIteratorSync[Trade]:
        """Walk every page of `trades`, re-sending these filters on each one."""
        ...
    def holders(
        self,
        conditions: list[str],
        *,
        min_balance: float | None = None,
        include_pnl: bool | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Page[MetaHolder]:
        """Top holders per outcome token, for these markets (at most 20)."""
        ...
    def iter_holders(
        self,
        conditions: list[str],
        *,
        min_balance: float | None = None,
        include_pnl: bool | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIteratorSync[MetaHolder]:
        """Walk every page of `holders`, re-sending these filters on each one."""
        ...
    def live_volume(
        self,
        event_ids: list[str],
    ) -> LiveVolume:
        """Taker volume per market under these Gamma events."""
        ...
    def open_interest(
        self,
        *,
        conditions: list[str] | None = None,
    ) -> list[OpenInterest]:
        """Open interest per market; one GLOBAL row without `conditions`."""
        ...
    def prices_history(
        self,
        token_id: str,
        *,
        start: int | None = None,
        end: int | None = None,
        interval: str | None = None,
        bucket_seconds: int | None = None,
        as_of: int | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Page[PricePoint]:
        """An outcome token's price series."""
        ...
    def iter_prices_history(
        self,
        token_id: str,
        *,
        start: int | None = None,
        end: int | None = None,
        interval: str | None = None,
        bucket_seconds: int | None = None,
        as_of: int | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIteratorSync[PricePoint]:
        """Walk every page of `prices_history`, re-sending these filters on each one."""
        ...
    def resolutions(
        self,
        *,
        question_id: str | None = None,
        conditions: list[str] | None = None,
        event_ids: list[str] | None = None,
    ) -> list[Resolution]:
        """Resolution state, by exactly one of `question_id`, `conditions` or `event_ids`."""
        ...
    def tokens(
        self,
        *,
        token_ids: list[str] | None = None,
        conditions: list[str] | None = None,
    ) -> list[TokenRef]:
        """Token ids, or the tokens of conditions, resolved to their condition, module, outcome, market and event; by exactly one of `token_ids` or `conditions`."""
        ...
    def biggest_winners(
        self,
        *,
        time_period: str | None = None,
        category: str | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Page[BiggestWinner]:
        """The largest single winning positions in a window."""
        ...
    def iter_biggest_winners(
        self,
        *,
        time_period: str | None = None,
        category: str | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIteratorSync[BiggestWinner]:
        """Walk every page of `biggest_winners`, re-sending these filters on each one."""
        ...
    def builders_leaderboard(
        self,
        *,
        time_period: str | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Page[BuilderStanding]:
        """Builders ranked by volume for a window."""
        ...
    def iter_builders_leaderboard(
        self,
        *,
        time_period: str | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIteratorSync[BuilderStanding]:
        """Walk every page of `builders_leaderboard`, re-sending these filters on each one."""
        ...
    def builder_volume(
        self,
        *,
        interval: str | None = None,
        limit: int | None = None,
    ) -> list[BuilderVolumePoint]:
        """Builder volume per time bucket."""
        ...
    def leaderboard(
        self,
        *,
        time_period: str | None = None,
        category: str | None = None,
        board: str | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> Page[LeaderboardEntry]:
        """The trader board. `board` picks PNL or VOLUME; volume is in shares, not USDC."""
        ...
    def iter_leaderboard(
        self,
        *,
        time_period: str | None = None,
        category: str | None = None,
        board: str | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> PageIteratorSync[LeaderboardEntry]:
        """Walk every page of `leaderboard`, re-sending these filters on each one."""
        ...
    def leaderboard_user(
        self,
        user: str,
        *,
        time_period: str | None = None,
        category: str | None = None,
    ) -> LeaderboardUserEntry | None:
        """One wallet's standing on both boards, or None for a wallet the API does not know."""
        ...
    def status(
        self,
    ) -> ServiceStatus:
        """How fresh the served data is. This is not a liveness check."""
        ...
