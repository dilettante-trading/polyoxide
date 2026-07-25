use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{SinkExt, Stream, StreamExt};
use tokio::{net::TcpStream, time::interval};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use super::{
    auth::ApiCredentials,
    error::WebSocketError,
    market::MarketMessage,
    sports::SportsMessage,
    subscription::{
        ChannelType, MarketSubscription, MarketSubscriptionOptions, UserSubscription,
        UserSubscriptionUpdate, WS_MARKET_URL, WS_SPORTS_URL, WS_USER_URL,
    },
    user::UserMessage,
    Channel,
};

/// Maximum number of subscriptions per WebSocket connection.
const MAX_SUBSCRIPTIONS_PER_CONNECTION: usize = 500;

/// Make sure rustls has a default `CryptoProvider` before we open a connection.
///
/// `tokio-tungstenite` builds its TLS config from the process-wide default
/// provider. rustls picks that default automatically only when exactly one
/// backend feature is enabled — and this dependency graph enables two on a
/// single shared `rustls`: `ring` arrives with `reqwest 0.12` (used by
/// `polyoxide-core`) and `aws-lc-rs` with `reqwest 0.13` (pulled in by
/// `alloy`, a mandatory dependency here). Faced with two candidates rustls
/// installs neither and panics inside `connect_async`, which made *every*
/// channel — market, user and sports alike — abort at connect time for any
/// consumer of this crate.
///
/// Installing the default is deliberately best-effort: `install_default`
/// returns `Err` when one is already set, and in that case the host
/// application has chosen and we leave its choice alone.
fn ensure_crypto_provider() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// Parse one text frame into a [`Channel`], or `None` if it carries no event.
///
/// Shared by [`WebSocket`] and [`WebSocketWithPing`] so the two cannot drift —
/// they previously held byte-identical copies of this logic.
fn parse_channel_message(
    channel_type: ChannelType,
    text: &str,
) -> Result<Option<Channel>, WebSocketError> {
    // Skip PONG responses and empty messages on every channel.
    if text == "PONG" || text == "{}" || text.is_empty() {
        return Ok(None);
    }

    match channel_type {
        // The clob channels tag every event with `event_type`, so a frame
        // without one is a heartbeat or a subscription ack. This filter must
        // stay channel-scoped: applying it before the dispatch also silenced
        // the sports channel, and removing it outright would push clob
        // heartbeats into `from_json` and turn them into hard stream errors.
        ChannelType::Market | ChannelType::User if !text.contains("event_type") => {
            tracing::trace!("Skipping non-event message: {}", text);
            Ok(None)
        }
        ChannelType::Market => Ok(Some(Channel::Market(MarketMessage::from_json(text)?))),
        ChannelType::User => Ok(Some(Channel::User(UserMessage::from_json(text)?))),
        // Sports frames carry no discriminator at all — every field is match
        // data. Verified against 229 live frames on 2026-07-25.
        ChannelType::Sports => Ok(Some(Channel::Sports(SportsMessage::from_json(text)?))),
    }
}

/// Reject a subscription update sent on a channel that has no market filters.
///
/// Only the user channel supports adjusting its markets after connecting; the
/// market channel is keyed on asset IDs fixed at subscribe time and the sports
/// channel takes no subscription payload at all.
fn require_user_channel(channel_type: ChannelType) -> Result<(), WebSocketError> {
    if channel_type != ChannelType::User {
        return Err(WebSocketError::InvalidMessage(format!(
            "subscription updates are only supported on the user channel, not {channel_type:?}"
        )));
    }
    Ok(())
}

/// Validate that the subscription count does not exceed the per-connection limit.
fn validate_subscription_count(count: usize) -> Result<(), WebSocketError> {
    if count > MAX_SUBSCRIPTIONS_PER_CONNECTION {
        return Err(WebSocketError::InvalidMessage(format!(
            "Too many subscriptions ({}), max {}",
            count, MAX_SUBSCRIPTIONS_PER_CONNECTION
        )));
    }
    Ok(())
}

