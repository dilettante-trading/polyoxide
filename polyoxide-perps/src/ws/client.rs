//! The bare tier: one socket, control methods, a `Stream` of frames.

use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{SinkExt, Stream, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::ws::{
    channel::Channel,
    ensure_crypto_provider,
    error::{PerpsWsError, Refusal},
    event::Frame,
    frame::{Incoming, Request, Response},
    WS_URL,
};

/// Which way a control request changes membership.
#[derive(Debug, Clone, Copy)]
enum Membership {
    Add,
    Remove,
}

/// A bare connection to the public channels.
///
/// The server closes a connection after 60 s without an inbound message, so
/// a caller must call [`ping`](Self::ping) periodically or use the
/// supervised tier. Control methods (`subscribe`, `unsubscribe`, `ping`)
/// send a request and read until its correlated response arrives; any push
/// frames read meanwhile are buffered and yielded by the stream afterwards.
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_perps::{types::InstrumentId, ws::{Channel, Frame, PerpsWs}};
///
/// # async fn example() -> Result<(), polyoxide_perps::ws::PerpsWsError> {
/// let mut ws = PerpsWs::connect([Channel::Bbo(InstrumentId(1))]).await?;
/// while let Some(frame) = ws.next().await {
///     if let Frame::Update(update) = frame? {
///         println!("{} sq={}", update.channel, update.sq);
///     }
/// }
/// # Ok(())
/// # }
/// ```
pub struct PerpsWs {
    inner: WebSocketStream<MaybeTlsStream<TcpStream>>,
    channels: Vec<Channel>,
    next_id: u64,
    buffered: VecDeque<Result<Frame, PerpsWsError>>,
}

impl std::fmt::Debug for PerpsWs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PerpsWs")
            .field("channels", &self.channels)
            .field("next_id", &self.next_id)
            .field("buffered", &self.buffered.len())
            .finish_non_exhaustive()
    }
}

impl PerpsWs {
    /// Connect to the production host and subscribe.
    pub async fn connect(
        channels: impl IntoIterator<Item = Channel>,
    ) -> Result<Self, PerpsWsError> {
        Self::connect_to(WS_URL, channels).await
    }

    /// Connect to a specific endpoint. Crate-internal: the supervised tier
    /// reconnects through it and tests point it at a local server.
    pub(crate) async fn connect_to(
        url: &str,
        channels: impl IntoIterator<Item = Channel>,
    ) -> Result<Self, PerpsWsError> {
        let channels: Vec<Channel> = channels.into_iter().collect();
        if channels.is_empty() {
            return Err(PerpsWsError::EmptySubscription);
        }
        ensure_crypto_provider();
        let (inner, _) = connect_async(url).await?;
        let mut ws = Self {
            inner,
            channels: Vec::new(),
            next_id: 1,
            buffered: VecDeque::new(),
        };
        ws.subscribe(channels).await?;
        Ok(ws)
    }

    /// Subscribe to more channels on the open connection.
    ///
    /// Fails with [`PerpsWsError::Refused`] naming every channel the server
    /// refused; the accepted ones in the same request are still live on the
    /// server, so on error the membership recorded here is the accepted set.
    pub async fn subscribe(
        &mut self,
        channels: impl IntoIterator<Item = Channel>,
    ) -> Result<(), PerpsWsError> {
        self.control(Membership::Add, channels.into_iter().collect())
            .await
    }

    /// Unsubscribe from channels on the open connection.
    pub async fn unsubscribe(
        &mut self,
        channels: impl IntoIterator<Item = Channel>,
    ) -> Result<(), PerpsWsError> {
        self.control(Membership::Remove, channels.into_iter().collect())
            .await
    }

    async fn control(
        &mut self,
        change: Membership,
        channels: Vec<Channel>,
    ) -> Result<(), PerpsWsError> {
        if channels.is_empty() {
            return Ok(());
        }
        let names: Vec<String> = channels.iter().map(ToString::to_string).collect();
        let id = self.take_id();
        let request = match change {
            Membership::Add => Request::subscribe(id, &names),
            Membership::Remove => Request::unsubscribe(id, &names),
        };
        self.send(&request).await?;
        let response = self.await_response(id).await?;
        let statuses = response.statuses().map_err(|_| PerpsWsError::Response {
            id,
            raw: response.data.to_string(),
        })?;
        if statuses.len() != names.len() {
            return Err(PerpsWsError::Response {
                id,
                raw: response.data.to_string(),
            });
        }
        let mut refused = Vec::new();
        for (channel, status) in channels.iter().zip(&statuses) {
            if status.is_ok() {
                match change {
                    Membership::Add => {
                        if !self.channels.contains(channel) {
                            self.channels.push(*channel);
                        }
                    }
                    Membership::Remove => self.channels.retain(|c| c != channel),
                }
            } else {
                refused.push(Refusal {
                    channel: *channel,
                    reason: status.error.clone().unwrap_or_else(|| "err".to_owned()),
                });
            }
        }
        if refused.is_empty() {
            Ok(())
        } else {
            Err(PerpsWsError::Refused { refused })
        }
    }

