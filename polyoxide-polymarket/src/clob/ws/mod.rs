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
//!   [`TickSizeChangeMessage`], and [`LastTradePriceMessage`] updates. Membership can
//!   change on a live socket: [`WebSocket::subscribe_assets`] /
//!   [`WebSocket::unsubscribe_assets`] on the plain socket, or a [`MembershipHandle`]
//!   taken from [`WebSocketWithPing::membership`] while `run` drives it. The venue
//!   sends a `book` snapshot for each newly added asset and nothing for one already
//!   subscribed (see [`MarketSubscriptionUpdate`]). A socket opened with no assets is
//!   valid and stays open under the keep-alive ping.
//!
//! - **User Channel**: Authenticated channel for user order and trade updates. Subscribe
//!   with market condition IDs and API credentials to receive [`OrderMessage`] and
//!   [`TradeMessage`] updates. Pass no condition IDs — via
//!   [`WebSocket::connect_user_all_markets`] — to receive events for every market, and
//!   use [`WebSocket::subscribe_markets`] to adjust the filter on a live connection.
//!
//! The live sports feed is not a CLOB channel. It is served by another host and
//! lives in this crate's `sports` module.
//!
//! # Basic Example
//!
//! ```no_run
//! use polyoxide_polymarket::clob::ws::{WebSocket, Channel, MarketMessage};
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
//! use polyoxide_polymarket::clob::ws::{ApiCredentials, WebSocket, Channel, UserMessage};
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
//! use polyoxide_polymarket::clob::ws::WebSocketBuilder;
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
//!
//! # Supervised Market Socket
//!
//! [`MarketWsBuilder`] opens the market channel on the socket kit's `Supervisor`,
//! which keeps it alive where the types above end on their first failure. It
//! sends the keep-alive `PING` itself, reconnects after a server close or a
//! stall, replays the whole subscription (membership changes included) on every
//! new connection, and marks each outage with a [`MarketEvent::Disconnected`]
//! and a [`MarketEvent::Reconnected`], so a consumer knows when its books are
//! stale and when to rebuild them. A frame that carries several books yields
//! one event for each. Membership changes on a live socket go through
//! [`MarketMembership`], on the handle that `Supervised::membership` returns.
//!
//! ```no_run
//! use futures_util::StreamExt;
//! use polyoxide_polymarket::clob::ws::{MarketEvent, MarketMessage, MarketWsBuilder};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut ws = MarketWsBuilder::new()
//!         .connect(vec!["asset_id".to_string()])
//!         .await?;
//!
//!     while let Some(item) = ws.next().await {
//!         match item {
//!             Ok(MarketEvent::Message(MarketMessage::Book(book))) => {
//!                 println!("{}: {} bids, {} asks", book.asset_id, book.bids.len(), book.asks.len());
//!             }
//!             Ok(MarketEvent::Disconnected { reason }) => eprintln!("down: {reason:?}"),
//!             Ok(MarketEvent::Reconnected) => eprintln!("up again: the books that follow replace yours"),
//!             Ok(_) => {}
//!             // A frame that did not decode, or the error that ended the stream.
//!             Err(err) => eprintln!("{err}"),
//!         }
//!     }
//!     Ok(())
//! }
//! ```

mod auth;
mod client;
mod error;
mod market;
#[cfg(test)]
mod scripted;
mod subscription;
mod supervised;
mod user;

pub use auth::ApiCredentials;
pub use client::{MembershipHandle, WebSocket, WebSocketBuilder, WebSocketWithPing};
pub use error::ClobWsError;
pub use market::{
    BestBidAskMessage, BookMessage, LastTradePriceMessage, MarketMessage, MarketResolvedMessage,
    NewMarketMessage, OrderSummary, PriceChange, PriceChangeMessage, TickSizeChangeMessage,
};
pub use subscription::{
    ChannelType, MarketSubscription, MarketSubscriptionOptions, MarketSubscriptionUpdate,
    SubscriptionLevel, SubscriptionOperation, UserSubscription, UserSubscriptionUpdate,
    WS_MARKET_URL, WS_USER_URL,
};
pub use supervised::{
    ClobMarketProtocol, DisconnectReason, MarketChange, MarketEvent, MarketMembership,
    MarketWsBuilder, SupervisedMarketWs,
};
pub use user::{MakerOrder, OrderEventType, OrderMessage, TradeMessage, TradeStatus, UserMessage};

/// All possible WebSocket channel messages
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Channel {
    /// Market channel message
    Market(MarketMessage),
    /// User channel message
    User(UserMessage),
}
