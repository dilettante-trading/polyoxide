//! The bare tier: one connection on one path, control methods, and a `Stream`
//! of updates.

use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use polyoxide_ws::{connect, tungstenite::Message, Connection, Correlated, Mode};
use tokio::time::Instant;

use super::{
    request::{no_answer, request, Answer},
    SocketError, StreamSet, MAX_STREAMS_PER_CONNECTION,
};

/// One connection on one path of a product's streams.
///
/// Requests are batched at most `S::MAX_NAMES_PER_REQUEST` names each and sent
/// no faster than one per `S::MIN_REQUEST_INTERVAL`; each waits for the answer
/// carrying its id, and updates read meanwhile are kept and yielded by the
/// stream afterwards. A subscribe that would take the connection past
/// [`MAX_STREAMS_PER_CONNECTION`] is refused before anything is sent. It does
/// not check paths: the product's wrapper does, because only it knows what a
/// stream on the wrong path is. The control methods are not cancel-safe, and
/// after a `NoAnswer` the server's membership is unknown: replace the
/// connection rather than reuse it. The server's protocol pings are answered
/// while the caller polls, and its close is answered with the close reply. The
/// stream ends after the server's close, or after the first transport error it
/// yields.
pub(crate) struct Bare<S: StreamSet> {
    connection: Connection,
    path: S::Path,
    streams: Vec<S::Name>,
}

// Nothing in it is structurally pinned, but `S::Path` is not provably `Unpin`,
// and the `Stream` impl below needs `Pin<&mut Self>` to be `DerefMut`.
impl<S: StreamSet> Unpin for Bare<S> {}

impl<S: StreamSet> Bare<S> {
    /// Open a connection with no streams, paced at `S::MIN_REQUEST_INTERVAL`.
    pub(crate) async fn open(
        base_url: &str,
        path: S::Path,
        connect_timeout: Duration,
        answer_timeout: Duration,
    ) -> Result<Self, S::Error> {
        let socket = connect(S::url(base_url, path), connect_timeout).await?;
        let connection = Connection::new(socket, Mode::Data)
            .min_request_interval(S::MIN_REQUEST_INTERVAL)
            .answer_timeout(answer_timeout);
        Ok(Self {
            connection,
            path,
            streams: Vec::new(),
        })
    }

    /// Subscribe to more streams. A stream already subscribed is skipped.
    ///
    /// Fails with `too_many_streams` before sending anything, and with
    /// `refused` when the server answers with an error; batches answered
    /// before it stay subscribed, and [`streams`](Self::streams) says which.
    pub(crate) async fn subscribe(&mut self, streams: &[S::Name]) -> Result<(), S::Error> {
        let mut new: Vec<S::Name> = Vec::new();
        for stream in streams {
            if !self.streams.contains(stream) && !new.contains(stream) {
                new.push(stream.clone());
            }
        }
        if self.streams.len() + new.len() > MAX_STREAMS_PER_CONNECTION {
            return Err(<S::Error as SocketError>::too_many_streams(
                self.path,
                MAX_STREAMS_PER_CONNECTION,
            ));
        }
        for batch in new.chunks(S::MAX_NAMES_PER_REQUEST) {
            request::<S::Name, S::Error>(&mut self.connection, "SUBSCRIBE", Some(batch)).await?;
            self.streams.extend(batch.iter().cloned());
        }
        Ok(())
    }

    /// Unsubscribe from streams. A stream not subscribed is skipped.
    pub(crate) async fn unsubscribe(&mut self, streams: &[S::Name]) -> Result<(), S::Error> {
        let mut gone: Vec<S::Name> = Vec::new();
        for stream in streams {
            if self.streams.contains(stream) && !gone.contains(stream) {
                gone.push(stream.clone());
            }
        }
        for batch in gone.chunks(S::MAX_NAMES_PER_REQUEST) {
            request::<S::Name, S::Error>(&mut self.connection, "UNSUBSCRIBE", Some(batch)).await?;
            self.streams.retain(|s| !batch.contains(s));
        }
        Ok(())
    }

    /// The stream names the server says this connection carries.
    pub(crate) async fn list_subscriptions(&mut self) -> Result<Vec<String>, S::Error> {
        let id = self.connection.next_id();
        let result =
            request::<S::Name, S::Error>(&mut self.connection, "LIST_SUBSCRIPTIONS", None).await?;
        serde_json::from_value(result.clone())
            .map_err(|_| <S::Error as SocketError>::response(id, result.to_string()))
    }

    /// Send a protocol ping and wait for the pong, returning the round trip.
    pub(crate) async fn ping(&mut self) -> Result<Duration, S::Error> {
        let start = Instant::now();
        self.send_ping().await?;
        self.connection
            .await_answer(|message| match message {
                Message::Pong(_) => Correlated::Answer(()),
                _ => Correlated::Push,
            })
            .await
            .map_err(|err| no_answer::<S::Error>(err, 0))?;
        Ok(start.elapsed())
    }

    /// Close the connection.
    pub(crate) async fn close(&mut self) -> Result<(), S::Error> {
        self.connection.close().await?;
        Ok(())
    }

    /// The streams this connection carries.
    pub(crate) fn streams(&self) -> &[S::Name] {
        &self.streams
    }

    /// The path this connection is on.
    pub(crate) fn path(&self) -> S::Path {
        self.path
    }

    /// The connection, for a product's `Debug`.
    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Send a protocol ping without waiting for its pong.
    async fn send_ping(&mut self) -> Result<(), S::Error> {
        self.connection
            .send(Message::Ping(Vec::new().into()))
            .await?;
        Ok(())
    }

    /// The server's close code and reason, once it has closed the connection
    /// or the stream has ended without a close frame.
    #[cfg(test)]
    pub(crate) fn close_frame(&self) -> Option<(Option<u16>, String)> {
        let closed = self.connection.closed()?;
        Some(match closed.kind() {
            polyoxide_ws::Kind::Closed { code, reason } => (code, reason.to_owned()),
            _ => (None, String::new()),
        })
    }
}

impl<S: StreamSet> Stream for Bare<S> {
    type Item = Result<S::Update, S::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            let message = match self.connection.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(message))) => message,
                Poll::Ready(Some(Err(err))) => return Poll::Ready(Some(Err(err.into()))),
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            };
            // The connection yields Text and Binary only; a Binary frame is
            // not part of Binance's protocol (D12).
            let Message::Text(text) = message else {
                continue;
            };
            match S::decode(&text) {
                Ok(update) => return Poll::Ready(Some(Ok(update))),
                // An answer nobody is waiting for, such as one to a request
                // whose caller gave up.
                Err(_) if Answer::parse(&text).is_some() => continue,
                Err(err) => return Poll::Ready(Some(Err(err))),
            }
        }
    }
}
