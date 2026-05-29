use std::time::{Duration, Instant};

use aa2_client::net_client::NetClient;
use aa2_net::{ClientMsg, ServerMsg};

fn poll_until(nc: &NetClient, pred: impl Fn(&ServerMsg) -> bool) -> Option<ServerMsg> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Some(msg) = nc.try_recv()
            && pred(&msg)
        {
            return Some(msg);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

#[test]
fn netclient_join_and_start() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let addr = rt.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let a = listener.local_addr().unwrap();
        tokio::spawn(aa2_server::serve(listener, std::path::PathBuf::from("../../data"), 42));
        a
    });

    let nc = NetClient::connect(format!("ws://{addr}"));
    nc.send(ClientMsg::Join { name: "Tester".into() });

    let welcome = poll_until(&nc, |m| matches!(m, ServerMsg::Welcome { .. }));
    // The first (and only) client to Join always gets seat 0.
    assert!(
        matches!(welcome, Some(ServerMsg::Welcome { your_player_id: 0, .. })),
        "expected Welcome with player_id 0, got {welcome:?}"
    );

    nc.send(ClientMsg::Start);

    let advanced = poll_until(&nc, |m| {
        matches!(m, ServerMsg::PhaseChange { .. } | ServerMsg::Snapshot(_))
    });
    assert!(advanced.is_some(), "expected PhaseChange/Snapshot after Start");

    // Keep rt alive until end of test
    drop(nc);
    drop(rt);
}
