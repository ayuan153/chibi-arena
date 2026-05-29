//! Authoritative game server — actor-model lobby + WebSocket transport.
//! Unauthenticated local dev server; auth/TLS deferred (see docs/design/networking.md §10).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use aa2_game::{GameConfig, GamePhase, GameState};
use aa2_data::{AbilityDef, God, HeroDef};
use aa2_game::pool::AbilityPool;
use aa2_game::scenario::Action;
use aa2_net::{ClientMsg, HeroView, OwnView, Phase, PlayerView, ServerMsg, ShopView, StateSnapshot};
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
    bots: HashSet<u8>,
    game: Option<GameState>,
    hero_defs: HashMap<String, HeroDef>,
    ability_defs: HashMap<String, AbilityDef>,
    gods: Vec<God>,
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
                    let phase = map_phase(&self.game.as_ref().unwrap().phase);
                    self.broadcast(&ServerMsg::PhaseChange {
                        phase,
                        round: self.game.as_ref().unwrap().round,
                        timer_secs: self.game.as_ref().unwrap().timer,
                    });
                    self.step_bots();
                    // Send initial snapshots to all seated humans
                    for (&cid, &seat) in &self.conn_seat {
                        let snap = project_snapshot(self.game.as_ref().unwrap(), seat);
                        if let Some(tx) = self.conns.get(&cid) {
                            tx.send(ServerMsg::Snapshot(Box::new(snap))).ok();
                        }
                    }
                }
                ClientMsg::Action { action_type, param } => {
                    if !self.started || self.game.is_none() { return; }
                    let seat = match self.conn_seat.get(&conn_id) {
                        Some(&s) => s,
                        None => {
                            if let Some(tx) = self.conns.get(&conn_id) {
                                tx.send(ServerMsg::ActionResult { ok: false, reason: "not seated".into() }).ok();
                            }
                            return;
                        }
                    };
                    let action = match aa2_game::scenario::parse_action(&action_type, &param, &self.gods) {
                        Ok(a) => a,
                        Err(e) => {
                            if let Some(tx) = self.conns.get(&conn_id) {
                                tx.send(ServerMsg::ActionResult { ok: false, reason: e }).ok();
                            }
                            return;
                        }
                    };
                    let game = self.game.as_mut().unwrap();
                    let prev_phase = game.phase.clone();
                    let res = game.apply_action(seat, action, &self.hero_defs, &mut self.rng);
                    if let Some(tx) = self.conns.get(&conn_id) {
                        tx.send(ServerMsg::ActionResult {
                            ok: res.is_ok(),
                            reason: res.as_ref().err().cloned().unwrap_or_default(),
                        }).ok();
                    }
                    if res.is_ok() {
                        self.step_bots();
                        self.broadcast_state(prev_phase);
                    }
                }
            },
            Inbound::Disconnected { conn_id } => {
                self.conns.remove(&conn_id);
                self.conn_seat.remove(&conn_id);
            }
        }
    }

    fn step_bots(&mut self) {
        let mut sorted_bots: Vec<u8> = self.bots.iter().copied().collect();
        sorted_bots.sort();

        for _ in 0..32 {
            let game = self.game.as_ref().unwrap();
            let phase = game.phase.clone();
            if matches!(phase, GamePhase::Combat | GamePhase::Finished) { break; }

            let mut acted = false;
            for &seat in &sorted_bots {
                let game = self.game.as_ref().unwrap();
                if !game.players[seat as usize].alive { continue; }

                let action = match &game.phase {
                    GamePhase::GodPick => {
                        if game.players[seat as usize].god.is_none() {
                            Some(Action::PickGod(self.gods[0].clone()))
                        } else if !game.ready_players.contains(&seat) {
                            Some(Action::Ready)
                        } else { None }
                    }
                    GamePhase::Shop => {
                        if game.draft_choices.get(&seat).is_some_and(|c| c.iter().any(|x| x.is_some())) {
                            Some(Action::DraftHero(0))
                        } else if !game.ready_players.contains(&seat) {
                            Some(Action::Ready)
                        } else { None }
                    }
                    GamePhase::GracePeriod => {
                        if !game.ready_players.contains(&seat) {
                            Some(Action::Ready)
                        } else { None }
                    }
                    _ => None,
                };

                if let Some(action) = action {
                    let game = self.game.as_mut().unwrap();
                    let _ = game.apply_action(seat, action, &self.hero_defs, &mut self.rng);
                    acted = true;
                    // Re-check phase after apply (Ready can cascade transitions)
                    if self.game.as_ref().unwrap().phase != phase {
                        break; // restart outer loop in new phase
                    }
                }
            }
            if !acted { break; }
        }
    }

    fn broadcast_state(&self, prev_phase: GamePhase) {
        let game = self.game.as_ref().unwrap();
        if game.phase != prev_phase {
            self.broadcast(&ServerMsg::PhaseChange {
                phase: map_phase(&game.phase),
                round: game.round,
                timer_secs: game.timer,
            });
        }
        for (&cid, &seat) in &self.conn_seat {
            let snap = project_snapshot(game, seat);
            if let Some(tx) = self.conns.get(&cid) {
                tx.send(ServerMsg::Snapshot(Box::new(snap))).ok();
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

        let config = GameConfig {
            auto_advance: true,
            ..GameConfig::default()
        };
        let mut game = GameState::new(pool, ultimates, config);
        game.gods = self.gods.clone();
        self.game = Some(game);
    }
}

fn project_snapshot(game: &GameState, viewer: u8) -> StateSnapshot {
    let p = &game.players[viewer as usize];

    let heroes: Vec<HeroView> = p.heroes.iter().map(|name| HeroView {
        name: name.clone(),
        position: p.hero_positions.get(name).copied().unwrap_or((0.0, 0.0)),
        equipped: p.equipped.get(name).cloned().unwrap_or_default(),
    }).collect();

    let mut abilities: Vec<(String, u32)> = p.abilities.iter().map(|(k, v)| (k.clone(), *v)).collect();
    abilities.sort_by(|a, b| a.0.cmp(&b.0));

    let own = OwnView {
        gold: p.gold,
        heroes,
        abilities,
        bench: p.bench.clone(),
        shop: ShopView {
            level: p.shop.level,
            offerings: p.shop.offerings.clone(),
            locked: p.shop.locked,
            upgrade_cost: p.shop.upgrade_cost(),
        },
        draft_choices: game.draft_choices.get(&viewer).cloned().unwrap_or([None, None, None]),
    };

    let players: Vec<PlayerView> = game.players.iter().map(|pl| PlayerView {
        id: pl.id,
        hp: pl.hp,
        alive: pl.alive,
        god: pl.god.as_ref().map(|g| g.name.clone()),
        hero_count: pl.heroes.len(),
    }).collect();

    StateSnapshot {
        your_player_id: viewer,
        phase: map_phase(&game.phase),
        round: game.round,
        timer_secs: game.timer,
        own,
        players,
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