    /// Send the application ping and wait for the pong, returning the
    /// server's sequence stamp from it when present.
    pub async fn ping(&mut self) -> Result<Option<u64>, PerpsWsError> {
        let id = self.take_id();
        self.send(&Request::ping(id)).await?;
        let response = self.await_response(id).await?;
        let pong = response.pong().map_err(|_| PerpsWsError::Response {
            id,
            raw: response.data.to_string(),
        })?;
        if pong.is_ok() {
            Ok(pong.sq)
        } else {
            Err(PerpsWsError::Response {
                id,
                raw: response.data.to_string(),
            })
        }
    }

    /// Close the connection.
    pub async fn close(&mut self) -> Result<(), PerpsWsError> {
        self.inner.close(None).await?;
        Ok(())
    }

    /// The channels currently subscribed on this connection.
    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }

    fn take_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    async fn send(&mut self, request: &Request) -> Result<(), PerpsWsError> {
        let text = serde_json::to_string(request).expect("request serialises");
        self.inner.send(Message::Text(text.into())).await?;
        Ok(())
    }

    /// Read until the response with `id` arrives, parking pushes meanwhile.
    async fn await_response(&mut self, id: u64) -> Result<Response, PerpsWsError> {
        loop {
            match self.inner.next().await {
                None => return Err(PerpsWsError::ConnectionClosed),
                Some(Err(err)) => return Err(err.into()),
                Some(Ok(Message::Text(text))) => match Incoming::parse(&text) {
                    Ok(Incoming::Response(response)) if response.id == Some(id) => {
                        return Ok(response)
                    }
                    Ok(Incoming::Response(other)) => {
                        tracing::debug!(?other.id, "uncorrelated response while awaiting {id}");
                    }
                    Ok(Incoming::Push(push)) => {
                        self.buffered.push_back(Frame::from_push(push, &text))
                    }
                    Err(_) => self.buffered.push_back(Err(PerpsWsError::Unrecognised {
                        raw: text.to_string(),
                    })),
                },
                Some(Ok(Message::Close(_))) => return Err(PerpsWsError::ConnectionClosed),
                Some(Ok(_)) => continue,
            }
        }
    }
}