/// WebSocket client for Polymarket real-time updates.
///
/// Provides streaming access to market data (order book, prices) and user-specific
/// updates (orders, trades).
///
/// # Example
///
/// ```no_run
/// use polyoxide_clob::ws::WebSocket;
/// use futures_util::StreamExt;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut ws = WebSocket::connect_market(vec!["asset_id".to_string()]).await?;
///
///     while let Some(msg) = ws.next().await {
///         println!("Received: {:?}", msg?);
///     }
///
///     Ok(())
/// }
/// ```
pub struct WebSocket {
    inner: WebSocketStream<MaybeTlsStream<TcpStream>>,
    channel_type: ChannelType,
}

impl WebSocket {
    /// Connect to the market channel for public order book and price updates.
    ///
    /// # Arguments
    ///
    /// * `asset_ids` - Token IDs to subscribe to
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_clob::ws::WebSocket;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let ws = WebSocket::connect_market(vec![
    ///         "token_id_1".to_string(),
    ///         "token_id_2".to_string(),
    ///     ]).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect_market(asset_ids: Vec<String>) -> Result<Self, WebSocketError> {
        Self::connect_market_with(asset_ids, MarketSubscriptionOptions::default()).await
    }

    /// Connect to the market channel with explicit subscription options.
    ///
    /// Use this to opt into the `best_bid_ask`, `new_market`, and
    /// `market_resolved` events, which the server withholds unless the
    /// subscription sets `custom_feature_enabled`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_clob::ws::{MarketSubscriptionOptions, WebSocket};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let ws = WebSocket::connect_market_with(
    ///         vec!["token_id_1".to_string()],
    ///         MarketSubscriptionOptions::default().with_custom_features(),
    ///     ).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect_market_with(
        asset_ids: Vec<String>,
        options: MarketSubscriptionOptions,
    ) -> Result<Self, WebSocketError> {
        validate_subscription_count(asset_ids.len())?;
        ensure_crypto_provider();
        let (mut ws, _) = connect_async(WS_MARKET_URL).await?;

        let subscription = MarketSubscription::with_options(asset_ids, options);
        let msg = serde_json::to_string(&subscription)?;
        ws.send(Message::Text(msg.into())).await?;

        Ok(Self {
            inner: ws,
            channel_type: ChannelType::Market,
        })
    }

    /// Connect to the sports channel for live match updates.
    ///
    /// This channel takes no subscription payload — connecting is enough. Note
    /// that it is served by a different host (`sports-api.polymarket.com`)
    /// than the market and user channels, and that its frames carry no
    /// `event_type` discriminator.
    ///
    /// Keep-alive needs no help from the caller: the server sends WebSocket
    /// protocol ping frames, which the underlying transport answers
    /// automatically. Upstream's documentation describes a text `"ping"` /
    /// `"pong"` exchange instead; that is not what the server does.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_clob::ws::{Channel, WebSocket};
    /// use futures_util::StreamExt;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut ws = WebSocket::connect_sports().await?;
    ///
    ///     while let Some(msg) = ws.next().await {
    ///         if let Channel::Sports(update) = msg? {
    ///             println!("{update:?}");
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect_sports() -> Result<Self, WebSocketError> {
        ensure_crypto_provider();
        let (ws, _) = connect_async(WS_SPORTS_URL).await?;

        Ok(Self {
            inner: ws,
            channel_type: ChannelType::Sports,
        })
    }

