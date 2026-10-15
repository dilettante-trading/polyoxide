//! The runner every `stream` command shares: `-n`, `-t`, the end of the feed,
//! a reader that went away, and how each event is printed.
//!
//! A command describes its socket as a [`Feed`]: what each event and each
//! error means. [`run`] owns the limits and the end of the run, so every
//! stream stops, and reports why, the same way.

use std::{
    io::{self, Write},
    time::Duration,
};

use color_eyre::eyre::Result;
use futures_util::{Stream, StreamExt};

/// How each event is printed.
#[derive(Debug, Clone, Copy, clap::ValueEnum, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable, one line per event.
    #[default]
    Pretty,
    /// The event as compact JSON, one object per line.
    Json,
}

/// What a [`Feed`] did with one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handled {
    /// It wrote the event to `out`, which counts towards `-n`.
    Printed,
    /// It wrote nothing to `out`: the event was filtered out, or was a marker
    /// written to `err`.
    Skipped,
}

/// A command's half of a run: what each event and each error means.
pub trait Feed {
    /// What the stream yields.
    type Event;
    /// What the stream fails with.
    type Error;

    /// Write an update to `out`, or a marker to `err`, and say which. An
    /// error writing to `out` because its reader went away, as with
    /// `| head -1`, ends the run quietly; any other error ends it with that
    /// error.
    fn event(
        &mut self,
        event: Self::Event,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<Handled>;

    /// Write an error the feed carries on after, such as one frame that did
    /// not parse, to `err` and return `Ok`; or return the error to end the
    /// run with it.
    fn error(&mut self, error: Self::Error, err: &mut dyn Write) -> Result<()>;
}

/// `-n` and `-t`: when a run stops on its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Limits {
    /// Stop once this many events are printed.
    pub count: Option<u64>,
    /// Stop once this long has passed.
    pub timeout: Option<Duration>,
}

/// Run `feed` over `events` until `-n` or `-t` is reached, the stream ends,
/// the reader of `out` goes away, or `feed` ends the run with an error.
///
/// Updates go to `out`. Why the run stopped, and every marker, go to `err`,
/// so JSON output stays clean JSONL.
pub async fn run<S, F>(
    limits: Limits,
    mut events: S,
    feed: &mut F,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<()>
where
    S: Stream<Item = std::result::Result<F::Event, F::Error>> + Unpin,
    F: Feed,
{
    // A timeout too large to add, such as `-t 18446744073709551615s`, means
    // no deadline rather than a panic.
    let deadline = limits
        .timeout
        .and_then(|t| tokio::time::Instant::now().checked_add(t));
    let mut out = Watched::new(out);
    let mut printed: u64 = 0;
    loop {
        // Checked before waiting, so a feed that goes quiet after the last
        // wanted event ends the run then, not on the next frame.
        if let Some(n) = limits.count {
            if printed >= n {
                writeln!(err, "Reached {n} update(s)")?;
                break;
            }
        }
        // Checked before waiting too: `timeout_at` polls the stream first, so
        // a stream that is always ready, such as a backlog, would never let
        // the timer fire.
        if deadline.is_some_and(|deadline| tokio::time::Instant::now() >= deadline) {
            writeln!(err, "Timeout reached")?;
            break;
        }
        let next = match deadline {
            Some(deadline) => match tokio::time::timeout_at(deadline, events.next()).await {
                Ok(next) => next,
                Err(_) => {
                    writeln!(err, "Timeout reached")?;
                    break;
                }
            },
            None => events.next().await,
        };
        match next {
            Some(Ok(event)) => match feed.event(event, &mut out, err) {
                Ok(Handled::Printed) => printed += 1,
                Ok(Handled::Skipped) => {}
                // The reader went away, as with `| head -1`. That ends the
                // run; it is not an error.
                Err(_) if out.closed => break,
                Err(error) => return Err(error),
            },
            Some(Err(error)) => feed.error(error, err)?,
            None => {
                writeln!(err, "The feed ended")?;
                break;
            }
        }
    }
    Ok(())
}

/// The first 200 characters of a frame from the server, escaped, so an
/// oversized frame or one carrying newlines or terminal escapes stays one
/// readable stderr line.
pub fn excerpt(raw: &str) -> String {
    const LIMIT: usize = 200;
    let escaped: String = raw
        .chars()
        .take(LIMIT)
        .flat_map(char::escape_debug)
        .collect();
    if raw.chars().count() > LIMIT {
        escaped + "…"
    } else {
        escaped
    }
}

/// `out`, noting whether a write failed because its reader went away.
struct Watched<'a> {
    inner: &'a mut dyn Write,
    closed: bool,
}

