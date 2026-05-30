use aa2_net::{ClientMsg, Phase, ServerMsg, StateSnapshot};
use aa2_sim::CombatEvent;

/// Pure-Rust networked game state derived from server snapshots.
/// No gdext dependency — fully unit-testable.
#[derive(Default)]
pub struct NetState {
    pub your_player_id: u8,
    pub snapshot: Option<StateSnapshot>,
    pub lobby: Vec<Option<String>>,
    pub placements: Option<Vec<u8>>,
    pub combat: Option<Vec<CombatEvent>>,
}

impl NetState {
    pub fn apply(&mut self, msg: &ServerMsg) {
        match msg {
            ServerMsg::Welcome { your_player_id, .. } => {
                self.your_player_id = *your_player_id;
            }
            ServerMsg::Lobby { seats } => {
                self.lobby = seats.clone();
            }
            ServerMsg::Snapshot(s) => {
                debug_assert_eq!(
                    s.your_player_id, self.your_player_id,
                    "snapshot viewer id must match the Welcome-assigned id"
                );
                self.snapshot = Some(*s.clone());
            }
            ServerMsg::GameOver { placements } => {
                self.placements = Some(placements.clone());
            }
            ServerMsg::CombatStart { event_log, .. } => {
                self.combat = Some(event_log.clone());
            }
            ServerMsg::PhaseChange { .. }
            | ServerMsg::ActionResult { .. } => {}
        }
    }

    pub fn lobby_player_count(&self) -> usize {
        self.lobby.iter().filter(|s| s.is_some()).count()
    }

    pub fn my_player_id(&self) -> u8 {
        self.your_player_id
    }

    pub fn phase(&self) -> String {
        self.snapshot.as_ref().map(|s| match s.phase {
            Phase::GodPick => "GodPick",
            Phase::Combat => "Combat",
            Phase::GracePeriod => "GracePeriod",
            Phase::Shop => "Shop",
            Phase::Finished => "Finished",
        }).unwrap_or("GodPick").to_string()
    }

    pub fn round(&self) -> u32 {
        self.snapshot.as_ref().map(|s| s.round).unwrap_or(0)
    }

    pub fn player_count(&self) -> usize {
        self.snapshot.as_ref().map(|s| s.players.len()).unwrap_or(0)
    }

    fn is_own_seat(&self, seat: usize) -> bool {
        self.snapshot.as_ref().is_some_and(|s| seat as u8 == s.your_player_id)
    }

    pub fn gold(&self, seat: usize) -> u32 {
        if !self.is_own_seat(seat) { return 0; }
        self.snapshot.as_ref().map(|s| s.own.gold).unwrap_or(0)
    }

    pub fn shop_level(&self, seat: usize) -> u32 {
        if !self.is_own_seat(seat) { return 1; }
        self.snapshot.as_ref().map(|s| s.own.shop.level).unwrap_or(1)
    }

    pub fn shop_offerings(&self, seat: usize) -> Vec<Option<String>> {
        if !self.is_own_seat(seat) { return Vec::new(); }
        self.snapshot.as_ref().map(|s| s.own.shop.offerings.clone()).unwrap_or_default()
    }

    pub fn shop_locked(&self, seat: usize) -> bool {
        if !self.is_own_seat(seat) { return false; }
        self.snapshot.as_ref().map(|s| s.own.shop.locked).unwrap_or(false)
    }

    pub fn upgrade_cost(&self, seat: usize) -> Option<u32> {
        if !self.is_own_seat(seat) { return None; }
        self.snapshot.as_ref().and_then(|s| s.own.shop.upgrade_cost)
    }

    pub fn heroes(&self, seat: usize) -> Vec<String> {
        if !self.is_own_seat(seat) { return Vec::new(); }
        self.snapshot.as_ref()
            .map(|s| s.own.heroes.iter().map(|h| h.name.clone()).collect())
            .unwrap_or_default()
    }

    pub fn hero_position(&self, seat: usize, name: &str) -> (f32, f32) {
        if !self.is_own_seat(seat) { return (500.0, 1500.0); }
        self.snapshot.as_ref()
            .and_then(|s| s.own.heroes.iter().find(|h| h.name == name))
            .map(|h| h.position)
            .unwrap_or((500.0, 1500.0))
    }