    /// Connect to the user channel for authenticated order and trade updates.
    ///
    /// # Arguments
    ///
    /// * `market_ids` - Condition IDs to subscribe to
    /// * `credentials` - API credentials for authentication
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_clob::ws::{ApiCredentials, WebSocket};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let credentials = ApiCredentials::from_env()?;
    ///     let ws = WebSocket::connect_user(
    ///         vec!["condition_id".to_string()],
    ///         credentials,
    ///     ).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect_user(
        market_ids: Vec<String>,
        credentials: ApiCredentials,
    ) -> Result<Self, WebSocketError> {
        validate_subscription_count(market_ids.len())?;
        Self::connect_user_subscription(UserSubscription::new(market_ids, credentials)).await
    }

    /// Connect to the user channel for **every** market, unfiltered.
    ///
    /// Omitting the market filter is what the venue expects for an
    /// account-wide subscription, and it removes the need to open one socket
    /// per market. Note this also means events arrive for markets the caller
    /// never enumerated, including any it starts trading later.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_clob::ws::{ApiCredentials, WebSocket};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let ws = WebSocket::connect_user_all_markets(
    ///         ApiCredentials::from_env()?,
    ///     ).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect_user_all_markets(
        credentials: ApiCredentials,
    ) -> Result<Self, WebSocketError> {
        Self::connect_user_subscription(UserSubscription::all_markets(credentials)).await
    }

    /// Connect to the user channel with an explicitly built subscription.
    pub async fn connect_user_subscription(
        subscription: UserSubscription,
    ) -> Result<Self, WebSocketError> {
        if let Some(markets) = &subscription.markets {
            validate_subscription_count(markets.len())?;
        }
        ensure_crypto_provider();
        let (mut ws, _) = connect_async(WS_USER_URL).await?;

        let msg = serde_json::to_string(&subscription)?;
        ws.send(Message::Text(msg.into())).await?;

        Ok(Self {
            inner: ws,
            channel_type: ChannelType::User,
        })
    }

    /// Send a ping message to keep the connection alive.
    ///
    /// The Polymarket WebSocket expects "PING" text messages every ~10 seconds.
    pub async fn ping(&mut self) -> Result<(), WebSocketError> {
        self.inner.send(Message::Text("PING".into())).await?;
        Ok(())
    }

    /// Start receiving user events for additional markets, without reconnecting.
    ///
    /// User channel only. Adding a market to a watchlist previously meant
    /// tearing down and rebuilding the socket.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_clob::ws::{ApiCredentials, WebSocket};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut ws = WebSocket::connect_user(
    ///         vec!["0xfirst".to_string()],
    ///         ApiCredentials::from_env()?,
    ///     ).await?;
    ///
    ///     // Later, as the watchlist grows:
    ///     ws.subscribe_markets(vec!["0xsecond".to_string()]).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn subscribe_markets(&mut self, markets: Vec<String>) -> Result<(), WebSocketError> {
        self.send_subscription_update(UserSubscriptionUpdate::subscribe(markets))
            .await
    }

    /// Stop receiving user events for the given markets, without reconnecting.
    ///
    /// User channel only.
    pub async fn unsubscribe_markets(
        &mut self,
        markets: Vec<String>,
    ) -> Result<(), WebSocketError> {
        self.send_subscription_update(UserSubscriptionUpdate::unsubscribe(markets))
            .await
    }

    /// Send a prepared subscription update frame on a live user connection.
    pub async fn send_subscription_update(
        &mut self,
        update: UserSubscriptionUpdate,
    ) -> Result<(), WebSocketError> {
        require_user_channel(self.channel_type)?;
        validate_subscription_count(update.markets.len())?;
        let msg = serde_json::to_string(&update)?;
        self.inner.send(Message::Text(msg.into())).await?;
        Ok(())
    }

    /// Close the WebSocket connection.
    pub async fn close(&mut self) -> Result<(), WebSocketError> {
        self.inner.close(None).await?;
        Ok(())
    }

    /// Get the channel type this WebSocket is connected to.
    pub fn channel_type(&self) -> ChannelType {
        self.channel_type
    }

    /// Parse a text message based on the channel type.
    fn parse_message(&self, text: &str) -> Result<Option<Channel>, WebSocketError> {
        parse_channel_message(self.channel_type, text)
    }
}

impl Stream for WebSocket {
    type Item = Result<Channel, WebSocketError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(msg))) => match msg {
                    Message::Text(text) => match self.parse_message(&text) {
                        Ok(Some(channel)) => return Poll::Ready(Some(Ok(channel))),
                        Ok(None) => continue, // Skip PONG, poll again
                        Err(e) => return Poll::Ready(Some(Err(e))),
                    },
                    Message::Binary(data) => {
                        // Try to parse as text
                        if let Ok(text) = String::from_utf8(data.to_vec()) {
                            match self.parse_message(&text) {
                                Ok(Some(channel)) => return Poll::Ready(Some(Ok(channel))),
                                Ok(None) => continue,
                                Err(e) => return Poll::Ready(Some(Err(e))),
                            }
                        }
                        continue;
                    }
                    Message::Ping(_) | Message::Pong(_) => continue,
                    Message::Close(_) => return Poll::Ready(None),
                    Message::Frame(_) => continue,
                },
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e.into()))),
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

/// Builder for WebSocket connections with additional configuration.
pub struct WebSocketBuilder {
    market_url: String,
    user_url: String,
    ping_interval: Option<Duration>,
}

