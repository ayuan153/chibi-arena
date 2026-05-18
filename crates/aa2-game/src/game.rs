//! Game state, round state machine, and configuration.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::damage;
use crate::draft;
use crate::economy;
use crate::player::PlayerState;
use crate::pool::AbilityPool;

/// Game configuration — gods can modify these parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    /// Number of ability equip slots per hero.
    pub ability_slots_per_hero: u32,
    /// Bonus shop slots added to base size.
    pub shop_size_bonus: u32,
    /// Bonus gold per round.
    pub gold_bonus: u32,
    /// Override for reroll cost (None = use default).
    pub reroll_cost_override: Option<u32>,
    /// Shop level at which ultimates become available.
    pub ultimate_unlock_level: u32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            ability_slots_per_hero: 4,
            shop_size_bonus: 0,
            gold_bonus: 0,
            reroll_cost_override: None,
            ultimate_unlock_level: 3,
        }
    }
}

/// Phases of a game round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamePhase {
    /// Round 1 only: pick god.
    GodPick,
    /// Hero draft (rounds 1/3/6/9/12).
    HeroDraft,
    /// Combat phase (not round 1).
    Combat,
    /// Damage applied after combat (also eliminates players at 0 HP).
    Damage,
    /// Shop/equip phase.
    Shop,
    /// Round over, advance to next.
    RoundEnd,
}

/// Top-level game state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    /// All player states.
    pub players: Vec<PlayerState>,
    /// Current round number (starts at 1).
    pub round: u32,
    /// Current game phase.
    pub phase: GamePhase,
    /// Shared ability pool.
    pub pool: AbilityPool,
    /// Current round matchups: pairs of player IDs.
    pub matchups: Vec<(u8, u8)>,
    /// Set of ability names that are ultimates.
    pub ultimates: HashSet<String>,
    /// Game configuration.
    pub config: GameConfig,
}

impl GameState {
    /// Create a new game with 8 players and the given pool/ultimates.
    pub fn new(pool: AbilityPool, ultimates: HashSet<String>, config: GameConfig) -> Self {
        let players = (0..8).map(PlayerState::new).collect();
        Self {
            players,
            round: 1,
            phase: GamePhase::GodPick,
            pool,
            matchups: Vec::new(),
            ultimates,
            config,
        }
    }

    /// Start a new round: distribute gold, tick shop decay.
    pub fn start_round(&mut self) {
        let base_gold = economy::gold_for_round(self.round) + self.config.gold_bonus;
        for player in &mut self.players {
            if player.alive {
                player.gold = base_gold;
                player.shop.tick_decay();
            }
        }
    }

    /// Advance to the next phase in the state machine.
    /// Returns the new phase.
    ///
    /// Transitions:
    /// - Round 1: GodPick → HeroDraft → Shop → RoundEnd
    /// - Draft rounds (3/6/9/12): HeroDraft → Combat → Damage → Shop → RoundEnd
    /// - Normal rounds: Combat → Damage → Shop → RoundEnd
    pub fn advance_phase(&mut self) -> GamePhase {
        self.phase = match &self.phase {
            GamePhase::GodPick => GamePhase::HeroDraft,
            GamePhase::HeroDraft => {
                if self.round == 1 {
                    GamePhase::Shop
                } else {
                    GamePhase::Combat
                }
            }
            GamePhase::Combat => GamePhase::Damage,
            GamePhase::Damage => {
                // Eliminate dead players in the same step
                self.eliminate_dead();
                GamePhase::Shop
            }
            GamePhase::Shop => GamePhase::RoundEnd,
            GamePhase::RoundEnd => {
                self.round += 1;
                self.start_round();
                if draft::is_draft_round(self.round) {
                    GamePhase::HeroDraft
                } else {
                    GamePhase::Combat
                }
            }
        };
        self.phase.clone()
    }

