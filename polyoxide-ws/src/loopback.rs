//! One-connection local servers for the connect, bare-tier and correlation
//! tests, until the scripted server replaces them.

use futures_util::{SinkExt, StreamExt};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
use tokio_tungstenite::{
    accept_async, accept_hdr_async,
    tungstenite::{
        handshake::server::{ErrorResponse, Request, Response},
        http::{HeaderName, HeaderValue, StatusCode},
        protocol::{frame::coding::CloseCode, CloseFrame},
        Message,
    },
    WebSocketStream,
};

type Server = WebSocketStream<TcpStream>;

/// Accept one TCP connection and hand it to `serve`.
async fn one<F, Fut>(serve: F) -> String
where
    F: FnOnce(TcpStream) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("ws://{}", listener.local_addr().expect("address"));
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        serve(stream).await;
    });
    url
}

/// Accept one connection, upgrade it, and hand it to `serve`.
async fn upgraded<F, Fut>(serve: F) -> String
where
    F: FnOnce(Server) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    one(|stream| async move {
        if let Ok(ws) = accept_async(stream).await {
            serve(ws).await;
        }
    })
    .await
}

/// Read until the client goes.
async fn drain(ws: &mut Server) {
    while let Some(Ok(_)) = ws.next().await {}
}

/// Accept the TCP connection and never answer the handshake.
pub(crate) async fn stall() -> String {
    one(|stream| async move {
        let _held = stream;
        std::future::pending::<()>().await;
    })
    .await
}

/// Refuse the upgrade with `status` and these headers.
pub(crate) async fn refuse(status: u16, headers: &[(&'static str, &'static str)]) -> String {
    let headers = headers.to_vec();
    one(move |stream| async move {
        // The return type is tungstenite's `Callback` contract.
        #[allow(clippy::result_large_err)]
        let refuse = move |_: &Request, _: Response| -> Result<Response, ErrorResponse> {
            let mut refusal = ErrorResponse::new(None);
            *refusal.status_mut() = StatusCode::from_u16(status).expect("a status");
            for (name, value) in headers {
                refusal.headers_mut().insert(
                    HeaderName::from_bytes(name.as_bytes()).expect("a header name"),
                    HeaderValue::from_static(value),
                );
            }
            Err(refusal)
        };
        let _ = accept_hdr_async(stream, refuse).await;
    })
    .await
}

/// Send `messages`, then read until the client goes.
pub(crate) async fn push(messages: Vec<Message>) -> String {
    upgraded(|mut ws| async move {
        for message in messages {
            ws.send(message).await.expect("send");
        }
        drain(&mut ws).await;
    })
    .await
}

/// Answer each text frame with what `answer` returns, in order.
pub(crate) async fn serve<A>(first: Vec<Message>, mut answer: A) -> String
where
    A: FnMut(&str) -> Vec<Message> + Send + 'static,
{
    upgraded(|mut ws| async move {
        for message in first {
            ws.send(message).await.expect("send");
        }
        while let Some(Ok(message)) = ws.next().await {
            if let Message::Text(text) = message {
                for reply in answer(&text) {
                    if ws.send(reply).await.is_err() {
                        return;
                    }
                }
            }
        }
    })
    .await
}

/// Send a ping with `payload`, and report the first pong's payload.
pub(crate) async fn ping(payload: &'static [u8]) -> (String, oneshot::Receiver<Vec<u8>>) {
    let (tx, rx) = oneshot::channel();
    let url = upgraded(move |mut ws| async move {
        ws.send(Message::Ping(payload.to_vec().into()))
            .await
            .expect("send");
        while let Some(Ok(message)) = ws.next().await {
            if let Message::Pong(pong) = message {
                let _ = tx.send(pong.to_vec());
                break;
            }
        }
        drain(&mut ws).await;
    })
    .await;
    (url, rx)
}

/// Close with this code and reason, then read until the client goes.
pub(crate) async fn close_with(code: u16, reason: &'static str) -> String {
    upgraded(move |mut ws| async move {
        let frame = CloseFrame {
            code: CloseCode::from(code),
            reason: reason.into(),
        };
        let _ = ws.close(Some(frame)).await;
        drain(&mut ws).await;
    })
    .await
}

/// Complete the handshake, then drop the TCP connection without a close.
pub(crate) async fn drop_after_handshake() -> String {
    upgraded(|ws| async move { drop(ws) }).await
}

/// Report when the client sends a close frame.
pub(crate) async fn await_close() -> (String, oneshot::Receiver<()>) {
    let (tx, rx) = oneshot::channel();
    let url = upgraded(move |mut ws| async move {
        while let Some(Ok(message)) = ws.next().await {
            if let Message::Close(_) = message {
                let _ = tx.send(());
                return;
            }
        }
    })
    .await;
    (url, rx)
}

/// Close with this code and reason, and report whether the client answered
/// with its own close frame within a second.
pub(crate) async fn close_awaiting_reply(
    code: u16,
    reason: &'static str,
) -> (String, oneshot::Receiver<bool>) {
    let (tx, rx) = oneshot::channel();
    let url = upgraded(move |mut ws| async move {
        let _ = tx.send(close_and_await_reply(&mut ws, code, reason).await);
    })
    .await;
    (url, rx)
}

/// Close on the client's first text frame, and report whether it answered
/// with its own close frame within a second.
pub(crate) async fn close_on_request(
    code: u16,
    reason: &'static str,
) -> (String, oneshot::Receiver<bool>) {
    let (tx, rx) = oneshot::channel();
    let url = upgraded(move |mut ws| async move {
        while let Some(Ok(message)) = ws.next().await {
            if let Message::Text(_) = message {
                let _ = tx.send(close_and_await_reply(&mut ws, code, reason).await);
                return;
            }
        }
    })
    .await;
    (url, rx)
}

async fn close_and_await_reply(ws: &mut Server, code: u16, reason: &'static str) -> bool {
    let frame = CloseFrame {
        code: CloseCode::from(code),
        reason: reason.into(),
    };
    if ws.close(Some(frame)).await.is_err() {
        return false;
    }
    matches!(
        tokio::time::timeout(std::time::Duration::from_secs(1), ws.next()).await,
        Ok(Some(Ok(Message::Close(_))))
    )
}
