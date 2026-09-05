//! The bare RTDS client.

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{SinkExt, Stream, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::{
    error::RtdsError,
    event::PriceEvent,
    subscription::{Subscription, SubscriptionRequest},
};

/// The RTDS endpoint.
pub const RTDS_URL: &str = "wss://ws-live-data.polymarket.com";

/// Make sure rustls has a default `CryptoProvider` before we open a connection.
///
/// This is a deliberate twin of `ensure_crypto_provider` in
/// `polyoxide-clob/src/ws/client.rs`. `tokio-tungstenite` builds its TLS
/// config from the process-wide default provider, and rustls picks one
/// automatically only when exactly one backend feature is enabled. Faced with
/// two candidates it installs neither and panics inside `connect_async`.
///
/// Duplicating rather than sharing is safe: `install_default` returns `Err`
/// when a provider is already set, so two crates racing is a no-op rather than
/// a conflict, and the host application's own choice is left alone.
fn ensure_crypto_provider() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// Reject a subscription set that would produce a silent connection.
fn validate_subscriptions(subscriptions: &[Subscription]) -> Result<(), RtdsError> {
    if subscriptions.is_empty() {
        return Err(RtdsError::EmptySubscription);
    }
    Ok(())
}

/// A connected RTDS stream.
///
/// Ends when the connection drops. For a feed that recovers on its own, use
/// [`RtdsBuilder`](crate::supervisor::RtdsBuilder).
///
/// # Example
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_rtds::{PriceEvent, Rtds, Subscription, Topic, TwapWindow};
///
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let mut stream = Rtds::connect(
///     Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Thirty))
///         .symbols(["btc/usd"]),
/// )
/// .await?;
///
/// while let Some(event) = stream.next().await {
///     if let PriceEvent::Update(update) = event? {
///         println!("{} {}", update.symbol(), update.value());
///     }
/// }
/// # Ok(())
/// # }
/// ```
pub struct Rtds {
    inner: WebSocketStream<MaybeTlsStream<TcpStream>>,
    subscriptions: Vec<Subscription>,
}

impl Rtds {
    /// Connect to RTDS and subscribe.
    pub async fn connect(
        subscriptions: impl IntoIterator<Item = Subscription>,
    ) -> Result<Self, RtdsError> {
        Self::connect_to(RTDS_URL, subscriptions).await
    }

    /// Connect to a specific endpoint.
    ///
    /// Crate-internal: the supervised tier uses it to reconnect, and tests
    /// point it at a local server. Callers who need a different endpoint use
    /// `RtdsBuilder::url` at tier 2 — not written as an intra-doc link yet,
    /// since `supervisor` does not exist until Task 11 and a link to a
    /// missing item is a hard error under `RUSTDOCFLAGS=-D warnings`.
    pub(crate) async fn connect_to(
        url: &str,
        subscriptions: impl IntoIterator<Item = Subscription>,
    ) -> Result<Self, RtdsError> {
        let request = SubscriptionRequest::new(subscriptions);
        validate_subscriptions(request.subscriptions())?;

        ensure_crypto_provider();
        let (mut inner, _) = connect_async(url).await?;
        let frame = serde_json::to_string(&request)
            .map_err(|e| RtdsError::json(format!("{request:?}"), e))?;
        inner.send(Message::Text(frame.into())).await?;

        Ok(Self {
            inner,
            subscriptions: request.subscriptions().to_vec(),
        })
    }

    /// Send the application keep-alive.
    ///
    /// RTDS documents a five-second cadence, but the connection survives far
    /// longer without it, and the server never replies. Do not use this to
    /// detect liveness — nothing comes back.
    pub async fn ping(&mut self) -> Result<(), RtdsError> {
        self.inner.send(Message::Text("PING".into())).await?;
        Ok(())
    }

    /// Close the connection.
    pub async fn close(&mut self) -> Result<(), RtdsError> {
        self.inner.close(None).await?;
        Ok(())
    }

    /// The subscriptions this connection was opened with.
    pub fn subscriptions(&self) -> &[Subscription] {
        &self.subscriptions
    }
}

impl Stream for Rtds {
    type Item = Result<PriceEvent, RtdsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            return match self.inner.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(message))) => match message {
                    Message::Text(text) => match PriceEvent::from_json(&text) {
                        Ok(Some(event)) => Poll::Ready(Some(Ok(event))),
                        // Greetings, keep-alives and unmodelled topics.
                        Ok(None) => continue,
                        Err(err) => Poll::Ready(Some(Err(err))),
                    },
                    Message::Close(frame) => {
                        // The only place this reason is ever visible. Tier 2
                        // turns stream-end into `ConnectionClosed` and
                        // reconnects, by which point it is gone.
                        tracing::debug!(?frame, "RTDS closed the connection");
                        Poll::Ready(None)
                    }
                    Message::Ping(_) | Message::Pong(_) | Message::Binary(_) => continue,
                    Message::Frame(_) => continue,
                },
                Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err.into()))),
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Pending => Poll::Pending,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topic::{Topic, TwapWindow};

    #[test]
    fn the_default_url_is_the_live_data_host() {
        assert_eq!(RTDS_URL, "wss://ws-live-data.polymarket.com");
    }

    #[test]
    fn connecting_with_no_subscriptions_is_refused() {
        // An empty `subscriptions` array produces a connection that receives
        // nothing, which is indistinguishable from a broken feed.
        let request = SubscriptionRequest::new(Vec::new());
        assert!(
            matches!(
                validate_subscriptions(request.subscriptions()),
                Err(RtdsError::EmptySubscription)
            ),
            "an empty subscription set must be refused up front"
        );
    }

    #[test]
    fn a_populated_subscription_set_is_accepted() {
        let subs =
            Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Thirty)).symbols(["btc/usd"]);
        let request = SubscriptionRequest::new(subs);
        assert!(validate_subscriptions(request.subscriptions()).is_ok());
    }
}
