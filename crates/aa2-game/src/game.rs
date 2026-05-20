//! Game state, round state machine, and configuration.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::combat::{self, CombatResult};
use crate::damage;
use crate::draft;
use crate::economy;
use crate::god;
use crate::matchup;
use crate::player::PlayerState;
use crate::pool::AbilityPool;

/// Total round duration in seconds.
pub const ROUND_DURATION: f32 = 80.0;
/// Maximum combat duration (combat ends at 30s remaining).
pub const COMBAT_TIMEOUT: f32 = 50.0;
/// Grace period after combat ends (seconds).
pub const GRACE_PERIOD: f32 = 3.0;
/// Round 1 special duration (no combat, just shop+draft).
pub const ROUND1_DURATION: f32 = 40.0;

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

/// The main game phases. GodPick is pre-game.
/// The core loop is Combat → GracePeriod → Shop, repeating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamePhase {
    /// Pre-game: players pick their god. Not a round.
    GodPick,
    /// Combat simulation running. Timeout at 30s remaining.
    Combat,
    /// 3s after combat: damage animation, old gold still usable, shop auto-rerolls.
    GracePeriod,
    /// Main shop phase. Draft overlay active on draft rounds.
    Shop,
    /// Game over.
    Finished,
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
    /// Timer remaining in current phase (seconds).
    pub timer: f32,
    /// Whether a hero draft is pending this round (concurrent with shop).
    pub draft_pending: bool,
    /// Current round-robin rotation order (shuffled at cycle start).
    pub matchup_rotation: Vec<u8>,
    /// Current position within the round-robin cycle.
    pub cycle_round: usize,
}

