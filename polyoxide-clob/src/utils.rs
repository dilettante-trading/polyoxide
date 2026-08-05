use rand::Rng;
use rust_decimal::prelude::ToPrimitive;

use crate::{
    api::markets::OrderLevel,
    types::{OrderSide, TickSize},
};

/// Calculate maker and taker amounts for a LIMIT order.
///
/// The venue enforces two decimal limits beyond the price precision, both
/// varying by tick size. The size the caller supplies is truncated to the
/// `size` limit; the cost derived from it by multiplying by the price is capped
/// at the `amount` limit:
///
/// | tick     | price | size | amount |
/// |----------|-------|------|--------|
/// | `0.1`    | 1     | 2    | 3      |
/// | `0.01`   | 2     | 2    | 4      |
/// | `0.001`  | 3     | 2    | 5      |
/// | `0.0001` | 4     | 2    | 6      |
///
/// The `amount` column is `price + size` for every row, which is what makes the
/// product exact: a price of at most `price` decimals times a size of at most
/// two never needs more than `amount` to write down. That matters because
/// Polymarket never reads the price off the wire — it derives
/// `makerAmount / takerAmount` from the signed order and tick-checks *that*.
/// Rounding both legs to a flat six decimals, as this did previously, breaks
/// the identity: a size of `18.181818` shares (what `$10` at `0.55` comes to)
/// carries four decimals more than the `0.01` tick permits.
///
/// Mirrors `get_order_amounts` in py-clob-client; see
/// [`calculate_market_order_amounts`] for the market-order half of the same
/// contract.
///
/// # Arguments
///
/// * `price` - Order price (0.0 to 1.0)
/// * `size` - Order size in shares
/// * `side` - Buy or Sell
/// * `tick_size` - Minimum price increment for rounding
///
/// # Returns
///
/// A tuple of (maker_amount, taker_amount) as strings suitable for the CLOB API.
pub fn calculate_order_amounts(
    price: f64,
    size: f64,
    side: OrderSide,
    tick_size: TickSize,
) -> (String, String) {
    /// Raw wire amounts are always scaled by 10^6, independent of the decimal
    /// limits applied to the values themselves.
    const WIRE_DECIMALS: u32 = 6;

    let price_rounded = round_bankers(price, tick_size.decimals());

    // Shares are what the caller asked for, so truncate — never round up to a
    // larger order than was requested.
    let size_rounded = truncate_decimals(size, tick_size.size_decimals());
    let cost = cap_decimals(price_rounded * size_rounded, tick_size.amount_decimals());

    let share_amount = to_raw_amount(size_rounded, WIRE_DECIMALS);
    let cost_amount = to_raw_amount(cost, WIRE_DECIMALS);

    match side {
        OrderSide::Buy => (cost_amount, share_amount),
        OrderSide::Sell => (share_amount, cost_amount),
    }
}

/// Calculate maker and taker amounts for a MARKET order.
///
/// The venue enforces two decimal limits beyond the price precision, both
/// varying by tick size. The leg the caller supplies is truncated to the `size`
/// limit; the leg derived from it by dividing or multiplying by the price is
/// capped at the `amount` limit:
///
/// | tick     | price | size | amount |
/// |----------|-------|------|--------|
/// | `0.1`    | 1     | 2    | 3      |
/// | `0.01`   | 2     | 2    | 4      |
/// | `0.001`  | 3     | 2    | 5      |
/// | `0.0001` | 4     | 2    | 6      |
///
/// Rounding both to a flat six decimals — as this did previously — breaks
/// market buys at almost every price, because the division rarely terminates:
/// a plain `$10` buy at `0.55` yields `18.1818…`, which at six decimals is two
/// past what the `0.01` tick permits. Only prices that divide cleanly, like
/// `10 / 0.2 = 50`, slipped through, which is exactly the shape of input the
/// unit tests happened to use.
///
/// Mirrors `get_market_order_amounts` in py-clob-client.
pub fn calculate_market_order_amounts(
    amount: f64,
    price: f64,
    side: OrderSide,
    tick_size: TickSize,
) -> (String, String) {
    /// Raw wire amounts are always scaled by 10^6, independent of the decimal
    /// limits applied to the values themselves.
    const WIRE_DECIMALS: u32 = 6;

    let price_rounded = round_bankers(price, tick_size.decimals());
    if price_rounded == 0.0 {
        return ("0".to_string(), "0".to_string());
    }

    // The supplied leg: USDC for a buy, shares for a sell. Truncated, never
    // rounded up — rounding up would spend or sell more than was asked for.
    let maker_amount = truncate_decimals(amount, tick_size.size_decimals());

    let derived = match side {
        // Buy: USDC in, shares out.
        OrderSide::Buy => maker_amount / price_rounded,
        // Sell: shares in, USDC out.
        OrderSide::Sell => maker_amount * price_rounded,
    };
    let taker_amount = cap_decimals(derived, tick_size.amount_decimals());

    (
        to_raw_amount(maker_amount, WIRE_DECIMALS),
        to_raw_amount(taker_amount, WIRE_DECIMALS),
    )
}

