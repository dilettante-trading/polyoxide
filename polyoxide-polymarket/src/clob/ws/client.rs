use std::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use polyoxide_ws::{
    connect_over,
    tungstenite::{self, Message},
    with_timeout, Connection, Kind, Mode, WsError,
};
use tokio::net::TcpStream;

use super::{
    auth::ApiCredentials,
    error::ClobWsError,
    market::MarketMessage,
    subscription::{
        ChannelType, MarketSubscription, MarketSubscriptionOptions, MarketSubscriptionUpdate,
        UserSubscription, UserSubscriptionUpdate, WS_MARKET_URL, WS_USER_URL,
    },
    user::UserMessage,
    Channel,
};

/// Maximum number of subscriptions per WebSocket connection.
const MAX_SUBSCRIPTIONS_PER_CONNECTION: usize = 500;

/// Outbound subscription-update frames queued for a [`WebSocketWithPing`]'s
/// run loop. Small on purpose: a caller that outruns the socket should feel
/// back-pressure rather than buffer membership changes unboundedly.
const OUTBOUND_QUEUE_DEPTH: usize = 64;

/// Default per-address connect timeout, overridable via
/// [`WebSocketBuilder::connect_timeout`].
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Interleave IPv6 and IPv4 addresses, Happy-Eyeballs style (RFC 8305 §4).
///
/// `lookup_host` typically returns all AAAA records before all A records, so a
/// network with broken IPv6 would burn one full timeout per v6 address before
/// ever trying v4. Alternating families bounds the damage to a single timeout.
fn interleave_address_families(addrs: Vec<SocketAddr>) -> Vec<SocketAddr> {
    let (v6, v4): (Vec<_>, Vec<_>) = addrs.into_iter().partition(SocketAddr::is_ipv6);
    let mut out = Vec::with_capacity(v6.len() + v4.len());
    let (mut v6, mut v4) = (v6.into_iter(), v4.into_iter());
    loop {
        match (v6.next(), v4.next()) {
            (None, None) => return out,
            (a, b) => out.extend(a.into_iter().chain(b)),
        }
    }
}

/// An I/O failure while connecting, as the kit reports a transport's.
fn io_error(err: std::io::Error) -> ClobWsError {
    WsError::from(tungstenite::Error::Io(err)).into()
}

/// A host that resolved to no address: a DNS miss, so a `Network` failure
/// like any other I/O error, not a refusal of the request.
fn no_addresses(url: &str) -> ClobWsError {
    io_error(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("no addresses resolved for {url}"),
    ))
}

/// Open a TLS WebSocket connection with a per-address timeout and fallback,
/// read through the kit's [`Connection`] in [`Mode::Data`].
///
/// [`polyoxide_ws::connect`] resolves DNS and connects to only the *first*
/// address — a blackholed route (the classic broken-IPv6 case) burns the
/// whole timeout there. This resolves every address up front, tries each in
/// family-interleaved order with `connect_timeout` covering its TCP connect
/// and again its TLS + WebSocket handshake ([`with_timeout`] around each,
/// the upgrade through [`connect_over`]), and falls back to the next address
/// on any failure but a refused upgrade, which it returns at once. This
/// connect is clob's own (divergence row D20).
async fn connect_ws(url: &str, connect_timeout: Duration) -> Result<Connection, ClobWsError> {
    let parsed = url::Url::parse(url)?;
    let host = parsed
        .host_str()
        .ok_or_else(|| ClobWsError::InvalidMessage(format!("URL has no host: {url}")))?;
    let port = parsed.port().unwrap_or(443);

    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(io_error)?
        .collect();

    // A step that outlived its bound is clob's own `ConnectTimeout`, which
    // names the URL and says the timeout is per address and per step.
    let timed_out = |err: WsError| match err.kind() {
        Kind::ConnectTimeout { .. } => ClobWsError::ConnectTimeout {
            url: url.to_string(),
            timeout: connect_timeout,
        },
        _ => ClobWsError::Connection(err),
    };
    let mut last_err = no_addresses(url);
    for addr in interleave_address_families(addrs) {
        let stream = match with_timeout(connect_timeout, TcpStream::connect(addr)).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                last_err = io_error(e);
                continue;
            }
            Err(err) => {
                last_err = timed_out(err);
                continue;
            }
        };
        // TCP is up; bound the TLS + WebSocket handshake by the same budget.
        match with_timeout(connect_timeout, connect_over(url, stream)).await {
            Ok(Ok(socket)) => return Ok(Connection::new(socket, Mode::Data)),
            // The host answered and refused the upgrade. Another of its
            // addresses would repeat the refusal, inside a 429's window, and
            // its own failure would hide this one's status and `Retry-After`.
            Ok(Err(e)) if matches!(e.kind(), Kind::Refused { .. }) => return Err(e.into()),
            Ok(Err(e)) => last_err = e.into(),
            Err(err) => last_err = timed_out(err),
        }
    }
    Err(last_err)
}

