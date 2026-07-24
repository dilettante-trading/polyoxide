//! Example: Connect to the authenticated user WebSocket channel
//!
//! Requires environment variables:
//! - POLYMARKET_API_KEY
//! - POLYMARKET_API_SECRET
//! - POLYMARKET_API_PASSPHRASE
//!
//! Run with:
//! ```
//! cargo run --example ws_user
//! ```

use futures_util::StreamExt;
use polyoxide_clob::ws::{ApiCredentials, Channel, UserMessage, WebSocket};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt::init();

    // Load credentials from environment
    let credentials = ApiCredentials::from_env()?;
    println!("Loaded credentials: {:?}", credentials);

    // Market condition IDs to subscribe to
    // You can get these from the Gamma API
    let market_ids = vec![
        // Example condition ID - replace with actual market condition IDs
        "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
    ];

    println!("Connecting to Polymarket User WebSocket...");
    println!("Subscribing to {} market(s)", market_ids.len());

    let mut ws = WebSocket::connect_user(market_ids, credentials).await?;

    println!("Connected! Waiting for order/trade updates...\n");

    while let Some(msg) = ws.next().await {
        match msg {
            Ok(Channel::User(user_msg)) => match user_msg {
                UserMessage::Order(order) => {
                    println!("📋 Order Update");
                    println!("   ID: {}", order.id);
                    println!("   Type: {:?}", order.order_type);
                    println!("   Side: {}, Outcome: {}", order.side, order.outcome);
                    println!("   Price: {}", order.price);
                    println!(
                        "   Size: {} / {} matched",
                        order.original_size, order.size_matched
                    );
                    println!("   Timestamp: {}", order.timestamp);
                    println!();
                }
                UserMessage::Trade(trade) => {
                    println!("💱 Trade Update");
                    println!("   ID: {}", trade.id);
                    println!("   Status: {:?}", trade.status);
                    println!("   Side: {}, Outcome: {}", trade.side, trade.outcome);
                    println!("   Price: {}, Size: {}", trade.price, trade.size);
                    println!("   Maker orders: {}", trade.maker_orders.len());
                    if let Some(tx) = &trade.transaction_hash {
                        println!("   TX: {}", tx);
                    }
                    println!("   Timestamp: {}", trade.timestamp);
                    println!();
                }
            },
            Ok(_) => {
                // Other channels won't appear on a user connection.
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }

    ws.close().await?;
    println!("Connection closed.");

    Ok(())
}
