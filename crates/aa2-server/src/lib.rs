//! Authoritative game server — actor-model lobby + WebSocket transport.
//! Unauthenticated local dev server; auth/TLS deferred (see docs/design/networking.md §10).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use aa2_game::{GameConfig, GamePhase, GameState};
use aa2_data::{AbilityDef, God, HeroDef};
use aa2_game::pool::AbilityPool;
use aa2_game::scenario::Action;
use aa2_net::{ClientMsg, HeroView, OwnView, Phase, PlayerView, ServerMsg, ShopView, StateSnapshot};
use futures_util::{SinkExt, StreamExt};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
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
    combat_window: Option<f32>,
    elim_order: Vec<u8>,
    finished_announced: bool,
}

impl Central {
    fn new(data_dir: &Path, seed: u64) -> Self {
        let (hero_defs, ability_defs, gods) = match aa2_data::load_game_data(data_dir) {
            Ok(data) => (data.heroes, data.abilities, data.gods),
            Err(_) => (
                HashMap::new(),
                HashMap::new(),
                aa2_game::god::all_gods(),
            ),
        };

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
            combat_window: None,
            elim_order: Vec::new(),
            finished_announced: false,
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
                        self.reconcile(prev_phase);
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

    fn on_tick(&mut self, dt: f32) {
        if !self.started || self.game.is_none() {
            return;
        }
        // Game over: stop ticking (no more transitions or snapshots).
        if self.finished_announced {
            return;
        }
        if let Some(rem) = self.combat_window {
            let new_rem = rem - dt;
            if new_rem <= 0.0 {
                self.combat_window = None;
                let game = self.game.as_mut().unwrap();
                let prev = game.phase.clone();
                game.end_combat(false);
                self.reconcile(prev);
            } else {
                self.combat_window = Some(new_rem);
            }
            return;
        }
        let game = self.game.as_mut().unwrap();
        let prev = game.phase.clone();
        game.tick(dt, &mut self.rng);
        self.reconcile(prev);
    }

    fn reconcile(&mut self, prev_phase: GamePhase) {
        self.step_bots();
        let game = self.game.as_ref().unwrap();
        let now = game.phase.clone();
        if now == GamePhase::Combat && self.combat_window.is_none() {
            self.resolve_combat();
        }
        if now == GamePhase::Finished && !self.finished_announced {
            self.announce_gameover();
        }
        let game = self.game.as_ref().unwrap();
        if game.phase != prev_phase {
            self.broadcast(&ServerMsg::PhaseChange {
                phase: map_phase(&game.phase),
                round: game.round,
                timer_secs: game.timer,
            });
        }
        for (&cid, &seat) in &self.conn_seat {
            let snap = project_snapshot(self.game.as_ref().unwrap(), seat);
            if let Some(tx) = self.conns.get(&cid) {
                tx.send(ServerMsg::Snapshot(Box::new(snap))).ok();
            }
        }
    }

    fn resolve_combat(&mut self) {
        let game = self.game.as_ref().unwrap();
        let alive_before: Vec<u8> = game.players.iter()
            .filter(|p| p.alive)
            .map(|p| p.id)
            .collect();

        let seed: u32 = self.rng.r#gen();
        let game = self.game.as_mut().unwrap();
        let results = game.run_combat_round(&self.hero_defs, &self.ability_defs, seed, &mut self.rng);

        // Determine newly dead
        let game = self.game.as_ref().unwrap();
        let mut newly_dead: Vec<u8> = alive_before.iter()
            .filter(|&&id| !game.players[id as usize].alive)
            .copied()
            .collect();
        newly_dead.sort();
        self.elim_order.extend(newly_dead);

        // Stream CombatStart per-viewer
        for (idx, res) in results.iter().enumerate() {
            for (&cid, &seat) in &self.conn_seat {
                if (seat == res.matchup.player_a || seat == res.matchup.player_b)
                    && let Some(tx) = self.conns.get(&cid)
                {
                    tx.send(ServerMsg::CombatStart {
                        matchup_index: idx as u32,
                        event_log: res.combat_log.clone(),
                    }).ok();
                }
            }
        }

        // Set combat window based on max event tick
        let max_tick = results.iter()
            .filter_map(|r| r.combat_log.iter().map(|e| e.tick()).max())
            .max()
            .unwrap_or(0);
        self.combat_window = Some(
            ((max_tick as f32) / aa2_sim::TICK_RATE).min(aa2_game::COMBAT_TIMEOUT)
        );
    }

    fn announce_gameover(&mut self) {
        self.finished_announced = true;
        let game = self.game.as_ref().unwrap();
        let mut alive_sorted: Vec<u8> = game.players.iter()
            .filter(|p| p.alive)
            .map(|p| p.id)
            .collect();
        alive_sorted.sort();
        let placements = compute_placements(&self.elim_order, &alive_sorted);
        self.broadcast(&ServerMsg::GameOver { placements });
    }
}

/// Compute final placements: winners first, then eliminated in reverse order.
fn compute_placements(elim_order: &[u8], alive_sorted: &[u8]) -> Vec<u8> {
    let mut result: Vec<u8> = alive_sorted.to_vec();
    result.extend(elim_order.iter().rev());
    result
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
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        loop {
            tokio::select! {
                maybe = inbound_rx.recv() => match maybe {
                    Some(inb) => central.handle(inb),
                    None => break,
                },
                _ = interval.tick() => central.on_tick(0.1),
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_placements() {
        // elim_order=[3,5,2], alive=[7] -> winner 7, then last-eliminated-first: 2,5,3
        assert_eq!(compute_placements(&[3, 5, 2], &[7]), vec![7, 2, 5, 3]);
        // Multiple alive (tie for 1st)
        assert_eq!(compute_placements(&[1, 4], &[0, 2]), vec![0, 2, 4, 1]);
        // Empty elim
        assert_eq!(compute_placements(&[], &[5]), vec![5]);
    }
}