impl<'a> Watched<'a> {
    fn new(inner: &'a mut dyn Write) -> Self {
        Self {
            inner,
            closed: false,
        }
    }

    fn note<T>(&mut self, result: io::Result<T>) -> io::Result<T> {
        if matches!(&result, Err(error) if error.kind() == io::ErrorKind::BrokenPipe) {
            self.closed = true;
        }
        result
    }
}

impl Write for Watched<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let result = self.inner.write(buf);
        self.note(result)
    }

    fn flush(&mut self) -> io::Result<()> {
        let result = self.inner.flush();
        self.note(result)
    }
}

#[cfg(test)]
mod tests {
    use futures_util::stream;

    use super::*;

    /// A feed of numbers: each even one printed, each odd one skipped with a
    /// marker, an error of `0` skipped and any other error fatal.
    struct Numbers;

    impl Feed for Numbers {
        type Event = u32;
        type Error = u32;

        fn event(&mut self, n: u32, out: &mut dyn Write, err: &mut dyn Write) -> Result<Handled> {
            if n % 2 == 1 {
                writeln!(err, "# odd {n}")?;
                return Ok(Handled::Skipped);
            }
            writeln!(out, "{n}")?;
            out.flush()?;
            Ok(Handled::Printed)
        }

        fn error(&mut self, error: u32, err: &mut dyn Write) -> Result<()> {
            if error == 0 {
                writeln!(err, "# skipped error {error}")?;
                return Ok(());
            }
            color_eyre::eyre::bail!("fatal error {error}")
        }
    }

    type Item = std::result::Result<u32, u32>;

    /// What one run wrote, and how it ended.
    struct Run {
        result: Result<()>,
        out: String,
        err: String,
    }

