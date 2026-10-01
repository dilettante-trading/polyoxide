# Sports feed

Host: `sports-api.polymarket.com`

| Route | Kind | Auth | Implemented by |
|---|---|---|---|
| `wss://…/ws` | WebSocket, server push only, no subscription | None | `polyoxide-sports` (`SportsWs`, `SportsWsBuilder`) |
| `https://…/health` | `GET`, empty `200` | None | Not implemented |

Every other path probed answers `404 page not found`; the list is in
[OBSERVED.md](OBSERVED.md).

Gamma's sports routes (`/sports`, `/sports/market-types`, `/teams`,
`/teams/{id}`) live on `gamma-api.polymarket.com` and are documented in
[../gamma/sports.md](../gamma/sports.md).

| File | What it is |
|---|---|
| [asyncapi.json](asyncapi.json) | Upstream's AsyncAPI document for `/ws`, annotated with `x-observed-payload` and `x-observed-keepalive`. **It does not match the wire**, so it is excluded from `nightly-schema.yml`. Moved from `docs/specs/clob/asyncapi-sports.json` on 2026-10-01. |
| [OBSERVED.md](OBSERVED.md) | What the server actually does |
