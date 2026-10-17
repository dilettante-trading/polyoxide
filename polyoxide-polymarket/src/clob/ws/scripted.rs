//! The scripted-server answerer the supervised sockets' unit tests share, to
//! be passed to `polyoxide_ws::test_server::ScriptedServer::start_answering`.
//!
//! An integration test cannot see this module and writes its own.

use std::sync::LazyLock;

use polyoxide_ws::tungstenite::Message;
use tracing::subscriber::NoSubscriber;
use tracing::Dispatch;

/// A dispatcher that enables nothing, kept registered for the life of the
/// process.
static SECOND_DISPATCHER: LazyLock<Dispatch> =
    LazyLock::new(|| Dispatch::new(NoSubscriber::default()));

/// Registers a second `tracing` dispatcher for the rest of the process.
///
/// Every supervised-socket test builds its client through a builder that calls
/// this. With exactly one dispatcher registered, tracing-core works out the
/// interest of a callsite another test's thread hits first from that thread's
/// default, which has none, and caches it as never for every thread. The
/// supervisor's callsites in `polyoxide-ws` are hit by these tests on threads
/// of their own, and a test in this binary that listens for them under
/// `set_default` (perps' `supervision_logs_under_the_polyoxide_ws_target`)
/// then sees nothing. With a second dispatcher registered, tracing-core asks
/// each registered one instead. `polyoxide-binance`'s
/// `tests/supervision_usdm_kit.rs` holds a second `Dispatch` for the same
/// reason, for one test's duration.
pub(crate) fn hold_a_second_dispatcher() {
    LazyLock::force(&SECOND_DISPATCHER);
}

/// One connection's answerer: it answers the keep-alive text `PING` with a
/// text `PONG`, as the venue does, and every other frame with nothing, the
/// subscription frames included.
pub(crate) fn pong(_connection: usize) -> impl FnMut(&str) -> Vec<Message> + Send + 'static {
    |text| {
        if text == "PING" {
            vec![Message::Text("PONG".into())]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ping_is_answered_with_a_pong_and_nothing_else_is_answered() {
        let mut answer = pong(0);
        assert_eq!(answer("PING"), vec![Message::Text("PONG".into())]);
        assert_eq!(answer("PING"), vec![Message::Text("PONG".into())]);
        assert!(answer(r#"{"assets_ids":[],"type":"market"}"#).is_empty());
        assert!(answer(r#"{"operation":"subscribe","assets_ids":["a"]}"#).is_empty());
        assert!(answer("ping").is_empty());
        assert!(answer("").is_empty());
    }
}
