//! The smoke suite's local server.
//!
//! It serves mockito routes with CORS headers, which a Worker's `fetch` needs to read them
//! cross-origin (the route paths and the 429's `Retry-After` are `polyoxide_wasm_smoke`'s), and
//! one scripted socket on `polyoxide-ws`'s test server that echoes text and closes on command.
//! The slow route is on a second mockito server, since mockito answers it by sleeping its
//! server's one thread. A fourth server, a hand-written responder, sends one route's head at once
//! and stalls its body, which mockito cannot do. It writes `POLYOXIDE_SMOKE_HTTP=<url>`,
//! `POLYOXIDE_SMOKE_WS=<url>`, `POLYOXIDE_SMOKE_SLOW=<url>` and `POLYOXIDE_SMOKE_STALL=<url>` to
//! the file its one argument names, whole and once every server listens, then runs until it is
//! killed.
//!
//! On wasm32 it is an empty program: cargo builds a package's bins for an integration test, and
//! the browser tests live in the same package.

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::{io, path::Path};

    use mockito::{Matcher, Mock, Server, ServerGuard};
    use polyoxide_wasm_smoke::{
        CLOSE_CODE, CLOSE_COMMAND, CLOSE_REASON, ECHO_PREFIX, HTTP_ENV, LIMITED_BODY,
        LIMITED_EXPOSED_PATH, LIMITED_HIDDEN_PATH, OK_BODY, OK_PATH, RETRY_AFTER_SECS, SLOW_DELAY,
        SLOW_ENV, SLOW_PATH, STALL_BODY_DELAY, STALL_ENV, STALL_FIRST, STALL_PATH, STALL_REST,
        WS_ENV,
    };
    use polyoxide_ws::test_server::{Script, ScriptedServer};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        task::JoinHandle,
    };
    use tungstenite::{
        protocol::{frame::coding::CloseCode, CloseFrame},
        Message,
    };

    /// Every server, kept alive by holding them.
    pub(crate) struct Running {
        /// The HTTP server's base URL.
        pub(crate) http: String,
        /// The base URL of the HTTP server that holds the slow route.
        pub(crate) slow: String,
        /// The base URL of the responder that stalls the body of its one route.
        pub(crate) stall: String,
        /// The socket server's `ws://` URL.
        pub(crate) ws: String,
        _http: ServerGuard,
        _slow: ServerGuard,
        _mocks: Vec<Mock>,
        _ws: ScriptedServer,
        _stall: Stall,
    }

    /// The stalling responder's accept loop, stopped when it is dropped.
    struct Stall(JoinHandle<()>);

    impl Drop for Stall {
        fn drop(&mut self) {
            self.0.abort();
        }
    }

    const CORS: &str = "access-control-allow-origin: *\r\naccess-control-allow-headers: *\r\n";

    /// Answer one connection: a preflight at once, [`STALL_PATH`] as a head plus the first part
    /// of its body and, after [`STALL_BODY_DELAY`], the rest, anything else with a 404.
    ///
    /// mockito cannot do this: it writes a response whole, so a route that sends its head and
    /// stalls the body behind it is a responder of its own. Every response closes its
    /// connection, so the framing is the declared length and nothing else.
    async fn stall_connection(mut stream: TcpStream) -> io::Result<()> {
        let mut request = Vec::new();
        let mut chunk = [0u8; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut chunk).await?;
            if read == 0 {
                return Ok(());
            }
            request.extend_from_slice(&chunk[..read]);
        }
        let request = String::from_utf8_lossy(&request);
        let line = request.lines().next().unwrap_or_default();
        if line.starts_with("OPTIONS ") {
            let head = format!("HTTP/1.1 204 No Content\r\n{CORS}connection: close\r\n\r\n");
            return stream.write_all(head.as_bytes()).await;
        }
        if !line.starts_with(&format!("GET {STALL_PATH} ")) {
            let head = format!(
                "HTTP/1.1 404 Not Found\r\n{CORS}content-length: 0\r\nconnection: close\r\n\r\n"
            );
            return stream.write_all(head.as_bytes()).await;
        }
        let length = STALL_FIRST.len() + STALL_REST.len();
        let head = format!(
            "HTTP/1.1 200 OK\r\n{CORS}content-type: text/plain\r\ncontent-length: {length}\r\nconnection: close\r\n\r\n{STALL_FIRST}"
        );
        stream.write_all(head.as_bytes()).await?;
        stream.flush().await?;
        tokio::time::sleep(STALL_BODY_DELAY).await;
        stream.write_all(STALL_REST.as_bytes()).await?;
        stream.shutdown().await
    }

    /// Start the responder on `127.0.0.1`: its base URL, and its accept loop.
    async fn start_stall() -> (String, Stall) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("the stalling responder binds a port");
        let url = format!("http://{}", listener.local_addr().expect("a bound address"));
        let accepting = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                // A peer that hangs up mid-answer is the browser abandoning the exchange.
                tokio::spawn(stall_connection(stream));
            }
        });
        (url, Stall(accepting))
    }

    /// The socket's answer to a text frame: a close for the command, an echo for anything else.
    fn answerer(_connection: usize) -> impl FnMut(&str) -> Vec<Message> + Send {
        |text: &str| {
            if text == CLOSE_COMMAND {
                vec![Message::Close(Some(CloseFrame {
                    code: CloseCode::from(CLOSE_CODE),
                    reason: CLOSE_REASON.into(),
                }))]
            } else {
                vec![Message::Text(format!("{ECHO_PREFIX}{text}").into())]
            }
        }
    }

    /// Start every server on `127.0.0.1`, each on a port of its own choosing.
    pub(crate) async fn start() -> Running {
        let mut server = Server::new_async().await;
        let mut slow_server = Server::new_async().await;
        let mut mocks = Vec::new();
        let retry_after = RETRY_AFTER_SECS.to_string();

        // A preflight for any path: the browser sends one before a request with a header
        // beyond the safelisted ones.
        mocks.push(
            server
                .mock("OPTIONS", Matcher::Any)
                .with_status(204)
                .with_header("access-control-allow-origin", "*")
                .with_header("access-control-allow-methods", "GET, POST, OPTIONS")
                .with_header("access-control-allow-headers", "*")
                .with_header("access-control-max-age", "600")
                .create_async()
                .await,
        );
        mocks.push(
            server
                .mock("GET", OK_PATH)
                .with_status(200)
                .with_header("access-control-allow-origin", "*")
                .with_header("content-type", "application/json")
                .with_body(OK_BODY)
                .create_async()
                .await,
        );
        // The callback sleeps its server's one thread, so every route on that server waits
        // behind it. wasm-bindgen-test may run the browser suite's async tests concurrently, so
        // the slow route has a server of its own and no other test's request waits on it.
        mocks.push(
            slow_server
                .mock("OPTIONS", Matcher::Any)
                .with_status(204)
                .with_header("access-control-allow-origin", "*")
                .with_header("access-control-allow-headers", "*")
                .create_async()
                .await,
        );
        mocks.push(
            slow_server
                .mock("GET", SLOW_PATH)
                .with_status(200)
                .with_header("access-control-allow-origin", "*")
                .with_body_from_request(|_| {
                    std::thread::sleep(SLOW_DELAY);
                    b"late".to_vec()
                })
                .create_async()
                .await,
        );
        mocks.push(
            server
                .mock("GET", LIMITED_EXPOSED_PATH)
                .with_status(429)
                .with_header("access-control-allow-origin", "*")
                .with_header("access-control-expose-headers", "Retry-After")
                .with_header("retry-after", &retry_after)
                .with_body(LIMITED_BODY)
                .create_async()
                .await,
        );
        mocks.push(
            server
                .mock("GET", LIMITED_HIDDEN_PATH)
                .with_status(429)
                .with_header("access-control-allow-origin", "*")
                .with_header("retry-after", &retry_after)
                .with_body(LIMITED_BODY)
                .create_async()
                .await,
        );

        // One script is repeated for every connection, so each test gets a fresh socket.
        let ws = ScriptedServer::start_answering(vec![Script::hold()], answerer).await;

        let (stall, stalling) = start_stall().await;

        Running {
            http: server.url(),
            slow: slow_server.url(),
            stall,
            ws: ws.url.clone(),
            _http: server,
            _slow: slow_server,
            _mocks: mocks,
            _ws: ws,
            _stall: stalling,
        }
    }

    /// Write every address to `path`, whole: a reader that sees the file sees all its lines.
    pub(crate) fn write_env(path: &Path, running: &Running) -> io::Result<()> {
        let staged = path.with_extension("partial");
        std::fs::write(
            &staged,
            format!(
                "{HTTP_ENV}={}\n{WS_ENV}={}\n{SLOW_ENV}={}\n{STALL_ENV}={}\n",
                running.http, running.ws, running.slow, running.stall
            ),
        )?;
        std::fs::rename(staged, path)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    use std::{path::Path, process::ExitCode};

    let mut args = std::env::args().skip(1);
    let (Some(path), None) = (args.next(), args.next()) else {
        eprintln!("usage: wasm-smoke-server <env file>");
        return ExitCode::from(2);
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("wasm-smoke-server: no runtime: {err}");
            return ExitCode::from(1);
        }
    };
    runtime.block_on(async {
        let running = native::start().await;
        if let Err(err) = native::write_env(Path::new(&path), &running) {
            eprintln!("wasm-smoke-server: cannot write {path}: {err}");
            return ExitCode::from(1);
        }
        std::future::pending::<()>().await;
        ExitCode::SUCCESS
    })
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::time::Duration;

    use futures_util::{SinkExt, StreamExt};
    use polyoxide_core::reqwest::{Client, Method, StatusCode};
    use polyoxide_wasm_smoke::{
        CLOSE_CODE, CLOSE_COMMAND, CLOSE_REASON, HTTP_ENV, LIMITED_EXPOSED_PATH,
        LIMITED_HIDDEN_PATH, OK_BODY, OK_PATH, RETRY_AFTER_SECS, SLOW_ENV, SLOW_PATH,
        STALL_BODY_DELAY, STALL_ENV, STALL_FIRST, STALL_PATH, STALL_REST, WS_ENV,
    };
    use tungstenite::Message;

    use super::native::{start, write_env};

    #[tokio::test]
    async fn the_ok_route_answers_with_the_cors_header_a_browser_needs() {
        let running = start().await;
        let response = Client::new()
            .get(format!("{}{OK_PATH}", running.http))
            .send()
            .await
            .expect("the ok route answers");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
        assert_eq!(response.text().await.expect("a body"), OK_BODY);
    }

    #[tokio::test]
    async fn the_two_429s_differ_only_in_the_exposed_header() {
        let running = start().await;
        let client = Client::new();
        let exposed = client
            .get(format!("{}{LIMITED_EXPOSED_PATH}", running.http))
            .send()
            .await
            .expect("the exposed route answers");
        let hidden = client
            .get(format!("{}{LIMITED_HIDDEN_PATH}", running.http))
            .send()
            .await
            .expect("the hidden route answers");
        for response in [&exposed, &hidden] {
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(response.headers()["access-control-allow-origin"], "*");
            assert_eq!(
                response.headers()["retry-after"],
                RETRY_AFTER_SECS.to_string().as_str()
            );
        }
        assert_eq!(
            exposed.headers()["access-control-expose-headers"],
            "Retry-After"
        );
        assert!(!hidden
            .headers()
            .contains_key("access-control-expose-headers"));
    }

    #[tokio::test]
    async fn a_preflight_is_allowed_for_any_path() {
        let running = start().await;
        let response = Client::new()
            .request(Method::OPTIONS, format!("{}/anything", running.http))
            .send()
            .await
            .expect("the preflight answers");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
        assert_eq!(response.headers()["access-control-allow-headers"], "*");
    }

    #[tokio::test]
    async fn the_slow_route_outlasts_a_short_timeout() {
        let running = start().await;
        let pending = Client::new()
            .get(format!("{}{SLOW_PATH}", running.slow))
            .send();
        assert!(
            tokio::time::timeout(Duration::from_millis(300), pending)
                .await
                .is_err(),
            "the slow route must not answer within 300 ms"
        );
    }

    #[tokio::test]
    async fn a_slow_request_does_not_hold_up_the_other_routes() {
        let running = start().await;
        let client = Client::new();
        // Leave a slow request pending on the slow server, then ask the main one.
        let slow = tokio::spawn({
            let client = client.clone();
            let url = format!("{}{SLOW_PATH}", running.slow);
            async move { client.get(url).send().await }
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        let started = std::time::Instant::now();
        let response = client
            .get(format!("{}{OK_PATH}", running.http))
            .send()
            .await
            .expect("the ok route answers while the slow one is pending");
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "the ok route waited {:?} behind the slow one",
            started.elapsed()
        );
        slow.abort();
    }

    #[tokio::test]
    async fn the_slow_route_answers_once_its_delay_has_passed() {
        let running = start().await;
        let response = Client::new()
            .get(format!("{}{SLOW_PATH}", running.slow))
            .send()
            .await
            .expect("the slow route answers in the end");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
    }

    #[tokio::test]
    async fn the_socket_echoes_text_then_closes_with_its_code() {
        let running = start().await;
        let mut socket = polyoxide_ws::connect(running.ws.clone(), Duration::from_secs(5))
            .await
            .expect("the socket accepts a connection");

        socket
            .send(Message::Text("hello".into()))
            .await
            .expect("send");
        let reply = socket.next().await.expect("a reply").expect("a frame");
        assert_eq!(reply, Message::Text("echo:hello".into()));

        socket
            .send(Message::Text(CLOSE_COMMAND.into()))
            .await
            .expect("send");
        let Message::Close(Some(frame)) = socket.next().await.expect("a close").expect("a frame")
        else {
            panic!("the command must be answered with a close frame");
        };
        assert_eq!(u16::from(frame.code), CLOSE_CODE);
        assert_eq!(frame.reason.as_str(), CLOSE_REASON);
    }

    #[tokio::test]
    async fn the_stall_route_sends_its_head_at_once_and_stalls_the_body() {
        let running = start().await;
        let started = std::time::Instant::now();
        let mut response = Client::new()
            .get(format!("{}{STALL_PATH}", running.stall))
            .send()
            .await
            .expect("the head arrives");
        assert!(
            started.elapsed() < STALL_BODY_DELAY / 2,
            "the head took {:?}",
            started.elapsed()
        );
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
        // The first part of the body arrives with the head; the rest only after the delay.
        let first = response
            .chunk()
            .await
            .expect("a chunk")
            .expect("a first chunk");
        assert_eq!(first, STALL_FIRST.as_bytes());
        assert!(started.elapsed() < STALL_BODY_DELAY / 2);
        let rest = response.chunk().await.expect("a chunk").expect("the rest");
        assert_eq!(rest, STALL_REST.as_bytes());
        assert!(
            started.elapsed() >= STALL_BODY_DELAY,
            "the rest arrived after {:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn the_stall_route_answers_a_preflight_and_refuses_other_paths() {
        let running = start().await;
        let client = Client::new();
        let preflight = client
            .request(Method::OPTIONS, format!("{}{STALL_PATH}", running.stall))
            .send()
            .await
            .expect("the preflight answers");
        assert_eq!(preflight.status(), StatusCode::NO_CONTENT);
        assert_eq!(preflight.headers()["access-control-allow-origin"], "*");
        let other = client
            .get(format!("{}/elsewhere", running.stall))
            .send()
            .await
            .expect("the other path answers");
        assert_eq!(other.status(), StatusCode::NOT_FOUND);
        assert_eq!(other.headers()["access-control-allow-origin"], "*");
    }

    #[tokio::test]
    async fn the_env_file_names_every_address() {
        let running = start().await;
        let dir = std::env::temp_dir().join(format!("wasm-smoke-env-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a temp dir");
        let file = dir.join("smoke.env");
        write_env(&file, &running).expect("the env file is written");
        let text = std::fs::read_to_string(&file).expect("the env file is read");
        assert_eq!(
            text,
            format!(
                "{HTTP_ENV}={}\n{WS_ENV}={}\n{SLOW_ENV}={}\n{STALL_ENV}={}\n",
                running.http, running.ws, running.slow, running.stall
            )
        );
        assert!(running.http.starts_with("http://127.0.0.1:"));
        assert!(running.ws.starts_with("ws://127.0.0.1:"));
        assert!(running.slow.starts_with("http://127.0.0.1:"));
        assert_ne!(
            running.slow, running.http,
            "the slow route needs a server of its own"
        );
        assert!(running.stall.starts_with("http://127.0.0.1:"));
        assert_ne!(
            running.stall, running.http,
            "the stalled body needs a server of its own"
        );
        assert_ne!(running.stall, running.slow);
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