    /// Apply damage to a player.
    pub fn apply_damage(&mut self, player_id: u8, surviving_heroes: u32) {
        if let Some(player) = self.players.get_mut(player_id as usize) {
            let dmg = damage::calculate_damage(self.round, surviving_heroes);
            player.hp -= dmg;
            if player.hp <= 0.0 {
                player.hp = 0.0;
            }
        }
    }

    /// Eliminate players at 0 HP.
    pub fn eliminate_dead(&mut self) {
        for player in &mut self.players {
            if player.hp <= 0.0 {
                player.alive = false;
            }
        }
    }

    /// Count alive players.
    pub fn alive_count(&self) -> usize {
        self.players.iter().filter(|p| p.alive).count()
    }

    /// Get the hero level for the current round.
    pub fn hero_level(&self) -> u8 {
        (1 + self.round).min(255) as u8
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn test_game() -> GameState {
        let counts: HashMap<String, u32> = (0..10)
            .map(|i| (format!("ability_{i}"), 20))
            .collect();
        let pool = AbilityPool::from_counts(counts);
        GameState::new(pool, HashSet::new(), GameConfig::default())
    }

    #[test]
    fn test_round1_phases() {
        let mut game = test_game();
        assert_eq!(game.phase, GamePhase::GodPick);
        assert_eq!(game.advance_phase(), GamePhase::HeroDraft);
        assert_eq!(game.advance_phase(), GamePhase::Shop);
        assert_eq!(game.advance_phase(), GamePhase::RoundEnd);
    }

    #[test]
    fn test_draft_round_phases() {
        let mut game = test_game();
        game.round = 2;
        game.phase = GamePhase::RoundEnd;
        // RoundEnd advances round to 3 (draft round)
        let phase = game.advance_phase();
        assert_eq!(game.round, 3);
        assert_eq!(phase, GamePhase::HeroDraft);
        assert_eq!(game.advance_phase(), GamePhase::Combat);
        assert_eq!(game.advance_phase(), GamePhase::Damage);
        assert_eq!(game.advance_phase(), GamePhase::Shop);
        assert_eq!(game.advance_phase(), GamePhase::RoundEnd);
    }

    #[test]
    fn test_normal_round_phases() {
        let mut game = test_game();
        game.round = 3;
        game.phase = GamePhase::RoundEnd;
        // Round 4 is not a draft round
        let phase = game.advance_phase();
        assert_eq!(game.round, 4);
        assert_eq!(phase, GamePhase::Combat);
        assert_eq!(game.advance_phase(), GamePhase::Damage);
        assert_eq!(game.advance_phase(), GamePhase::Shop);
        assert_eq!(game.advance_phase(), GamePhase::RoundEnd);
    }

    #[test]
    fn test_start_round_gold() {
        let mut game = test_game();
        game.round = 1;
        game.start_round();
        for p in &game.players {
            assert_eq!(p.gold, 6);
        }
    }

    #[test]
    fn test_damage_and_elimination_in_one_step() {
        let mut game = test_game();
        game.round = 5;
        game.phase = GamePhase::Combat;
        // Apply damage to bring player 0 to 0 HP
        game.players[0].hp = 1.0;
        game.apply_damage(0, 5); // will exceed 1.0 HP
        assert_eq!(game.players[0].hp, 0.0);
        assert!(game.players[0].alive); // not yet eliminated

        // Advance from Combat → Damage
        assert_eq!(game.advance_phase(), GamePhase::Damage);
        assert!(game.players[0].alive); // still alive in Damage phase entry

        // Advance from Damage → Shop (eliminates dead in this transition)
        assert_eq!(game.advance_phase(), GamePhase::Shop);
        assert!(!game.players[0].alive); // now eliminated
    }

    #[test]
    fn test_hero_level() {
        let mut game = test_game();
        game.round = 1;
        assert_eq!(game.hero_level(), 2);
        game.round = 10;
        assert_eq!(game.hero_level(), 11);
    }

    #[test]
    fn test_alive_count() {
        let mut game = test_game();
        assert_eq!(game.alive_count(), 8);
        game.players[0].alive = false;
        game.players[3].alive = false;
        assert_eq!(game.alive_count(), 6);
    }
}