/// The text a data frame carries. A Binary frame is decoded as text (D12),
/// since the venue has sent events that way; one that is not UTF-8 carries
/// none, and is skipped.
fn frame_text(frame: &Message) -> Option<&str> {
    match frame {
        Message::Text(text) => Some(text.as_str()),
        Message::Binary(data) => std::str::from_utf8(data).ok(),
        // `Mode::Data` yields only Text and Binary frames.
        _ => None,
    }
}

/// Parse one text frame into a [`Channel`], or `None` if it carries no event.
///
/// Shared by [`WebSocket`] and [`WebSocketWithPing`] so the two cannot drift —
/// they previously held byte-identical copies of this logic.
fn parse_channel_message(
    channel_type: ChannelType,
    text: &str,
) -> Result<Option<Channel>, ClobWsError> {
    // Skip PONG responses and empty messages on every channel.
    if text == "PONG" || text == "{}" || text.is_empty() {
        return Ok(None);
    }

    match channel_type {
        // Both channels tag every event with `event_type`, so a frame
        // without one is a heartbeat or a subscription ack. Removing this
        // filter would push heartbeats into `from_json` and turn them into
        // hard stream errors.
        ChannelType::Market | ChannelType::User if !text.contains("event_type") => {
            tracing::trace!("Skipping non-event message: {}", text);
            Ok(None)
        }
        ChannelType::Market => Ok(Some(Channel::Market(MarketMessage::from_json(text)?))),
        ChannelType::User => Ok(Some(Channel::User(UserMessage::from_json(text)?))),
    }
}

/// Refuse a subscription-update frame on the wrong channel.
///
/// Each channel has its own update frame — the market channel's is keyed on
/// asset IDs ([`MarketSubscriptionUpdate`]), the user channel's on condition
/// IDs ([`UserSubscriptionUpdate`]).
fn require_channel(actual: ChannelType, expected: ChannelType) -> Result<(), ClobWsError> {
    if actual != expected {
        return Err(ClobWsError::InvalidMessage(format!(
            "subscription updates for the {expected:?} channel cannot be sent on the {actual:?} channel"
        )));
    }
    Ok(())
}

/// Gate, count-check and serialise one market-channel update frame.
///
/// Shared by every sender of a [`MarketSubscriptionUpdate`] — [`WebSocket`],
/// which writes the frame straight to its socket, and [`MembershipHandle`],
/// which queues it for a run loop — so the three steps cannot drift apart.
fn market_update_frame(
    channel: ChannelType,
    update: &MarketSubscriptionUpdate,
) -> Result<String, ClobWsError> {
    require_channel(channel, ChannelType::Market)?;
    validate_subscription_count(update.assets_ids.len())?;
    Ok(serde_json::to_string(update)?)
}

