//! One function reads any venue's best bid and ask, and one classifier reads
//! any venue's error. Written once, against `polyoxide::venue`'s
//! `MarketData` and `Classify`, they run unchanged on the Polymarket CLOB,
//! Polymarket perps and Binance USDⓈ-M; adding a venue adds no code here.
//!
//! ```text
//! cargo run -p polyoxide --example venue_neutral \
//!     --no-default-features --features polymarket-clob,polymarket-perps,binance-usdm
//! ```
//!
//! `tests/venue_neutral.rs` runs both functions against mock servers for each
//! module, and `tests/live_venue.rs` runs `best_bid_ask` against the live hosts.

use std::sync::Arc;
use std::time::Duration;

use polyoxide::binance::usdm::{self, types::Symbol, Usdm};
use polyoxide::polymarket::clob::Clob;
use polyoxide::polymarket::perps::{types::InstrumentId, Perps};
use polyoxide::polymarket::shared::keys::{self, TokenId};
use polyoxide::venue::{
    Class, ClassifiedError, Classify, DynMarketData, Level, MarketData, MarketKey,
};

/// The best bid and ask of `key`, whichever venue `venue` reads.
pub async fn best_bid_ask(
    venue: &DynMarketData<'_>,
    key: &MarketKey,
) -> Result<(Option<Level>, Option<Level>), ClassifiedError> {
    let quote = venue.quote(key).await?;
    Ok((quote.bid, quote.ask))
}

/// What a consumer does about an error, decided from its class alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Handling {
    /// Slow down, for the wait the venue asked for when it gave one.
    Backoff(Option<Duration>),
    /// Try again: no answer came, or the venue could not answer now.
    Retry,
    /// The credentials are missing, wrong or not allowed.
    Reauthenticate,
    /// The venue will not serve this caller. Alert only on a fault.
    Restricted {
        /// Whether the error is a fault.
        alert: bool,
    },
    /// The request was wrong, or the venue refused it. Alert only on a fault.
    Rejected {
        /// Whether the error is a fault.
        alert: bool,
    },
}

/// Reads any module's error, whichever venue raised it, through `Classify`.
pub fn classify<E: Classify + ?Sized>(err: &E) -> Handling {
    handling(&err.class(), err.is_fault(), err.retry_after())
}

/// Reads the error a venue trait returns, which carries what `Classify`
/// reported.
pub fn classify_classified(err: &ClassifiedError) -> Handling {
    handling(&err.class, err.is_fault(), err.retry_after())
}

fn handling(class: &Class, fault: bool, retry_after: Option<Duration>) -> Handling {
    match class {
        Class::RateLimited { .. } => Handling::Backoff(retry_after),
        Class::Network | Class::Unavailable { .. } => Handling::Retry,
        Class::Unauthorized => Handling::Reauthenticate,
        Class::Restricted => Handling::Restricted { alert: fault },
        _ => Handling::Rejected { alert: fault },
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let clob = Clob::public();
    let open = clob.markets().sampling().send().await?;
    let token = open
        .data
        .iter()
        .filter(|market| market.accepting_orders == Some(true))
        .flat_map(|market| &market.tokens)
        .find_map(|token| token.token_id.clone())
        .ok_or("no open market on the CLOB's first sampling page")?;

    let venues: [(Arc<DynMarketData<'static>>, MarketKey); 3] = [
        (
            DynMarketData::new_arc(Usdm::new()?),
            usdm::key(&Symbol::new("BTCUSDT")?),
        ),
        (
            DynMarketData::new_arc(Perps::new()?),
            keys::perps(InstrumentId(1)),
        ),
        (
            DynMarketData::new_arc(clob),
            keys::clob(&TokenId::new(token)?),
        ),
    ];
    for (venue, key) in &venues {
        match best_bid_ask(venue, key).await {
            Ok((bid, ask)) => println!("{key}: bid {bid:?}, ask {ask:?}"),
            Err(err) => println!("{key}: {err} ({:?})", classify_classified(&err)),
        }
    }
    Ok(())
}
