//! DRIFT R3's wire probe: does the bare tier send the RFC 6455 close reply?
//!
//! Connects to a host, perps' public socket by default, sends nothing, and
//! reads until the server closes; perps closes a connection after 60 s with no
//! inbound message. The stream under TLS counts every byte written to the TCP
//! socket after the server's Close frame is read.
//!
//! perps pings every 10 s and counts the pong tungstenite sends back as an
//! inbound message, so a client that reads never goes idle. The stream
//! therefore withholds every byte the client writes after the handshake until
//! the Close is read, the pongs included, and counts them apart: the host sees
//! a silent client and closes it.
//!
//! Without `--flush` it follows the bare pattern the venues had before R3:
//! return on the Close and never read or flush again, so any byte counted is
//! a reply tungstenite sent on its own. With `--flush` it flushes once after
//! the Close, as `polyoxide_ws::Connection` does since R3. Gives up after
//! three minutes without a close.
//!
//! ```text
//! cargo run -p polyoxide-ws --example close_reply_probe
//! cargo run -p polyoxide-ws --example close_reply_probe -- --flush
//! cargo run -p polyoxide-ws --example close_reply_probe -- wss://other.host/ws
//! ```

use std::{
    io,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll},
    time::{Duration, Instant},
};

use futures_util::{SinkExt, StreamExt};
use polyoxide_ws::tungstenite::{http::Uri, Message};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
};

/// perps' public socket, `polyoxide_perps::ws::WS_URL`.
const PERPS_WS_URL: &str = "wss://ws.perpetuals.polymarket.com/v1/ws";
/// How long to wait for the server's close.
const GIVE_UP_AFTER: Duration = Duration::from_secs(180);
/// How long to leave the socket open after the close, for a reply to go out.
const LINGER: Duration = Duration::from_secs(2);

/// A TCP stream that, once `withhold` is set and until `after_close` is,
/// drops what the client writes, and then counts what it writes.
struct Counting {
    inner: TcpStream,
    withhold: Arc<AtomicBool>,
    withheld: Arc<AtomicUsize>,
    after_close: Arc<AtomicBool>,
    written: Arc<AtomicUsize>,
}

impl AsyncRead for Counting {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for Counting {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.withhold.load(Ordering::SeqCst) && !self.after_close.load(Ordering::SeqCst) {
            self.withheld.fetch_add(buf.len(), Ordering::SeqCst);
            return Poll::Ready(Ok(buf.len()));
        }
        let written = Pin::new(&mut self.inner).poll_write(cx, buf);
        if let Poll::Ready(Ok(n)) = written {
            if self.after_close.load(Ordering::SeqCst) {
                self.written.fetch_add(n, Ordering::SeqCst);
            }
        }
        written
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flush = args.iter().any(|arg| arg == "--flush");
    let url = args
        .iter()
        .find(|arg| arg.starts_with("ws"))
        .map_or(PERPS_WS_URL, String::as_str);

    polyoxide_ws::ensure_crypto_provider();
    let uri: Uri = url.parse()?;
    let host = uri.host().ok_or("the URL names no host")?;
    let tls = uri.scheme_str() == Some("wss");
    let port = uri.port_u16().unwrap_or(if tls { 443 } else { 80 });

    let withhold = Arc::new(AtomicBool::new(false));
    let withheld = Arc::new(AtomicUsize::new(0));
    let after_close = Arc::new(AtomicBool::new(false));
    let written = Arc::new(AtomicUsize::new(0));
    let stream = Counting {
        inner: TcpStream::connect((host, port)).await?,
        withhold: Arc::clone(&withhold),
        withheld: Arc::clone(&withheld),
        after_close: Arc::clone(&after_close),
        written: Arc::clone(&written),
    };
    let (mut ws, _response) = tokio_tungstenite::client_async_tls(url, stream).await?;
    withhold.store(true, Ordering::SeqCst);
    let opened = Instant::now();
    let pattern = if flush {
        "flush once after the close"
    } else {
        "return on the close, no flush"
    };
    println!("connected to {url}; pattern: {pattern}");

    let ended = tokio::time::timeout(GIVE_UP_AFTER, async {
        loop {
            match ws.next().await {
                Some(Ok(Message::Close(frame))) => {
                    after_close.store(true, Ordering::SeqCst);
                    return format!("a Close frame {frame:?}");
                }
                Some(Ok(other)) => println!("{:?}: {other:?}", opened.elapsed()),
                Some(Err(err)) => return format!("a read error, no Close frame: {err}"),
                None => return "the end of the stream, no Close frame".to_owned(),
            }
        }
    })
    .await;
    let Ok(ended) = ended else {
        println!("no close within {GIVE_UP_AFTER:?}");
        return Ok(());
    };
    println!("after {:?} the server sent {ended}", opened.elapsed());

    if flush {
        let flushed = ws.flush().await;
        println!("flush: {flushed:?}");
    }
    tokio::time::sleep(LINGER).await;
    println!(
        "bytes withheld before the close (pongs): {}",
        withheld.load(Ordering::SeqCst)
    );
    println!(
        "bytes written to the TCP socket after the close was read: {}",
        written.load(Ordering::SeqCst)
    );
    Ok(())
}
