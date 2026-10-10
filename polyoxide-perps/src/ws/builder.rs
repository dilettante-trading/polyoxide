//! The supervised tier's builder: perps' timings and endpoint, handed to the
//! kit's `Supervisor` with [`PerpsProtocol`].

use polyoxide_ws::{Supervisor, Timing};

use crate::ws::{
    channel::Channel, error::PerpsWsError, protocol::PerpsProtocol, SupervisedPerpsWs, WS_URL,
};

/// Builder for a supervised connection.
#[derive(Debug, Clone)]
pub struct PerpsWsBuilder {
    url: String,
    timing: Timing,
}

impl Default for PerpsWsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PerpsWsBuilder {
    /// Defaults: the production host, a ping every 20 s, stale after 30 s of
    /// silence, reconnect backoff from 500 ms doubling to 60 s, and a 10 s
    /// connect timeout.
    pub fn new() -> Self {
        Self {
            url: WS_URL.to_owned(),
            timing: Timing::default(),
        }
    }

    /// Connect to a different endpoint.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    polyoxide_ws::timing_setters! {
        /// How often to send the application ping, whatever the traffic. It
        /// is sent without awaiting its pong, and goes out while the
        /// consumer's queue is full too.
        ping_interval;
        /// Silence after which the connection is presumed dead and replaced.
        /// Any text frame counts, a pong included, so a connection that is
        /// quiet but answers pings stays up; time spent waiting on a full
        /// queue does not count. Also bounds each membership change and each
        /// replay: one the server leaves unanswered that long is
        /// [`PerpsWsError::Stalled`], and replaces the connection.
        stale_after;
        /// Reconnect delay schedule: `initial`, doubling to `max`. There is no
        /// cap on reconnect attempts; only the delay is capped, at `max`. Both
        /// are raised to at least 1 ms, and an `initial` above `max` is lowered
        /// to `max`.
        ///
        /// The same schedule spaces the three retries of a rate-limited
        /// subscribe, and those retries share the change's `stale_after`
        /// bound, so `initial × 7` (the three delays, `1×`, `2×` and `4×`) must
        /// stay under `stale_after` or a rate-limited subscribe is reported as
        /// a stall and becomes a reconnect. No pings are sent during those
        /// retries, so keep `initial` well under the host's idle limit.
        backoff;
        /// How long one connection attempt may take: the TCP connect, the TLS
        /// handshake and the WebSocket upgrade together (DRIFT R2). A timed
        /// out attempt is retried like any other lost connection.
        connect_timeout;
    }

    /// Open the connection, subscribe, and start supervising.
    ///
    /// Fails with [`PerpsWsError::EmptySubscription`] for no channels, before
    /// connecting. A channel the server refuses fails here with
    /// [`PerpsWsError::Refused`]; later refusals arrive through
    /// [`PerpsMembership`](crate::ws::PerpsMembership).
    pub async fn connect(
        self,
        channels: impl IntoIterator<Item = Channel>,
    ) -> Result<SupervisedPerpsWs, PerpsWsError> {
        let channels: Vec<Channel> = channels.into_iter().collect();
        if channels.is_empty() {
            return Err(PerpsWsError::EmptySubscription);
        }
        Supervisor::new(PerpsProtocol::new(self.url, self.timing), self.timing)
            .connect(channels)
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    use futures_util::StreamExt;
    use polyoxide_ws::{
        test_server::{Script as Shared, ScriptedServer},
        tungstenite::Message,
        Backoff,
    };

    use super::*;
    use crate::{
        types::InstrumentId,
        ws::{
            protocol::MAX_SUBSCRIBE_RETRIES,
            scripted::{self, Script},
            Event, PerpsMembership, SupervisedPerpsWs,
        },
    };

    fn bbo(sq: u64) -> String {
        format!(
            r#"{{"ch":"bbo::1","ts":1,"ets":1,"sq":{sq},"data":{{"iid":1,"bp":"1","bq":"1","ap":"2","aq":"1"}}}}"#
        )
    }

    fn fast() -> PerpsWsBuilder {
        PerpsWsBuilder::new()
            .ping_interval(Duration::from_millis(50))
            .stale_after(Duration::from_millis(300))
            .backoff(Duration::from_millis(10), Duration::from_millis(20))
    }

    async fn next_event(ws: &mut SupervisedPerpsWs) -> Event {
        tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("an event within 2 s")
            .expect("stream open")
            .expect("not an error")
    }

    #[test]
    fn defaults_match_the_documented_cadences() {
        let b = PerpsWsBuilder::new();
        assert_eq!(b.url, WS_URL);
        assert_eq!(b.timing.ping_interval, Some(Duration::from_secs(20)));
        assert_eq!(b.timing.stale_after, Duration::from_secs(30));
        assert_eq!(b.timing.initial_backoff, Duration::from_millis(500));
        assert_eq!(b.timing.max_backoff, Duration::from_secs(60));
    }

    #[test]
    fn backoff_doubles_to_the_ceiling_and_resets_after_a_working_connection() {
        let mut b = Backoff::new(Duration::from_millis(100), Duration::from_millis(350));
        assert_eq!(b.take(), Duration::from_millis(100));
        assert_eq!(b.take(), Duration::from_millis(200));
        assert_eq!(b.take(), Duration::from_millis(350));
        b.after_connection_ended(true);
        assert_eq!(b.take(), Duration::from_millis(100));
        b.after_connection_ended(false);
        assert_eq!(b.take(), Duration::from_millis(200));
    }

    #[test]
    fn a_zero_initial_delay_still_grows() {
        // Zero doubles to zero, which would retry a dead server on every
        // timer tick for ever.
        let mut b = Backoff::new(Duration::ZERO, Duration::from_secs(1));
        assert_eq!(b.take(), Duration::from_millis(1));
        assert_eq!(b.take(), Duration::from_millis(2));
    }

    #[test]
    fn a_huge_delay_saturates_instead_of_panicking() {
        let mut b = Backoff::new(Duration::MAX, Duration::MAX);
        assert_eq!(b.take(), Duration::MAX);
        assert_eq!(b.take(), Duration::MAX);
    }

    #[tokio::test]
    async fn a_stale_limit_of_duration_max_means_never() {
        // `Duration::MAX` is the usual way to say "no timeout". Adding it to
        // an instant overflowed, which panicked the task on its first pass
        // and ended the stream with no error. The server does not answer
        // pings, so only the limit keeps this connection up.
        let server = scripted::start(vec![Script {
            pushes: vec![bbo(1), bbo(2)],
            answer_pings: false,
            ..Default::default()
        }])
        .await;
        let mut ws = fast()
            .stale_after(Duration::MAX)
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 1));
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 2));
        assert!(
            tokio::time::timeout(Duration::from_millis(600), ws.next())
                .await
                .is_err(),
            "went stale with Duration::MAX"
        );
        assert_eq!(server.connection_count(), 1);
        let pings = server
            .client_frames()
            .iter()
            .filter(|f| f.contains(r#""type":"ping""#))
            .count();
        assert!(pings >= 2, "pings stopped; saw {pings}");
        ws.close().await.unwrap();
    }

    #[tokio::test]
    async fn a_ping_interval_of_duration_max_means_never() {
        let server = scripted::start(vec![Script {
            pushes: vec![bbo(1), bbo(2)],
            ..Default::default()
        }])
        .await;
        let mut ws = fast()
            .ping_interval(Duration::MAX)
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 1));
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 2));
        // With no pings nothing refreshes the 300 ms window, so staleness
        // still replaces the connection.
        assert!(matches!(next_event(&mut ws).await, Event::Reconnected));
        let pings: Vec<String> = server
            .client_frames()
            .into_iter()
            .filter(|f| f.contains(r#""type":"ping""#))
            .collect();
        assert!(pings.is_empty(), "a ping went out: {pings:?}");
        ws.close().await.unwrap();
    }

    #[tokio::test]
    async fn a_faulted_pong_reconnects_rather_than_ending_the_stream() {
        // A pong that is not `ok` is `PerpsWsError::Response`. The control
        // channel of that connection is unreliable; a fresh connection is
        // the fix, not a permanent end of the stream.
        let server = scripted::start(vec![
            Script {
                pushes: vec![bbo(1)],
                pong_status: "err",
                ..Default::default()
            },
            Script {
                pushes: vec![bbo(2)],
                ..Default::default()
            },
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 1));
        assert!(matches!(next_event(&mut ws).await, Event::Reconnected));
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 2));
        assert_eq!(server.connection_count(), 2);
        ws.close().await.unwrap();
    }

    #[tokio::test]
    async fn a_dropped_connection_is_replaced_and_resubscribed() {
        let server = scripted::start(vec![
            Script {
                pushes: vec![bbo(1)],
                close_after: true,
                ..Default::default()
            },
            Script {
                pushes: vec![bbo(2)],
                ..Default::default()
            },
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 1));
        assert!(matches!(next_event(&mut ws).await, Event::Reconnected));
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 2));
        assert_eq!(server.connection_count(), 2);
        let subs = server.first_frames();
        assert_eq!(subs.len(), 2);
        assert!(
            subs[1].contains("bbo::1"),
            "reconnect did not resubscribe: {}",
            subs[1]
        );
    }

    #[tokio::test]
    async fn silence_is_a_stall_and_pings_go_out_meanwhile() {
        // A socket that answers pings is alive by definition, so the first
        // connection must not answer them for silence to mean a stall.
        let server = scripted::start(vec![
            Script {
                answer_pings: false,
                ..Default::default()
            },
            Script {
                pushes: vec![bbo(5)],
                ..Default::default()
            },
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Reconnected));
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 5));
        let pings = server
            .client_frames()
            .iter()
            .filter(|f| f.contains(r#""type":"ping""#))
            .count();
        assert!(
            pings >= 2,
            "expected pings during the silent 300 ms, saw {pings}"
        );
    }

    #[tokio::test]
    async fn membership_changes_reach_the_server_and_survive_a_reconnect() {
        // The first connection answers membership requests but not pings,
        // so once it goes quiet it is a stall and gets replaced.
        let server = scripted::start(vec![
            Script {
                pushes: vec![bbo(1)],
                answer_pings: false,
                ..Default::default()
            },
            Script {
                pushes: vec![bbo(2)],
                ..Default::default()
            },
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        let handle = ws.membership();
        assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
        handle
            .subscribe([Channel::Trades(InstrumentId(2))])
            .await
            .unwrap();
        handle
            .unsubscribe([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        server
            .wait_for("sub and unsub frames", |s| {
                s.client_frames()
                    .iter()
                    .filter(|f| f.contains("trades::2") || f.contains(r#""req":"unsub""#))
                    .count()
                    >= 2
            })
            .await;
        // Force a reconnect by staleness (the first script sends nothing more).
        assert!(matches!(next_event(&mut ws).await, Event::Reconnected));
        let subs = server.first_frames();
        assert!(
            subs[1].contains("trades::2") && !subs[1].contains("bbo::1"),
            "resubscribed with the wrong set: {}",
            subs[1]
        );
    }

    #[tokio::test]
    async fn a_sequence_going_backwards_is_reported() {
        let server = scripted::start(vec![Script {
            pushes: vec![bbo(5), bbo(3)],
            ..Default::default()
        }])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 5));
        assert!(matches!(
            next_event(&mut ws).await,
            Event::SequenceRegressed {
                previous: 5,
                got: 3,
                ..
            }
        ));
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 3));
    }

    #[tokio::test]
    async fn a_rate_limited_subscribe_is_retried_then_reported() {
        let server = scripted::start(vec![Script {
            refuse: vec![("trades::2".into(), "message_rate_limited".into())],
            ..Default::default()
        }])
        .await;
        let ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        let err = ws
            .membership()
            .subscribe([Channel::Trades(InstrumentId(2))])
            .await
            .unwrap_err();
        assert!(matches!(err, PerpsWsError::Refused { .. }));
        let attempts = server
            .client_frames()
            .iter()
            .filter(|f| f.contains("trades::2"))
            .count();
        assert_eq!(attempts, 1 + MAX_SUBSCRIBE_RETRIES);
    }

    #[tokio::test]
    async fn a_refused_channel_at_connect_is_an_immediate_error() {
        let server = scripted::start(vec![Script {
            refuse: vec![("bbo::1".into(), "invalid channel".into())],
            ..Default::default()
        }])
        .await;
        let err = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap_err();
        assert!(matches!(err, PerpsWsError::Refused { .. }));
    }

    #[tokio::test]
    async fn close_ends_the_stream_and_the_handle() {
        let server = scripted::start(vec![Script::default()]).await;
        let ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        let handle = ws.membership();
        ws.close().await.unwrap();
        server
            .wait_for("a close frame", |s| s.close_count() == 1)
            .await;
        assert!(matches!(
            handle.subscribe([Channel::Trades(InstrumentId(2))]).await,
            Err(PerpsWsError::Stopped)
        ));
    }

    #[tokio::test]
    async fn a_quiet_connection_that_answers_pings_is_alive() {
        // No pushes at all for three staleness windows; the pongs keep it.
        let server = scripted::start(vec![Script::default()]).await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        let quiet = tokio::time::timeout(Duration::from_millis(900), ws.next()).await;
        assert!(quiet.is_err(), "expected silence, got {quiet:?}");
        assert_eq!(server.connection_count(), 1);
        let pings = server
            .client_frames()
            .iter()
            .filter(|f| f.contains(r#""type":"ping""#))
            .count();
        assert!(pings >= 10, "expected pings on cadence, saw {pings}");
    }

    #[tokio::test]
    async fn pings_keep_going_under_steady_traffic() {
        let server = scripted::start(vec![Script {
            pushes: vec![bbo(1)],
            push_every: Some(Duration::from_millis(10)),
            ..Default::default()
        }])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
        let mut updates = 0;
        while let Ok(Some(event)) = tokio::time::timeout_at(deadline, ws.next()).await {
            match event.unwrap() {
                Event::Update(_) => updates += 1,
                other => panic!("unexpected {other:?}"),
            }
        }
        let pings = server
            .client_frames()
            .iter()
            .filter(|f| f.contains(r#""type":"ping""#))
            .count();
        assert!(updates >= 20, "traffic stopped flowing: {updates} updates");
        assert!(pings >= 5, "expected pings under traffic, saw {pings}");
        assert_eq!(server.connection_count(), 1);
    }

    #[tokio::test]
    async fn close_does_not_wait_on_a_consumer_that_never_read() {
        // More pushes than the event buffer holds, and nobody reading: the
        // task is parked in `events.send`. `close` must still return.
        let pushes: Vec<String> = (1..=1100).map(bbo).collect();
        let server = scripted::start(vec![Script {
            pushes,
            ..Default::default()
        }])
        .await;
        let ws = fast()
            .url(&server.url)
            .stale_after(Duration::from_secs(5))
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        server
            .wait_for("the buffer to fill", |s| !s.client_frames().is_empty())
            .await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        let closed = tokio::time::timeout(Duration::from_secs(2), ws.close()).await;
        assert!(matches!(closed, Ok(Ok(()))), "close: {closed:?}");
    }

    #[tokio::test]
    async fn close_is_honoured_during_the_reconnect_backoff() {
        let server = scripted::start(vec![
            Script {
                pushes: vec![bbo(1)],
                close_after: true,
                ..Default::default()
            },
            Script {
                reject_handshake: true,
                ..Default::default()
            },
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .backoff(Duration::from_secs(3), Duration::from_secs(3))
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
        // The server has closed; give the task a moment to enter its sleep.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let started = tokio::time::Instant::now();
        let closed = tokio::time::timeout(Duration::from_secs(2), ws.close()).await;
        assert!(matches!(closed, Ok(Ok(()))), "close: {closed:?}");
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "close waited out the backoff: {:?}",
            started.elapsed()
        );
        assert_eq!(server.connection_count(), 1);
    }

    #[tokio::test]
    async fn a_membership_change_the_server_never_answers_is_a_stall() {
        let server = scripted::start(vec![
            Script {
                answer_subscribes_after_first: false,
                ..Default::default()
            },
            Script::default(),
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        let handle = ws.membership();
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            handle.subscribe([Channel::Trades(InstrumentId(2))]),
        )
        .await
        .expect("the subscribe returns");
        assert!(
            matches!(result, Err(PerpsWsError::Stalled { .. })),
            "{result:?}"
        );
        assert!(matches!(next_event(&mut ws).await, Event::Reconnected));
    }

    #[tokio::test]
    async fn a_failed_reconnect_attempt_is_retried() {
        let server = scripted::start(vec![
            Script {
                pushes: vec![bbo(1)],
                close_after: true,
                ..Default::default()
            },
            Script {
                reject_handshake: true,
                ..Default::default()
            },
            Script {
                pushes: vec![bbo(2)],
                ..Default::default()
            },
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 1));
        assert!(matches!(next_event(&mut ws).await, Event::Reconnected));
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 2));
        assert_eq!(server.connection_count(), 3);
    }

    #[tokio::test]
    async fn a_refusal_on_reconnect_is_fatal_and_ends_the_stream() {
        let server = scripted::start(vec![
            Script {
                pushes: vec![bbo(1)],
                close_after: true,
                ..Default::default()
            },
            Script {
                refuse: vec![("bbo::1".into(), "invalid channel".into())],
                ..Default::default()
            },
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 1));
        let err = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("an item")
            .expect("stream open");
        assert!(matches!(err, Err(PerpsWsError::Refused { .. })), "{err:?}");
        let end = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("the end");
        assert!(end.is_none(), "{end:?}");
    }

    // The behaviour Story 4.6 changes (L1-D4), each named.

    /// The shared server with perps' answer, for the scripts the adapter
    /// cannot write: a refused or stalled upgrade.
    async fn shared(scripts: Vec<Shared>) -> ScriptedServer {
        ScriptedServer::start_answering(scripts, |_| scripted::answer(Script::default())).await
    }

    fn pushes_then(frames: Vec<String>) -> Shared {
        Shared::messages(
            frames
                .into_iter()
                .map(|frame| Message::Text(frame.into()))
                .collect(),
        )
        .after_first_frame()
    }

    fn is_ping(frame: &str) -> bool {
        frame.contains(r#""type":"ping""#)
    }

    #[tokio::test]
    async fn a_408_or_503_refusal_on_reconnect_is_retried_drift_r1() {
        let server = shared(vec![
            pushes_then(vec![bbo(1)]).then_close(),
            Shared::refuse_upgrade(503),
            Shared::refuse_upgrade(408),
            Shared::refuse_upgrade(425),
            pushes_then(vec![bbo(2)]),
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 1));
        assert!(matches!(next_event(&mut ws).await, Event::Reconnected));
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 2));
        assert_eq!(server.connection_count(), 5);
        ws.close().await.unwrap();
    }

    #[tokio::test]
    async fn a_hung_handshake_on_reconnect_times_out_and_is_retried_drift_r2() {
        let server = shared(vec![
            pushes_then(vec![bbo(1)]).then_close(),
            Shared::stall(),
            pushes_then(vec![bbo(2)]),
        ])
        .await;
        let mut ws = fast()
            .connect_timeout(Duration::from_millis(100))
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 1));
        assert!(matches!(next_event(&mut ws).await, Event::Reconnected));
        assert!(matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == 2));
        assert_eq!(server.connection_count(), 3);
        ws.close().await.unwrap();
    }

    #[tokio::test]
    async fn pings_go_out_while_the_queue_is_full_ad_11() {
        // More pushes than the queue holds, nobody reading for two
        // staleness windows: the task stops reading but keeps pinging, and
        // the wait is not silence, so nothing is replaced and nothing lost.
        let pushes: Vec<String> = (1..=1100).map(bbo).collect();
        let server = scripted::start(vec![Script {
            pushes,
            ..Default::default()
        }])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        server
            .wait_for("the first ping", |s| s.count_frames(is_ping) >= 1)
            .await;
        let before = server.count_frames(is_ping);
        tokio::time::sleep(Duration::from_millis(600)).await;
        let during = server.count_frames(is_ping) - before;
        assert!(during >= 6, "pings behind the full queue: {during}");
        for sq in 1..=1100 {
            assert!(
                matches!(next_event(&mut ws).await, Event::Update(u) if u.sq == sq),
                "update {sq}"
            );
        }
        let after = tokio::time::timeout(Duration::from_millis(150), ws.next()).await;
        assert!(after.is_err(), "expected nothing more, got {after:?}");
        assert_eq!(server.connection_count(), 1);
        ws.close().await.unwrap();
    }

    #[tokio::test]
    async fn a_ping_goes_out_on_schedule_though_the_last_is_unanswered() {
        // The ping is not awaited: with no pong ever, one still goes out
        // every interval, where awaiting each for an interval halved them.
        let server = scripted::start(vec![Script {
            answer_pings: false,
            ..Default::default()
        }])
        .await;
        let ws = fast()
            .stale_after(Duration::MAX)
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        let pings = server.count_frames(is_ping);
        assert!(pings >= 8, "one ping per 50 ms interval; saw {pings}");
        ws.close().await.unwrap();
    }

    #[tokio::test]
    async fn an_empty_membership_stays_wanted_and_a_reconnect_replays_nothing() {
        // Unsubscribing every channel keeps the connection; the first one
        // does not answer pings, so it goes stale and is replaced, and the
        // fresh connection gets no subscription at all, where a reconnect
        // with no channels used to end the stream with `EmptySubscription`.
        let server = scripted::start(vec![
            Script {
                answer_pings: false,
                ..Default::default()
            },
            Script::default(),
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        ws.membership()
            .unsubscribe([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Reconnected));
        server
            .wait_for("a ping on the fresh connection", |s| {
                s.received()
                    .iter()
                    .any(|r| r.connection == 1 && is_ping(&r.text))
            })
            .await;
        let replayed: Vec<String> = server
            .received()
            .into_iter()
            .filter(|r| r.connection == 1 && !is_ping(&r.text))
            .map(|r| r.text)
            .collect();
        assert!(replayed.is_empty(), "replayed {replayed:?}");
        let quiet = tokio::time::timeout(Duration::from_millis(150), ws.next()).await;
        assert!(quiet.is_err(), "the stream must not end: {quiet:?}");
        ws.close().await.unwrap();
    }

    /// Records the target and level of every event, for the log-target test.
    #[derive(Clone, Default)]
    struct Targets(Arc<Mutex<Vec<(String, tracing::Level)>>>);

    impl tracing::Subscriber for Targets {
        fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            tracing::span::Id::from_u64(1)
        }
        fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
        fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
        fn event(&self, event: &tracing::Event<'_>) {
            let metadata = event.metadata();
            self.0
                .lock()
                .unwrap()
                .push((metadata.target().to_owned(), *metadata.level()));
        }
        fn enter(&self, _: &tracing::span::Id) {}
        fn exit(&self, _: &tracing::span::Id) {}
    }

    #[tokio::test]
    async fn supervision_logs_under_the_polyoxide_ws_target() {
        // The current-thread runtime runs the supervising task on this
        // thread, so the scoped subscriber sees its events.
        let targets = Targets::default();
        let _scope = tracing::subscriber::set_default(targets.clone());
        let server = scripted::start(vec![
            Script {
                pushes: vec![bbo(1)],
                close_after: true,
                ..Default::default()
            },
            Script::default(),
        ])
        .await;
        let mut ws = fast()
            .url(&server.url)
            .connect([Channel::Bbo(InstrumentId(1))])
            .await
            .unwrap();
        assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
        assert!(matches!(next_event(&mut ws).await, Event::Reconnected));
        let warnings: Vec<String> = targets
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, level)| *level == tracing::Level::WARN)
            .map(|(target, _)| target.clone())
            .collect();
        assert!(
            !warnings.is_empty() && warnings.iter().all(|t| t.starts_with("polyoxide_ws")),
            "the reconnect's warnings: {warnings:?}"
        );
        ws.close().await.unwrap();
    }
}