    /// Run `Numbers` over `events`, failing the test if the run hangs.
    async fn run_over<S>(limits: Limits, events: S) -> Run
    where
        S: Stream<Item = Item> + Unpin,
    {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            run(limits, events, &mut Numbers, &mut out, &mut err),
        )
        .await
        .expect("the run ended");
        Run {
            result,
            out: String::from_utf8(out).unwrap(),
            err: String::from_utf8(err).unwrap(),
        }
    }

    #[tokio::test]
    async fn count_stops_after_n_printed_events_without_waiting_for_another() {
        // Skipped events and skipped errors do not count, and the feed stays
        // open, so only reaching `-n` can end this run.
        let events =
            stream::iter(vec![Ok(1), Ok(2), Err(0), Ok(3), Ok(4)]).chain(stream::pending());
        let limits = Limits {
            count: Some(2),
            timeout: None,
        };
        let run = run_over(limits, events).await;
        run.result.unwrap();
        assert_eq!(run.out, "2\n4\n");
        assert!(
            run.err.contains("# odd 1") && run.err.contains("# odd 3"),
            "{}",
            run.err
        );
        assert!(run.err.contains("# skipped error 0"), "{}", run.err);
        assert!(run.err.ends_with("Reached 2 update(s)\n"), "{}", run.err);
    }

    #[tokio::test]
    async fn the_deadline_ends_a_quiet_feed_and_one_too_large_is_none() {
        let events = stream::iter(vec![Ok(2)]).chain(stream::pending());
        let limits = Limits {
            count: None,
            timeout: Some(Duration::from_millis(100)),
        };
        let run = run_over(limits, events).await;
        run.result.unwrap();
        assert_eq!(run.out, "2\n");
        assert!(run.err.ends_with("Timeout reached\n"), "{}", run.err);

        // Adding this to the clock overflows; it means no deadline, so the
        // run ends with the feed rather than panicking.
        let limits = Limits {
            count: None,
            timeout: Some(Duration::from_secs(u64::MAX)),
        };
        let run = run_over(limits, stream::iter(vec![Ok(2)])).await;
        run.result.unwrap();
        assert_eq!(run.out, "2\n");
        assert!(run.err.ends_with("The feed ended\n"), "{}", run.err);
    }

    /// A feed that skips every event without a word.
    struct Silent;

    impl Feed for Silent {
        type Event = u32;
        type Error = u32;

        fn event(&mut self, _: u32, _: &mut dyn Write, _: &mut dyn Write) -> Result<Handled> {
            Ok(Handled::Skipped)
        }

        fn error(&mut self, _: u32, _: &mut dyn Write) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn the_deadline_ends_a_feed_that_is_always_ready() {
        // A backlog never leaves the stream pending, so only a deadline checked
        // before each wait can end this run. The run has a thread and runtime
        // of its own, so a regression fails here instead of hanging the suite.
        let (done, ended) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap();
            let limits = Limits {
                count: None,
                timeout: Some(Duration::from_millis(100)),
            };
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let result = runtime.block_on(run(
                limits,
                stream::repeat(Ok(1)),
                &mut Silent,
                &mut out,
                &mut err,
            ));
            let _ = done.send((result.is_ok(), String::from_utf8(err).unwrap()));
        });
        let (ok, err) = ended
            .recv_timeout(Duration::from_secs(5))
            .expect("the deadline did not end a feed that is always ready");
        assert!(ok);
        assert_eq!(err, "Timeout reached\n");
    }

    /// A writer whose reader has gone away, as stdout is under `| head -1`.
    struct ClosedPipe;

    impl Write for ClosedPipe {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }

    #[tokio::test]
    async fn a_closed_reader_ends_the_run_quietly_and_another_write_error_does_not() {
        let events = stream::iter(vec![Ok(2)]).chain(stream::pending());
        let mut err = Vec::new();
        tokio::time::timeout(
            Duration::from_secs(2),
            run(
                Limits::default(),
                events,
                &mut Numbers,
                &mut ClosedPipe,
                &mut err,
            ),
        )
        .await
        .expect("the run ended when the reader went away")
        .expect("a closed reader is not an error");
        assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));

        /// A writer that fails for some other reason.
        struct Full;
        impl Write for Full {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::StorageFull.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut err = Vec::new();
        let result = run(
            Limits::default(),
            stream::iter(vec![Ok(2)]),
            &mut Numbers,
            &mut Full,
            &mut err,
        )
        .await;
        assert!(result.is_err(), "a full disk ended the run quietly");
    }

    #[tokio::test]
    async fn the_end_of_the_feed_is_reported_and_a_fatal_error_ends_the_run() {
        let run = run_over(Limits::default(), stream::iter(vec![Ok(2), Ok(4)])).await;
        run.result.unwrap();
        assert_eq!(run.out, "2\n4\n");
        assert!(run.err.ends_with("The feed ended\n"), "{}", run.err);

        let run = run_over(Limits::default(), stream::iter(vec![Ok(2), Err(7), Ok(4)])).await;
        let error = run.result.unwrap_err();
        assert!(error.to_string().contains("fatal error 7"), "{error}");
        assert_eq!(run.out, "2\n", "the run went on past a fatal error");
        assert!(!run.err.contains("The feed ended"), "{}", run.err);
    }

    #[test]
    fn an_excerpt_is_one_escaped_line_of_bounded_length() {
        assert_eq!(excerpt("a\nb\u{1b}[31m"), "a\\nb\\u{1b}[31m");
        let long = "y".repeat(300);
        let cut = excerpt(&long);
        assert_eq!(cut.chars().count(), 201);
        assert!(cut.ends_with('…'));
    }
}
