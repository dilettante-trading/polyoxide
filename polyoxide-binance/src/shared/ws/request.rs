//! Request correlation: the frames Binance answers a request with, and the
//! request itself, sent on the kit's `Connection` and matched by id.

use std::fmt;

use polyoxide_ws::{tungstenite::Message, Connection, Correlated, Kind, WsError};
use serde_json::{json, Value};

use super::SocketError;

/// What a text frame that is not an update is.
pub(crate) enum Answer {
    /// `{"result": .., "id": n}`.
    Result { id: Option<u64>, result: Value },
    /// `{"error": {"code", "msg"}, "id": n}`.
    Error {
        id: Option<u64>,
        code: i64,
        msg: String,
    },
}

impl Answer {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text).ok()?;
        let object = value.as_object()?;
        if object.contains_key("stream") {
            return None;
        }
        let id = object.get("id").and_then(Value::as_u64);
        if let Some(error) = object.get("error") {
            return Some(Self::Error {
                id,
                code: error
                    .get("code")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                msg: error
                    .get("msg")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            });
        }
        if object.contains_key("result") || object.contains_key("id") {
            return Some(Self::Result {
                id,
                result: object.get("result").cloned().unwrap_or(Value::Null),
            });
        }
        None
    }

    fn id(&self) -> Option<u64> {
        match self {
            Self::Result { id, .. } | Self::Error { id, .. } => *id,
        }
    }
}

/// The request `method` with `params`, for request `id`.
pub(crate) fn request_frame<N: fmt::Display>(
    method: &str,
    params: Option<&[N]>,
    id: u64,
) -> Message {
    let mut body = json!({ "method": method, "id": id });
    if let Some(params) = params {
        let names: Vec<String> = params.iter().map(ToString::to_string).collect();
        body["params"] = json!(names);
    }
    Message::Text(body.to_string().into())
}

/// What a frame read while request `id` awaits its answer is: the answer, an
/// answer to another request, or anything else, which the stream yields
/// afterwards.
fn correlate(id: u64, message: &Message) -> Correlated<Answer> {
    let Message::Text(text) = message else {
        return Correlated::Push;
    };
    match Answer::parse(text) {
        Some(answer) if answer.id() == Some(id) => Correlated::Answer(answer),
        Some(other) => {
            tracing::debug!(id = ?other.id(), "uncorrelated answer while awaiting {id}");
            Correlated::Uncorrelated
        }
        None => Correlated::Push,
    }
}

/// Send request `method` with `params` on `connection`, paced as the
/// connection is, and wait for its answer's `result`. A request left
/// unanswered for the connection's answer timeout is the error's `no_answer`
/// with its id.
pub(crate) async fn request<N: fmt::Display, E: SocketError>(
    connection: &mut Connection,
    method: &str,
    params: Option<&[N]>,
) -> Result<Value, E> {
    let id = connection.next_id();
    let answer = connection
        .request(|id| request_frame(method, params, id), correlate)
        .await
        .map_err(|err| no_answer::<E>(err, id))?;
    match answer {
        Answer::Result { result, .. } => Ok(result),
        Answer::Error { code, msg, .. } => Err(E::refused(code, msg)),
    }
}

/// The kit's `NoAnswer` as the product's, with the id of what went
/// unanswered; every other failure as the socket error maps it.
pub(crate) fn no_answer<E: SocketError>(err: WsError, id: u64) -> E {
    match err.kind() {
        Kind::NoAnswer { after } => E::no_answer(id, after),
        _ => E::from(err),
    }
}
