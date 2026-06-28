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
    subscription::{ChannelType, MarketSubscription, UserSubscription, WS_MARKET_URL, WS_USER_URL},
    user::UserMessage,
    Channel,
};

/// Maximum number of subscriptions per WebSocket connection.
const MAX_SUBSCRIPTIONS_PER_CONNECTION: usize = 500;

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
        validate_subscription_count(asset_ids.len())?;
        let (mut ws, _) = connect_async(WS_MARKET_URL).await?;

        let subscription = MarketSubscription::new(asset_ids);
        let msg = serde_json::to_string(&subscription)?;
        ws.send(Message::Text(msg.into())).await?;

        Ok(Self {
            inner: ws,
            channel_type: ChannelType::Market,
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
        let (mut ws, _) = connect_async(WS_USER_URL).await?;

        let subscription = UserSubscription::new(market_ids, credentials);
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
        // Skip PONG responses and empty messages
        if text == "PONG" || text == "{}" || text.is_empty() {
            return Ok(None);
        }

        // Skip messages without event_type (heartbeats, acks, etc.)
        if !text.contains("event_type") {
            tracing::trace!("Skipping non-event message: {}", text);
            return Ok(None);
        }

        match self.channel_type {
            ChannelType::Market => {
                let msg = MarketMessage::from_json(text)?;
                Ok(Some(Channel::Market(msg)))
            }
            ChannelType::User => {
                let msg = UserMessage::from_json(text)?;
                Ok(Some(Channel::User(msg)))
            }
        }
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
        let (mut ws, _) = connect_async(&self.user_url).await?;

        let subscription = UserSubscription::new(market_ids, credentials);
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
        // Skip PONG responses and empty messages
        if text == "PONG" || text == "{}" || text.is_empty() {
            return Ok(None);
        }

        // Skip messages without event_type (heartbeats, acks, etc.)
        if !text.contains("event_type") {
            tracing::trace!("Skipping non-event message: {}", text);
            return Ok(None);
        }

        match self.channel_type {
            ChannelType::Market => {
                let msg = MarketMessage::from_json(text)?;
                Ok(Some(Channel::Market(msg)))
            }
            ChannelType::User => {
                let msg = UserMessage::from_json(text)?;
                Ok(Some(Channel::User(msg)))
            }
        }
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