/// Calculate the worst price needed to fill the requested amount from the orderbook.
pub fn calculate_market_price(levels: &[OrderLevel], amount: f64, side: OrderSide) -> Option<f64> {
    if levels.is_empty() {
        return None;
    }

    let mut sum = 0.0;

    for level in levels {
        let p = level.price.to_f64()?;
        let s = level.size.to_f64()?;

        match side {
            OrderSide::Buy => {
                sum += p * s;
            }
            OrderSide::Sell => {
                sum += s;
            }
        }

        if sum >= amount {
            return Some(p);
        }
    }

    // Not enough liquidity to fill the requested amount
    None
}

/// Convert f64 to raw integer string by multiplying by 10^decimals
fn to_raw_amount(val: f64, decimals: u32) -> String {
    let factor = 10f64.powi(decimals as i32);
    // Use matching rounding? Usually if we already rounded 'val', we just multiply and round to int.
    let raw = (val * factor).round();
    // Handle potential overflow if needed, but f64 goes up to 10^308. u128 is 10^38.
    // We assume amounts fit in u128.
    format!("{:.0}", raw)
}

/// Generate a random order `salt`, masked to the JavaScript-safe-integer range
/// (`2^53 - 1`) so it survives Polymarket's numeric wire round-trip. A raw `u64`
/// would be corrupted server-side and, because `salt` is part of the EIP-712
/// signed order struct, would invalidate the signature (see the body note).
pub fn generate_salt() -> String {
    // Polymarket serializes the order `salt` as a JSON number and its backend
    // treats it as a JavaScript-safe integer (and parses it as a signed 64-bit
    // int). A raw `u64` above 2^53-1 is mangled by that numeric round-trip — and
    // because `salt` is part of the EIP-712 signed order struct, the mangled value
    // no longer matches the signature, so the exchange rejects it with
    // "Invalid order payload". Mask the random nonce into the JS-safe range, as
    // the official Polymarket clients do. The salt is a uniqueness nonce, not
    // security-sensitive, so the reduced entropy is fine.
    const JS_SAFE_INTEGER_MAX: u64 = (1 << 53) - 1;
    (rand::rng().random::<u64>() & JS_SAFE_INTEGER_MAX).to_string()
}

// Helpers for rounding

/// Round half to even (Banker's rounding)
fn round_bankers(val: f64, decimals: u32) -> f64 {
    let factor = 10f64.powi(decimals as i32);
    let v = val * factor;
    let r = v.round();
    let diff = (v - r).abs();

    if (diff - 0.5).abs() < 1e-10 {
        // Half-way case
        if r % 2.0 != 0.0 {
            // Odd, so move to even.
            // if v was 1.5, round() gives 2. 2 is even. ok.
            // if v was 2.5, round() gives 3. 3 is odd. We want 2.
            // if v was 0.5, round() gives 1. We want 0.

            // Wait, round() rounds away from zero for .5.
            // 0.5 -> 1.0. 1.5 -> 2.0. 2.5 -> 3.0.
            // We want 2.5 -> 2.0.
            if v > 0.0 {
                return (r - 1.0) / factor;
            } else {
                return (r + 1.0) / factor;
            }
        }
    }
    r / factor
}

/// Round towards zero (Truncate)
fn round_to_zero(val: f64, decimals: u32) -> f64 {
    let factor = 10f64.powi(decimals as i32);
    (val * factor).trunc() / factor
}

