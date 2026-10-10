# Proposed spine amendments: Epic 2

Recorded per AD-21: an epic records each proposed amendment here, and one session later merges this file into the spine and regenerates the guide.

## A2-1: a Restricted failure tags by its fault flag (amends AD-14's class-to-tag map)

- **Proposed by:** Claude, as the user's delegate, 2026-10-08, during Epic 2's bundle D1 (Stories 2.3 and 2.4). Found by the D2 investigation.
- **Current rule (AD-14):** the tag comes from the class alone: `Restricted` → `environmental`.
- **Proposed rule:** `Restricted` tags `environmental` when `is_fault()` is false, and `real` when it is true. Every other class keeps its AD-14 tag.
- **Why:**
  - **A 418 is the client's own bug.** Binance's IP ban (`BinanceError::IpBanned`) follows a breach of the weight budget, so skipping it as `environmental` silently loses the one alarm for that bug.
  - **The regex-era classifier agrees.** It pinned the 418 ban and the WAF 403 (`Forbidden`, D14) as `real` in `test_other_binance_refusals_are_not_environmental`.
  - **`Classify` already says to alert on `is_fault`**, and filing an issue is alerting.
  - **The flag already splits the cases correctly.** `RegionBlocked` and a 451 from any status-rule impl are non-faults, so they become `environmental`. `IpBanned` and `Forbidden` are faults, so they become `real`.
- **Supersedes:** Story 2.3's "with the tag mapped from the class alone". The tag now comes from the class plus `is_fault()`.
- **Follow-up in the code:** `UsdmWsError` must report a handshake 451 as a non-fault, as `BinanceError` does.
