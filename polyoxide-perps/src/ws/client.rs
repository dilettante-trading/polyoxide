//! The bare tier: one socket, control methods, a `Stream` of frames.

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{Stream, StreamExt};
use polyoxide_ws::{
    connect, tungstenite::Message, Connection, Correlated, Kind, Mode, DEFAULT_CONNECT_TIMEOUT,
};

use crate::ws::{
    channel::Channel,
    error::{PerpsWsError, Refusal, NO_REASON},
    event::Frame,
    frame::{Incoming, Request, Response},
    WS_URL,
};

/// Which way a control request changes membership.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Control {
    Subscribe,
    Unsubscribe,
}

/// What the server answered to a control request, channel by channel.
#[derive(Debug)]
pub(crate) struct Answered {
    /// The channels it accepted, in request order.
    pub accepted: Vec<Channel>,
    /// The channels it refused, with its identifiers.
    pub refused: Vec<Refusal>,
}

/// Send the request `build` writes for the connection's next id, and read
/// until its response, keeping every other frame for the stream.
///
/// A server close while waiting is [`PerpsWsError::ConnectionClosed`], as
/// before the kit; any other failure is the transport's.
pub(crate) async fn request(
    connection: &mut Connection,
    build: impl FnOnce(u64) -> Request,
) -> Result<(u64, Response), PerpsWsError> {
    connection
        .request(
            |id| {
                let request = serde_json::to_string(&build(id)).expect("request serialises");
                Message::Text(request.into())
            },
            |id, frame| match frame {
                Message::Text(text) => match Incoming::parse(text) {
                    Ok(Incoming::Response(response)) if response.id == Some(id) => {
                        Correlated::Answer((id, response))
                    }
                    // A push, a frame that does not parse, or an answer
                    // nobody awaits: the stream decides what each is.
                    _ => Correlated::Push,
                },
                _ => Correlated::Push,
            },
        )
        .await
        .map_err(|err| match err.kind() {
            Kind::Closed { .. } => PerpsWsError::ConnectionClosed,
            _ => PerpsWsError::Connection(err),
        })
}

/// Subscribe or unsubscribe `channels`, and read the per-channel answer.
pub(crate) async fn control(
    connection: &mut Connection,
    control: Control,
    channels: &[Channel],
) -> Result<Answered, PerpsWsError> {
    let names: Vec<String> = channels.iter().map(ToString::to_string).collect();
    let (id, response) = request(connection, |id| match control {
        Control::Subscribe => Request::subscribe(id, &names),
        Control::Unsubscribe => Request::unsubscribe(id, &names),
    })
    .await?;
    let malformed = || PerpsWsError::Response {
        id,
        raw: response.data.to_string(),
    };
    let statuses = response.statuses().map_err(|_| malformed())?;
    if statuses.len() != names.len() {
        return Err(malformed());
    }
    let mut answered = Answered {
        accepted: Vec::new(),
        refused: Vec::new(),
    };
    for (channel, status) in channels.iter().zip(&statuses) {
        if status.is_ok() {
            answered.accepted.push(*channel);
        } else {
            answered.refused.push(Refusal {
                channel: *channel,
                reason: status.error.clone().unwrap_or_else(|| NO_REASON.to_owned()),
            });
        }
    }
    Ok(answered)
}

/// A bare connection to the public channels.
///
/// The server sends a protocol Ping every 10 s and closes a connection that
/// stops answering them (`1011 pong_timeout`, after about 70 s). The pongs go
/// out while the stream is read, so read it steadily, or use the supervised
/// tier, which also sends the application [`ping`](Self::ping) on the wall
/// clock and reconnects. Control methods (`subscribe`, `unsubscribe`, `ping`)
/// send a request and read until its correlated response arrives; any push
/// frames read meanwhile are buffered and yielded by the stream afterwards.
/// A connect gives up after 10 s (DRIFT R2), and the server's close is
/// answered with the RFC 6455 close reply (DRIFT R3).
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
    connection: Connection,
    channels: Vec<Channel>,
}

impl std::fmt::Debug for PerpsWs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PerpsWs")
            .field("channels", &self.channels)
            .field("next_id", &self.connection.next_id())
            .field("connection", &self.connection)
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

    /// Connect to a specific endpoint, within the kit's default connect
    /// timeout. Crate-internal: tests point it at a local server.
    pub(crate) async fn connect_to(
        url: &str,
        channels: impl IntoIterator<Item = Channel>,
    ) -> Result<Self, PerpsWsError> {
        let channels: Vec<Channel> = channels.into_iter().collect();
        if channels.is_empty() {
            return Err(PerpsWsError::EmptySubscription);
        }
        let socket = connect(url, DEFAULT_CONNECT_TIMEOUT).await?;
        let mut ws = Self {
            connection: Connection::new(socket, Mode::Data),
            channels: Vec::new(),
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
        self.control(Control::Subscribe, channels.into_iter().collect())
            .await
    }

    /// Unsubscribe from channels on the open connection.
    pub async fn unsubscribe(
        &mut self,
        channels: impl IntoIterator<Item = Channel>,
    ) -> Result<(), PerpsWsError> {
        self.control(Control::Unsubscribe, channels.into_iter().collect())
            .await
    }

    async fn control(
        &mut self,
        change: Control,
        channels: Vec<Channel>,
    ) -> Result<(), PerpsWsError> {
        if channels.is_empty() {
            return Ok(());
        }
        let answered = control(&mut self.connection, change, &channels).await?;
        for channel in answered.accepted {
            match change {
                Control::Subscribe => {
                    if !self.channels.contains(&channel) {
                        self.channels.push(channel);
                    }
                }
                Control::Unsubscribe => self.channels.retain(|c| *c != channel),
            }
        }
        if answered.refused.is_empty() {
            Ok(())
        } else {
            Err(PerpsWsError::Refused {
                refused: answered.refused,
            })
        }
    }

    /// Send the application ping and wait for the pong, returning the
    /// server's sequence stamp from it when present.
    pub async fn ping(&mut self) -> Result<Option<u64>, PerpsWsError> {
        let (id, response) = request(&mut self.connection, Request::ping).await?;
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
        self.connection.close().await?;
        Ok(())
    }

    /// The channels currently subscribed on this connection.
    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }
}

