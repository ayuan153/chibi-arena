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
/// God pick phase duration in seconds.
pub const GOD_PICK_DURATION: f32 = 30.0;

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
    /// When true, timer reaching 0 auto-triggers phase transitions.
    /// When false (dev mode), transitions require manual trigger.
    pub auto_advance: bool,
    /// Maximum bench capacity.
    pub bench_capacity: u32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            ability_slots_per_hero: 4,
            shop_size_bonus: 0,
            gold_bonus: 0,
            reroll_cost_override: None,
            ultimate_unlock_level: 3,
            auto_advance: true,
            bench_capacity: 5,
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

/// Events produced by the tick system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GameEvent {
    /// Phase transitioned automatically.
    PhaseTransition(GamePhase),
    /// Combat timed out (draw forced).
    CombatTimeout,
    /// Random god assigned to player.
    RandomGodAssigned(u8),
    /// Random hero assigned to player (draft timeout).
    RandomHeroAssigned(u8, String),
    /// Archmage sorcery triggered.
    SorceryTriggered(u8, String),
    /// Game over.
    GameOver,
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
    /// Players who have signaled ready this phase.
    pub ready_players: HashSet<u8>,
    /// Available gods for this game.
    #[serde(default)]
    pub gods: Vec<god::God>,
    /// Draft choices per player: [STR, AGI, INT] hero names (migrated from client).
    #[serde(default)]
    pub draft_choices: HashMap<u8, [Option<String>; 3]>,
    /// If set, the next DraftHero action replaces this hero index instead of adding.
    #[serde(default)]
    pub pending_reroll: Option<usize>,
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
            timer: GOD_PICK_DURATION,
            draft_pending: false,
            matchup_rotation: Vec::new(),
            cycle_round: 0,
            ready_players: HashSet::new(),
            gods: god::all_gods(),
            draft_choices: HashMap::new(),
            pending_reroll: None,
        }
    }

    /// Start a new round: distribute gold, tick shop decay.
    pub fn start_round(&mut self) {
        let base_gold = economy::gold_for_round(self.round) + self.config.gold_bonus;
        for player in &mut self.players {
            if player.alive {
                player.gold = base_gold;
                // Only tick decay after round 1 (decay = rounds completed at level)
                if self.round > 1 {
                    player.shop.tick_decay();
                }
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
    /// Transitions to GracePeriod, or Finished if ≤1 player alive.
    pub fn end_combat(&mut self, _timed_out: bool) {
        // Check if game is over (≤1 alive)
        if self.alive_count() <= 1 {
            self.phase = GamePhase::Finished;
            self.timer = 0.0;
            return;
        }

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

    /// Apply a player action, validating state and dispatching logic.
    pub fn apply_action(&mut self, player_id: u8, action: crate::scenario::Action, hero_defs: &HashMap<String, aa2_data::HeroDef>, rng: &mut impl rand::Rng) -> Result<(), String> {
        let p_idx = player_id as usize;
        if p_idx >= self.players.len() {
            return Err("invalid player_id".to_string());
        }
        if !self.players[p_idx].alive {
            return Err("player is dead".to_string());
        }

        use crate::scenario::Action;
        match action {
            Action::Buy(slot) => {
                if let Some(Some(name)) = self.players[p_idx].shop.offerings.get(slot).cloned() {
                    let bench_cap = self.config.bench_capacity as usize;
                    self.players[p_idx].buy_ability(&name, &mut self.pool, bench_cap)
                        .map_err(|e| e.to_string())?;
                    self.players[p_idx].shop.offerings[slot] = None;
                } else {
                    return Err("invalid or empty shop slot".to_string());
                }
                Ok(())
            }
            Action::Sell(ref name) => {
                self.players[p_idx].sell_ability(name, &mut self.pool)
                    .map_err(|e| e.to_string())
            }
            Action::Equip(ref ability, ref hero) => {
                self.players[p_idx].equip_ability(ability, hero, &self.ultimates.clone(), &self.config.clone())
                    .map_err(|e| e.to_string())
            }
            Action::Unequip(ref ability, ref hero) => {
                self.players[p_idx].unequip_ability(ability, hero)
                    .map_err(|e| e.to_string())
            }
            Action::RerollShop => {
                let cost = self.config.reroll_cost_override.unwrap_or(economy::REROLL_COST);
                let ult_level = self.config.ultimate_unlock_level;
                let bonus = self.config.shop_size_bonus;
                let ultimates = self.ultimates.clone();
                self.players[p_idx].reroll_shop(&mut self.pool, &ultimates, ult_level, bonus, cost, rng)
                    .map_err(|e| e.to_string())
            }
            Action::UpgradeShop => {
                let mut gold = self.players[p_idx].gold;
                let old_level = self.players[p_idx].shop.level;
                if self.players[p_idx].shop.upgrade(&mut gold).is_some() {
                    self.players[p_idx].gold = gold;
                    let new_level = self.players[p_idx].shop.level;
                    // Reroll shop on upgrade (except L2→L3 which only enables ults)
                    if new_level != 3 || old_level != 2 {
                        self.players[p_idx].shop.roll(
                            &mut self.pool,
                            &self.ultimates,
                            self.config.ultimate_unlock_level,
                            self.config.shop_size_bonus,
                            rng,
                        );
                    }
                    Ok(())
                } else {
                    Err("cannot upgrade shop".to_string())
                }
            }
            Action::LockShop => {
                self.players[p_idx].shop.toggle_lock();
                Ok(())
            }
            Action::SetPosition(ref hero, x, y) => {
                self.players[p_idx].hero_positions.insert(hero.clone(), (x, y));
                Ok(())
            }
            Action::SetGodBuff(ref hero) => {
                self.players[p_idx].god_buff_target = Some(hero.clone());
                Ok(())
            }
            Action::PickGod(god) => {
                if self.phase != GamePhase::GodPick {
                    return Err("not in god pick phase".to_string());
                }
                self.players[p_idx].god = Some(god);
                Ok(())
            }
            Action::DraftHero(idx) => {
                if !self.draft_pending {
                    return Err("no draft active".to_string());
                }
                if idx > 2 {
                    return Err("draft index must be 0-2".to_string());
                }
                // Perform the actual hero assignment
                let hero_name = self.draft_choices.get(&player_id)
                    .and_then(|choices| choices.get(idx))
                    .and_then(|c| c.clone())
                    .ok_or_else(|| "no draft choice at index".to_string())?;
                if let Some(reroll_idx) = self.pending_reroll {
                    // Reroll: replace existing hero, keep abilities and position
                    if reroll_idx < self.players[p_idx].heroes.len() {
                        let old_hero = self.players[p_idx].heroes[reroll_idx].clone();
                        self.players[p_idx].heroes[reroll_idx] = hero_name.clone();
                        if let Some(pos) = self.players[p_idx].hero_positions.remove(&old_hero) {
                            self.players[p_idx].hero_positions.insert(hero_name.clone(), pos);
                        }
                        if let Some(abilities) = self.players[p_idx].equipped.remove(&old_hero) {
                            self.players[p_idx].equipped.insert(hero_name.clone(), abilities);
                        }
                    }
                    self.pending_reroll = None;
                } else {
                    // Normal draft: add new hero
                    self.players[p_idx].heroes.push(hero_name.clone());
                    self.players[p_idx].hero_positions.insert(hero_name, (500.0, 1500.0));
                }
                self.draft_choices.remove(&player_id);
                // Clear draft_pending if no more players need to draft
                if self.draft_choices.is_empty() {
                    self.draft_pending = false;
                }
                Ok(())
            }
            Action::RerollHero(ref hero_idx_str) => {
                let hero_idx: usize = hero_idx_str.parse().map_err(|_| "invalid hero index".to_string())?;
                if self.players[p_idx].gold < 2 {
                    return Err("not enough gold".to_string());
                }
                if hero_idx >= self.players[p_idx].heroes.len() {
                    return Err("invalid hero index".to_string());
                }
                self.players[p_idx].gold -= 2;
                let owned: Vec<&str> = self.players[p_idx].heroes.iter().map(|s| s.as_str()).collect();
                let mut available: Vec<&aa2_data::HeroDef> = hero_defs.values()
                    .filter(|h| !owned.contains(&h.name.as_str()))
                    .collect();
                available.sort_by_key(|h| &h.name);
                let choices = draft::generate_reroll_choices(&available, rng);
                self.draft_choices.insert(player_id, choices);
                self.pending_reroll = Some(hero_idx);
                Ok(())
            }
            Action::SwapAbilities(ref hero_name, slot_a, slot_b) => {
                if let Some(abilities) = self.players[p_idx].equipped.get_mut(hero_name)
                    && slot_a < abilities.len() && slot_b < abilities.len()
                {
                    abilities.swap(slot_a, slot_b);
                    Ok(())
                } else {
                    Err("invalid slot".to_string())
                }
            }
            Action::Ready => {
                // Pre-Ready: auto-pick random draft choice if pending (reroll or round draft)
                if let Some(choices) = self.draft_choices.get(&player_id).cloned() {
                    let valid: Vec<usize> = choices.iter().enumerate()
                        .filter(|(_, c)| c.is_some())
                        .map(|(i, _)| i)
                        .collect();
                    let pick_idx = if valid.is_empty() { 0 } else {
                        use rand::seq::SliceRandom;
                        *valid.choose(rng).unwrap()
                    };
                    let hero_name = choices[pick_idx].clone().unwrap_or_default();
                    if !hero_name.is_empty() {
                        if let Some(reroll_idx) = self.pending_reroll {
                            if reroll_idx < self.players[p_idx].heroes.len() {
                                let old = self.players[p_idx].heroes[reroll_idx].clone();
                                self.players[p_idx].heroes[reroll_idx] = hero_name.clone();
                                if let Some(pos) = self.players[p_idx].hero_positions.remove(&old) {
                                    self.players[p_idx].hero_positions.insert(hero_name.clone(), pos);
                                }
                                if let Some(abilities) = self.players[p_idx].equipped.remove(&old) {
                                    self.players[p_idx].equipped.insert(hero_name.clone(), abilities);
                                }
                            }
                        } else {
                            self.players[p_idx].heroes.push(hero_name.clone());
                            self.players[p_idx].hero_positions.insert(hero_name, (500.0, 1500.0));
                        }
                    }
                    self.draft_choices.remove(&player_id);
                    self.pending_reroll = None;
                }

                self.ready_players.insert(player_id);
                // Check if all alive players are ready
                let all_ready = self.players.iter()
                    .filter(|p| p.alive)
                    .all(|p| self.ready_players.contains(&p.id));
                if all_ready {
                    self.ready_players.clear();
                    self.timer = 0.0;
                    // Force auto_advance for one tick to trigger transition
                    let was = self.config.auto_advance;
                    self.config.auto_advance = true;
                    let events = self.tick(0.01, rng);
                    self.config.auto_advance = was;
                    let _ = events;

                    // Roll shops for all alive players if entering Shop phase
                    if self.phase == GamePhase::Shop {
                        for i in 0..self.players.len() {
                            if self.players[i].alive && !self.players[i].shop.locked {
                                self.players[i].shop.roll(
                                    &mut self.pool,
                                    &self.ultimates,
                                    self.config.ultimate_unlock_level,
                                    self.config.shop_size_bonus,
                                    rng,
                                );
                            }
                        }
                    }

                    // Post-Ready: generate draft choices if we just entered a draft round
                    if self.phase == GamePhase::Shop && self.draft_pending && !self.draft_choices.contains_key(&0) {
                        use crate::draft::{generate_draft_choices, tier_for_draft_round};
                        let tier = tier_for_draft_round(self.round).unwrap_or(0);
                        let mut all_heroes: Vec<&aa2_data::HeroDef> = hero_defs.values().collect();
                        all_heroes.sort_by_key(|h| &h.name);
                        for i in 0..self.players.len() {
                            if self.players[i].alive {
                                let owned: Vec<&str> = self.players[i].heroes.iter().map(|s| s.as_str()).collect();
                                let available: Vec<&aa2_data::HeroDef> = all_heroes.iter()
                                    .filter(|h| !owned.contains(&h.name.as_str()))
                                    .copied()
                                    .collect();
                                let choices = generate_draft_choices(&available, tier, rng);
                                self.draft_choices.insert(self.players[i].id, choices);
                            }
                        }
                    }
                }
                Ok(())
            }
        }
    }

    /// Get the hero level for the current round.
    pub fn hero_level(&self) -> u8 {
        (1 + self.round).min(255) as u8
    }

    /// Advance game time by `dt` seconds. Returns events that occurred.
    /// Mid-phase events (combat timeout) always fire.
    /// Phase transitions only fire if `config.auto_advance` is true.
    pub fn tick(&mut self, dt: f32, rng: &mut impl rand::Rng) -> Vec<GameEvent> {
        let mut events = Vec::new();
        self.timer -= dt;

        // Mid-phase events (always fire)
        if self.phase == GamePhase::Combat && self.timer <= 0.0 {
            events.push(GameEvent::CombatTimeout);
        }

        // Phase transition events (only if auto_advance)
        if self.config.auto_advance && self.timer <= 0.0 {
            match self.phase {
                GamePhase::GodPick => self.auto_end_god_pick(rng, &mut events),
                GamePhase::Shop => self.auto_end_shop(&mut events),
                GamePhase::GracePeriod => {
                    self.end_grace_period(rng);
                    events.push(GameEvent::PhaseTransition(GamePhase::Shop));
                }
                GamePhase::Combat | GamePhase::Finished => {}
            }
        }

        events
    }

    /// Auto-end god pick: assign random gods to players who haven't picked.
    fn auto_end_god_pick(&mut self, rng: &mut impl rand::Rng, events: &mut Vec<GameEvent>) {
        let gods = self.gods.clone();
        for player in &mut self.players {
            if player.alive && player.god.is_none() {
                let god = gods[rng.gen_range(0..gods.len())].clone();
                events.push(GameEvent::RandomGodAssigned(player.id));
                player.god = Some(god);
            }
        }
        self.start_round1();
        events.push(GameEvent::PhaseTransition(GamePhase::Shop));
    }

    /// Auto-end shop: clear draft if pending, start combat.
    fn auto_end_shop(&mut self, events: &mut Vec<GameEvent>) {
        self.draft_pending = false;
        self.end_shop();
        events.push(GameEvent::PhaseTransition(GamePhase::Combat));
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
        let expected_dmg_to_0 = damage::calculate_damage(5, 3);
        assert!((game.players[0].hp - (200.0 - expected_dmg_to_0)).abs() < 0.001);
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

    #[test]
    fn test_tick_shop_auto_advance() {
        let mut game = test_game();
        game.phase = GamePhase::Shop;
        game.timer = 27.0;
        game.round = 1;
        let mut rng = rand::thread_rng();

        let events = game.tick(26.9, &mut rng);
        assert!(events.is_empty());
        assert_eq!(game.phase, GamePhase::Shop);

        let events = game.tick(0.2, &mut rng);
        assert!(events.contains(&GameEvent::PhaseTransition(GamePhase::Combat)));
        assert_eq!(game.phase, GamePhase::Combat);
    }

    #[test]
    fn test_tick_shop_no_auto_advance() {
        let mut game = test_game();
        game.config.auto_advance = false;
        game.phase = GamePhase::Shop;
        game.timer = 27.0;
        game.round = 1;
        let mut rng = rand::thread_rng();

        let events = game.tick(28.0, &mut rng);
        assert!(events.is_empty());
        assert_eq!(game.phase, GamePhase::Shop);
    }

    #[test]
    fn test_tick_combat_timeout_always_fires() {
        let mut game = test_game();
        game.config.auto_advance = false;
        game.phase = GamePhase::Combat;
        game.timer = COMBAT_TIMEOUT;
        let mut rng = rand::thread_rng();

        let events = game.tick(50.1, &mut rng);
        assert!(events.contains(&GameEvent::CombatTimeout));
        // Phase unchanged — no auto_advance
        assert_eq!(game.phase, GamePhase::Combat);
    }

    #[test]
    fn test_tick_grace_period_auto_advance() {
        let mut game = test_game();
        game.phase = GamePhase::GracePeriod;
        game.timer = 3.0;
        game.round = 2;
        let mut rng = rand::thread_rng();

        let events = game.tick(3.1, &mut rng);
        assert!(events.contains(&GameEvent::PhaseTransition(GamePhase::Shop)));
        assert_eq!(game.phase, GamePhase::Shop);
        assert_eq!(game.round, 3);
    }

    #[test]
    fn test_tick_god_pick_timeout() {
        let mut game = test_game();
        game.phase = GamePhase::GodPick;
        game.timer = GOD_PICK_DURATION;
        let mut rng = rand::thread_rng();

        // All players have no god
        let events = game.tick(30.1, &mut rng);
        // Should assign random gods and transition
        assert!(events.contains(&GameEvent::PhaseTransition(GamePhase::Shop)));
        assert_eq!(game.phase, GamePhase::Shop);
        assert_eq!(game.round, 1);
        for player in &game.players {
            assert!(player.god.is_some());
        }
    }
}
