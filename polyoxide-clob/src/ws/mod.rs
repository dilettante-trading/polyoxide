//! WebSocket client for real-time Polymarket CLOB updates.
//!
//! This module provides WebSocket connectivity to Polymarket's real-time data streams,
//! including market data (order book updates, price changes) and user-specific updates
//! (orders, trades).
//!
//! # Channels
//!
//! Two channels are available:
//!
//! - **Market Channel**: Public channel for order book and price updates. Subscribe with
//!   asset IDs (token IDs) to receive [`BookMessage`], [`PriceChangeMessage`],
//!   [`TickSizeChangeMessage`], and [`LastTradePriceMessage`] updates.
//!
//! - **User Channel**: Authenticated channel for user order and trade updates. Subscribe
//!   with market condition IDs and API credentials to receive [`OrderMessage`] and
//!   [`TradeMessage`] updates.
//!
//! # Basic Example
//!
//! ```no_run
//! use polyoxide_clob::ws::{WebSocket, Channel, MarketMessage};
//! use futures_util::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Connect to market channel (no auth required)
//!     let mut ws = WebSocket::connect_market(vec![
//!         "asset_id_1".to_string(),
//!         "asset_id_2".to_string(),
//!     ]).await?;
//!
//!     // Process incoming messages
//!     while let Some(msg) = ws.next().await {
//!         match msg? {
//!             Channel::Market(MarketMessage::Book(book)) => {
//!                 println!("Order book: {} bids, {} asks", book.bids.len(), book.asks.len());
//!             }
//!             Channel::Market(MarketMessage::PriceChange(pc)) => {
//!                 println!("Price change: {:?}", pc.price_changes);
//!             }
//!             _ => {}
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # Authenticated User Channel
//!
//! ```no_run
//! use polyoxide_clob::ws::{ApiCredentials, WebSocket, Channel, UserMessage};
//! use futures_util::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let credentials = ApiCredentials::from_env()?;
//!
//!     let mut ws = WebSocket::connect_user(
//!         vec!["condition_id".to_string()],
//!         credentials,
//!     ).await?;
//!
//!     while let Some(msg) = ws.next().await {
//!         match msg? {
//!             Channel::User(UserMessage::Order(order)) => {
//!                 println!("Order update: {} {:?}", order.id, order.order_type);
//!             }
//!             Channel::User(UserMessage::Trade(trade)) => {
//!                 println!("Trade: {} @ {}", trade.size, trade.price);
//!             }
//!             _ => {}
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # Auto-Ping with WebSocketBuilder
//!
//! For long-running connections, use [`WebSocketBuilder`] to automatically send
//! keep-alive pings:
//!
//! ```no_run
//! use polyoxide_clob::ws::WebSocketBuilder;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let ws = WebSocketBuilder::new()
//!         .ping_interval(Duration::from_secs(10))
//!         .connect_market(vec!["asset_id".to_string()])
//!         .await?;
//!
//!     ws.run(|msg| async move {
//!         println!("Received: {:?}", msg);
//!         Ok(())
//!     }).await?;
//!
//!     Ok(())
//! }
//! ```

mod auth;
mod client;
mod error;
mod market;
mod subscription;
mod user;

pub use auth::ApiCredentials;
pub use client::{WebSocket, WebSocketBuilder, WebSocketWithPing};
pub use error::WebSocketError;
pub use market::{
    BookMessage, LastTradePriceMessage, MarketMessage, OrderSummary, PriceChange,
    PriceChangeMessage, TickSizeChangeMessage,
};
pub use subscription::ChannelType;
pub use user::{MakerOrder, OrderEventType, OrderMessage, TradeMessage, TradeStatus, UserMessage};

/// All possible WebSocket channel messages
#[derive(Debug, Clone)]
pub enum Channel {
    /// Market channel message
    Market(MarketMessage),
    /// User channel message
    User(UserMessage),
}
