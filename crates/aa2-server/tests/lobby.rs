use std::path::PathBuf;
use std::time::Duration;

use aa2_net::{ClientMsg, Phase, ServerMsg};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

async fn send(ws: &mut futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>, msg: &ClientMsg) {
    let text = serde_json::to_string(msg).unwrap();
    ws.send(Message::text(text)).await.unwrap();
}

async fn recv_until<F>(read: &mut futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>, pred: F) -> ServerMsg
where
    F: Fn(&ServerMsg) -> bool,
{
    let deadline = Duration::from_secs(2);
    timeout(deadline, async {
        loop {
            let m = read.next().await.unwrap().unwrap();
            if let Ok(t) = m.to_text()
                && let Ok(sm) = serde_json::from_str::<ServerMsg>(t)
                && pred(&sm)
            {
                return sm;
            }
        }
    })
    .await
    .expect("timed out waiting for expected message")
}

#[tokio::test]
async fn lobby_join_start() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(aa2_server::serve(listener, PathBuf::from("../../data"), 42));

    // Client 0
    let (ws0, _) = tokio_tungstenite::connect_async(format!("ws://{addr}")).await.unwrap();
    let (mut w0, mut r0) = ws0.split();

    // Client 1
    let (ws1, _) = tokio_tungstenite::connect_async(format!("ws://{addr}")).await.unwrap();
    let (mut w1, mut r1) = ws1.split();

    // c0 joins
    send(&mut w0, &ClientMsg::Join { name: "a".into() }).await;
    let welcome0 = recv_until(&mut r0, |m| matches!(m, ServerMsg::Welcome { .. })).await;
    assert_eq!(welcome0, ServerMsg::Welcome { your_player_id: 0, player_count: 1 });

    let lobby0 = recv_until(&mut r0, |m| matches!(m, ServerMsg::Lobby { .. })).await;
    match &lobby0 {
        ServerMsg::Lobby { seats } => {
            assert_eq!(seats[0], Some("a".into()));
        }
        _ => panic!("expected Lobby"),
    }

    // c1 joins
    send(&mut w1, &ClientMsg::Join { name: "b".into() }).await;
    let welcome1 = recv_until(&mut r1, |m| matches!(m, ServerMsg::Welcome { .. })).await;
    assert_eq!(welcome1, ServerMsg::Welcome { your_player_id: 1, player_count: 2 });

    // c1 gets lobby with both seats
    let lobby1 = recv_until(&mut r1, |m| matches!(m, ServerMsg::Lobby { .. })).await;
    match &lobby1 {
        ServerMsg::Lobby { seats } => {
            assert_eq!(seats[0], Some("a".into()));
            assert_eq!(seats[1], Some("b".into()));
        }
        _ => panic!("expected Lobby"),
    }

    // c0 also gets the updated lobby broadcast from c1 joining
    let lobby0b = recv_until(&mut r0, |m| match m {
        ServerMsg::Lobby { seats } => seats[1].is_some(),
        _ => false,
    }).await;
    match &lobby0b {
        ServerMsg::Lobby { seats } => {
            assert_eq!(seats[0], Some("a".into()));
            assert_eq!(seats[1], Some("b".into()));
        }
        _ => panic!("expected Lobby"),
    }

    // c0 sends Start
    send(&mut w0, &ClientMsg::Start).await;

    // Both clients receive PhaseChange GodPick
    let pc0 = recv_until(&mut r0, |m| matches!(m, ServerMsg::PhaseChange { .. })).await;
    match &pc0 {
        ServerMsg::PhaseChange { phase, .. } => assert_eq!(*phase, Phase::GodPick),
        _ => panic!("expected PhaseChange"),
    }

    let pc1 = recv_until(&mut r1, |m| matches!(m, ServerMsg::PhaseChange { .. })).await;
    match &pc1 {
        ServerMsg::PhaseChange { phase, .. } => assert_eq!(*phase, Phase::GodPick),
        _ => panic!("expected PhaseChange"),
    }
}