impl GameState {
    /// Create a new game with 8 players and the given pool/ultimates.
    pub fn new(pool: AbilityPool, ultimates: HashSet<String>, config: GameConfig) -> Self {
        let players = (0..8).map(PlayerState::new).collect();
        Self {
            players,
            round: 0,
            phase: GamePhase::GodPick,
            pool,
            matchups: Vec::new(),
            ultimates,
            config,
            timer: 0.0,
            draft_pending: false,
            matchup_rotation: Vec::new(),
            cycle_round: 0,
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

    /// Start round 1 (special: no combat, just shop+draft).
    pub fn start_round1(&mut self) {
        self.round = 1;
        self.phase = GamePhase::Shop;
        self.timer = ROUND1_DURATION;
        self.draft_pending = true;
        self.start_round();
    }

    /// Called when combat resolves (winner determined or timeout).
    /// Transitions to GracePeriod.
    pub fn end_combat(&mut self, _timed_out: bool) {
        self.phase = GamePhase::GracePeriod;
        self.timer = GRACE_PERIOD;
        for player in &mut self.players {
            if player.alive {
                if !player.shop.locked {
                    player.shop.needs_reroll = true;
                } else {
                    // Lock auto-clears after preserving once
                    player.shop.locked = false;
                }
            }
        }
    }

    /// Called when grace period ends. Resets gold, starts shop phase.
    pub fn end_grace_period(&mut self, rng: &mut impl rand::Rng) {
        self.round += 1;
        self.phase = GamePhase::Shop;
        self.timer = ROUND_DURATION - COMBAT_TIMEOUT - GRACE_PERIOD;
        self.start_round();
        self.draft_pending = draft::is_draft_round(self.round);
        // Trigger Archmage sorcery for eligible players
        for player in &mut self.players {
            if player.alive {
                god::maybe_trigger_sorcery(player, rng);
            }
        }
    }

    /// Called when shop timer hits 0. Starts next combat.
    pub fn end_shop(&mut self) {
        self.phase = GamePhase::Combat;
        self.timer = COMBAT_TIMEOUT;
        self.draft_pending = false;
    }

    /// Run combat for all matchups this round.
    ///
    /// Generates matchups via round-robin, runs simulations, applies damage.
    /// Returns combat results for each matchup.
    pub fn run_combat_round(
        &mut self,
        hero_defs: &HashMap<String, aa2_data::HeroDef>,
        ability_defs: &HashMap<String, aa2_data::AbilityDef>,
        seed: u32,
        rng: &mut impl rand::Rng,
    ) -> Vec<CombatResult> {
        let alive: Vec<u8> = self.players.iter()
            .filter(|p| p.alive)
            .map(|p| p.id)
            .collect();

        if alive.len() < 2 {
            return Vec::new();
        }

        // Refresh rotation at cycle start
        let cycle_len = matchup::cycle_length(&self.matchup_rotation);
        if self.matchup_rotation.is_empty() || self.cycle_round >= cycle_len {
            self.matchup_rotation = matchup::new_rotation(&alive, rng);
            self.cycle_round = 0;
        }

        let matchups = matchup::generate_matchups(
            &alive,
            &self.matchup_rotation,
            self.cycle_round,
            rng,
        );
        self.cycle_round += 1;

        let hero_level = self.hero_level();
        let results = combat::run_all_matchups(
            &matchups,
            &self.players,
            hero_defs,
            ability_defs,
            hero_level,
            self.round,
            seed,
        );

        // Apply damage based on results
        for result in &results {
            match result.winner {
                Some(winner_id) => {
                    // Loser takes damage
                    let loser = if winner_id == result.matchup.player_a {
                        result.matchup.player_b
                    } else {
                        result.matchup.player_a
                    };
                    // Ghost is immune to damage
                    if result.matchup.ghost && loser != result.matchup.player_a {
                        // Ghost lost — no damage to anyone
                    } else {
                        let survivors = if winner_id == result.matchup.player_a {
                            result.survivors_a
                        } else {
                            result.survivors_b
                        };
                        self.apply_damage(loser, survivors);
                    }
                }
                None => {
                    // Draw: both take damage, but ghost is immune
                    if result.matchup.ghost {
                        // Only real player takes damage
                        self.apply_damage(result.matchup.player_a, result.survivors_b);
                    } else {
                        self.apply_draw_damage(
                            result.matchup.player_a,
                            result.matchup.player_b,
                            result.survivors_a,
                            result.survivors_b,
                        );
                    }
                }
            }
        }

        self.eliminate_dead();
        results
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

    /// Apply draw damage to both players in a matchup.
    /// Each player takes damage based on the other's surviving heroes.
    pub fn apply_draw_damage(&mut self, player_a: u8, player_b: u8, survivors_a: u32, survivors_b: u32) {
        let dmg_to_a = damage::calculate_damage(self.round, survivors_b);
        let dmg_to_b = damage::calculate_damage(self.round, survivors_a);
        if let Some(p) = self.players.get_mut(player_a as usize) {
            p.hp = (p.hp - dmg_to_a).max(0.0);
        }
        if let Some(p) = self.players.get_mut(player_b as usize) {
            p.hp = (p.hp - dmg_to_b).max(0.0);
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
    fn test_start_round1() {
        let mut game = test_game();
        game.start_round1();
        assert_eq!(game.round, 1);
        assert_eq!(game.phase, GamePhase::Shop);
        assert_eq!(game.timer, ROUND1_DURATION);
        assert!(game.draft_pending);
        // Gold should be set for round 1
        assert_eq!(game.players[0].gold, 6);
    }

    #[test]
    fn test_end_combat_transitions_to_grace_period() {
        let mut game = test_game();
        game.phase = GamePhase::Combat;
        game.timer = COMBAT_TIMEOUT;
        game.end_combat(false);
        assert_eq!(game.phase, GamePhase::GracePeriod);
        assert_eq!(game.timer, GRACE_PERIOD);
    }

    #[test]
    fn test_end_combat_marks_shops_for_reroll() {
        let mut game = test_game();
        game.phase = GamePhase::Combat;
        game.end_combat(false);
        for player in &game.players {
            assert!(player.shop.needs_reroll);
        }
    }

    #[test]
    fn test_end_combat_locked_shop_preserves_and_clears_lock() {
        let mut game = test_game();
        game.phase = GamePhase::Combat;
        game.players[0].shop.locked = true;
        game.end_combat(false);
        // Locked shop should NOT be marked for reroll
        assert!(!game.players[0].shop.needs_reroll);
        // Lock should be cleared
        assert!(!game.players[0].shop.locked);
        // Other players should be marked for reroll
        assert!(game.players[1].shop.needs_reroll);
    }

    #[test]
    fn test_end_grace_period_increments_round() {
        let mut game = test_game();
        game.round = 2;
        game.phase = GamePhase::GracePeriod;
        game.end_grace_period(&mut rand::thread_rng());
        assert_eq!(game.round, 3);
        assert_eq!(game.phase, GamePhase::Shop);
        assert_eq!(game.timer, ROUND_DURATION - COMBAT_TIMEOUT - GRACE_PERIOD);
    }

    #[test]
    fn test_end_grace_period_resets_gold() {
        let mut game = test_game();
        game.round = 2;
        game.players[0].gold = 0;
        game.end_grace_period(&mut rand::thread_rng());
        // Round 3 gold = 6 + 2*(3-1) = 10
        assert_eq!(game.players[0].gold, 10);
    }

    #[test]
    fn test_end_grace_period_sets_draft_pending() {
        let mut game = test_game();
        // Round 2 → 3 is a draft round
        game.round = 2;
        game.end_grace_period(&mut rand::thread_rng());
        assert!(game.draft_pending);

        // Round 3 → 4 is NOT a draft round
        game.round = 3;
        game.draft_pending = false;
        game.end_grace_period(&mut rand::thread_rng());
        assert!(!game.draft_pending);
    }

    #[test]
    fn test_end_shop_transitions_to_combat() {
        let mut game = test_game();
        game.phase = GamePhase::Shop;
        game.draft_pending = true;
        game.end_shop();
        assert_eq!(game.phase, GamePhase::Combat);
        assert_eq!(game.timer, COMBAT_TIMEOUT);
        assert!(!game.draft_pending);
    }

    #[test]
    fn test_apply_draw_damage() {
        let mut game = test_game();
        game.round = 5;
        game.apply_draw_damage(0, 1, 2, 3);
        // Player 0 takes damage from 3 survivors: 5*0.5 + (1+5*0.1)*3 = 2.5 + 4.5 = 7.0
        let expected_dmg_to_0 = damage::calculate_damage(5, 3);
        assert!((game.players[0].hp - (200.0 - expected_dmg_to_0)).abs() < 0.001);
        // Player 1 takes damage from 2 survivors: 5*0.5 + (1+5*0.1)*2 = 2.5 + 3.0 = 5.5
        let expected_dmg_to_1 = damage::calculate_damage(5, 2);
        assert!((game.players[1].hp - (200.0 - expected_dmg_to_1)).abs() < 0.001);
    }

    #[test]
    fn test_apply_draw_damage_clamps_to_zero() {
        let mut game = test_game();
        game.round = 10;
        game.players[0].hp = 1.0;
        game.apply_draw_damage(0, 1, 0, 5);
        assert_eq!(game.players[0].hp, 0.0);
    }

    #[test]
    fn test_timer_values_full_cycle() {
        let mut game = test_game();
        // Start round 1
        game.start_round1();
        assert_eq!(game.timer, 40.0);

        // End shop → combat
        game.end_shop();
        assert_eq!(game.timer, 50.0);

        // End combat → grace
        game.end_combat(false);
        assert_eq!(game.timer, 3.0);

        // End grace → shop
        game.end_grace_period(&mut rand::thread_rng());
        assert_eq!(game.timer, 27.0); // 80 - 50 - 3
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

    #[test]
    fn test_eliminate_dead() {
        let mut game = test_game();
        game.players[0].hp = 0.0;
        game.players[2].hp = -5.0;
        game.eliminate_dead();
        assert!(!game.players[0].alive);
        assert!(!game.players[2].alive);
        assert!(game.players[1].alive);
    }

    #[test]
    fn test_needs_reroll_flag() {
        let mut game = test_game();
        // Initially false
        assert!(!game.players[0].shop.needs_reroll);
        game.end_combat(false);
        assert!(game.players[0].shop.needs_reroll);
    }

    #[test]
    fn test_dead_players_not_rerolled() {
        let mut game = test_game();
        game.players[0].alive = false;
        game.end_combat(false);
        assert!(!game.players[0].shop.needs_reroll);
        assert!(game.players[1].shop.needs_reroll);
    }
}
