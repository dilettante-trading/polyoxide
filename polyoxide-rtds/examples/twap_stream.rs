//! Stream Chainlink 30s TWAP prices for BTC and ETH.
//!
//! Run with: `cargo run -p polyoxide-rtds --example twap_stream`

use std::time::Duration;

use polyoxide_rtds::{PriceEvent, RtdsBuilder, Subscription, Topic, TwapWindow};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let subscriptions = Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Thirty))
        .symbols(["btc/usd", "eth/usd"]);

    let stream = RtdsBuilder::new()
        .stale_after(Duration::from_secs(30))
        .connect(subscriptions)
        .await?;

    stream
        .run(|event| async move {
            match event {
                // Sent on connect and again after every reconnect — this is
                // how you re-initialise state, not a one-off.
                PriceEvent::Snapshot(snapshot) => {
                    println!(
                        "backfill: {} points for {}",
                        snapshot.points.len(),
                        snapshot.symbol
                    );
                }
                PriceEvent::Update(update) => {
                    // `value()` is exact. `display_value` is a lossy float and
                    // must not be used for arithmetic.
                    println!(
                        "{:>8} {:>4}s {}",
                        update.symbol(),
                        update.window().map(TwapWindow::seconds).unwrap_or(0),
                        update.value()
                    );
                }
                // `PriceEvent` is `#[non_exhaustive]`: upstream can add event
                // variants (it already did once, for the two above), so a
                // client that matches exhaustively would fail to compile the
                // moment a new one ships. Ignore anything not yet modelled.
                _ => {}
            }
            Ok(())
        })
        .await?;

    Ok(())
}
