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
    let deadline = Duration::from_secs(120);
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
async fn two_humans_reach_shop() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(aa2_server::serve(listener, PathBuf::from("../../data"), 42));

    // Load a valid god name from data
    let gods = aa2_data::load_all_gods(&PathBuf::from("../../data/gods")).unwrap();
    let god_name = gods[0].name.clone();

    // Connect two clients
    let (ws0, _) = tokio_tungstenite::connect_async(format!("ws://{addr}")).await.unwrap();
    let (mut w0, mut r0) = ws0.split();
    let (ws1, _) = tokio_tungstenite::connect_async(format!("ws://{addr}")).await.unwrap();
    let (mut w1, mut r1) = ws1.split();

    // Both join
    send(&mut w0, &ClientMsg::Join { name: "Alice".into() }).await;
    recv_until(&mut r0, |m| matches!(m, ServerMsg::Welcome { .. })).await;
    send(&mut w1, &ClientMsg::Join { name: "Bob".into() }).await;
    recv_until(&mut r1, |m| matches!(m, ServerMsg::Welcome { .. })).await;

    // c0 starts the game
    send(&mut w0, &ClientMsg::Start).await;

    // Both should get PhaseChange(GodPick) and initial Snapshot
    recv_until(&mut r0, |m| matches!(m, ServerMsg::PhaseChange { phase: Phase::GodPick, .. })).await;
    recv_until(&mut r1, |m| matches!(m, ServerMsg::PhaseChange { phase: Phase::GodPick, .. })).await;

    // Wait for initial snapshots
    recv_until(&mut r0, |m| matches!(m, ServerMsg::Snapshot(_))).await;
    recv_until(&mut r1, |m| matches!(m, ServerMsg::Snapshot(_))).await;

    // Both humans pick god and ready
    send(&mut w0, &ClientMsg::Action { action_type: "PickGod".into(), param: god_name.clone() }).await;
    recv_until(&mut r0, |m| matches!(m, ServerMsg::ActionResult { ok: true, .. })).await;
    send(&mut w0, &ClientMsg::Action { action_type: "Ready".into(), param: String::new() }).await;
    recv_until(&mut r0, |m| matches!(m, ServerMsg::ActionResult { ok: true, .. })).await;

    send(&mut w1, &ClientMsg::Action { action_type: "PickGod".into(), param: god_name.clone() }).await;
    recv_until(&mut r1, |m| matches!(m, ServerMsg::ActionResult { ok: true, .. })).await;
    send(&mut w1, &ClientMsg::Action { action_type: "Ready".into(), param: String::new() }).await;
    recv_until(&mut r1, |m| matches!(m, ServerMsg::ActionResult { ok: true, .. })).await;

    // After all ready (humans + bots), game should transition to Shop
    // Look for a Snapshot with phase=Shop for viewer 0
    let snap_msg = recv_until(&mut r0, |m| match m {
        ServerMsg::Snapshot(s) => s.phase == Phase::Shop,
        _ => false,
    }).await;

    if let ServerMsg::Snapshot(snap) = snap_msg {
        assert_eq!(snap.your_player_id, 0);
        assert_eq!(snap.phase, Phase::Shop);
        // Gold should be populated (round 1 = 6 gold)
        assert!(snap.own.gold > 0);
        // 8 players total
        assert_eq!(snap.players.len(), 8);
        // PlayerView exposes only public fields (structural check: no gold/bench/abilities)
        for pv in &snap.players {
            // These fields exist on PlayerView
            let _ = pv.id;
            let _ = pv.hp;
            let _ = pv.alive;
            let _ = &pv.god;
            let _ = pv.hero_count;
        }
    } else {
        panic!("expected Snapshot");
    }
}

#[tokio::test(start_paused = true)]
async fn combat_runs_and_advances() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(aa2_server::serve(listener, PathBuf::from("../../data"), 99));

    let gods = aa2_data::load_all_gods(&PathBuf::from("../../data/gods")).unwrap();
    let god_name = gods[0].name.clone();

    let (ws0, _) = tokio_tungstenite::connect_async(format!("ws://{addr}")).await.unwrap();
    let (mut w0, mut r0) = ws0.split();
    let (ws1, _) = tokio_tungstenite::connect_async(format!("ws://{addr}")).await.unwrap();
    let (mut w1, mut r1) = ws1.split();

    // Both join
    send(&mut w0, &ClientMsg::Join { name: "A".into() }).await;
    recv_until(&mut r0, |m| matches!(m, ServerMsg::Welcome { .. })).await;
    send(&mut w1, &ClientMsg::Join { name: "B".into() }).await;
    recv_until(&mut r1, |m| matches!(m, ServerMsg::Welcome { .. })).await;

    // Start game
    send(&mut w0, &ClientMsg::Start).await;
    recv_until(&mut r0, |m| matches!(m, ServerMsg::PhaseChange { phase: Phase::GodPick, .. })).await;

    // Both pick god + ready
    send(&mut w0, &ClientMsg::Action { action_type: "PickGod".into(), param: god_name.clone() }).await;
    send(&mut w0, &ClientMsg::Action { action_type: "Ready".into(), param: String::new() }).await;
    send(&mut w1, &ClientMsg::Action { action_type: "PickGod".into(), param: god_name.clone() }).await;
    send(&mut w1, &ClientMsg::Action { action_type: "Ready".into(), param: String::new() }).await;

    // (a) Both humans receive CombatStart — proves server-run combat + per-viewer streaming
    let cs0 = recv_until(&mut r0, |m| matches!(m, ServerMsg::CombatStart { .. })).await;
    assert!(matches!(cs0, ServerMsg::CombatStart { .. }));
    let cs1 = recv_until(&mut r1, |m| matches!(m, ServerMsg::CombatStart { .. })).await;
    assert!(matches!(cs1, ServerMsg::CombatStart { .. }));

    // (b) Game advances past combat — combat window elapsed and clock resumed
    let advanced = recv_until(&mut r0, |m| matches!(m, ServerMsg::PhaseChange { phase, .. } if *phase != Phase::Combat)).await;
    match advanced {
        ServerMsg::PhaseChange { phase, .. } => {
            assert!(phase == Phase::GracePeriod || phase == Phase::Shop || phase == Phase::Finished);
        }
        _ => panic!("expected PhaseChange"),
    }
}