/// Round away from zero's lower side (ceiling) at `decimals` places.
fn round_up(val: f64, decimals: u32) -> f64 {
    let factor = 10f64.powi(decimals as i32);
    (val * factor).ceil() / factor
}

/// Count the decimals in `val`'s shortest round-trip representation.
///
/// Mirrors py-clob-client's `decimal_places`, which reads the exponent of
/// `Decimal(str(x))` — Python's `str` and Rust's `{}` both emit the shortest
/// representation that round-trips, so the counts agree. Values that format in
/// exponential notation are reported as maximally precise, which routes them
/// through the capping path rather than silently passing.
fn decimal_places(val: f64) -> u32 {
    let s = format!("{val}");
    if s.contains(['e', 'E']) {
        return u32::MAX;
    }
    s.split_once('.')
        .map(|(_, frac)| frac.len() as u32)
        .unwrap_or(0)
}

/// Constrain `val` to at most `max_decimals`, the way the venue's reference
/// client does.
///
/// The two-step dance is not redundant. Dividing or multiplying by a price
/// routinely lands a hair off an exact value — 49.999999999999996 for what is
/// really 50 — and truncating that outright would silently drop most of a
/// share. Rounding up at a *higher* precision first snaps those artifacts back
/// to the clean value, which then passes the decimal check on its own; only a
/// genuinely repeating result falls through to truncation.
fn cap_decimals(val: f64, max_decimals: u32) -> f64 {
    if decimal_places(val) <= max_decimals {
        return val;
    }
    let bumped = round_up(val, max_decimals + 4);
    if decimal_places(bumped) <= max_decimals {
        return bumped;
    }
    round_to_zero(bumped, max_decimals)
}