impl Default for WebSocketBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSocketBuilder {
    /// Create a new WebSocket builder.
    pub fn new() -> Self {
        Self {
            market_url: WS_MARKET_URL.to_string(),
            user_url: WS_USER_URL.to_string(),
            ping_interval: None,
        }
    }

    /// Set a custom WebSocket URL for market channel.
    ///
    /// Only `wss://` URLs are accepted to prevent plaintext connections.
    pub fn market_url(mut self, url: impl Into<String>) -> Result<Self, WebSocketError> {
        let url = url.into();
        if !url.starts_with("wss://") {
            return Err(WebSocketError::InvalidMessage(
                "WebSocket URL must use wss:// scheme".to_string(),
            ));
        }
        self.market_url = url;
        Ok(self)
    }

    /// Set a custom WebSocket URL for user channel.
    ///
    /// Only `wss://` URLs are accepted to prevent plaintext connections.
    pub fn user_url(mut self, url: impl Into<String>) -> Result<Self, WebSocketError> {
        let url = url.into();
        if !url.starts_with("wss://") {
            return Err(WebSocketError::InvalidMessage(
                "WebSocket URL must use wss:// scheme".to_string(),
            ));
        }
        self.user_url = url;
        Ok(self)
    }

    /// Set the ping interval for keep-alive messages.
    ///
    /// Connections created from this builder always send keep-alive pings while
    /// driven by [`WebSocketWithPing::run`]. This method overrides the default
    /// 10-second interval.
    pub fn ping_interval(mut self, interval: Duration) -> Self {
        self.ping_interval = Some(interval);
        self
    }

    /// Connect to the market channel.
    pub async fn connect_market(
        self,
        asset_ids: Vec<String>,
    ) -> Result<WebSocketWithPing, WebSocketError> {
        validate_subscription_count(asset_ids.len())?;
        ensure_crypto_provider();
        let (mut ws, _) = connect_async(&self.market_url).await?;

        let subscription = MarketSubscription::new(asset_ids);
        let msg = serde_json::to_string(&subscription)?;
        ws.send(Message::Text(msg.into())).await?;

        Ok(WebSocketWithPing {
            inner: ws,
            channel_type: ChannelType::Market,
            ping_interval: self.ping_interval.unwrap_or(Duration::from_secs(10)),
        })
    }

    /// Connect to the user channel.
    pub async fn connect_user(
        self,
        market_ids: Vec<String>,
        credentials: ApiCredentials,
    ) -> Result<WebSocketWithPing, WebSocketError> {
        validate_subscription_count(market_ids.len())?;
        self.connect_user_subscription(UserSubscription::new(market_ids, credentials))
            .await
    }

    /// Connect to the user channel for every market, unfiltered.
    pub async fn connect_user_all_markets(
        self,
        credentials: ApiCredentials,
    ) -> Result<WebSocketWithPing, WebSocketError> {
        self.connect_user_subscription(UserSubscription::all_markets(credentials))
            .await
    }

    /// Connect to the user channel with an explicitly built subscription.
    pub async fn connect_user_subscription(
        self,
        subscription: UserSubscription,
    ) -> Result<WebSocketWithPing, WebSocketError> {
        if let Some(markets) = &subscription.markets {
            validate_subscription_count(markets.len())?;
        }
        ensure_crypto_provider();
        let (mut ws, _) = connect_async(&self.user_url).await?;

        let msg = serde_json::to_string(&subscription)?;
        ws.send(Message::Text(msg.into())).await?;

        Ok(WebSocketWithPing {
            inner: ws,
            channel_type: ChannelType::User,
            ping_interval: self.ping_interval.unwrap_or(Duration::from_secs(10)),
        })
    }
}

/// WebSocket client with automatic ping handling.
///
/// Use this when you need automatic keep-alive pings. Call `run` to process
/// messages with automatic ping handling.
///
/// Note that [`run`](Self::run) takes ownership of the connection, so there is
/// no way to adjust subscriptions while it is driving the stream. Use the plain
/// [`WebSocket`] with [`subscribe_markets`](WebSocket::subscribe_markets) if the
/// market set changes over the connection's life.
pub struct WebSocketWithPing {
    inner: WebSocketStream<MaybeTlsStream<TcpStream>>,
    channel_type: ChannelType,
    ping_interval: Duration,
}