/// Decode one text frame as the stream yields it: a push, `None` for a
/// response nobody is waiting for (a late pong, or a reply to a request the
/// caller abandoned), or an error for a frame that is neither.
pub(crate) fn decode_text(text: &str) -> Option<Result<Frame, PerpsWsError>> {
    match Incoming::parse(text) {
        Ok(Incoming::Push(push)) => Some(Frame::from_push(push, text)),
        Ok(Incoming::Response(_)) => None,
        Err(_) => Some(Err(PerpsWsError::Unrecognised {
            raw: text.to_owned(),
        })),
    }
}

impl Stream for PerpsWs {
    type Item = Result<Frame, PerpsWsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            return match self.connection.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(Message::Text(text)))) => match decode_text(&text) {
                    Some(item) => Poll::Ready(Some(item)),
                    None => continue,
                },
                // A Binary frame is not part of the protocol (D12).
                Poll::Ready(Some(Ok(_))) => continue,
                Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(PerpsWsError::from(err)))),
                Poll::Ready(None) => {
                    tracing::debug!(closed = ?self.connection.closed(), "perps WebSocket ended");
                    Poll::Ready(None)
                }
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
            scripted::{self, Script},
            PerpsProtocol,
        },
    };
    use futures_util::StreamExt;
    use polyoxide_ws::{tungstenite::Message, Protocol};

    const BBO: &str = r#"{"ch":"bbo::1","ts":1,"ets":1,"sq":10,"data":{"iid":1,"bp":"1","bq":"1","ap":"2","aq":"1"}}"#;
    const FILLS: &str = r#"{"ch":"fills","ts":1,"sq":11,"data":{}}"#;

    #[tokio::test]
    async fn connect_subscribes_and_yields_pushes_in_order() {
        let server = scripted::start(vec![Script {
            pushes: vec![BBO.into(), FILLS.into()],
            ..Default::default()
        }])
        .await;
        let mut ws = PerpsWs::connect_to(&server.url, [Channel::Bbo(InstrumentId(1))])
            .await
            .expect("connect");
        assert_eq!(ws.channels(), &[Channel::Bbo(InstrumentId(1))]);
        assert_eq!(
            server.first_frames(),
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
        let server = scripted::start(vec![Script {
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
        let server = scripted::start(vec![Script {
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
        let server = scripted::start(vec![Script::default()]).await;
        let mut ws = PerpsWs::connect_to(&server.url, [Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert_eq!(ws.ping().await.unwrap(), Some(1));
    }

    #[tokio::test]
    async fn subscribe_and_unsubscribe_adjust_membership_and_reach_the_server() {
        let server = scripted::start(vec![Script::default()]).await;
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
        let server = scripted::start(vec![Script {
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

    #[tokio::test(start_paused = true)]
    async fn a_hung_handshake_times_out_at_ten_seconds_drift_r2() {
        // DRIFT R2: the connect had no timeout, so a host that accepted TCP
        // and never upgraded hung it for ever.
        let server = polyoxide_ws::test_server::ScriptedServer::start(vec![
            polyoxide_ws::test_server::Script::stall(),
        ])
        .await;
        let err = PerpsWs::connect_to(&server.url, [Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap_err();
        match &err {
            PerpsWsError::Connection(ws) => assert_eq!(
                ws.kind(),
                Kind::ConnectTimeout {
                    after: std::time::Duration::from_secs(10)
                }
            ),
            other => panic!("expected a connect timeout, got {other:?}"),
        }
        let protocol = PerpsProtocol::new(WS_URL.to_owned(), polyoxide_ws::Timing::default());
        assert_eq!(protocol.recover(&err), polyoxide_ws::Recovery::Reconnect);
    }

    #[tokio::test]
    async fn a_server_close_is_answered_with_the_close_reply_drift_r3() {
        let server = scripted::start(vec![Script {
            close_after: true,
            ..Default::default()
        }])
        .await;
        let mut ws = PerpsWs::connect_to(&server.url, [Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(ws.next().await.is_none());
        server
            .wait_for("the close reply", |s| s.close_reply_count() == 1)
            .await;
    }

    #[tokio::test]
    async fn connecting_with_no_channels_is_refused_locally() {
        let server = scripted::start(vec![Script::default()]).await;
        let err = PerpsWs::connect_to(&server.url, []).await.unwrap_err();
        assert!(matches!(err, PerpsWsError::EmptySubscription));
        assert_eq!(server.connection_count(), 0);
    }

    #[tokio::test]
    async fn a_malformed_text_frame_is_an_error_the_stream_survives() {
        let server = scripted::start(vec![Script {
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
        let server = scripted::start(vec![Script {
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
