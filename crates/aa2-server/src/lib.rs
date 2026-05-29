//! Authoritative game server — actor-model lobby + WebSocket transport.
//! Unauthenticated local dev server; auth/TLS deferred (see docs/design/networking.md §10).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use aa2_game::{GameConfig, GamePhase, GameState};
use aa2_data::{AbilityDef, God, HeroDef};
use aa2_game::pool::AbilityPool;
use aa2_net::{ClientMsg, Phase, ServerMsg};
use futures_util::{SinkExt, StreamExt};
use rand::rngs::StdRng;
use rand::SeedableRng;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

enum Inbound {
    Connected { conn_id: u64, out_tx: mpsc::UnboundedSender<ServerMsg> },
    Msg { conn_id: u64, msg: ClientMsg },
    Disconnected { conn_id: u64 },
}

struct Central {
    conns: HashMap<u64, mpsc::UnboundedSender<ServerMsg>>,
    conn_seat: HashMap<u64, u8>,
    seats: Vec<Option<String>>,
    started: bool,
    #[allow(dead_code)]
    bots: HashSet<u8>,
    game: Option<GameState>,
    #[allow(dead_code)]
    hero_defs: HashMap<String, HeroDef>,
    ability_defs: HashMap<String, AbilityDef>,
    gods: Vec<God>,
    #[allow(dead_code)]
    rng: StdRng,
}

impl Central {
    fn new(data_dir: &Path, seed: u64) -> Self {
        let mut hero_defs = HashMap::new();
        if let Ok(heroes) = aa2_data::load_all_heroes(&data_dir.join("heroes")) {
            for h in heroes {
                hero_defs.insert(h.name.clone(), h);
            }
        }

        let mut ability_defs = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(data_dir.join("abilities")) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "ron")
                    && let Ok(def) = aa2_data::load_ability_def(&path)
                {
                    ability_defs.insert(def.name.clone(), def);
                }
            }
        }

        let gods = aa2_data::load_all_gods(&data_dir.join("gods"))
            .unwrap_or_else(|_| aa2_game::god::all_gods());

        Self {
            conns: HashMap::new(),
            conn_seat: HashMap::new(),
            seats: vec![None; 8],
            started: false,
            bots: HashSet::new(),
            game: None,
            hero_defs,
            ability_defs,
            gods,
            rng: StdRng::seed_from_u64(seed),
        }
    }

    fn broadcast(&self, msg: &ServerMsg) {
        for tx in self.conns.values() {
            tx.send(msg.clone()).ok();
        }
    }

    fn human_count(&self) -> u8 {
        self.seats.iter().filter(|s| s.is_some()).count() as u8
    }

    fn handle(&mut self, inb: Inbound) {
        match inb {
            Inbound::Connected { conn_id, out_tx } => {
                self.conns.insert(conn_id, out_tx);
            }
            Inbound::Msg { conn_id, msg } => match msg {
                ClientMsg::Join { name } => {
                    if self.started { return; }
                    let seat = self.human_count();
                    if seat >= 8 { return; }
                    self.seats[seat as usize] = Some(name);
                    self.conn_seat.insert(conn_id, seat);
                    let human_count = self.human_count();
                    if let Some(tx) = self.conns.get(&conn_id) {
                        tx.send(ServerMsg::Welcome { your_player_id: seat, player_count: human_count }).ok();
                    }
                    self.broadcast(&ServerMsg::Lobby { seats: self.seats.clone() });
                }
                ClientMsg::Start => {
                    if self.started { return; }
                    self.started = true;
                    let human_count = self.human_count();
                    self.bots = (human_count..8).collect();
                    self.create_game();
                    let game = self.game.as_ref().unwrap();
                    let phase = map_phase(&game.phase);
                    self.broadcast(&ServerMsg::PhaseChange {
                        phase,
                        round: game.round,
                        timer_secs: game.timer,
                    });
                }
                ClientMsg::Action { .. } => { /* no-op for commit A */ }
            },
            Inbound::Disconnected { conn_id } => {
                self.conns.remove(&conn_id);
                self.conn_seat.remove(&conn_id);
            }
        }
    }

    fn create_game(&mut self) {
        let ultimates: HashSet<String> = self.ability_defs.iter()
            .filter(|(_, d)| d.is_ultimate)
            .map(|(n, _)| n.clone())
            .collect();
        let pool_counts: HashMap<String, u32> = self.ability_defs.keys()
            .map(|n| (n.clone(), 20))
            .collect();
        let pool = AbilityPool::from_counts(pool_counts);

        // Server drives the phase clock, so timers auto-advance (unlike the
        // client's manual dev mode, which uses auto_advance: false).
        let config = GameConfig {
            auto_advance: true,
            ..GameConfig::default()
        };
        let mut game = GameState::new(pool, ultimates, config);
        game.gods = self.gods.clone();
        // Server keeps all 8 players alive (humans + bots).
        self.game = Some(game);
    }
}

fn map_phase(phase: &GamePhase) -> Phase {
    match phase {
        GamePhase::GodPick => Phase::GodPick,
        GamePhase::Combat => Phase::Combat,
        GamePhase::GracePeriod => Phase::GracePeriod,
        GamePhase::Shop => Phase::Shop,
        GamePhase::Finished => Phase::Finished,
    }
}

static NEXT_CONN_ID: AtomicU64 = AtomicU64::new(1);

/// Run the game server on the given listener.
pub async fn serve(listener: TcpListener, data_dir: PathBuf, seed: u64) {
    let (inbound_tx, mut inbound_rx) = mpsc::unbounded_channel::<Inbound>();

    // Central game task
    tokio::spawn(async move {
        let mut central = Central::new(&data_dir, seed);
        while let Some(inb) = inbound_rx.recv().await {
            central.handle(inb);
        }
    });

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => break,
        };
        let inbound_tx = inbound_tx.clone();
        tokio::spawn(async move {
            let conn_id = NEXT_CONN_ID.fetch_add(1, Ordering::Relaxed);
            let ws = match tokio_tungstenite::accept_async(stream).await {
                Ok(ws) => ws,
                Err(_) => return,
            };
            let (mut write, mut read) = ws.split();
            let (out_tx, mut out_rx) = mpsc::unbounded_channel::<ServerMsg>();

            inbound_tx.send(Inbound::Connected { conn_id, out_tx }).ok();

            // Writer task
            tokio::spawn(async move {
                while let Some(msg) = out_rx.recv().await {
                    let text = serde_json::to_string(&msg).expect("ServerMsg serialization");
                    if write.send(Message::text(text)).await.is_err() {
                        break;
                    }
                }
            });

            // Reader loop
            while let Some(Ok(m)) = read.next().await {
                if let Ok(t) = m.to_text()
                    && let Ok(cm) = serde_json::from_str::<ClientMsg>(t)
                {
                    inbound_tx.send(Inbound::Msg { conn_id, msg: cm }).ok();
                }
            }
            inbound_tx.send(Inbound::Disconnected { conn_id }).ok();
        });
    }
}
