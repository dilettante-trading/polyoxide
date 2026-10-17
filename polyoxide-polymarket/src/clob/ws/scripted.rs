//! The scripted-server answerer the supervised sockets' unit tests share, to
//! be passed to `polyoxide_ws::test_server::ScriptedServer::start_answering`.
//!
//! An integration test cannot see this module and writes its own.

use polyoxide_ws::tungstenite::Message;

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