/// Constrain a caller-supplied leg to at most `max_decimals` by truncating.
///
/// The guard is not an optimization. `round_to_zero` scales by a power of ten
/// before flooring, and that scaling has its own representation error: `0.29`
/// times 100 is `28.999999999999996`, which floors to a `0.28` order — 3.4%
/// short of what was asked for. 573 of the 10,000 two-decimal values from
/// `0.01` to `100.00` fail that way. Returning an already-compliant value
/// untouched sidesteps the scaling entirely — the same guard [`cap_decimals`]
/// opens with, for the same reason.
fn truncate_decimals(val: f64, max_decimals: u32) -> f64 {
    if decimal_places(val) <= max_decimals {
        return val;
    }
    round_to_zero(val, max_decimals)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_order_amounts_buy() {
        let (maker, taker) =
            calculate_order_amounts(0.52, 100.0, OrderSide::Buy, TickSize::Hundredth);
        assert_eq!(maker, "52000000");
        assert_eq!(taker, "100000000");
    }

    #[test]
    fn test_calculate_order_amounts_sell() {
        let (maker, taker) =
            calculate_order_amounts(0.52, 100.0, OrderSide::Sell, TickSize::Hundredth);
        assert_eq!(maker, "100000000");
        assert_eq!(taker, "52000000");
    }

    #[test]
    fn test_round_bankers() {
        assert_eq!(round_bankers(0.5, 0), 0.0);
        assert_eq!(round_bankers(1.5, 0), 2.0);
        assert_eq!(round_bankers(2.5, 0), 2.0);
        assert_eq!(round_bankers(3.5, 0), 4.0);
    }

    #[test]
    fn test_calculate_market_order_amounts_buy() {
        // 100 USDC, 0.50 price.
        // Maker = 100 * 10^6. Taker = 200 * 10^6.
        let (maker, taker) =
            calculate_market_order_amounts(100.0, 0.50, OrderSide::Buy, TickSize::Hundredth);
        assert_eq!(maker, "100000000");
        assert_eq!(taker, "200000000");
    }

    #[test]
    fn test_calculate_market_price_buy_simple() {
        use rust_decimal_macros::dec;
        // Should find match at 0.50
        let levels = vec![OrderLevel {
            price: dec!(0.50),
            size: dec!(1000),
        }];
        let price = calculate_market_price(&levels, 100.0, OrderSide::Buy);
        assert_eq!(price, Some(0.50));
    }

    #[test]
    fn test_calculate_market_price_insufficient_liquidity() {
        use rust_decimal_macros::dec;
        // Only 10 shares available at 0.50, but we want 1000 USDC worth
        let levels = vec![OrderLevel {
            price: dec!(0.50),
            size: dec!(10),
        }];
        // Buy: sum += price * size = 0.50 * 10 = 5.0, which is < 1000.0
        let price = calculate_market_price(&levels, 1000.0, OrderSide::Buy);
        assert_eq!(
            price, None,
            "Should return None when liquidity is insufficient"
        );
    }

    #[test]
    fn test_calculate_market_price_empty_levels() {
        let price = calculate_market_price(&[], 100.0, OrderSide::Buy);
        assert_eq!(price, None);
    }

    #[test]
    fn test_calculate_market_price_sell_insufficient() {
        use rust_decimal_macros::dec;
        let levels = vec![OrderLevel {
            price: dec!(0.50),
            size: dec!(10),
        }];
        // Sell: sum += size = 10, which is < 100
        let price = calculate_market_price(&levels, 100.0, OrderSide::Sell);
        assert_eq!(
            price, None,
            "Should return None when sell liquidity is insufficient"
        );
    }

    #[test]
    fn test_generate_salt_u64_range() {
        let salt = generate_salt();
        let _parsed: u64 = salt.parse().expect("Salt should parse as u64");
        // Two random salts should (almost certainly) differ
        let salt2 = generate_salt();
        assert_ne!(salt, salt2, "Two random salts should differ");
    }

    #[test]
    fn generate_salt_is_js_safe_integer() {
        // Polymarket parses the order salt as a JS-safe integer / signed int64; it
        // must stay <= 2^53-1 so the JSON-number round-trip (and therefore the
        // EIP-712 signature over `salt`) survives. A raw u64 exceeds this ~99.95%
        // of the time, so this loop reliably catches a regression.
        const JS_SAFE_INTEGER_MAX: u64 = (1 << 53) - 1;
        for _ in 0..10_000 {
            let salt: u64 = generate_salt().parse().expect("salt parses as u64");
            assert!(
                salt <= JS_SAFE_INTEGER_MAX,
                "salt {salt} exceeds JS safe-integer max {JS_SAFE_INTEGER_MAX}"
            );
        }
    }

    // ── calculate_market_order_amounts ──

    #[test]
    fn test_calculate_market_order_amounts_sell() {
        // 100 shares at 0.50 price → maker=shares, taker=USDC
        let (maker, taker) =
            calculate_market_order_amounts(100.0, 0.50, OrderSide::Sell, TickSize::Hundredth);
        assert_eq!(maker, "100000000"); // 100 shares * 10^6
        assert_eq!(taker, "50000000"); // 100 * 0.50 = 50 USDC * 10^6
    }

    #[test]
    fn test_calculate_market_order_amounts_zero_price() {
        let (maker, taker) =
            calculate_market_order_amounts(100.0, 0.0, OrderSide::Buy, TickSize::Hundredth);
        assert_eq!(maker, "0");
        assert_eq!(taker, "0");
    }

    #[test]
    fn test_calculate_market_order_amounts_sell_zero_price() {
        let (maker, taker) =
            calculate_market_order_amounts(100.0, 0.0, OrderSide::Sell, TickSize::Hundredth);
        assert_eq!(maker, "0");
        assert_eq!(taker, "0");
    }

    #[test]
    fn test_calculate_market_order_amounts_tenth_tick() {
        // With Tenth tick size, price rounds to 1 decimal
        let (maker, taker) =
            calculate_market_order_amounts(100.0, 0.5, OrderSide::Buy, TickSize::Tenth);
        assert_eq!(maker, "100000000");
        assert_eq!(taker, "200000000");
    }

    #[test]
    fn test_calculate_market_order_amounts_thousandth_tick() {
        let (maker, taker) =
            calculate_market_order_amounts(100.0, 0.555, OrderSide::Buy, TickSize::Thousandth);
        assert_eq!(maker, "100000000");
        // taker = 100 / 0.555 = 180.180180... truncated to 6 decimals
        let taker_val: u64 = taker.parse().unwrap();
        assert!(taker_val > 180_000_000); // ~180.18 shares
    }

    #[test]
    fn test_calculate_order_amounts_tenth_tick() {
        let (maker, taker) = calculate_order_amounts(0.5, 100.0, OrderSide::Buy, TickSize::Tenth);
        assert_eq!(maker, "50000000");
        assert_eq!(taker, "100000000");
    }

    #[test]
    fn test_calculate_order_amounts_thousandth_tick() {
        let (maker, taker) =
            calculate_order_amounts(0.555, 100.0, OrderSide::Buy, TickSize::Thousandth);
        assert_eq!(maker, "55500000");
        assert_eq!(taker, "100000000");
    }

    // ── calculate_market_price ──

    #[test]
    fn test_calculate_market_price_sell_simple() {
        use rust_decimal_macros::dec;
        let levels = vec![OrderLevel {
            price: dec!(0.50),
            size: dec!(200),
        }];
        // Sell: sum += size. 200 >= 100 → price = 0.50
        let price = calculate_market_price(&levels, 100.0, OrderSide::Sell);
        assert_eq!(price, Some(0.50));
    }

    #[test]
    fn test_calculate_market_price_buy_multiple_levels() {
        use rust_decimal_macros::dec;
        let levels = vec![
            OrderLevel {
                price: dec!(0.50),
                size: dec!(100),
            }, // sum = 50
            OrderLevel {
                price: dec!(0.55),
                size: dec!(100),
            }, // sum = 105
            OrderLevel {
                price: dec!(0.60),
                size: dec!(100),
            }, // sum = 165
        ];
        // Buy: sum += price*size. Need 100 USDC.
        // Level 1: 0.50*100=50 (sum=50 < 100)
        // Level 2: 0.55*100=55 (sum=105 >= 100) → price = 0.55
        let price = calculate_market_price(&levels, 100.0, OrderSide::Buy);
        assert_eq!(price, Some(0.55));
    }

    // ── rounding helpers ──

    #[test]
    fn test_round_to_zero() {
        assert_eq!(round_to_zero(1.999999, 6), 1.999999);
        assert_eq!(round_to_zero(1.9999999, 6), 1.999999);
        assert_eq!(round_to_zero(-1.9999999, 6), -1.999999);
        assert_eq!(round_to_zero(0.0, 6), 0.0);
    }

    #[test]
    fn test_round_bankers_decimals() {
        // 2 decimal places — f64 representation means epsilon-based half
        // detection treats these as half-way cases, rounding to even digit
        assert_eq!(round_bankers(1.235, 2), 1.24); // 124 is even
        assert_eq!(round_bankers(1.245, 2), 1.24); // 124 is even
        assert_eq!(round_bankers(1.265, 2), 1.26); // 126 is even
    }

    #[test]
    fn test_round_bankers_negative() {
        assert_eq!(round_bankers(-0.5, 0), 0.0);
        assert_eq!(round_bankers(-1.5, 0), -2.0);
        assert_eq!(round_bankers(-2.5, 0), -2.0);
    }

    #[test]
    fn test_to_raw_amount() {
        assert_eq!(to_raw_amount(1.0, 6), "1000000");
        assert_eq!(to_raw_amount(0.5, 6), "500000");
        assert_eq!(to_raw_amount(0.0, 6), "0");
        assert_eq!(to_raw_amount(123.456789, 6), "123456789");
    }

    #[test]
    fn test_calculate_market_order_amounts_negative_price_treated_as_zero() {
        // Negative price rounds to negative, which != 0.0, so division proceeds
        // This documents the current behavior (no explicit rejection of negatives)
        let (maker, taker) =
            calculate_market_order_amounts(100.0, -0.5, OrderSide::Buy, TickSize::Hundredth);
        // -0.5 rounds to -0.5, not zero, so division: 100 / -0.5 = -200
        let taker_val: i64 = taker.parse().unwrap();
        assert!(
            taker_val < 0,
            "Negative price produces negative taker amount"
        );
        // This reveals that callers MUST validate price > 0 before calling
        assert_eq!(maker, "100000000");
    }

    // ── Venue decimal-precision limits ──────────────────────────────
    //
    // Polymarket rejects orders whose legs carry more decimals than it allows.
    // The limits are per tick size, mirroring py-clob-client's ROUNDING_CONFIG:
    //
    //   tick     price  size  amount
    //   0.1        1      2      3
    //   0.01       2      2      4
    //   0.001      3      2      5
    //   0.0001     4      2      6
    //
    // `size` bounds the leg the caller supplies, `amount` bounds the leg we
    // derive from it. Both were previously rounded to 6 decimals regardless.

    /// Wire amounts are `value × 10^6`, so a value with at most `max` decimals
    /// leaves a raw amount divisible by `10^(6 - max)`.
    fn assert_max_decimals(raw: &str, max: u32, label: &str) {
        let v: i64 = raw.parse().expect("raw amount should parse");
        let modulus = 10i64.pow(6 - max);
        assert_eq!(
            v % modulus,
            0,
            "{label} {raw} carries more than {max} decimals \
             (raw must be divisible by {modulus})"
        );
    }

    /// The headline case: a clean $10 buy at a perfectly ordinary price.
    ///
    /// 10 / 0.55 = 18.1818… which the old code truncated to 6 decimals
    /// (18.181818) — two more than the 0.01 tick allows. Nothing about the
    /// input is unusual, which is why market buys failed at most prices rather
    /// than only on odd user input.
    #[test]
    fn market_buy_taker_respects_amount_precision() {
        let (maker, taker) =
            calculate_market_order_amounts(10.0, 0.55, OrderSide::Buy, TickSize::Hundredth);
        assert_eq!(maker, "10000000", "maker is a clean $10");
        assert_max_decimals(&taker, 4, "market buy taker");
        assert_eq!(taker, "18181800", "18.1818 shares, truncated to 4 decimals");
    }

    /// The maker leg is truncated to 2 decimals, not passed through at 6.
    #[test]
    fn market_buy_maker_respects_size_precision() {
        let (maker, _taker) =
            calculate_market_order_amounts(10.129, 0.5, OrderSide::Buy, TickSize::Hundredth);
        assert_max_decimals(&maker, 2, "market buy maker");
        assert_eq!(maker, "10120000", "$10.129 truncates to $10.12");
    }

    /// The `amount` cap varies by tick size; a single 6-decimal constant cannot
    /// satisfy all four.
    #[test]
    fn market_buy_taker_precision_across_tick_sizes() {
        for (tick, max) in [
            (TickSize::Tenth, 3),
            (TickSize::Hundredth, 4),
            (TickSize::Thousandth, 5),
            (TickSize::TenThousandth, 6),
        ] {
            let (_maker, taker) = calculate_market_order_amounts(10.0, 0.3, OrderSide::Buy, tick);
            assert_max_decimals(&taker, max, &format!("{tick:?} taker"));
        }
    }

    /// The float-artifact guard is load-bearing, not defensive decoration.
    ///
    /// `1.03 / 0.05` is exactly `20.6`, but in `f64` it evaluates to
    /// `20.599999999999998`. Truncating that at four decimals yields `20.5999`
    /// — a silently short order. Rounding up at a higher precision first snaps
    /// it back to `20.6`, which then satisfies the decimal limit unaided.
    ///
    /// A sweep of $1.00–$50.00 against every hundredth-tick price turns up
    /// thousands of such cases, so this is the common path, not a corner.
    #[test]
    fn market_buy_recovers_float_representation_artifacts() {
        let (_maker, taker) =
            calculate_market_order_amounts(1.03, 0.05, OrderSide::Buy, TickSize::Hundredth);
        assert_eq!(
            taker, "20600000",
            "1.03 / 0.05 is 20.6; naive truncation would give 20.5999"
        );
        assert_max_decimals(&taker, 4, "artifact-recovered taker");
    }

    /// Sells have the same contract with the legs swapped: shares in are capped
    /// at `size`, the derived USDC at `amount`.
    #[test]
    fn market_sell_respects_precision() {
        let (maker, taker) =
            calculate_market_order_amounts(33.333333, 0.55, OrderSide::Sell, TickSize::Hundredth);
        assert_max_decimals(&maker, 2, "market sell maker");
        assert_eq!(maker, "33330000", "33.333333 shares truncate to 33.33");
        assert_max_decimals(&taker, 4, "market sell taker");
    }

    /// Truncating an input that is *already* within the limit must not move it.
    ///
    /// `0.29 * 100` is `28.999999999999996` in f64, so a bare floor turns a
    /// perfectly legal `$0.29` into `$0.28`. 573 of the 10,000 two-decimal
    /// values between `0.01` and `100.00` land this way, so the guard is not a
    /// corner case.
    #[test]
    fn market_buy_preserves_amount_already_within_precision() {
        let (maker, taker) =
            calculate_market_order_amounts(0.29, 0.5, OrderSide::Buy, TickSize::Hundredth);
        assert_eq!(
            maker, "290000",
            "$0.29 is already 2 decimals; leave it alone"
        );
        assert_eq!(taker, "580000", "0.29 / 0.5 = 0.58 shares");
    }

    // ── calculate_order_amounts (limit orders) ──────────────────────
    //
    // Same two limits, with `size` bounding the shares the caller supplies and
    // `amount` bounding the cost derived from them. Mirrors `get_order_amounts`
    // in py-clob-client.

    /// The size leg is truncated to 2 decimals, not rounded at 6.
    ///
    /// `18.181818` shares is what "$10 at 0.55" comes to, so this is the shape
    /// of size a caller sizing by budget passes in — not an odd input.
    #[test]
    fn limit_buy_truncates_size_to_size_precision() {
        let (maker, taker) =
            calculate_order_amounts(0.55, 18.181818, OrderSide::Buy, TickSize::Hundredth);
        assert_max_decimals(&taker, 2, "limit buy size leg");
        assert_eq!(taker, "18180000", "18.181818 shares truncate to 18.18");
        assert_max_decimals(&maker, 4, "limit buy cost leg");
        assert_eq!(maker, "9999000", "0.55 * 18.18 = 9.999 USDC");
    }

    /// Sells carry the same contract with the legs swapped.
    #[test]
    fn limit_sell_truncates_size_to_size_precision() {
        let (maker, taker) =
            calculate_order_amounts(0.55, 18.181818, OrderSide::Sell, TickSize::Hundredth);
        assert_max_decimals(&maker, 2, "limit sell size leg");
        assert_eq!(maker, "18180000", "18.181818 shares truncate to 18.18");
        assert_max_decimals(&taker, 4, "limit sell cost leg");
        assert_eq!(taker, "9999000", "0.55 * 18.18 = 9.999 USDC");
    }

    /// The `amount` cap varies by tick; a flat six decimals cannot satisfy all
    /// four rows.
    #[test]
    fn limit_legs_respect_precision_across_tick_sizes() {
        for (tick, amount_max) in [
            (TickSize::Tenth, 3),
            (TickSize::Hundredth, 4),
            (TickSize::Thousandth, 5),
            (TickSize::TenThousandth, 6),
        ] {
            let (maker, taker) = calculate_order_amounts(0.5, 100.123456, OrderSide::Buy, tick);
            assert_max_decimals(&taker, 2, &format!("{tick:?} size leg"));
            assert_max_decimals(&maker, amount_max, &format!("{tick:?} cost leg"));
        }
    }

    /// A size already within the limit must survive untouched.
    ///
    /// Same f64 trap as the market path: flooring `0.29` at two decimals yields
    /// `0.28`, quietly shrinking the order by 3.4%.
    #[test]
    fn limit_preserves_size_already_within_precision() {
        let (maker, taker) =
            calculate_order_amounts(0.5, 0.29, OrderSide::Buy, TickSize::Hundredth);
        assert_eq!(
            taker, "290000",
            "0.29 is already 2 decimals; leave it alone"
        );
        assert_eq!(maker, "145000", "0.5 * 0.29 = 0.145 USDC");
    }

    /// The invariant the whole table exists to protect, checked exhaustively.
    ///
    /// Polymarket never reads a price field off the wire: it recovers
    /// `makerAmount / takerAmount` from the signed order and tick-checks the
    /// result. So the real contract is not "few enough decimals" but "the two
    /// legs divide back to exactly the price the caller asked for" — verified
    /// here in integer arithmetic, where no float artifact can hide.
    ///
    /// Sweeps every tick price against both a compliant size and the same size
    /// made over-precise, on both sides — ~600k orders. An over-precise size
    /// must produce byte-identical legs to its truncated twin, which is the
    /// property the old flat-six-decimal code could not hold.
    #[test]
    fn limit_legs_divide_back_to_the_requested_price() {
        for (tick, price_steps, size_steps) in [
            (TickSize::Tenth, 9, 500),
            (TickSize::Hundredth, 99, 250),
            (TickSize::Thousandth, 999, 25),
            (TickSize::TenThousandth, 9999, 10),
        ] {
            let price_scale = 10i64.pow(tick.decimals());
            let amount_modulus = 10i64.pow(6 - tick.amount_decimals());

            for price_int in 1..=price_steps {
                let price = price_int as f64 / price_scale as f64;
                for size_int in 1..=size_steps {
                    let size = size_int as f64 / 100.0;
                    // The same order, requested with a size the venue cannot
                    // express. Truncation must land it back on `size`.
                    let over_precise = size + 0.006789;

                    for side in [OrderSide::Buy, OrderSide::Sell] {
                        let legs = calculate_order_amounts(price, size, side, tick);
                        assert_eq!(
                            calculate_order_amounts(price, over_precise, side, tick),
                            legs,
                            "{tick:?} {side:?} {price}: size {over_precise} must \
                             truncate to the same order as {size}"
                        );
                        let (maker, taker) = legs;
                        let (shares, cost) = match side {
                            OrderSide::Buy => (&taker, &maker),
                            OrderSide::Sell => (&maker, &taker),
                        };
                        let shares: i64 = shares.parse().expect("shares parse");
                        let cost: i64 = cost.parse().expect("cost parse");

                        assert_eq!(
                            shares,
                            size_int * 10_000,
                            "{tick:?} {side:?} {price}x{size}: a compliant size must survive intact"
                        );
                        assert_eq!(
                            cost % amount_modulus,
                            0,
                            "{tick:?} {side:?} {price}x{size}: cost {cost} exceeds the amount limit"
                        );
                        // cost / shares == price, in exact integers.
                        assert_eq!(
                            cost * price_scale,
                            price_int * shares,
                            "{tick:?} {side:?} {price}x{size}: legs imply a price off the tick"
                        );
                    }
                }
            }
        }
    }

    /// The derived cost needs the same two-step artifact recovery the market
    /// path uses, for the same reason.
    ///
    /// `0.29 * 3` is exactly `0.87`, but f64 evaluates it to
    /// `0.8699999999999999`. Truncating that at four decimals gives `0.8699` —
    /// an order priced a hundredth of a cent light, which no longer divides
    /// back to a valid tick.
    #[test]
    fn limit_cost_recovers_float_representation_artifacts() {
        let (maker, taker) =
            calculate_order_amounts(0.29, 3.0, OrderSide::Buy, TickSize::Hundredth);
        assert_eq!(taker, "3000000", "3 shares");
        assert_eq!(
            maker, "870000",
            "0.29 * 3 is 0.87; naive truncation would give 0.8699"
        );
        assert_max_decimals(&maker, 4, "artifact-recovered cost leg");
    }

    /// Sizes below the venue's 2-decimal floor truncate away entirely.
    ///
    /// `0.000001` shares is not expressible in an order, so both legs come out
    /// zero rather than as a 6-decimal size the venue would reject.
    ///
    /// Orders built through the client never reach this state:
    /// [`CreateOrderParams::validate`] rejects sizes below `0.01` up front. The
    /// helper stays total anyway, since it is public and callers may drive it
    /// directly.
    ///
    /// [`CreateOrderParams::validate`]: crate::client::CreateOrderParams::validate
    #[test]
    fn test_calculate_order_amounts_small_fractional_size() {
        let (maker, taker) =
            calculate_order_amounts(0.50, 0.000001, OrderSide::Buy, TickSize::Hundredth);
        assert_eq!(taker, "0", "0.000001 shares truncates to 0 at 2 decimals");
        assert_eq!(maker, "0", "and so the cost is 0 too");
    }

    #[test]
    fn test_calculate_market_price_exact_boundary() {
        use rust_decimal_macros::dec;
        // Amount exactly matches the sum at a level boundary
        let levels = vec![
            OrderLevel {
                price: dec!(0.50),
                size: dec!(100),
            },
            OrderLevel {
                price: dec!(0.60),
                size: dec!(100),
            },
        ];
        // Buy: level 1 sum = 0.50*100 = 50. Exactly 50 requested → price = 0.50
        let price = calculate_market_price(&levels, 50.0, OrderSide::Buy);
        assert_eq!(price, Some(0.50));
    }
}
