//! `Trading` for the CLOB: orders placed and cancelled through the native
//! routes, the account's orders, positions and balance read back, and the
//! in-process ledger of client order ids. The events stream is in `fills`.
//!
//! This module also holds the options a request carries to the CLOB
//! ([`ClobOrderOptions`]) and the CLOB's facts that ride beside each trading
//! record, one `<Record>Ext` type per record (NFR8).

use std::collections::BTreeMap;

use polyoxide_venue::UnixMillis;
use rust_decimal::Decimal;

use crate::clob::types::TickSize;

/// Options only the CLOB reads, carried in an
/// [`OrderRequest`](polyoxide_venue::OrderRequest)'s `extensions`.
///
/// A request carrying any other extension type is refused. Each field is a
/// fact the caller already holds, so the client does not fetch it: it looks a
/// missing `neg_risk` or `tick_size` up. A V2 order signs no fee rate, so
/// there are no fee terms here.
///
/// It is not `Eq`, because [`TickSize`] is not.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub struct ClobOrderOptions {
    /// Whether the market is in a negative-risk event, which selects the
    /// exchange contract the order is signed for.
    pub neg_risk: Option<bool>,
    /// The market's tick size now in force.
    pub tick_size: Option<TickSize>,
    /// When a good-til-cancelled order expires. Set, it becomes a
    /// good-til-date order; the CLOB reads whole seconds, so the time is
    /// rounded down to one.
    pub expiration: Option<UnixMillis>,
}

/// The CLOB's facts about an order, beside
/// [`Order`](polyoxide_venue::Order)'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct OrderExt {
    /// The CLOB's own status word: `live`, `matched` or `delayed` from a
    /// placement, `LIVE`, `MATCHED`, `CANCELED` and the rest from a listing.
    pub status: Option<String>,
    /// The condition id of the market the order is in.
    pub condition_id: Option<String>,
    /// The outcome's label, such as `Yes`.
    pub outcome: Option<String>,
    /// The maker address: the proxy or deposit wallet, not the signing key.
    pub maker_address: Option<String>,
    /// When the order expires, when it does.
    pub expiration: Option<UnixMillis>,
    /// Whether the order was signed for the negative-risk exchange.
    pub neg_risk: Option<bool>,
    /// The ids of the trades the order took part in.
    pub trade_ids: Vec<String>,
    /// The settling transactions' hashes a placement reported.
    pub transaction_hashes: Vec<String>,
}

/// The CLOB's facts about a fill, beside
/// [`Fill`](polyoxide_venue::Fill)'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct FillExt {
    /// The condition id of the market the trade is in.
    pub condition_id: Option<String>,
    /// The outcome's label, such as `Yes`.
    pub outcome: Option<String>,
    /// The taker's order id.
    pub taker_order_id: Option<String>,
    /// The settling transaction's hash, once the CLOB reports one.
    pub transaction_hash: Option<String>,
    /// The maker's fee rate in basis points. A taker's fill has none: the fee
    /// is taken on chain at match time.
    pub fee_rate_bps: Option<Decimal>,
}

/// The CLOB's facts about a position, beside
/// [`Position`](polyoxide_venue::Position)'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct PositionExt {
    /// The condition id of the market the outcome is in.
    pub condition_id: Option<String>,
    /// The outcome's label, such as `Yes`.
    pub outcome: Option<String>,
    /// The outcome's index in its market.
    pub outcome_index: Option<u32>,
    /// The outcome's current price.
    pub current_price: Option<Decimal>,
    /// The position's current value.
    pub current_value: Option<Decimal>,
    /// The position's initial value.
    pub initial_value: Option<Decimal>,
    /// The cash profit and loss.
    pub cash_pnl: Option<Decimal>,
    /// The realized profit and loss.
    pub realized_pnl: Option<Decimal>,
    /// Whether the position can be redeemed.
    pub redeemable: Option<bool>,
    /// Whether the position can be merged.
    pub mergeable: Option<bool>,
    /// Whether the market is in a negative-risk event.
    pub negative_risk: Option<bool>,
    /// The market's title.
    pub title: Option<String>,
    /// When the market ends, as the Data API writes it.
    pub end_date: Option<String>,
}

/// The CLOB's facts about a balance, beside
/// [`Balance`](polyoxide_venue::Balance)'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct BalanceExt {
    /// Each spender's allowance, by the spender's address, divided by 10^6
    /// as the balance is. An unlimited approval is larger than a `Decimal`
    /// holds and reads as `Decimal::MAX`.
    pub allowances: BTreeMap<String, Decimal>,
}

#[cfg(test)]
mod option_tests {
    use polyoxide_venue::Extensions;

    use super::*;

    #[test]
    fn the_options_default_to_nothing_set() {
        assert_eq!(
            ClobOrderOptions::default(),
            ClobOrderOptions {
                neg_risk: None,
                tick_size: None,
                expiration: None,
            }
        );
    }

    #[test]
    fn the_options_ride_in_a_requests_extensions() {
        let options = ClobOrderOptions {
            neg_risk: Some(true),
            tick_size: Some(TickSize::Thousandth),
            expiration: Some(UnixMillis(1_792_000_000_000)),
        };
        let extensions = Extensions::of(options);
        assert_eq!(extensions.get::<ClobOrderOptions>(), Some(&options));
    }

    #[test]
    fn the_ext_types_hold_nothing_until_filled() {
        assert_eq!(OrderExt::default().trade_ids, Vec::<String>::new());
        assert_eq!(FillExt::default().fee_rate_bps, None);
        assert_eq!(PositionExt::default().redeemable, None);
        assert!(BalanceExt::default().allowances.is_empty());
    }
}
