//! A `418`'s body names when the ban ends, and the error has no field for it,
//! so the client logs it at WARN under `polyoxide_binance::spot::error`. A
//! binary of its own, because it installs the process's global subscriber,
//! which `mock_spot`'s other `418` tests would otherwise race to reach first.

use std::fmt::Debug;
use std::sync::Mutex;

use mockito::{Matcher, Server};
use polyoxide_binance::spot::{Spot, SpotError};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, SubscriberExt};

/// Every `WARN` logged while this binary runs, as `(target, message)`.
static WARNINGS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// Installs, once and before any request is sent, the global subscriber that
/// fills [`WARNINGS`]. A subscriber scoped to one test would race: a callsite
/// first reached from a test with none caches `Interest::never` for every
/// thread, and the scoped test then sees nothing.
fn capture_warnings() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        tracing::subscriber::set_global_default(tracing_subscriber::registry().with(Warnings))
            .expect("this binary installs no other subscriber");
    });
}

struct Warnings;

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Warnings {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        if *event.metadata().level() != tracing::Level::WARN {
            return;
        }
        let mut message = Message(String::new());
        event.record(&mut message);
        WARNINGS
            .lock()
            .unwrap()
            .push((event.metadata().target().to_owned(), message.0));
    }
}

struct Message(String);

impl Visit for Message {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        }
    }
}

#[tokio::test]
async fn a_418_logs_its_body_at_warn_under_the_spot_error_target() {
    capture_warnings();
    let mut server = Server::new_async().await;
    let banned = server
        .mock("GET", "/api/v3/time")
        .match_query(Matcher::Any)
        .with_status(418)
        .with_body(
            r#"{"code":-1003,"msg":"Way too many requests; IP banned until 1700000000000."}"#,
        )
        .expect(1)
        .create_async()
        .await;
    let client = Spot::builder().base_url(server.url()).build().unwrap();

    let err = client.health().time().send().await.unwrap_err();
    assert!(matches!(err, SpotError::IpBanned { .. }), "{err:?}");
    banned.assert_async().await;

    // The send loop warns of the hold under `polyoxide_core`, naming the
    // path; the body is Binance's own warning, from the module's decode,
    // which sees the response but not the request.
    let warnings = WARNINGS.lock().unwrap().clone();
    let seen: Vec<_> = warnings
        .iter()
        .filter(|(target, _)| target.starts_with("polyoxide_binance"))
        .cloned()
        .collect();
    assert_eq!(seen.len(), 1, "one 418, one warning: {seen:?}");
    let (target, message) = &seen[0];
    assert_eq!(target, "polyoxide_binance::spot::error");
    assert!(
        message.contains("418") && message.contains("IP banned until 1700000000000"),
        "{message:?} does not name the ban's end"
    );
    assert!(
        warnings
            .iter()
            .any(|(target, message)| target.starts_with("polyoxide_core")
                && message.starts_with("Status 418")
                && message.contains(" on /api/v3/time,")),
        "no warning names the path: {warnings:?}"
    );
}