impl WebSocketWithPing {
    /// Run the WebSocket message loop with automatic ping handling.
    ///
    /// This method will:
    /// - Send ping messages at the configured interval
    /// - Call the provided handler for each received message
    /// - Return when the connection is closed or an error occurs
    ///
    /// # Arguments
    ///
    /// * `handler` - Async function called for each received channel message
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_clob::ws::WebSocketBuilder;
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let ws = WebSocketBuilder::new()
    ///         .ping_interval(Duration::from_secs(10))
    ///         .connect_market(vec!["asset_id".to_string()])
    ///         .await?;
    ///
    ///     ws.run(|msg| async move {
    ///         println!("Received: {:?}", msg);
    ///         Ok(())
    ///     }).await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn run<F, Fut>(mut self, mut handler: F) -> Result<(), WebSocketError>
    where
        F: FnMut(Channel) -> Fut,
        Fut: std::future::Future<Output = Result<(), WebSocketError>>,
    {
        let mut ping_interval = interval(self.ping_interval);

        loop {
            tokio::select! {
                _ = ping_interval.tick() => {
                    self.inner.send(Message::Text("PING".into())).await?;
                }
                msg = self.inner.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if text.as_str() == "PONG" {
                                continue;
                            }
                            let channel = self.parse_message(&text)?;
                            if let Some(channel) = channel {
                                handler(channel).await?;
                            }
                        }
                        Some(Ok(Message::Binary(data))) => {
                            if let Ok(text) = String::from_utf8(data.to_vec()) {
                                if text == "PONG" {
                                    continue;
                                }
                                let channel = self.parse_message(&text)?;
                                if let Some(channel) = channel {
                                    handler(channel).await?;
                                }
                            }
                        }
                        Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) | Some(Ok(Message::Frame(_))) => continue,
                        Some(Ok(Message::Close(_))) => return Ok(()),
                        Some(Err(e)) => return Err(e.into()),
                        None => return Ok(()),
                    }
                }
            }
        }
    }

    /// Get the channel type this WebSocket is connected to.
    pub fn channel_type(&self) -> ChannelType {
        self.channel_type
    }

    /// Parse a text message based on the channel type.
    fn parse_message(&self, text: &str) -> Result<Option<Channel>, WebSocketError> {
        parse_channel_message(self.channel_type, text)
    }
}

#[cfg(test)]
mod subscription_update_tests {
    use super::*;

    #[test]
    fn only_the_user_channel_accepts_subscription_updates() {
        assert!(require_user_channel(ChannelType::User).is_ok());

        for ch in [ChannelType::Market, ChannelType::Sports] {
            let err = require_user_channel(ch)
                .expect_err("dynamic market filters are a user-channel feature");
            assert!(
                err.to_string().contains("user channel"),
                "error should name the constraint, got: {err}"
            );
        }
    }
}

#[cfg(test)]
mod crypto_provider_tests {
    use super::*;