    pub fn equipped(&self, seat: usize, name: &str) -> Vec<String> {
        if !self.is_own_seat(seat) { return Vec::new(); }
        self.snapshot.as_ref()
            .and_then(|s| s.own.heroes.iter().find(|h| h.name == name))
            .map(|h| h.equipped.clone())
            .unwrap_or_default()
    }

    pub fn ability_level(&self, seat: usize, name: &str) -> u32 {
        if !self.is_own_seat(seat) { return 0; }
        self.snapshot.as_ref()
            .and_then(|s| s.own.abilities.iter().find(|(n, _)| n == name))
            .map(|(_, lvl)| *lvl)
            .unwrap_or(0)
    }

    pub fn bench(&self, seat: usize) -> Vec<String> {
        if !self.is_own_seat(seat) { return Vec::new(); }
        self.snapshot.as_ref().map(|s| s.own.bench.clone()).unwrap_or_default()
    }

    pub fn draft_choices(&self, seat: usize) -> [Option<String>; 3] {
        if !self.is_own_seat(seat) { return [None, None, None]; }
        self.snapshot.as_ref()
            .map(|s| s.own.draft_choices.clone())
            .unwrap_or([None, None, None])
    }

    pub fn is_draft_active(&self, seat: usize) -> bool {
        if !self.is_own_seat(seat) { return false; }
        self.snapshot.as_ref()
            .is_some_and(|s| s.own.draft_choices.iter().any(|c| c.is_some()))
    }

    pub fn player_hp(&self, seat: usize) -> f32 {
        self.snapshot.as_ref()
            .and_then(|s| s.players.get(seat))
            .map(|p| p.hp)
            .unwrap_or(0.0)
    }

    pub fn player_alive(&self, seat: usize) -> bool {
        self.snapshot.as_ref()
            .and_then(|s| s.players.get(seat))
            .map(|p| p.alive)
            .unwrap_or(false)
    }

    pub fn player_god(&self, seat: usize) -> Option<String> {
        self.snapshot.as_ref()
            .and_then(|s| s.players.get(seat))
            .and_then(|p| p.god.clone())
    }

    pub fn has_combat(&self) -> bool {
        self.combat.is_some()
    }

    pub fn combat_event_count(&self) -> usize {
        self.combat.as_ref().map_or(0, |v| v.len())
    }

    pub fn combat_event(&self, i: usize) -> Option<&CombatEvent> {
        self.combat.as_ref().and_then(|v| v.get(i))
    }

    pub fn placements(&self) -> Option<&Vec<u8>> {
        self.placements.as_ref()
    }
}

/// WebSocket transport to the authoritative server.
/// Runs a tokio runtime on a background thread; communicates via channels.
pub struct NetClient {
    to_server: tokio::sync::mpsc::UnboundedSender<ClientMsg>,
    from_server: std::sync::mpsc::Receiver<ServerMsg>,
    _handle: std::thread::JoinHandle<()>,
}

impl NetClient {
    pub fn connect(url: String) -> NetClient {
        let (to_tx, mut to_rx) = tokio::sync::mpsc::unbounded_channel::<ClientMsg>();
        let (from_tx, from_rx) = std::sync::mpsc::channel::<ServerMsg>();

        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            rt.block_on(async move {
                use futures_util::{SinkExt, StreamExt};
                use tokio_tungstenite::tungstenite::Message;

                // TODO(commit 2+): surface connection failure to the UI. For now the
                // thread exits and try_recv yields None indefinitely.
                let Ok((ws, _)) = tokio_tungstenite::connect_async(&url).await else {
                    return;
                };
                let (mut write, mut read) = ws.split();

                loop {
                    tokio::select! {
                        msg = to_rx.recv() => {
                            let Some(msg) = msg else { break; };
                            let json = serde_json::to_string(&msg).expect("ClientMsg is always serializable");
                            if write.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                        frame = read.next() => {
                            let Some(Ok(frame)) = frame else { break; };
                            if let Message::Text(text) = frame
                                && let Ok(msg) = serde_json::from_str::<ServerMsg>(&text)
                                && from_tx.send(msg).is_err()
                            {
                                break;
                            }
                        }
                    }
                }
            });
        });

