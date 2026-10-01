//! The bare tier against the scripted server.

use std::time::Duration;

use futures_util::StreamExt;
use polyoxide_sports::{
    fixtures,
    test_server::{Script, ScriptedServer},
    SportsError, SportsWs,
};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

const WINDOW: Duration = Duration::from_secs(3);

fn text(frame: &str) -> Message {
    Message::Text(frame.into())
}

#[tokio::test]
async fn yields_updates_and_skips_protocol_pings() {
    let server = ScriptedServer::start(vec![Script {
        send: vec![Message::Ping(b"p".to_vec().into()), text(fixtures::SOCCER)],
        ..Script::silent()
    }])
    .await;
    let mut feed = SportsWs::connect_to(&server.url).await.unwrap();
    let update = timeout(WINDOW, feed.next())
        .await
        .expect("an update within the window")
        .expect("the stream is open")
        .expect("the frame parses");
    assert_eq!(update.league_abbreviation, "kor");
}

#[tokio::test]
async fn answers_a_protocol_ping_with_a_matching_pong() {
    let server = ScriptedServer::start(vec![Script {
        send: vec![Message::Ping(b"p1".to_vec().into())],
        ..Script::silent()
    }])
    .await;
    let mut feed = SportsWs::connect_to(&server.url).await.unwrap();
    let reader = tokio::spawn(async move { while feed.next().await.is_some() {} });
    server
        .wait_for("a pong", |s| s.pongs() == [b"p1".to_vec()])
        .await;
    reader.abort();
}

#[tokio::test]
async fn reports_a_bad_frame_and_keeps_reading() {
    let server = ScriptedServer::start(vec![Script {
        send: vec![text("not json"), text(fixtures::SOCCER)],
        ..Script::silent()
    }])
    .await;
    let mut feed = SportsWs::connect_to(&server.url).await.unwrap();
    match timeout(WINDOW, feed.next()).await.unwrap() {
        Some(Err(SportsError::Decode { raw, .. })) => assert_eq!(raw, "not json"),
        other => panic!("expected a decode error, got {other:?}"),
    }
    let update = timeout(WINDOW, feed.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(update.league_abbreviation, "kor");
}

#[tokio::test]
async fn ends_when_the_server_closes() {
    let server =
        ScriptedServer::start(vec![Script::frames(&[fixtures::SOCCER]).then_close()]).await;
    let mut feed = SportsWs::connect_to(&server.url).await.unwrap();
    assert!(matches!(
        timeout(WINDOW, feed.next()).await.unwrap(),
        Some(Ok(_))
    ));
    assert!(timeout(WINDOW, feed.next()).await.unwrap().is_none());
    assert!(
        timeout(WINDOW, feed.next()).await.unwrap().is_none(),
        "the stream yielded again after ending"
    );
}

#[tokio::test]
async fn replies_to_the_servers_close_frame() {
    let server =
        ScriptedServer::start(vec![Script::frames(&[fixtures::SOCCER]).then_close()]).await;
    let mut feed = SportsWs::connect_to(&server.url).await.unwrap();
    assert!(matches!(
        timeout(WINDOW, feed.next()).await.unwrap(),
        Some(Ok(_))
    ));
    assert!(timeout(WINDOW, feed.next()).await.unwrap().is_none());
    server
        .wait_for("the client's close reply", |s| s.close_reply_count() == 1)
        .await;
}

#[tokio::test]
async fn dropping_the_stream_closes_the_socket() {
    let server = ScriptedServer::start(vec![Script::silent()]).await;
    let feed = SportsWs::connect_to(&server.url).await.unwrap();
    server
        .wait_for("the handshake", |s| s.handshake_count() == 1)
        .await;
    drop(feed);
    server
        .wait_for("the client to end the connection", |s| {
            s.client_ended_count() == 1
        })
        .await;
}
