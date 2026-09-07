//! Capturing `tracing` events in tests.
//!
//! Some of this crate's behaviour is *only* observable as a log line. A
//! `window_s` that contradicts its topic changes nothing about the decoded
//! price, and the reconnect delay is consumed by a `sleep` that returns
//! nothing — so asserting on the parsed value or the return type proves
//! nothing in either case. Timing a real reconnect instead trades a missing
//! test for a flaky one.

use std::{
    fmt::Write as _,
    future::Future,
    sync::{Arc, Mutex},
};

use tracing_subscriber::layer::SubscriberExt as _;

/// One captured `tracing` event, reduced to what a test can assert on.
#[derive(Debug, Clone)]
pub struct CapturedEvent {
    /// The event's level.
    pub level: tracing::Level,
    /// Every field, rendered as `name=value ` pairs in declaration order.
    pub fields: String,
}

impl CapturedEvent {
    /// Whether the event carries `name` with exactly this rendered `value`.
    ///
    /// Values are rendered with `Debug`, so a `Duration` field reads as
    /// `20ms` and a string as `"btc/usd"`.
    pub fn has_field(&self, name: &str, value: &str) -> bool {
        self.fields.contains(&format!("{name}={value} "))
    }

    /// The rendered value of `name`, if the event carries it.
    pub fn field(&self, name: &str) -> Option<&str> {
        let prefix = format!("{name}=");
        let start = self.fields.find(&prefix)? + prefix.len();
        let rest = &self.fields[start..];
        Some(&rest[..rest.find(' ').unwrap_or(rest.len())])
    }
}

/// The events captured while a subscriber was installed.
///
/// The `expect`s cannot fire in practice: poisoning would require a panic
/// inside a `Vec` push or clone.
#[derive(Clone, Default)]
pub struct CapturedLogs(Arc<Mutex<Vec<CapturedEvent>>>);

impl CapturedLogs {
    /// Every event captured, in order.
    pub fn events(&self) -> Vec<CapturedEvent> {
        self.0.lock().expect("captured logs poisoned").clone()
    }

    /// Only the events at `level`, in order.
    pub fn at(&self, level: tracing::Level) -> Vec<CapturedEvent> {
        self.events()
            .into_iter()
            .filter(|event| event.level == level)
            .collect()
    }

    /// The rendered values of `field` across every event that carries it.
    ///
    /// This is the shape most assertions want: a *sequence*, so that a
    /// schedule can be checked as a whole rather than one entry at a time.
    pub fn field_values(&self, field: &str) -> Vec<String> {
        self.events()
            .iter()
            .filter_map(|event| event.field(field).map(str::to_string))
            .collect()
    }
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for CapturedLogs {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut fields = String::new();
        let mut visit = |field: &tracing::field::Field, value: &dyn std::fmt::Debug| {
            let _ = write!(fields, "{}={value:?} ", field.name());
        };
        event.record(&mut visit);
        self.0
            .lock()
            .expect("captured logs poisoned")
            .push(CapturedEvent {
                level: *event.metadata().level(),
                fields,
            });
    }
}

/// Run `body` with a capturing subscriber installed and return what it logged.
pub fn capture(body: impl FnOnce()) -> CapturedLogs {
    let logs = CapturedLogs::default();
    let subscriber = tracing_subscriber::registry().with(logs.clone());
    tracing::subscriber::with_default(subscriber, body);
    logs
}

/// Await `body` with a capturing subscriber installed, returning its output
/// alongside what it logged.
///
/// Uses [`WithSubscriber`](tracing::instrument::WithSubscriber) rather than a
/// thread-local guard, so it holds across await points and on a multi-threaded
/// runtime.
///
/// **A task spawned inside `body` is not captured.** The subscriber is
/// installed around `body`'s own polls, and a spawned task is polled outside
/// them; it must opt in with `with_current_subscriber` at the spawn site. That
/// suits the callers here — `SupervisedRtds::run` is awaited directly, so its
/// logs are captured while the scripted server's per-connection tasks stay out
/// of the way — but a test that expects a spawned task's events would silently
/// capture nothing, which is why the behaviour is pinned below rather than
/// left to be discovered.
pub async fn capture_async<T>(body: impl Future<Output = T>) -> (T, CapturedLogs) {
    use tracing::instrument::WithSubscriber as _;

    let logs = CapturedLogs::default();
    let subscriber = tracing_subscriber::registry().with(logs.clone());
    let out = body.with_subscriber(subscriber).await;
    (out, logs)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The capture is load-bearing for assertions that have no other observable
    // signal, so a layer that silently dropped events, mangled a field or
    // mislabelled a level would turn those tests into ones that pass on
    // anything. These pin it directly.

    #[test]
    fn events_are_captured_in_order_with_their_level_and_fields() {
        let logs = capture(|| {
            tracing::warn!(symbol = "btc/usd", expected = 30, "first");
            tracing::debug!(delay = ?std::time::Duration::from_millis(20), "second");
        });

        let events = logs.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].level, tracing::Level::WARN);
        assert_eq!(events[1].level, tracing::Level::DEBUG);
        assert!(events[0].has_field("expected", "30"), "{:?}", events[0]);
        assert_eq!(events[0].field("symbol"), Some("\"btc/usd\""));
        assert_eq!(events[1].field("delay"), Some("20ms"));
    }

    #[test]
    fn nothing_logged_means_nothing_captured() {
        // The control every "must not warn" assertion rests on.
        assert!(capture(|| {}).events().is_empty());
    }

    #[test]
    fn filtering_by_level_and_collecting_a_field_sequence() {
        let logs = capture(|| {
            tracing::debug!(delay = "1ms", "a");
            tracing::warn!(delay = "2ms", "b");
            tracing::debug!(delay = "4ms", "c");
        });

        assert_eq!(logs.at(tracing::Level::WARN).len(), 1);
        assert_eq!(logs.at(tracing::Level::DEBUG).len(), 2);
        assert_eq!(
            logs.field_values("delay"),
            vec!["\"1ms\"", "\"2ms\"", "\"4ms\""],
            "field_values must preserve order across every event carrying it"
        );
    }

    #[test]
    fn a_field_that_is_absent_reads_as_absent() {
        let logs = capture(|| tracing::warn!(present = 1, "x"));
        assert_eq!(logs.events()[0].field("missing"), None);
        assert!(!logs.events()[0].has_field("present", "2"));
    }

    #[tokio::test]
    async fn capture_async_captures_across_await_points() {
        let (out, logs) = capture_async(async {
            tracing::warn!(step = 1, "before");
            tokio::task::yield_now().await;
            tracing::warn!(step = 2, "after");
            "done"
        })
        .await;

        assert_eq!(out, "done");
        assert_eq!(logs.field_values("step"), vec!["1", "2"]);
    }

    #[tokio::test]
    async fn a_spawned_task_is_not_captured_unless_it_opts_in() {
        // Pins the sharp edge documented on `capture_async`. A test that
        // assumed otherwise would assert against an empty capture and pass on
        // anything.
        use tracing::instrument::WithSubscriber as _;

        let (_, logs) = capture_async(async {
            tokio::spawn(async { tracing::warn!(step = "bare", "not captured") })
                .await
                .expect("bare task");
            tokio::spawn(
                async { tracing::warn!(step = "opted-in", "captured") }.with_current_subscriber(),
            )
            .await
            .expect("opted-in task");
        })
        .await;

        assert_eq!(
            logs.field_values("step"),
            vec!["\"opted-in\""],
            "a bare spawn must not be captured, an opted-in one must be"
        );
    }
}
