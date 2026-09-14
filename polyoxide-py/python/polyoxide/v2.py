"""Data API v2 (``/v2/*``): cursor-paged routes, snake_case fields, structured errors.

Obtain the routes with ``DataApi().v2()`` or ``DataApiSync().v2()``. Several
classes here share a name with a v1 class in ``polyoxide`` (``Trade``,
``Position``, ``Activity``, ...); they are different types with different fields.
"""

from ._polyoxide import v2 as _v2

DataV2 = _v2.DataV2
DataV2Sync = _v2.DataV2Sync
Page = _v2.Page
PageIterator = _v2.PageIterator
PageIteratorSync = _v2.PageIteratorSync
Pagination = _v2.Pagination
Activity = _v2.Activity
Approvals = _v2.Approvals
BiggestWinner = _v2.BiggestWinner
BuilderStanding = _v2.BuilderStanding
BuilderVolumePoint = _v2.BuilderVolumePoint
ComboActivity = _v2.ComboActivity
ComboPosition = _v2.ComboPosition
LeaderboardEntry = _v2.LeaderboardEntry
LeaderboardUserEntry = _v2.LeaderboardUserEntry
LiveVolume = _v2.LiveVolume
MetaHolder = _v2.MetaHolder
OpenInterest = _v2.OpenInterest
PortfolioValue = _v2.PortfolioValue
Position = _v2.Position
PricePoint = _v2.PricePoint
Resolution = _v2.Resolution
ServiceStatus = _v2.ServiceStatus
Trade = _v2.Trade
UserPnlSeries = _v2.UserPnlSeries
UserStats = _v2.UserStats
UserVolume = _v2.UserVolume

__all__ = [
    "DataV2",
    "DataV2Sync",
    "Page",
    "PageIterator",
    "PageIteratorSync",
    "Pagination",
    "Activity",
    "Approvals",
    "BiggestWinner",
    "BuilderStanding",
    "BuilderVolumePoint",
    "ComboActivity",
    "ComboPosition",
    "LeaderboardEntry",
    "LeaderboardUserEntry",
    "LiveVolume",
    "MetaHolder",
    "OpenInterest",
    "PortfolioValue",
    "Position",
    "PricePoint",
    "Resolution",
    "ServiceStatus",
    "Trade",
    "UserPnlSeries",
    "UserStats",
    "UserVolume",
]
