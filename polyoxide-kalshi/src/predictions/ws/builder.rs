//! The builder: Kalshi's socket timings and endpoint, handed to the kit's `Supervisor`
//! with `PredictionsProtocol`.

use std::{collections::BTreeSet, time::Duration};

use polyoxide_ws::{Supervisor, Timing};

use crate::predictions::{
    key::Ticker,
    ws::{
        protocol::PredictionsProtocol, Channel, PredictionsWsError, SupervisedPredictionsWs,
        PRODUCTION_WS,
    },
    Credentials,
};

/// Builder for a supervised socket of Kalshi's channels.
#[derive(Debug, Clone)]
pub struct PredictionsWsBuilder {
    url: String,
    timing: Timing,
    channels: BTreeSet<Channel>,
}

impl Default for PredictionsWsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PredictionsWsBuilder {
    /// Defaults: the production host ([`PRODUCTION_WS`]; the demo's is
    /// [`DEMO_WS`](crate::predictions::ws::DEMO_WS)), a protocol Ping every 10 s, stale
    /// after 30 s of silence, reconnect backoff from 500 ms doubling to 60 s, a 10 s
    /// connect timeout for each attempt, and no rotation.
    pub fn new() -> Self {
        let mut timing = Timing::default();
        timing.ping_interval = Some(Duration::from_secs(10));
        timing.stale_after = Duration::from_secs(30);
        timing.initial_backoff = Duration::from_millis(500);
        timing.max_backoff = Duration::from_secs(60);
        timing.connect_timeout = Duration::from_secs(10);
        Self {
            url: PRODUCTION_WS.to_owned(),
            timing,
            channels: BTreeSet::from([Channel::OrderbookDelta]),
        }
    }

    /// The channels to carry, replacing the default, the order book alone. Duplicates are
    /// dropped, and the order given does not matter: the channels go out, and a membership
    /// change reaches them, in [`Channel`]'s order. At least one is needed, and an empty set
    /// is refused at [`connect`](Self::connect) as [`PredictionsWsError::InvalidRequest`].
    ///
    /// One set of tickers serves every channel. Kalshi refuses a `ticker` or `trade`
    /// subscribe with no markets (code 14 on the demo host, 2026-10-21) and the protocol
    /// never sends one, so an empty ticker set is no subscription on any channel.
    pub fn channels(mut self, channels: impl IntoIterator<Item = Channel>) -> Self {
        self.channels = channels.into_iter().collect();
        self
    }

    /// Connect to a different endpoint, such as the demo's
    /// ([`DEMO_WS`](crate::predictions::ws::DEMO_WS)). The URL must be `wss://`, or
    /// `ws://` to a loopback host (`127.0.0.1`, `[::1]` or `localhost`), so a signed
    /// handshake never travels unencrypted to a remote host; anything else, and a URL
    /// that does not parse, is refused with [`PredictionsWsError::InvalidUrl`].
    pub fn url(mut self, url: &str) -> Result<Self, PredictionsWsError> {
        self.url = checked_url(url)?;
        Ok(self)
    }

    polyoxide_ws::timing_setters! {
        /// How often to send a protocol Ping, whatever the traffic (default 10 s, the
        /// cadence of Kalshi's own). The server's Pings count as liveness too, so this
        /// only keeps a quiet connection answering.
        ping_interval;
        /// Silence after which the connection is presumed dead and replaced (default
        /// 30 s). Any frame counts, the server's Pings included. Also bounds each
        /// membership change and each replay: one Kalshi leaves unanswered that long
        /// replaces the connection.
        stale_after;
        /// Reconnect delay schedule: `initial`, doubling to `max` (default 500 ms to
        /// 60 s). There is no cap on attempts. A refused upgrade's `Retry-After` only
        /// lengthens the wait, up to `max`.
        backoff;
        /// How long one connection attempt may take: the TCP connect, the TLS handshake
        /// and the upgrade together (default 10 s). A timed out attempt is retried like
        /// any other lost connection.
        connect_timeout;
    }

    /// Connect, sign the handshake, subscribe to `tickers` on every channel chosen and start
    /// supervising. With no tickers the connection opens and subscribes nothing, on any
    /// channel; the first
    /// [`subscribe`](crate::predictions::ws::PredictionsMembership::subscribe) does.
    ///
    /// The first attempt's failure is returned, not retried, so a key Kalshi refuses or a
    /// URL that is wrong is reported at once; later refusals arrive through the stream and
    /// [`PredictionsMembership`](crate::predictions::ws::PredictionsMembership).
    pub async fn connect(
        self,
        credentials: Credentials,
        tickers: impl IntoIterator<Item = Ticker>,
    ) -> Result<SupervisedPredictionsWs, PredictionsWsError> {
        if self.channels.is_empty() {
            return Err(PredictionsWsError::InvalidRequest {
                reason: "the socket needs at least one channel".to_owned(),
            });
        }
        let membership: BTreeSet<Ticker> = tickers.into_iter().collect();
        let protocol = PredictionsProtocol::new(self.url, credentials)
            .configured(self.channels, self.timing.stale_after);
        Supervisor::new(protocol, self.timing)
            .connect(membership)
            .await
    }
}

