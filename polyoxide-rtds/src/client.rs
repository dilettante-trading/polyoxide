//! The bare RTDS client.

use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{SinkExt, Stream, StreamExt};
use polyoxide_ws::{connect, tungstenite::Message, Socket, WsError, DEFAULT_CONNECT_TIMEOUT};

use crate::{
    error::RtdsError,
    event::PriceEvent,
    subscription::{Subscription, SubscriptionRequest},
};

/// The RTDS endpoint.
pub const RTDS_URL: &str = "wss://ws-live-data.polymarket.com";

/// Reject a subscription set that would produce a silent connection.
fn validate_subscriptions(subscriptions: &[Subscription]) -> Result<(), RtdsError> {
    if subscriptions.is_empty() {
        return Err(RtdsError::EmptySubscription);
    }
    Ok(())
}

/// A connected RTDS stream.
///
/// Ends when the connection drops. A connection not open within 10 s fails
/// with the kit's `ConnectTimeout` (DRIFT R2). For a feed that recovers on its own, use
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
    inner: Socket,
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
        Self::open(url, subscriptions, DEFAULT_CONNECT_TIMEOUT).await
    }

    /// Connect to `url` within `connect_timeout`, through the kit's connect,
    /// which installs the TLS provider, and subscribe.
    pub(crate) async fn open(
        url: &str,
        subscriptions: impl IntoIterator<Item = Subscription>,
        connect_timeout: Duration,
    ) -> Result<Self, RtdsError> {
        let request = SubscriptionRequest::new(subscriptions);
        validate_subscriptions(request.subscriptions())?;

        let mut inner = connect(url, connect_timeout).await?;
        let frame = serde_json::to_string(&request)
            .map_err(|e| RtdsError::json(format!("{request:?}"), e))?;
        inner
            .send(Message::Text(frame.into()))
            .await
            .map_err(WsError::from)?;

        Ok(Self {
            inner,
            subscriptions: request.subscriptions().to_vec(),
        })
    }

    /// Send an additional subscribe frame on an open connection.
    ///
    /// Whether RTDS honours this is recorded in
    /// `docs/specs/rtds/OBSERVED.md`; see the live test that established it.
    pub async fn subscribe_more(
        &mut self,
        subscriptions: impl IntoIterator<Item = Subscription>,
    ) -> Result<(), RtdsError> {
        let request = SubscriptionRequest::new(subscriptions);
        validate_subscriptions(request.subscriptions())?;
        let frame = serde_json::to_string(&request)
            .map_err(|e| RtdsError::json(format!("{request:?}"), e))?;
        self.inner
            .send(Message::Text(frame.into()))
            .await
            .map_err(WsError::from)?;
        self.subscriptions
            .extend(request.subscriptions().iter().cloned());
        Ok(())
    }

    /// Send the application keep-alive.
    ///
    /// RTDS documents a five-second cadence, but the connection survives far
    /// longer without it, and the server never replies. Do not use this to
    /// detect liveness — nothing comes back.
    pub async fn ping(&mut self) -> Result<(), RtdsError> {
        self.inner
            .send(Message::Text("PING".into()))
            .await
            .map_err(WsError::from)?;
        Ok(())
    }

    /// Close the connection.
    pub async fn close(&mut self) -> Result<(), RtdsError> {
        self.inner.close(None).await.map_err(WsError::from)?;
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
                Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(WsError::from(err).into()))),
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

    // The tests below drive a real socket against the local scripted server.
    // They are unit tests rather than integration tests because `connect_to`
    // is `pub(crate)`: pointing the client at a test endpoint is not something
    // the public API offers, and widening it just to test it would be the
    // wrong trade.
    mod against_a_local_server {
        use polyoxide_ws::test_server::{Script, ScriptedServer};

        use super::*;
        use crate::fixtures;

        fn subs() -> Vec<Subscription> {
            Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Thirty)).symbols(["btc/usd"])
        }

        async fn connected(server: &ScriptedServer) -> Rtds {
            Rtds::connect_to(&server.url, subs())
                .await
                .expect("connect")
        }

        #[tokio::test]
        async fn connecting_sends_the_subscription_frame_and_records_it() {
            let server = ScriptedServer::start(vec![Script::hold().after_first_frame()]).await;
            let stream = connected(&server).await;

            server
                .wait_for("the subscription frame", |s| s.first_frames().len() == 1)
                .await;

            let sent = &server.first_frames()[0];
            assert!(sent.contains(r#"{\"symbol\":\"btc/usd\"}"#), "{sent}");
            assert_eq!(
                stream.subscriptions(),
                subs(),
                "the connection must remember what it subscribed to, or a \
                 reconnect resubscribes to something else"
            );
        }

        #[tokio::test]
        async fn ping_sends_the_application_keepalive() {
            // RTDS never answers a PING, so nothing about the client's own
            // stream can tell you the frame went out. The server is the only
            // observer.
            let server = ScriptedServer::start(vec![Script::hold().after_first_frame()]).await;
            let mut stream = connected(&server).await;

            stream.ping().await.expect("ping");

            server
                .wait_for("a PING frame", |s| {
                    s.client_frames().iter().any(|f| f == "PING")
                })
                .await;
        }

        #[tokio::test]
        async fn subscribe_more_sends_a_second_frame_and_extends_the_set() {
            let server = ScriptedServer::start(vec![Script::hold().after_first_frame()]).await;
            let mut stream = connected(&server).await;

            stream
                .subscribe_more(Subscription::for_topic(Topic::ChainlinkSpot).symbols(["eth/usd"]))
                .await
                .expect("subscribe_more");

            server
                .wait_for("two client frames", |s| s.client_frames().len() == 2)
                .await;

            let second = &server.client_frames()[1];
            assert!(second.contains("crypto_prices_chainlink"), "{second}");
            assert!(second.contains(r#"{\"symbol\":\"eth/usd\"}"#), "{second}");

            // A reconnect replays `subscriptions()`. If the added topic is not
            // in there, the feed silently narrows back on the first drop.
            assert_eq!(stream.subscriptions().len(), 2);
            assert_eq!(stream.subscriptions()[1].topic(), Topic::ChainlinkSpot);
            assert_eq!(stream.subscriptions()[1].symbol_filter(), Some("eth/usd"));
        }

        #[tokio::test]
        async fn subscribe_more_refuses_an_empty_set_without_sending_anything() {
            // An empty `subscriptions` array is answered with silence, which is
            // indistinguishable from an idle feed. The guard has to be at the
            // call site, not merely available.
            let server = ScriptedServer::start(vec![Script::hold().after_first_frame()]).await;
            let mut stream = connected(&server).await;
            server
                .wait_for("the subscription frame", |s| s.client_frames().len() == 1)
                .await;

            let err = stream
                .subscribe_more(Vec::new())
                .await
                .expect_err("an empty subscribe_more must be refused");
            assert!(matches!(err, RtdsError::EmptySubscription), "{err:?}");

            // Give a frame that should not exist time to arrive before denying
            // that it did.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            assert_eq!(
                server.client_frames().len(),
                1,
                "nothing may go out for a refused subscribe_more"
            );
            assert_eq!(stream.subscriptions().len(), 1);
        }

        #[tokio::test]
        async fn non_text_frames_are_skipped_rather_than_ending_the_stream() {
            // A server-initiated Ping or a Binary frame must not look like the
            // end of the feed. Nothing else in the suite drives these arms.
            let server = ScriptedServer::start(vec![Script::messages(vec![
                Message::Ping(vec![7u8].into()),
                Message::Pong(vec![].into()),
                Message::Binary(vec![1u8, 2, 3].into()),
                Message::Text(fixtures::TWAP_THIRTY_UPDATE.into()),
            ])
            .after_first_frame()])
            .await;
            let mut stream = connected(&server).await;

            let event = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
                .await
                .expect("the update must arrive, not be cut off by a control frame")
                .expect("stream must not end at a control frame")
                .expect("the update must parse");

            assert!(matches!(event, PriceEvent::Update(_)), "{event:?}");
        }

        #[tokio::test]
        async fn greetings_and_keepalives_are_skipped_on_a_live_socket() {
            // The unit tests prove `from_json` returns Ok(None) for these. This
            // proves the Stream impl loops on that rather than yielding a gap
            // the caller has to interpret.
            let server = ScriptedServer::start(vec![Script::frames(&[
                fixtures::EMPTY_GREETING,
                "PONG",
                "{}",
                fixtures::BINANCE_UPDATE,
            ])
            .after_first_frame()])
            .await;
            let mut stream = connected(&server).await;

            let event = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
                .await
                .expect("the update must arrive")
                .expect("stream must not end")
                .expect("the update must parse");

            match event {
                PriceEvent::Update(update) => assert_eq!(update.symbol(), "btcusdt"),
                other => panic!("expected the Binance update, got {other:?}"),
            }
        }

        #[tokio::test]
        async fn a_server_close_ends_the_stream_after_delivering_its_frames() {
            let server =
                ScriptedServer::start(vec![Script::frames(&[fixtures::TWAP_THIRTY_UPDATE])
                    .after_first_frame()
                    .then_close()])
                .await;
            let mut stream = connected(&server).await;

            assert!(matches!(
                stream.next().await,
                Some(Ok(PriceEvent::Update(_)))
            ));
            assert!(
                stream.next().await.is_none(),
                "a Close frame ends the stream rather than surfacing as an error"
            );
        }

        #[tokio::test]
        async fn a_hung_handshake_times_out_drift_r2() {
            // DRIFT R2: the connect had no timeout, so a host that accepted
            // TCP and never upgraded hung it for good.
            let server = ScriptedServer::start(vec![Script::stall()]).await;
            let after = std::time::Duration::from_millis(100);
            let Err(err) = Rtds::open(&server.url, subs(), after).await else {
                panic!("a server that never upgrades must not connect");
            };
            assert!(
                matches!(&err, RtdsError::Connection(ws) if ws.kind() == polyoxide_ws::Kind::ConnectTimeout { after }),
                "{err:?}"
            );
            assert_eq!(err.recovery(), crate::error::Recovery::Reconnect);
        }

        #[tokio::test]
        async fn close_sends_a_close_frame_to_the_server() {
            let server = ScriptedServer::start(vec![Script::hold().after_first_frame()]).await;
            let mut stream = connected(&server).await;

            stream.close().await.expect("close");

            server
                .wait_for("a client-initiated close", |s| s.close_count() == 1)
                .await;
        }
    }
}
