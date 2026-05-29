//! AA2 authoritative game server binary.
//! Unauthenticated local dev server — no auth/TLS (see docs/design/networking.md §10).

use std::path::PathBuf;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let port = std::env::var("PORT").unwrap_or_else(|_| "9001".into());
    let data_dir = std::env::args().nth(1).unwrap_or_else(|| "data".into());
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).await.expect("failed to bind");
    eprintln!("aa2-server listening on {addr}");
    aa2_server::serve(listener, PathBuf::from(data_dir), rand::random()).await;
}
