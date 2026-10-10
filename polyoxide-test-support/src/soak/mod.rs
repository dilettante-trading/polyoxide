//! What the rate-limit harnesses share: a pacer that drives a fixed rate, the
//! percentile they report, their stage and route arguments, an observer for
//! the throttles a client retries away ([`observe`]), and the two rulebooks
//! that judge a stage ([`verdict`]).
//!
//! A harness asks what rate a server tolerates, which means driving a rate the
//! client would not pick, outside the client's own limiter. A harness that
//! paces through a polyoxide client can only go below that client's sustained
//! rate, or its limiter binds first and the run measures polyoxide instead of
//! the server.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

pub mod observe;
pub mod verdict;

/// Hands out send slots at a fixed interval, shared by every worker.
///
/// A slot is never in the past: an idle pacer does not bank credit and
/// release it as a burst when traffic resumes, which is the defect of a token
/// bucket with depth that the harnesses exist to measure the absence of.
#[derive(Debug)]
pub struct Pacer {
    interval: Duration,
    /// The earliest unclaimed slot; `None` until the first reservation.
    next: Mutex<Option<Instant>>,
}

impl Pacer {
    /// A pacer handing out one slot per `interval`.
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            next: Mutex::new(None),
        }
    }

    /// Claims the next slot, given the current time.
    pub fn reserve(&self, now: Instant) -> Instant {
        let mut next = self
            .next
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let slot = next.map_or(now, |claimed| claimed.max(now));
        *next = Some(slot + self.interval);
        slot
    }

    /// Claims the next slot and sleeps until it.
    pub async fn wait(&self) {
        let slot = self.reserve(Instant::now());
        tokio::time::sleep_until(tokio::time::Instant::from_std(slot)).await;
    }
}

/// Nearest-rank percentile over an ascending slice; zero for an empty one.
pub fn percentile(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let last = sorted.len() - 1;
    let rank = (p / 100.0 * last as f64).round() as usize;
    sorted[rank.min(last)]
}

/// Parses `--stages`: comma-separated rates in requests per second, each
/// positive and at most `ceiling`, strictly ascending.
pub fn parse_stages(raw: &str, ceiling: f64) -> Result<Vec<f64>, String> {
    let stages: Vec<f64> = raw
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<f64>()
                .map_err(|_| format!("bad stage rate: {s:?}"))
        })
        .collect::<Result<_, _>>()?;
    if stages.is_empty() {
        return Err("--stages needs at least one rate".into());
    }
    for rate in &stages {
        if !rate.is_finite() || *rate <= 0.0 {
            return Err(format!("stage rate {rate} must be positive"));
        }
        if *rate > ceiling {
            return Err(format!(
                "stage rate {rate} is above the {ceiling} req/s ceiling"
            ));
        }
    }
    if stages.windows(2).any(|w| w[1] <= w[0]) {
        return Err("--stages must be strictly ascending".into());
    }
    Ok(stages)
}