/// The URL a socket takes: `wss://`, or `ws://` to a loopback host.
fn checked_url(url: &str) -> Result<String, PredictionsWsError> {
    let invalid = |reason: &'static str| PredictionsWsError::InvalidUrl {
        url: url.to_owned(),
        reason,
    };
    let parsed = url::Url::parse(url).map_err(|_| invalid("it does not parse"))?;
    let allowed = match parsed.scheme() {
        "wss" => true,
        "ws" => match parsed.host() {
            Some(url::Host::Domain(host)) => host == "localhost",
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            None => false,
        },
        _ => false,
    };
    if allowed {
        Ok(url.to_owned())
    } else {
        Err(invalid("it must be wss://, or ws:// to a loopback host"))
    }
}

#[cfg(test)]
mod tests {
    use polyoxide_venue::{Class, Classify};

    use super::*;
    use crate::predictions::ws::DEMO_WS;

    #[test]
    fn defaults_match_kalshis_cadences() {
        let builder = PredictionsWsBuilder::new();
        assert_eq!(builder.url, PRODUCTION_WS);
        assert_eq!(builder.timing.ping_interval, Some(Duration::from_secs(10)));
        assert_eq!(builder.timing.stale_after, Duration::from_secs(30));
        assert_eq!(builder.timing.initial_backoff, Duration::from_millis(500));
        assert_eq!(builder.timing.max_backoff, Duration::from_secs(60));
        assert_eq!(builder.timing.connect_timeout, Duration::from_secs(10));
        assert_eq!(builder.timing.max_connection_age, None);
    }

    #[test]
    fn setters_write_the_timing() {
        let builder = PredictionsWsBuilder::new()
            .ping_interval(Duration::from_secs(3))
            .stale_after(Duration::from_secs(9))
            .backoff(Duration::from_millis(5), Duration::from_millis(50))
            .connect_timeout(Duration::from_secs(2));
        assert_eq!(builder.timing.ping_interval, Some(Duration::from_secs(3)));
        assert_eq!(builder.timing.stale_after, Duration::from_secs(9));
        assert_eq!(builder.timing.initial_backoff, Duration::from_millis(5));
        assert_eq!(builder.timing.max_backoff, Duration::from_millis(50));
        assert_eq!(builder.timing.connect_timeout, Duration::from_secs(2));
    }

    #[test]
    fn the_url_rule_is_wss_or_a_loopback_ws() {
        for good in [
            PRODUCTION_WS,
            DEMO_WS,
            "ws://127.0.0.1:8080/trade-api/ws/v2",
            "ws://localhost:9/x",
            "ws://[::1]:9/x",
        ] {
            let builder = PredictionsWsBuilder::new().url(good).unwrap();
            assert_eq!(builder.url, good);
        }
        for bad in [
            "ws://example.com/trade-api/ws/v2",
            "ws://192.168.0.1:9/x",
            "http://127.0.0.1:9/x",
            "not a url",
        ] {
            let err = PredictionsWsBuilder::new().url(bad).unwrap_err();
            assert!(
                matches!(&err, PredictionsWsError::InvalidUrl { url, .. } if url == bad),
                "{bad}: {err:?}"
            );
            assert_eq!(err.class(), Class::InvalidRequest);
        }
    }
    #[test]
    fn the_default_channel_is_the_order_book_and_channels_are_deduplicated_in_order() {
        assert_eq!(
            PredictionsWsBuilder::new().channels,
            BTreeSet::from([Channel::OrderbookDelta])
        );
        let builder = PredictionsWsBuilder::new().channels([
            Channel::Trade,
            Channel::OrderbookDelta,
            Channel::Trade,
        ]);
        assert_eq!(
            builder.channels.iter().copied().collect::<Vec<_>>(),
            [Channel::OrderbookDelta, Channel::Trade]
        );
        // An empty set is the builder's to hold and `connect`'s to refuse.
        assert!(PredictionsWsBuilder::new()
            .channels(Vec::new())
            .channels
            .is_empty());
    }
}