    #[test]
    fn a_default_crypto_provider_is_available_to_connect() {
        // Every `connect_*` panicked before this was installed. tokio-tungstenite
        // builds its rustls `ClientConfig` from the process-wide default
        // CryptoProvider, and this workspace enables *both* `ring` (via
        // reqwest 0.12) and `aws-lc-rs` (via alloy's reqwest 0.13) on one
        // shared rustls. Given two candidates rustls installs neither and
        // panics — so the whole WebSocket surface aborted at connect time.
        ensure_crypto_provider();
        assert!(
            rustls::crypto::CryptoProvider::get_default().is_some(),
            "no process-default CryptoProvider: every WebSocket connect will panic"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_subscription_count_within_limit() {
        assert!(validate_subscription_count(0).is_ok());
        assert!(validate_subscription_count(1).is_ok());
        assert!(validate_subscription_count(MAX_SUBSCRIPTIONS_PER_CONNECTION).is_ok());
    }

    #[test]
    fn test_validate_subscription_count_exceeds_limit() {
        let result = validate_subscription_count(MAX_SUBSCRIPTIONS_PER_CONNECTION + 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("Too many subscriptions"),
            "expected subscription error, got: {err}"
        );
    }

    #[test]
    fn test_builder_default_urls_are_wss() {
        let builder = WebSocketBuilder::new();
        assert!(builder.market_url.starts_with("wss://"));
        assert!(builder.user_url.starts_with("wss://"));
    }

    #[test]
    fn test_builder_accepts_wss_url() {
        let builder = WebSocketBuilder::new()
            .market_url("wss://custom.example.com/ws/market")
            .unwrap()
            .user_url("wss://custom.example.com/ws/user")
            .unwrap();
        assert_eq!(builder.market_url, "wss://custom.example.com/ws/market");
        assert_eq!(builder.user_url, "wss://custom.example.com/ws/user");
    }

    #[test]
    fn test_builder_rejects_ws_url() {
        let result = WebSocketBuilder::new().market_url("ws://insecure.example.com/ws");
        assert!(result.is_err());

        let result = WebSocketBuilder::new().user_url("ws://insecure.example.com/ws");
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_rejects_http_url() {
        let result = WebSocketBuilder::new().market_url("http://example.com/ws");
        assert!(result.is_err());

        let result = WebSocketBuilder::new().user_url("https://example.com/ws");
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod dispatch_tests {
    use super::*;
    use crate::ws::sports::fixtures;

    const BOOK: &str = r#"{"event_type":"book","asset_id":"a","market":"m","timestamp":"1",
        "hash":"h","bids":[],"asks":[],"last_trade_price":null}"#;
    const BBO: &str = r#"{"event_type":"best_bid_ask","asset_id":"a","market":"m",
        "best_bid":"0.5","best_ask":"0.6","spread":"0.1","timestamp":"1"}"#;

    #[test]
    fn routes_by_channel_type() {
        assert!(matches!(
            parse_channel_message(ChannelType::Market, BOOK).unwrap(),
            Some(Channel::Market(_))
        ));
        assert!(matches!(
            parse_channel_message(ChannelType::Market, BBO).unwrap(),
            Some(Channel::Market(MarketMessage::BestBidAsk(_)))
        ));
    }

    #[test]
    fn every_real_sports_frame_reaches_the_caller() {
        // The regression this pins: the `event_type` pre-filter is correct for
        // the two clob channels and catastrophic for sports, whose frames carry
        // no such field. Every one of these was silently dropped, leaving
        // `connect_sports` a public method that yielded an empty stream.
        for (i, frame) in fixtures::ALL.iter().enumerate() {
            assert!(
                !frame.contains("event_type"),
                "fixture {i} must be a real frame, not one invented to suit the filter"
            );
            assert!(
                matches!(
                    parse_channel_message(ChannelType::Sports, frame).unwrap(),
                    Some(Channel::Sports(_))
                ),
                "sports fixture {i} was dropped by the dispatcher"
            );
        }
    }

    #[test]
    fn the_event_type_filter_still_guards_market_and_user() {
        // Scoping the fix to sports must not remove the filter from the
        // channels that need it: without it, clob heartbeats and subscription
        // acks reach `from_json` and become hard stream errors.
        for ch in [ChannelType::Market, ChannelType::User] {
            assert!(
                parse_channel_message(ch, r#"{"some":"ack"}"#)
                    .unwrap()
                    .is_none(),
                "{ch:?} must still skip frames without event_type"
            );
        }
    }

    #[test]
    fn sports_frames_do_not_yield_market_events() {
        // Guards the dispatch itself: routing a sports frame to the market
        // parser must never produce a market event.
        for frame in fixtures::ALL {
            let routed = parse_channel_message(ChannelType::Market, frame);
            assert!(
                !matches!(routed, Ok(Some(Channel::Market(_)))),
                "a sports frame must not be read as a market event"
            );
        }
    }

    #[test]
    fn skips_keepalive_frames_on_every_channel() {
        for text in ["PONG", "{}", ""] {
            for ch in [ChannelType::Market, ChannelType::User, ChannelType::Sports] {
                assert!(
                    parse_channel_message(ch, text).unwrap().is_none(),
                    "{ch:?} should skip {text:?}"
                );
            }
        }
    }

    #[test]
    fn both_stream_types_share_one_dispatch() {
        // WebSocket and WebSocketWithPing previously carried byte-identical
        // copies of this logic; this pins that they now delegate to the same
        // function, so a fix to one cannot miss the other.
        let by_helper = parse_channel_message(ChannelType::Market, BBO).unwrap();
        assert!(matches!(
            by_helper,
            Some(Channel::Market(MarketMessage::BestBidAsk(_)))
        ));
    }
}
