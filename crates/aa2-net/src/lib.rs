//! Transport DTOs only; the GameState→StateSnapshot projection lives in aa2-server
//! so aa2-game/aa2-sim stay networking-free (sim compiles to WASM).
//! Ref docs/design/networking.md §4/§5.

use aa2_sim::CombatEvent;
use serde::{Deserialize, Serialize};

/// Game phase as seen by the client. Mirrors aa2_game::GamePhase; server maps between them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    /// God selection at match start.
    GodPick,
    /// Automated combat round.
    Combat,
    /// Brief pause between combat and shop.
    GracePeriod,
    /// Shopping/equip phase.
    Shop,
    /// Match is over.
    Finished,
}

/// A single hero as visible to its owner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeroView {
    /// Hero body name.
    pub name: String,
    /// Board position (x, y).
    pub position: (f32, f32),
    /// Names of equipped abilities.
    pub equipped: Vec<String>,
}

/// Shop state for the owning player.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShopView {
    /// Current shop level.
    pub level: u32,
    /// Ability offerings (None = already bought).
    pub offerings: Vec<Option<String>>,
    /// Whether the shop is locked for next round.
    pub locked: bool,
    /// Gold cost to upgrade shop, if available.
    pub upgrade_cost: Option<u32>,
}

/// Full private state for the owning player.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnView {
    /// Current gold.
    pub gold: u32,
    /// Heroes on board.
    pub heroes: Vec<HeroView>,
    /// Owned abilities as (name, level), sorted by name (deterministic wire order).
    pub abilities: Vec<(String, u32)>,
    /// Benched hero names.
    pub bench: Vec<String>,
    /// Current shop state.
    pub shop: ShopView,
    /// Draft choices (up to 3).
    pub draft_choices: [Option<String>; 3],
}

/// Public info about any player.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerView {
    /// Player slot index.
    pub id: u8,
    /// Current hit points.
    pub hp: f32,
    /// Whether the player is still in the game.
    pub alive: bool,
    /// Selected god name, if any.
    pub god: Option<String>,
    /// Number of heroes on board.
    pub hero_count: usize,
}

/// Full game state snapshot sent to a single client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Recipient's player id.
    pub your_player_id: u8,
    /// Current game phase.
    pub phase: Phase,
    /// Current round number.
    pub round: u32,
    /// Seconds remaining in current phase.
    pub timer_secs: f32,
    /// Full private state for the recipient.
    pub own: OwnView,
    /// Public info for all players.
    pub players: Vec<PlayerView>,
}

/// Message sent from client to server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg {
    /// Initial join request.
    Join { name: String },
    /// A game action, reusing the existing string action protocol: `action_type`
    /// is the action name (e.g. "Buy", "Equip") and `param` is its argument
    /// payload (some actions pack multiple args, e.g. SetPosition as "hero,x,y").
    /// The server parses and validates this pair into a typed action.
    Action { action_type: String, param: String },
    /// Request to begin the game (any client can send).
    Start,
}

/// Message sent from server to client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    /// Sent after successful join.
    Welcome { your_player_id: u8, player_count: u8 },
    /// Current lobby roster; index = seat id, Some(name) = occupied, None = empty.
    Lobby { seats: Vec<Option<String>> },
    /// Periodic state snapshot.
    Snapshot(Box<StateSnapshot>),
    /// Result of a client action.
    ActionResult { ok: bool, reason: String },
    /// Combat phase started with full event log.
    CombatStart { matchup_index: u32, event_log: Vec<CombatEvent> },
    /// Phase transition notification.
    PhaseChange { phase: Phase, round: u32, timer_secs: f32 },
    /// Match ended with final placements.
    GameOver { placements: Vec<u8> },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T>(val: &T)
    where
        T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(val).unwrap();
        let back: T = serde_json::from_str(&json).unwrap();
        assert_eq!(*val, back);
    }

    /// ClientMsg variants must survive a JSON round trip.
    #[test]
    fn client_msg_round_trip() {
        round_trip(&ClientMsg::Join { name: "Alice".into() });
        round_trip(&ClientMsg::Action { action_type: "buy".into(), param: "3".into() });
        round_trip(&ClientMsg::Start);
    }

    /// ServerMsg variants must survive a JSON round trip (all variants).
    #[test]
    fn server_msg_round_trip() {
        round_trip(&ServerMsg::Welcome { your_player_id: 0, player_count: 8 });
        round_trip(&ServerMsg::Lobby { seats: vec![Some("Alice".into()), None, Some("Bob".into()), None, None, None, None, None] });
        round_trip(&ServerMsg::ActionResult { ok: true, reason: String::new() });
        round_trip(&ServerMsg::CombatStart {
            matchup_index: 1,
            event_log: vec![
                CombatEvent::Attack { tick: 1, attacker_id: 0, target_id: 1, damage: 10.0 },
                CombatEvent::Death { tick: 5, unit_id: 1 },
            ],
        });
        round_trip(&ServerMsg::PhaseChange { phase: Phase::Combat, round: 3, timer_secs: 30.0 });
        round_trip(&ServerMsg::GameOver { placements: vec![2, 0, 1, 3, 4, 5, 6, 7] });
    }

    /// A fully-populated StateSnapshot (via ServerMsg::Snapshot) must round-trip.
    #[test]
    fn snapshot_round_trip() {
        let snap = StateSnapshot {
            your_player_id: 0,
            phase: Phase::Shop,
            round: 2,
            timer_secs: 25.0,
            own: OwnView {
                gold: 10,
                heroes: vec![HeroView {
                    name: "Axe".into(),
                    position: (1.0, 2.0),
                    equipped: vec!["Blink".into()],
                }],
                abilities: vec![("Blink".into(), 1), ("Rage".into(), 2)],
                bench: vec!["Lina".into()],
                shop: ShopView {
                    level: 2,
                    offerings: vec![Some("Fireball".into()), None, Some("Heal".into())],
                    locked: false,
                    upgrade_cost: Some(5),
                },
                draft_choices: [Some("BodyA".into()), None, Some("BodyC".into())],
            },
            players: vec![
                PlayerView { id: 0, hp: 100.0, alive: true, god: Some("Zeus".into()), hero_count: 1 },
                PlayerView { id: 1, hp: 80.0, alive: true, god: None, hero_count: 2 },
            ],
        };
        round_trip(&ServerMsg::Snapshot(Box::new(snap)));
    }
}