impl Stream for PerpsWs {
    type Item = Result<Frame, PerpsWsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(parked) = self.buffered.pop_front() {
            return Poll::Ready(Some(parked));
        }
        loop {
            return match self.inner.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(Message::Text(text)))) => match Incoming::parse(&text) {
                    Ok(Incoming::Push(push)) => Poll::Ready(Some(Frame::from_push(push, &text))),
                    // A response nobody is waiting for: a late pong, or a
                    // reply to a request the caller abandoned.
                    Ok(Incoming::Response(_)) => continue,
                    Err(_) => Poll::Ready(Some(Err(PerpsWsError::Unrecognised {
                        raw: text.to_string(),
                    }))),
                },
                Poll::Ready(Some(Ok(Message::Close(frame)))) => {
                    tracing::debug!(?frame, "perps WebSocket closed by the server");
                    Poll::Ready(None)
                }
                Poll::Ready(Some(Ok(_))) => continue,
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
    use crate::{
        types::InstrumentId,
        ws::{
            event::Payload,
            test_server::{Script, ScriptedServer},
        },
    };
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::Message;

    const BBO: &str = r#"{"ch":"bbo::1","ts":1,"ets":1,"sq":10,"data":{"iid":1,"bp":"1","bq":"1","ap":"2","aq":"1"}}"#;
    const FILLS: &str = r#"{"ch":"fills","ts":1,"sq":11,"data":{}}"#;

    #[tokio::test]
    async fn connect_subscribes_and_yields_pushes_in_order() {
        let server = ScriptedServer::start(vec![Script {
            pushes: vec![BBO.into(), FILLS.into()],
            ..Default::default()
        }])
        .await;
        let mut ws = PerpsWs::connect_to(&server.url, [Channel::Bbo(InstrumentId(1))])
            .await
            .expect("connect");
        assert_eq!(ws.channels(), &[Channel::Bbo(InstrumentId(1))]);
        assert_eq!(
            server.subscriptions(),
            vec![r#"{"id":1,"req":"sub","chs":["bbo::1"]}"#.to_owned()]
        );

        let first = ws.next().await.unwrap().unwrap();
        let Frame::Update(update) = first else {
            panic!("expected an update")
        };
        assert_eq!(update.sq, 10);
        assert!(matches!(update.payload, Payload::Bbo(_)));

        let second = ws.next().await.unwrap().unwrap();
        assert!(matches!(second, Frame::Unknown { ref channel, .. } if channel == "fills"));
    }

    #[tokio::test]
    async fn a_refused_channel_fails_connect_with_the_identifier() {
        let server = ScriptedServer::start(vec![Script {
            refuse: vec![("bbo::1".into(), "invalid channel".into())],
            ..Default::default()
        }])
        .await;
        let err = PerpsWs::connect_to(
            &server.url,
            [
                Channel::Bbo(InstrumentId(1)),
                Channel::Trades(InstrumentId(1)),
            ],
        )
        .await
        .unwrap_err();
        match err {
            PerpsWsError::Refused { refused } => {
                assert_eq!(refused.len(), 1);
                assert_eq!(refused[0].channel, Channel::Bbo(InstrumentId(1)));
                assert_eq!(refused[0].reason, "invalid channel");
            }
            other => panic!("expected Refused, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn pushes_that_arrive_before_a_pong_are_buffered_not_lost() {
        // The server sends its pushes right after the subscribe response, so
        // by the time the ping goes out they are already in flight ahead of
        // the pong. `ping` must park them and the stream must yield them.
        let server = ScriptedServer::start(vec![Script {
            pushes: vec![BBO.into(), BBO.into()],
            ..Default::default()
        }])
        .await;
        let mut ws = PerpsWs::connect_to(&server.url, [Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        ws.ping().await.expect("pong");
        let mut seen = 0;
        while let Some(Ok(Frame::Update(_))) = ws.next().await {
            seen += 1;
            if seen == 2 {
                break;
            }
        }
        assert_eq!(seen, 2);
    }

    #[tokio::test]
    async fn a_pong_carries_the_servers_sequence() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = PerpsWs::connect_to(&server.url, [Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert_eq!(ws.ping().await.unwrap(), Some(1));
    }

    #[tokio::test]
    async fn subscribe_and_unsubscribe_adjust_membership_and_reach_the_server() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = PerpsWs::connect_to(&server.url, [Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        ws.subscribe([Channel::Trades(InstrumentId(2))])
            .await
            .unwrap();
        assert_eq!(ws.channels().len(), 2);
        ws.unsubscribe([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert_eq!(ws.channels(), &[Channel::Trades(InstrumentId(2))]);
        server
            .wait_for("three control frames", |s| s.client_frames().len() == 3)
            .await;
        assert_eq!(
            server.client_frames()[2],
            r#"{"id":3,"req":"unsub","chs":["bbo::1"]}"#
        );
    }

    #[tokio::test]
    async fn a_server_close_ends_the_stream() {
        let server = ScriptedServer::start(vec![Script {
            pushes: vec![BBO.into()],
            close_after: true,
            ..Default::default()
        }])
        .await;
        let mut ws = PerpsWs::connect_to(&server.url, [Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(ws.next().await, Some(Ok(Frame::Update(_)))));
        assert!(ws.next().await.is_none());
    }

    #[tokio::test]
    async fn connecting_with_no_channels_is_refused_locally() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let err = PerpsWs::connect_to(&server.url, []).await.unwrap_err();
        assert!(matches!(err, PerpsWsError::EmptySubscription));
        assert_eq!(server.connection_count(), 0);
    }

    #[tokio::test]
    async fn a_malformed_text_frame_is_an_error_the_stream_survives() {
        let server = ScriptedServer::start(vec![Script {
            pushes: vec!["not json".into(), BBO.into()],
            ..Default::default()
        }])
        .await;
        let mut ws = PerpsWs::connect_to(&server.url, [Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(
            ws.next().await,
            Some(Err(PerpsWsError::Unrecognised { .. }))
        ));
        assert!(matches!(ws.next().await, Some(Ok(Frame::Update(_)))));
    }

    #[tokio::test]
    async fn non_text_frames_are_skipped_and_the_next_push_still_arrives() {
        // A server Ping and a Binary frame are not the end of the stream and
        // not frames either; the text push queued behind them must come out.
        let server = ScriptedServer::start(vec![Script {
            raw_pushes: vec![
                Message::Ping(vec![1, 2, 3].into()),
                Message::Binary(vec![0xff].into()),
                Message::Text(BBO.into()),
            ],
            ..Default::default()
        }])
        .await;
        let mut ws = PerpsWs::connect_to(&server.url, [Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(ws.next().await, Some(Ok(Frame::Update(u))) if u.sq == 10));
    }
}