/// Parses `--route`: `all`, or comma-separated names, each one of `all`'s
/// spelled as `name` spells it, none listed twice.
pub fn parse_routes<R: Copy + PartialEq>(
    raw: &str,
    all: &[R],
    name: impl Fn(R) -> &'static str,
) -> Result<Vec<R>, String> {
    if raw == "all" {
        return Ok(all.to_vec());
    }
    let routes = raw
        .split(',')
        .map(|s| {
            let s = s.trim();
            all.iter().copied().find(|r| name(*r) == s).ok_or_else(|| {
                let names: Vec<_> = all.iter().map(|r| name(*r)).collect();
                format!("unknown route {s:?}; expected one of {}", names.join(", "))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (i, route) in routes.iter().enumerate() {
        if routes[..i].contains(route) {
            return Err(format!("{} is listed twice", name(*route)));
        }
    }
    Ok(routes)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── pacer ───────────────────────────────────────────────────

    const TEN_MS: Duration = Duration::from_millis(10);

    #[test]
    fn pacer_hands_out_the_first_slot_immediately() {
        let now = Instant::now();
        assert_eq!(Pacer::new(TEN_MS).reserve(now), now);
    }

    #[test]
    fn pacer_spaces_consecutive_slots_by_the_interval() {
        let pacer = Pacer::new(TEN_MS);
        let now = Instant::now();

        assert_eq!(pacer.reserve(now), now);
        assert_eq!(pacer.reserve(now), now + TEN_MS);
        assert_eq!(pacer.reserve(now), now + 2 * TEN_MS);
    }

    #[test]
    fn pacer_does_not_bank_credit_while_idle() {
        // The whole point of the harness is to hold a rate, and a pacer that
        // carries its cursor forward from an idle period releases the backlog
        // in one burst the moment traffic resumes — the same defect as a token
        // bucket with depth, which is what this run exists to measure the
        // absence of. A slot may never be in the past.
        let pacer = Pacer::new(TEN_MS);
        let start = Instant::now();
        pacer.reserve(start);

        let after_a_long_stall = start + Duration::from_secs(10);
        assert_eq!(
            pacer.reserve(after_a_long_stall),
            after_a_long_stall,
            "the pacer banked credit during the stall and would now burst"
        );
        assert_eq!(
            pacer.reserve(after_a_long_stall),
            after_a_long_stall + TEN_MS,
            "the cursor did not resume from the stall, so the backlog survives it"
        );
    }

    #[test]
    fn pacer_never_hands_out_a_slot_in_the_past() {
        let pacer = Pacer::new(Duration::from_millis(10));
        let start = Instant::now();
        pacer.reserve(start);
        let later = start + Duration::from_secs(10);
        assert_eq!(pacer.reserve(later), later);
        assert_eq!(pacer.reserve(later), later + Duration::from_millis(10));
    }

    #[test]
    fn percentile_of_empty_is_zero() {
        assert_eq!(percentile(&[], 50.0), Duration::ZERO);
    }

    #[test]
    fn percentile_picks_by_nearest_rank() {
        let sorted: Vec<Duration> = (1..=100).map(Duration::from_millis).collect();
        assert_eq!(percentile(&sorted, 50.0), Duration::from_millis(51));
        assert_eq!(percentile(&sorted, 99.0), Duration::from_millis(99));
        assert_eq!(percentile(&sorted, 100.0), Duration::from_millis(100));
    }

    // ── arguments ───────────────────────────────────────────────

    #[test]
    fn stages_are_positive_ascending_and_under_the_ceiling() {
        assert_eq!(parse_stages("3, 5,7.5", 40.0), Ok(vec![3.0, 5.0, 7.5]));
        assert_eq!(parse_stages("40", 40.0), Ok(vec![40.0]));
        assert_eq!(
            parse_stages("41", 40.0),
            Err("stage rate 41 is above the 40 req/s ceiling".into())
        );
        assert_eq!(
            parse_stages("5,5", 40.0),
            Err("--stages must be strictly ascending".into())
        );
        assert_eq!(
            parse_stages("-1", 40.0),
            Err("stage rate -1 must be positive".into())
        );
        assert_eq!(parse_stages("", 40.0), Err("bad stage rate: \"\"".into()));
        assert!(parse_stages("inf", 40.0).is_err());
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Route {
        Trades,
        Book,
    }

    const ALL: [Route; 2] = [Route::Trades, Route::Book];

    fn name(route: Route) -> &'static str {
        match route {
            Route::Trades => "trades",
            Route::Book => "book",
        }
    }

    #[test]
    fn routes_parse_from_all_or_a_list_by_name() {
        assert_eq!(parse_routes("all", &ALL, name), Ok(ALL.to_vec()));
        assert_eq!(
            parse_routes(" book ,trades", &ALL, name),
            Ok(vec![Route::Book, Route::Trades])
        );
    }

    #[test]
    fn an_unknown_route_lists_the_names_and_a_repeat_is_refused() {
        assert_eq!(
            parse_routes("trades,nope", &ALL, name),
            Err("unknown route \"nope\"; expected one of trades, book".into())
        );
        assert_eq!(
            parse_routes("book,trades,book", &ALL, name),
            Err("book is listed twice".into())
        );
        // An unknown name is reported before a repeat.
        assert!(parse_routes("book,book,nope", &ALL, name)
            .unwrap_err()
            .starts_with("unknown route"));
    }
}