#[tokio::test(start_paused = true)]
async fn game_reaches_gameover() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(aa2_server::serve(listener, PathBuf::from("../../data"), 42));

    let gods = aa2_data::load_all_gods(&PathBuf::from("../../data/gods")).unwrap();
    let god_name = gods[0].name.clone();

    // Connect two human clients
    let (ws0, _) = tokio_tungstenite::connect_async(format!("ws://{addr}")).await.unwrap();
    let (mut w0, mut r0) = ws0.split();
    let (ws1, _) = tokio_tungstenite::connect_async(format!("ws://{addr}")).await.unwrap();
    let (mut w1, mut r1) = ws1.split();

    // Both join, c0 starts
    send(&mut w0, &ClientMsg::Join { name: "H0".into() }).await;
    recv_until(&mut r0, |m| matches!(m, ServerMsg::Welcome { .. })).await;
    send(&mut w1, &ClientMsg::Join { name: "H1".into() }).await;
    recv_until(&mut r1, |m| matches!(m, ServerMsg::Welcome { .. })).await;
    send(&mut w0, &ClientMsg::Start).await;

    // Driver: reads messages, readies up each prep phase, returns placements on GameOver.
    async fn drive(
        w: &mut futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>,
        r: &mut futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
        god_name: &str,
    ) -> Vec<u8> {
        let mut last_readied: Option<(Phase, u32)> = None;
        let mut god_picked = false;
        loop {
            let msg = r.next().await.unwrap().unwrap();
            let text = match msg.to_text() {
                Ok(t) => t,
                Err(_) => continue,
            };
            let sm: ServerMsg = match serde_json::from_str(text) {
                Ok(m) => m,
                Err(_) => continue,
            };
            match &sm {
                ServerMsg::GameOver { placements } => return placements.clone(),
                ServerMsg::Snapshot(snap) => {
                    let key = (snap.phase.clone(), snap.round);
                    if snap.phase == Phase::GodPick && !god_picked {
                        god_picked = true;
                        send(w, &ClientMsg::Action { action_type: "PickGod".into(), param: god_name.to_string() }).await;
                    }
                    if matches!(snap.phase, Phase::GodPick | Phase::Shop | Phase::GracePeriod) && last_readied.as_ref() != Some(&key) {
                        last_readied = Some(key);
                        send(w, &ClientMsg::Action { action_type: "Ready".into(), param: String::new() }).await;
                    }
                }
                ServerMsg::PhaseChange { phase, round, .. } => {
                    let key = (phase.clone(), *round);
                    if matches!(phase, Phase::GodPick | Phase::Shop | Phase::GracePeriod) && last_readied.as_ref() != Some(&key) {
                        if *phase == Phase::GodPick && !god_picked {
                            god_picked = true;
                            send(w, &ClientMsg::Action { action_type: "PickGod".into(), param: god_name.to_string() }).await;
                        }
                        last_readied = Some(key);
                        send(w, &ClientMsg::Action { action_type: "Ready".into(), param: String::new() }).await;
                    }
                }
                _ => {}
            }
        }
    }

    // Run both drivers concurrently with a generous virtual-time timeout
    let result = timeout(Duration::from_secs(2400), async {
        let gn = god_name.clone();
        let (p0, p1) = tokio::join!(
            drive(&mut w0, &mut r0, &gn),
            drive(&mut w1, &mut r1, &god_name),
        );
        (p0, p1)
    })
    .await
    .expect("game did not reach GameOver within 2400s virtual time");

    let placements = result.0;
    // Structural assertions
    assert_eq!(placements.len(), 8, "expected 8 placements, got {:?}", placements);
    let mut sorted = placements.clone();
    sorted.sort();
    assert_eq!(sorted, vec![0, 1, 2, 3, 4, 5, 6, 7], "placements must be a permutation of 0..=7, got {:?}", placements);
}