        NetClient {
            to_server: to_tx,
            from_server: from_rx,
            _handle: handle,
        }
    }

    pub fn send(&self, msg: ClientMsg) {
        let _ = self.to_server.send(msg);
    }

    pub fn try_recv(&self) -> Option<ServerMsg> {
        self.from_server.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aa2_net::*;

    #[test]
    fn welcome_sets_player_id() {
        let mut state = NetState::default();
        state.apply(&ServerMsg::Welcome { your_player_id: 3, player_count: 8 });
        assert_eq!(state.my_player_id(), 3);
    }

    #[test]
    fn lobby_sets_seats() {
        let mut state = NetState::default();
        let seats = vec![Some("Alice".into()), None, Some("Bob".into())];
        state.apply(&ServerMsg::Lobby { seats: seats.clone() });
        assert_eq!(state.lobby, seats);
    }

    #[test]
    fn lobby_player_count_counts_some_entries() {
        let mut state = NetState::default();
        assert_eq!(state.lobby_player_count(), 0);
        state.apply(&ServerMsg::Lobby {
            seats: vec![Some("A".into()), None, Some("B".into()), None],
        });
        assert_eq!(state.lobby_player_count(), 2);
    }

    #[test]
    fn snapshot_accessors() {
        let mut state = NetState::default();
        let snap = StateSnapshot {
            your_player_id: 0,
            phase: Phase::Shop,
            round: 3,
            timer_secs: 20.0,
            own: OwnView {
                gold: 7,
                heroes: vec![HeroView {
                    name: "Axe".into(),
                    position: (2.0, 3.0),
                    equipped: vec!["Blink".into()],
                }],
                abilities: vec![("Blink".into(), 2)],
                bench: vec!["Lina".into()],
                shop: ShopView {
                    level: 3,
                    offerings: vec![Some("Fireball".into()), None],
                    locked: true,
                    upgrade_cost: Some(8),
                },
                draft_choices: [Some("BodyA".into()), None, Some("BodyC".into())],
            },
            players: vec![
                PlayerView { id: 0, hp: 100.0, alive: true, god: Some("Zeus".into()), hero_count: 1 },
                PlayerView { id: 1, hp: 60.0, alive: true, god: None, hero_count: 2 },
            ],
        };
        state.apply(&ServerMsg::Snapshot(Box::new(snap)));

        assert_eq!(state.phase(), "Shop");
        assert_eq!(state.round(), 3);
        assert_eq!(state.player_count(), 2);

        // Own seat (0) private data
        assert_eq!(state.gold(0), 7);
        assert_eq!(state.shop_level(0), 3);
        assert_eq!(state.shop_offerings(0), vec![Some("Fireball".into()), None]);
        assert!(state.shop_locked(0));
        assert_eq!(state.upgrade_cost(0), Some(8));
        assert_eq!(state.heroes(0), vec!["Axe".to_string()]);
        assert_eq!(state.hero_position(0, "Axe"), (2.0, 3.0));
        assert_eq!(state.equipped(0, "Axe"), vec!["Blink".to_string()]);
        assert_eq!(state.ability_level(0, "Blink"), 2);
        assert_eq!(state.bench(0), vec!["Lina".to_string()]);
        assert_eq!(state.draft_choices(0), [Some("BodyA".into()), None, Some("BodyC".into())]);
        assert!(state.is_draft_active(0));

        // Opponent seat (1) — private data returns defaults
        assert_eq!(state.gold(1), 0);
        assert_eq!(state.heroes(1), Vec::<String>::new());
        assert!(!state.is_draft_active(1));

        // Public data for all seats
        assert_eq!(state.player_hp(0), 100.0);
        assert_eq!(state.player_hp(1), 60.0);
        assert!(state.player_alive(1));
        assert_eq!(state.player_god(0), Some("Zeus".into()));
        assert_eq!(state.player_god(1), None);
    }

    #[test]
    fn game_over_sets_placements() {
        let mut state = NetState::default();
        let placements = vec![2, 0, 1, 3, 4, 5, 6, 7];
        state.apply(&ServerMsg::GameOver { placements: placements.clone() });
        assert_eq!(state.placements, Some(placements));
    }

    #[test]
    fn combat_start_sets_event_log() {
        let mut state = NetState::default();
        assert!(!state.has_combat());
        let log = vec![
            CombatEvent::Attack { tick: 1, attacker_id: 0, target_id: 1, damage: 12.0 },
            CombatEvent::Death { tick: 5, unit_id: 1 },
        ];
        state.apply(&ServerMsg::CombatStart { matchup_index: 0, event_log: log });
        assert!(state.has_combat());
        assert_eq!(state.combat_event_count(), 2);
        assert!(state.combat_event(0).is_some());
        assert!(state.combat_event(2).is_none());
    }
}
