# CLOB Per-Signer Trading Rate Limits

Base URL: `https://clob.polymarket.com`

Source: <https://docs.polymarket.com/api-reference/trading-rate-limits>, fetched 2026-08-05.

**This is a second, independent limiter.** Polymarket evaluates CLOB order and
cancellation requests against separate token buckets *for each signer address*.
These operate independently of the Cloudflare IP-based limits in
[`rate-limits.md`](rate-limits.md), which remain unchanged. A request must
satisfy **both** layers.

The two layers count different things. Cloudflare counts **requests**; this
layer counts **orders**. For batch endpoints they diverge by the batch size.

## Token cost

Each signer has two buckets — one for orders, one for cancellations.

| Bucket | Request | Token cost |
|--------|---------|-----------|
| Order | `POST /order` | 1 |
| Order | `POST /orders` | number of orders in batch |
| Cancel | `DELETE /order` | 1 |
| Cancel | `DELETE /orders` | number of submitted order IDs |
| Cancel | `DELETE /cancel-all` | 1 + orders canceled |
| Cancel | `DELETE /cancel-market-orders` | 1 + matching orders canceled |

## Volume tiers

Tier is determined by 30-day volume. Rates are per second; burst is the bucket's
capacity.

| Tier | 30-day volume | Order rate | Order burst | Cancel rate | Cancel burst | Negative balance |
|------|---------------|-----------|------------|------------|-------------|-----------------|
| Standard | — | 40/s | 60 | 80/s | 120 | Yes |
| Copper | $30,000+ | 60/s | 90 | 120/s | 180 | Yes |
| Bronze | $50,000+ | 80/s | 120 | 160/s | 240 | Yes |
| Silver | $100,000+ | 200/s | 300 | 400/s | 600 | Yes |
| Gold | $500,000+ | 400/s | 600 | 800/s | 1,200 | Yes |
| Platinum | $2.5M+ | 450/s | 675 | 900/s | 1,350 | No |
| Diamond | $5M+ | 525/s | 787 | 1,050/s | 1,575 | No |
| Elite | $10M+ | 600/s | 900 | 1,200/s | 1,800 | No |

## Response headers

Present on successful evaluation:

| Header | Meaning |
|--------|---------|
| `Poly-RateLimit-Remaining` | token balance after accounting |
| `Poly-RateLimit-Reset` | Unix timestamp for end of the wait period |
| `Poly-RateLimit-Tier` | the tier applied |

Conditional:

| Header | Meaning |
|--------|---------|
| `Retry-After` | minimum retry delay in seconds, on `429` |
| `Poly-RateLimit-Warning` | `"true"` during warning mode |

## Exceeding the limit

Returns **429 Too Many Requests** with `Retry-After`. **Batch requests are
rejected entirely** if tokens are insufficient — there is no partial acceptance.

### A cost above burst capacity can never succeed

This is the consequence that matters for client design. A token bucket never
holds more than its burst capacity, so a request costing more than that capacity
is **permanently** rejected, not transiently throttled. Retrying cannot help.

Worked examples:

- A **Standard** account with 200 open orders calling `DELETE /cancel-all` costs
  201 tokens against a cancel burst of 120. It can never succeed.
- A `DELETE /orders` batch of 2,000 IDs costs 2,000 tokens. The largest cancel
  burst anywhere is **Elite's 1,800**, so that batch fails on every tier.
- A **Standard** account posting `POST /orders` with 100 orders costs 100 against
  an order burst of 60. It fails until the account reaches Silver.

Clients should therefore split batches to the tier's burst capacity rather than
treating the resulting 429 as retriable. `ClobError::is_retriable` classifies 429
as retriable in general, which is correct for the Cloudflare layer and wrong for
this one — the distinction is why polyoxide rejects over-capacity batches
client-side before sending.