/// Validate that the subscription count does not exceed the per-connection limit.
fn validate_subscription_count(count: usize) -> Result<(), ClobWsError> {
    if count > MAX_SUBSCRIPTIONS_PER_CONNECTION {
        return Err(ClobWsError::InvalidMessage(format!(
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
/// The stream ends when the server closes the connection, after answering its
/// close (RFC 6455), and after the first transport error, which it yields.
///
/// # Example
///
/// ```no_run
/// use polyoxide_polymarket::clob::ws::WebSocket;
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
    inner: Connection,
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
    /// use polyoxide_polymarket::clob::ws::WebSocket;
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
    pub async fn connect_market(asset_ids: Vec<String>) -> Result<Self, ClobWsError> {
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
    /// use polyoxide_polymarket::clob::ws::{MarketSubscriptionOptions, WebSocket};
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
    ) -> Result<Self, ClobWsError> {
        validate_subscription_count(asset_ids.len())?;
        let mut ws = connect_ws(WS_MARKET_URL, DEFAULT_CONNECT_TIMEOUT).await?;

        let subscription = MarketSubscription::with_options(asset_ids, options);
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
    /// use polyoxide_polymarket::clob::ws::{ApiCredentials, WebSocket};
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
    ) -> Result<Self, ClobWsError> {
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
    /// use polyoxide_polymarket::clob::ws::{ApiCredentials, WebSocket};
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
    ) -> Result<Self, ClobWsError> {
        Self::connect_user_subscription(UserSubscription::all_markets(credentials)).await
    }

    /// Connect to the user channel with an explicitly built subscription.
    pub async fn connect_user_subscription(
        subscription: UserSubscription,
    ) -> Result<Self, ClobWsError> {
        if let Some(markets) = &subscription.markets {
            validate_subscription_count(markets.len())?;
        }
        let mut ws = connect_ws(WS_USER_URL, DEFAULT_CONNECT_TIMEOUT).await?;

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
    pub async fn ping(&mut self) -> Result<(), ClobWsError> {
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
    /// use polyoxide_polymarket::clob::ws::{ApiCredentials, WebSocket};
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
    pub async fn subscribe_markets(&mut self, markets: Vec<String>) -> Result<(), ClobWsError> {
        self.send_subscription_update(UserSubscriptionUpdate::subscribe(markets))
            .await
    }

    /// Stop receiving user events for the given markets, without reconnecting.
    ///
    /// User channel only.
    pub async fn unsubscribe_markets(&mut self, markets: Vec<String>) -> Result<(), ClobWsError> {
        self.send_subscription_update(UserSubscriptionUpdate::unsubscribe(markets))
            .await
    }

    /// Send a prepared subscription update frame on a live user connection.
    pub async fn send_subscription_update(
        &mut self,
        update: UserSubscriptionUpdate,
    ) -> Result<(), ClobWsError> {
        require_channel(self.channel_type, ChannelType::User)?;
        validate_subscription_count(update.markets.len())?;
        let msg = serde_json::to_string(&update)?;
        self.inner.send(Message::Text(msg.into())).await?;
        Ok(())
    }

    /// Start receiving market events for additional assets, without
    /// reconnecting.
    ///
    /// The venue answers with a `book` snapshot for each newly added asset
    /// (~155 ms); an asset already on the socket gets nothing — to force a
    /// snapshot for one, [`unsubscribe_assets`](Self::unsubscribe_assets) it
    /// first (see [`MarketSubscriptionUpdate`]).
    ///
    /// Market channel only.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_polymarket::clob::ws::WebSocket;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut ws = WebSocket::connect_market(vec!["111".to_string()]).await?;
    ///     ws.subscribe_assets(vec!["222".to_string()]).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn subscribe_assets(&mut self, assets: Vec<String>) -> Result<(), ClobWsError> {
        self.send_market_update(MarketSubscriptionUpdate::subscribe(assets))
            .await
    }

    /// Stop receiving market events for the given assets, without reconnecting.
    ///
    /// Market channel only.
    pub async fn unsubscribe_assets(&mut self, assets: Vec<String>) -> Result<(), ClobWsError> {
        self.send_market_update(MarketSubscriptionUpdate::unsubscribe(assets))
            .await
    }

    /// Send a prepared subscription update frame on a live market connection.
    pub async fn send_market_update(
        &mut self,
        update: MarketSubscriptionUpdate,
    ) -> Result<(), ClobWsError> {
        let msg = market_update_frame(self.channel_type, &update)?;
        self.inner.send(Message::Text(msg.into())).await?;
        Ok(())
    }

    /// Close the WebSocket connection. After the server closed it, this only
    /// makes sure the close reply went out, and succeeds.
    pub async fn close(&mut self) -> Result<(), ClobWsError> {
        self.inner.close().await?;
        Ok(())
    }

    /// Get the channel type this WebSocket is connected to.
    pub fn channel_type(&self) -> ChannelType {
        self.channel_type
    }

    /// Parse a text message based on the channel type.
    fn parse_message(&self, text: &str) -> Result<Option<Channel>, ClobWsError> {
        parse_channel_message(self.channel_type, text)
    }
}

impl Stream for WebSocket {
    type Item = Result<Channel, ClobWsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            // The connection yields only data frames, answers the server's
            // pings as it reads, and ends after the server's close, whose
            // reply it has sent (DRIFT R3), or after its first error.
            let frame = match self.inner.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(frame))) => frame,
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e.into()))),
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            };
            let Some(text) = frame_text(&frame) else {
                continue;
            };
            match self.parse_message(text) {
                Ok(Some(channel)) => return Poll::Ready(Some(Ok(channel))),
                Ok(None) => continue, // Skip PONG, poll again
                Err(e) => return Poll::Ready(Some(Err(e))),
            }
        }
    }
}

