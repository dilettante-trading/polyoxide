use rand::Rng;
use rust_decimal::prelude::ToPrimitive;

use crate::{
    api::markets::OrderLevel,
    types::{OrderSide, TickSize},
};

/// Calculate maker and taker amounts for an order using f64 arithmetic.
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
    const SIZE_DECIMALS: u32 = 6;
    let tick_decimals = tick_size.decimals();

    let price_rounded = round_bankers(price, tick_decimals);
    let size_rounded = round_bankers(size, SIZE_DECIMALS);

    let cost = price_rounded * size_rounded;
    let cost_rounded = round_bankers(cost, SIZE_DECIMALS);

    let share_amount = to_raw_amount(size_rounded, SIZE_DECIMALS);
    let cost_amount = to_raw_amount(cost_rounded, SIZE_DECIMALS);

    match side {
        OrderSide::Buy => (cost_amount, share_amount),
        OrderSide::Sell => (share_amount, cost_amount),
    }
}

/// Calculate maker and taker amounts for a MARKET order.
pub fn calculate_market_order_amounts(
    amount: f64,
    price: f64,
    side: OrderSide,
    tick_size: TickSize,
) -> (String, String) {
    const SIZE_DECIMALS: u32 = 6;
    let tick_decimals = tick_size.decimals();

    let price_rounded = round_bankers(price, tick_decimals);
    let amount_rounded = round_bankers(amount, SIZE_DECIMALS); // Input amount (USDC or Shares)

    if price_rounded == 0.0 {
        return ("0".to_string(), "0".to_string());
    }

    match side {
        OrderSide::Buy => {
            // Market BUY: amount is USDC (maker amount).
            // Taker (shares) = usdc / price
            let maker_amount = amount_rounded;
            let taker_amount_raw = maker_amount / price_rounded;
            let taker_amount = round_to_zero(taker_amount_raw, SIZE_DECIMALS); // Round down/to-zero for shares?
                                                                               // Existing logic used round_dp_with_strategy(ToZero).

            (
                to_raw_amount(maker_amount, SIZE_DECIMALS),
                to_raw_amount(taker_amount, SIZE_DECIMALS),
            )
        }
        OrderSide::Sell => {
            // Market SELL: amount is Shares (maker amount)
            // Taker (USDC) = shares * price
            let maker_amount = round_to_zero(amount, SIZE_DECIMALS); // Shares input usually rounded?
            let taker_amount_raw = maker_amount * price_rounded;
            let taker_amount = round_bankers(taker_amount_raw, SIZE_DECIMALS);

            (
                to_raw_amount(maker_amount, SIZE_DECIMALS),
                to_raw_amount(taker_amount, SIZE_DECIMALS),
            )
        }
    }
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

    #[test]
    fn test_calculate_order_amounts_small_fractional_size() {
        // Very small size to test precision isn't lost
        let (maker, taker) =
            calculate_order_amounts(0.50, 0.000001, OrderSide::Buy, TickSize::Hundredth);
        // cost = 0.50 * 0.000001 = 0.0000005 → rounds to 0 at 6 decimals
        assert_eq!(taker, "1"); // 0.000001 * 10^6 = 1
        assert_eq!(maker, "0"); // 0.0000005 rounds to 0 at 6 decimals → 0
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