/// Builder for WebSocket connections with additional configuration.
pub struct WebSocketBuilder {
    market_url: String,
    user_url: String,
    ping_interval: Option<Duration>,
    connect_timeout: Duration,
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
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
        }
    }

    /// Set a custom WebSocket URL for market channel.
    ///
    /// Only `wss://` URLs are accepted to prevent plaintext connections.
    pub fn market_url(mut self, url: impl Into<String>) -> Result<Self, ClobWsError> {
        let url = url.into();
        if !url.starts_with("wss://") {
            return Err(ClobWsError::InvalidMessage(
                "WebSocket URL must use wss:// scheme".to_string(),
            ));
        }
        self.market_url = url;
        Ok(self)
    }

    /// Set a custom WebSocket URL for user channel.
    ///
    /// Only `wss://` URLs are accepted to prevent plaintext connections.
    pub fn user_url(mut self, url: impl Into<String>) -> Result<Self, ClobWsError> {
        let url = url.into();
        if !url.starts_with("wss://") {
            return Err(ClobWsError::InvalidMessage(
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
    /// 10-second interval. The interval is raised to at least 1 ms, and
    /// `Duration::MAX` sends only the ping that opens the run.
    pub fn ping_interval(mut self, interval: Duration) -> Self {
        self.ping_interval = Some(interval);
        self
    }

    /// Set the per-address connect timeout (default 10 seconds).
    ///
    /// Connecting resolves every address for the host and tries each in turn,
    /// IPv6 and IPv4 interleaved; this timeout bounds each address's TCP
    /// connect and, separately, its TLS + WebSocket handshake before falling
    /// back to the next address.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Connect to the market channel.
    pub async fn connect_market(
        self,
        asset_ids: Vec<String>,
    ) -> Result<WebSocketWithPing, ClobWsError> {
        validate_subscription_count(asset_ids.len())?;
        let mut ws = connect_ws(&self.market_url, self.connect_timeout).await?;

        let subscription = MarketSubscription::new(asset_ids);
        let msg = serde_json::to_string(&subscription)?;
        ws.send(Message::Text(msg.into())).await?;

        Ok(WebSocketWithPing::new(
            ws,
            ChannelType::Market,
            self.ping_interval.unwrap_or(Duration::from_secs(10)),
        ))
    }

    /// Connect to the user channel.
    pub async fn connect_user(
        self,
        market_ids: Vec<String>,
        credentials: ApiCredentials,
    ) -> Result<WebSocketWithPing, ClobWsError> {
        validate_subscription_count(market_ids.len())?;
        self.connect_user_subscription(UserSubscription::new(market_ids, credentials))
            .await
    }

    /// Connect to the user channel for every market, unfiltered.
    pub async fn connect_user_all_markets(
        self,
        credentials: ApiCredentials,
    ) -> Result<WebSocketWithPing, ClobWsError> {
        self.connect_user_subscription(UserSubscription::all_markets(credentials))
            .await
    }

    /// Connect to the user channel with an explicitly built subscription.
    pub async fn connect_user_subscription(
        self,
        subscription: UserSubscription,
    ) -> Result<WebSocketWithPing, ClobWsError> {
        if let Some(markets) = &subscription.markets {
            validate_subscription_count(markets.len())?;
        }
        let mut ws = connect_ws(&self.user_url, self.connect_timeout).await?;

        let msg = serde_json::to_string(&subscription)?;
        ws.send(Message::Text(msg.into())).await?;

        Ok(WebSocketWithPing::new(
            ws,
            ChannelType::User,
            self.ping_interval.unwrap_or(Duration::from_secs(10)),
        ))
    }
}

/// WebSocket client with automatic ping handling.
///
/// Use this when you need automatic keep-alive pings. Call `run` to process
/// messages with automatic ping handling.
///
/// [`run`](Self::run) takes ownership of the connection; take a
/// [`MembershipHandle`] via [`membership`](Self::membership) **before** calling
/// it to change a market subscription while the loop is driving the stream.
pub struct WebSocketWithPing {
    inner: Connection,
    channel_type: ChannelType,
    ping_interval: Duration,
    outbound_tx: tokio::sync::mpsc::Sender<String>,
    outbound_rx: tokio::sync::mpsc::Receiver<String>,
}

/// Sends market-channel subscription updates on a connection that
/// [`WebSocketWithPing::run`] is driving.
///
/// Cloneable and cheap; obtained from [`WebSocketWithPing::membership`]. A
/// successful call means the frame was **queued** for the run loop, which
/// sends it in order with everything queued before it. A frame the socket
/// refuses ends `run` with that error, exactly like a failed keep-alive ping.
/// Once `run` has returned, every call fails with
/// [`ClobWsError::MembershipClosed`] — never hangs.
///
/// The queue is bounded; a call awaits until the run loop has drained room.
/// Never await a handle send from inside the handler passed to
/// [`WebSocketWithPing::run`] — the handler would be waiting on the loop that
/// is waiting on the handler. Frames still queued when `run` returns are
/// dropped with the connection: on reconnect, re-send the full desired
/// membership from your own state rather than replaying a log of updates.
#[derive(Clone, Debug)]
pub struct MembershipHandle {
    tx: tokio::sync::mpsc::Sender<String>,
    channel_type: ChannelType,
}

impl MembershipHandle {
    /// Start receiving market events for additional assets. The venue answers
    /// with a `book` snapshot for each newly added asset; an asset already on
    /// the socket gets nothing — to force a snapshot for one,
    /// [`unsubscribe_assets`](Self::unsubscribe_assets) it first (see
    /// [`MarketSubscriptionUpdate`]).
    pub async fn subscribe_assets(&self, assets: Vec<String>) -> Result<(), ClobWsError> {
        self.send_market_update(MarketSubscriptionUpdate::subscribe(assets))
            .await
    }

    /// Stop receiving market events for the given assets.
    pub async fn unsubscribe_assets(&self, assets: Vec<String>) -> Result<(), ClobWsError> {
        self.send_market_update(MarketSubscriptionUpdate::unsubscribe(assets))
            .await
    }

    /// Queue a prepared market-channel update frame.
    pub async fn send_market_update(
        &self,
        update: MarketSubscriptionUpdate,
    ) -> Result<(), ClobWsError> {
        let msg = market_update_frame(self.channel_type, &update)?;
        self.tx
            .send(msg)
            .await
            .map_err(|_| ClobWsError::MembershipClosed)
    }
}

impl WebSocketWithPing {
    fn new(inner: Connection, channel_type: ChannelType, ping_interval: Duration) -> Self {
        let (outbound_tx, outbound_rx) = tokio::sync::mpsc::channel(OUTBOUND_QUEUE_DEPTH);
        Self {
            inner,
            channel_type,
            ping_interval,
            outbound_tx,
            outbound_rx,
        }
    }

    /// A handle for changing this connection's market subscription while
    /// [`run`](Self::run) drives it. Take it before calling `run`.
    ///
    /// Market channel only: a handle taken from a user-channel connection
    /// returns [`ClobWsError::InvalidMessage`] on every send.
    pub fn membership(&self) -> MembershipHandle {
        MembershipHandle {
            tx: self.outbound_tx.clone(),
            channel_type: self.channel_type,
        }
    }

    /// Run the WebSocket message loop with automatic ping handling.
    ///
    /// This method will:
    /// - Send ping messages at the configured interval
    /// - Send any subscription update queued through a [`MembershipHandle`]
    /// - Call the provided handler for each received message
    /// - Return `Ok` when the server closes the connection, after answering
    ///   its close (RFC 6455), and the error when one occurs
    ///
    /// # Arguments
    ///
    /// * `handler` - Async function called for each received channel message
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_polymarket::clob::ws::WebSocketBuilder;
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let ws = WebSocketBuilder::new()
    ///         .ping_interval(Duration::from_secs(10))
    ///         .connect_market(vec!["asset_id".to_string()])
    ///         .await?;
    ///     let membership = ws.membership();
    ///
    ///     tokio::spawn(ws.run(|msg| async move {
    ///         println!("Received: {:?}", msg);
    ///         Ok(())
    ///     }));
    ///
    ///     // Later, without reconnecting:
    ///     membership.subscribe_assets(vec!["another_asset_id".to_string()]).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn run<F, Fut>(mut self, mut handler: F) -> Result<(), ClobWsError>
    where
        F: FnMut(Channel) -> Fut,
        Fut: std::future::Future<Output = Result<(), ClobWsError>>,
    {
        // Not `interval`: it panics on a zero period, and on `Duration::MAX`
        // once a tick is late. `sleep` falls back to a far-future deadline.
        let period = self.ping_interval.max(Duration::from_millis(1));
        // The first ping goes out at once.
        let ping = tokio::time::sleep(Duration::ZERO);
        tokio::pin!(ping);

        loop {
            tokio::select! {
                () = &mut ping => {
                    ping.set(tokio::time::sleep(period));
                    self.inner.send(Message::Text("PING".into())).await?;
                }
                // `self` holds a sender, so this arm never sees `None`.
                Some(frame) = self.outbound_rx.recv() => {
                    self.inner.send(Message::Text(frame.into())).await?;
                }
                msg = self.inner.next() => {
                    match msg {
                        Some(Ok(frame)) => {
                            let Some(text) = frame_text(&frame) else {
                                continue;
                            };
                            if text == "PONG" {
                                continue;
                            }
                            let channel = self.parse_message(text)?;
                            if let Some(channel) = channel {
                                handler(channel).await?;
                            }
                        }
                        Some(Err(e)) => return Err(e.into()),
                        // The server closed, and its close is answered
                        // (DRIFT R3), or the stream ended.
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
    fn parse_message(&self, text: &str) -> Result<Option<Channel>, ClobWsError> {
        parse_channel_message(self.channel_type, text)
    }
}

#[cfg(test)]
mod connect_tests {
    use super::*;

    fn addr(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn require_channel_rejects_the_other_channels() {
        assert!(require_channel(ChannelType::Market, ChannelType::Market).is_ok());
        assert!(require_channel(ChannelType::User, ChannelType::User).is_ok());
        let err = require_channel(ChannelType::User, ChannelType::Market).unwrap_err();
        assert!(
            matches!(err, ClobWsError::InvalidMessage(ref m) if m.contains("Market") && m.contains("User")),
            "the error names both channels: {err}"
        );
        let err = require_channel(ChannelType::Market, ChannelType::User).unwrap_err();
        assert!(
            matches!(err, ClobWsError::InvalidMessage(ref m) if m.contains("User") && m.contains("Market")),
            "the error names both channels: {err}"
        );
    }

    #[test]
    fn address_families_are_interleaved() {
        // Resolvers return all AAAA then all A; a broken v6 network must not
        // pay one timeout per v6 address before the first v4 attempt.
        let addrs = vec![
            addr("[2001:db8::1]:443"),
            addr("[2001:db8::2]:443"),
            addr("192.0.2.1:443"),
            addr("192.0.2.2:443"),
            addr("192.0.2.3:443"),
        ];
        let out = interleave_address_families(addrs);
        assert_eq!(
            out,
            vec![
                addr("[2001:db8::1]:443"),
                addr("192.0.2.1:443"),
                addr("[2001:db8::2]:443"),
                addr("192.0.2.2:443"),
                addr("192.0.2.3:443"),
            ]
        );
    }

    #[test]
    fn single_family_lists_pass_through_unchanged() {
        let v4 = vec![addr("192.0.2.1:443"), addr("192.0.2.2:443")];
        assert_eq!(interleave_address_families(v4.clone()), v4);
        assert!(interleave_address_families(Vec::new()).is_empty());
    }

    #[tokio::test]
    async fn a_dead_address_times_out_rather_than_hanging() {
        // 192.0.2.0/24 (TEST-NET-1) is guaranteed unrouted, so the connect
        // either blackholes (timeout) or is refused (io error) — never Ok.
        // Either way connect_ws must return promptly instead of hanging.
        let url = "wss://192.0.2.1/ws";
        let step = Duration::from_millis(200);
        let started = std::time::Instant::now();
        let err = connect_ws(url, step)
            .await
            .expect_err("TEST-NET-1 must not accept connections");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "connect_ws did not honour the per-address timeout: {err}"
        );
        // A blackholed TCP connect is clob's own timeout, per address and
        // per step; a refused one is an I/O failure.
        assert!(
            matches!(&err, ClobWsError::ConnectTimeout { url: u, timeout }
                if u == url && *timeout == step)
                || matches!(&err, ClobWsError::Connection(e) if e.kind() == Kind::Io),
            "{err:?}"
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
    fn the_event_type_filter_still_guards_market_and_user() {
        // Without the filter, clob heartbeats and subscription
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
    fn skips_keepalive_frames_on_every_channel() {
        for text in ["PONG", "{}", ""] {
            for ch in [ChannelType::Market, ChannelType::User] {
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

#[cfg(test)]
mod connection_tests {
    use std::sync::{Arc, Mutex};

    use polyoxide_venue::{Class, Classify};
    use polyoxide_ws::test_server::{Script, ScriptedServer};

    use super::*;

    const WINDOW: Duration = Duration::from_secs(3);
    const BOOK: &str = r#"{"event_type":"book","asset_id":"a","market":"m","timestamp":"1",
        "hash":"h","bids":[],"asks":[],"last_trade_price":null}"#;

    /// A market-channel `WebSocket` over a plain loopback connection.
    async fn bare(server: &ScriptedServer) -> WebSocket {
        WebSocket {
            inner: connect_ws(&server.url, Duration::from_secs(2))
                .await
                .expect("connect"),
            channel_type: ChannelType::Market,
        }
    }

    async fn with_ping(server: &ScriptedServer) -> WebSocketWithPing {
        let inner = connect_ws(&server.url, Duration::from_secs(2))
            .await
            .expect("connect");
        WebSocketWithPing::new(inner, ChannelType::Market, Duration::from_secs(10))
    }

    async fn next(ws: &mut WebSocket) -> Option<Result<Channel, ClobWsError>> {
        tokio::time::timeout(WINDOW, ws.next())
            .await
            .expect("an item within the window")
    }

    /// Run `ws` until it returns, keeping every channel message it handled.
    async fn run_to_end(ws: WebSocketWithPing) -> (Result<(), ClobWsError>, Vec<Channel>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&seen);
        let ended = tokio::time::timeout(
            WINDOW,
            ws.run(move |channel| {
                kept.lock().unwrap().push(channel);
                async { Ok(()) }
            }),
        )
        .await
        .expect("run returns within the window");
        let seen = seen.lock().unwrap().drain(..).collect();
        (ended, seen)
    }

    #[tokio::test]
    async fn a_server_close_is_answered_with_the_close_reply_drift_r3() {
        // RFC 6455 requires the reply. clob's old read path ended on the
        // close and never read or flushed again, so the reply tungstenite
        // queued never went out.
        let server =
            ScriptedServer::start(vec![Script::close_at_once().close_with(1001, "bye")]).await;
        let mut ws = bare(&server).await;
        assert!(
            next(&mut ws).await.is_none(),
            "the stream ends on the close"
        );
        server
            .wait_for("the stream's close reply", |s| s.close_reply_count() == 1)
            .await;

        // `run` returns `Ok` on the close, and has replied to it.
        let (ended, _) = run_to_end(with_ping(&server).await).await;
        ended.expect("a server close ends `run` cleanly");
        server
            .wait_for("run's close reply", |s| s.close_reply_count() == 2)
            .await;
    }

    #[tokio::test]
    async fn close_after_a_server_close_succeeds() {
        let server = ScriptedServer::start(vec![Script::close_at_once()]).await;
        let mut ws = bare(&server).await;
        assert!(next(&mut ws).await.is_none());
        ws.close()
            .await
            .expect("closing a closed connection succeeds");
    }

    #[tokio::test]
    async fn the_stream_ends_after_its_first_error() {
        // The server drops the TCP connection without a closing handshake.
        let server = ScriptedServer::start(vec![Script::hold().then_cut()]).await;
        let mut ws = bare(&server).await;
        let err = next(&mut ws)
            .await
            .expect("the error")
            .expect_err("a cut connection fails the read");
        assert_eq!(err.class(), Class::Network, "{err:?}");
        assert!(next(&mut ws).await.is_none(), "the stream ends after it");
    }

    #[tokio::test]
    async fn a_binary_frame_is_decoded_as_text_d12() {
        let book = || Message::Binary(BOOK.as_bytes().to_vec().into());
        let server = ScriptedServer::start(vec![Script::messages(vec![book()]).then_close()]).await;

        let mut ws = bare(&server).await;
        let channel = next(&mut ws).await.expect("an event").expect("it parses");
        assert!(
            matches!(channel, Channel::Market(MarketMessage::Book(_))),
            "{channel:?}"
        );

        let (ended, seen) = run_to_end(with_ping(&server).await).await;
        ended.expect("run ends on the close");
        assert!(
            matches!(seen.as_slice(), [Channel::Market(MarketMessage::Book(_))]),
            "{seen:?}"
        );
    }

    #[tokio::test]
    async fn a_hung_handshake_times_out_after_one_steps_timeout() {
        // The TCP connect succeeds at once; the upgrade never finishes, and
        // its own step's bound ends it.
        let server = ScriptedServer::start(vec![Script::stall()]).await;
        let step = Duration::from_millis(200);
        let started = std::time::Instant::now();
        let err = connect_ws(&server.url, step)
            .await
            .expect_err("a server that never upgrades must not connect");
        let elapsed = started.elapsed();
        assert!(
            matches!(&err, ClobWsError::ConnectTimeout { url, timeout }
                if url == &server.url && *timeout == step),
            "{err:?}"
        );
        assert!(elapsed >= step, "timed out early: {elapsed:?}");
        assert!(elapsed < step * 5, "not bounded by one step: {elapsed:?}");
        assert_eq!(err.class(), Class::Network);
    }

    #[tokio::test]
    async fn a_451_refused_upgrade_is_restricted_and_not_a_fault() {
        let server = ScriptedServer::start(vec![Script::refuse_upgrade(451)]).await;
        let err = connect_ws(&server.url, Duration::from_secs(2))
            .await
            .expect_err("a refused upgrade must not connect");
        assert!(matches!(err, ClobWsError::Connection(_)), "{err:?}");
        assert_eq!(err.class(), Class::Restricted);
        assert!(!err.is_fault(), "{err:?}");
    }

    #[tokio::test]
    async fn a_429_refused_upgrade_carries_its_retry_after() {
        let server = ScriptedServer::start(vec![
            Script::refuse_upgrade(429).with_header("Retry-After", "30")
        ])
        .await;
        let err = connect_ws(&server.url, Duration::from_secs(2))
            .await
            .expect_err("a refused upgrade must not connect");
        assert_eq!(
            err.class(),
            Class::RateLimited {
                retry_after: Some(Duration::from_secs(30))
            }
        );
        assert_eq!(err.retry_after(), Some(Duration::from_secs(30)));
    }

    #[tokio::test]
    async fn a_wss_connect_reaches_tls_without_panicking() {
        // clob's graph enables both rustls backends (`ring` through reqwest,
        // `aws-lc-rs` through alloy), so rustls installs no default provider
        // by itself, and a TLS connect with none installed panics building
        // its config. Every other offline connect here is plain `ws://`. The
        // scripted server speaks no TLS, so the handshake fails, as a
        // transport error.
        let server = ScriptedServer::start(vec![Script::hold()]).await;
        let url = server.url.replacen("ws://", "wss://", 1);
        let err = connect_ws(&url, Duration::from_secs(2))
            .await
            .expect_err("a server that speaks no TLS must not connect");
        assert!(matches!(err, ClobWsError::Connection(_)), "{err:?}");
        // Failing to load the roots would stop short of the TLS config.
        assert!(
            !err.to_string().to_lowercase().contains("native root"),
            "{err}"
        );
    }

    #[test]
    fn a_dns_miss_is_a_network_failure() {
        // A host that resolves to no address was an `InvalidMessage`, an
        // `InvalidRequest` that nothing retried, though a DNS miss passes.
        let err = no_addresses("wss://ws-subscriptions-clob.polymarket.com/ws/market");
        assert!(matches!(err, ClobWsError::Connection(_)), "{err:?}");
        assert_eq!(err.class(), Class::Network);
        assert!(err.is_retriable());
        assert_eq!(
            err.to_string(),
            "WebSocket connection error: IO error: no addresses resolved for \
             wss://ws-subscriptions-clob.polymarket.com/ws/market"
        );
    }
}

#[cfg(test)]
mod membership_tests {
    use super::*;
    use polyoxide_ws::test_server::{Script, ScriptedServer};

    /// Long enough for a run to send what a test queued before the server
    /// closes, on a loaded runner too; the tests that end a run wait for the
    /// frames first.
    const CLOSE_AFTER: Duration = Duration::from_secs(2);

    /// How long a run may take to end: past the server's close, with margin.
    const RUN_ENDS: Duration = Duration::from_secs(5);

    /// The text frames the server received, keep-alive `PING`s excluded.
    fn non_ping_frames(server: &ScriptedServer) -> Vec<String> {
        server
            .client_frames()
            .into_iter()
            .filter(|frame| frame != "PING")
            .collect()
    }

    /// The keep-alive `PING`s the server received.
    fn pings(server: &ScriptedServer) -> usize {
        server.count_frames(|frame| frame == "PING")
    }

    /// Build a `WebSocketWithPing` of the given channel type over a plain
    /// loopback connection. `WebSocketBuilder::market_url` insists on
    /// `wss://`, so the connection goes through `connect_ws` directly,
    /// exactly as `connect_market` would.
    async fn connect_plain(
        server: &ScriptedServer,
        channel_type: ChannelType,
    ) -> WebSocketWithPing {
        connect_plain_with(server, channel_type, Duration::from_secs(10)).await
    }

    async fn connect_plain_with(
        server: &ScriptedServer,
        channel_type: ChannelType,
        ping_interval: Duration,
    ) -> WebSocketWithPing {
        let inner = connect_ws(&server.url, Duration::from_secs(2))
            .await
            .expect("connect");
        WebSocketWithPing::new(inner, channel_type, ping_interval)
    }

    #[tokio::test]
    async fn a_zero_ping_interval_pings_without_panicking() {
        // tokio's `interval` asserts a non-zero period, so this panicked the
        // moment `run` was first polled.
        let server = ScriptedServer::start(vec![Script::hold()]).await;
        let ws = connect_plain_with(&server, ChannelType::Market, Duration::ZERO).await;
        let run = tokio::spawn(ws.run(|_| async { Ok(()) }));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!run.is_finished(), "run ended: {:?}", run.await);
        assert!(pings(&server) >= 2, "pings were not repeated");
        run.abort();
    }

    #[tokio::test]
    async fn a_duration_max_ping_interval_sends_only_the_opening_ping() {
        // `interval` added the period to a late tick unchecked, so
        // `Duration::MAX` could panic depending on scheduling.
        let server = ScriptedServer::start(vec![Script::hold()]).await;
        let ws = connect_plain_with(&server, ChannelType::Market, Duration::MAX).await;
        let run = tokio::spawn(ws.run(|_| async { Ok(()) }));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!run.is_finished(), "run ended: {:?}", run.await);
        assert_eq!(pings(&server), 1);
        run.abort();
    }

    #[tokio::test]
    async fn run_forwards_membership_frames_in_order() {
        let server = ScriptedServer::start(vec![Script::hold().drop_after(CLOSE_AFTER)]).await;
        let ws = connect_plain(&server, ChannelType::Market).await;
        let handle = ws.membership();
        let run = tokio::spawn(ws.run(|_| async { Ok(()) }));

        handle.subscribe_assets(vec!["b".into()]).await.unwrap();
        handle.unsubscribe_assets(vec!["a".into()]).await.unwrap();

        server
            .wait_for("both frames", |s| non_ping_frames(s).len() == 2)
            .await;
        let frames = non_ping_frames(&server);
        assert_eq!(frames[0], r#"{"operation":"subscribe","assets_ids":["b"]}"#);
        assert_eq!(
            frames[1],
            r#"{"operation":"unsubscribe","assets_ids":["a"]}"#
        );
        // The server closes after the frames: run returns cleanly.
        tokio::time::timeout(RUN_ENDS, run)
            .await
            .expect("must not hang")
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn membership_handle_errors_with_membership_closed_after_run_exits() {
        let server = ScriptedServer::start(vec![Script::hold().drop_after(CLOSE_AFTER)]).await;
        let ws = connect_plain(&server, ChannelType::Market).await;
        let handle = ws.membership();
        let run = tokio::spawn(ws.run(|_| async { Ok(()) }));
        handle.subscribe_assets(vec!["b".into()]).await.unwrap();
        tokio::time::timeout(RUN_ENDS, run)
            .await
            .expect("must not hang")
            .unwrap()
            .unwrap();

        let err = tokio::time::timeout(
            Duration::from_secs(1),
            handle.subscribe_assets(vec!["c".into()]),
        )
        .await
        .expect("must not hang")
        .unwrap_err();
        assert!(matches!(err, ClobWsError::MembershipClosed), "{err}");
    }

    #[tokio::test]
    async fn membership_handle_refuses_the_user_channel() {
        let server = ScriptedServer::start(vec![Script::hold()]).await;
        let ws = connect_plain(&server, ChannelType::User).await;
        let handle = ws.membership();
        drop(ws);

        let err = handle.subscribe_assets(vec!["b".into()]).await.unwrap_err();
        assert!(matches!(err, ClobWsError::InvalidMessage(_)), "{err}");
    }
}
