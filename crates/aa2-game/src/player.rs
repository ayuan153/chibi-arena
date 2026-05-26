//! Player state management — inventory, heroes, abilities, gold.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::economy::{BUY_COST, UNEQUIP_COST};
use crate::game::GameConfig;
use crate::god::God;
use crate::pool::AbilityPool;
use crate::shop::ShopState;

/// Maximum number of heroes a player can have.
pub const MAX_HEROES: usize = 5;
/// Maximum ability level (copies purchased).
pub const MAX_ABILITY_LEVEL: u32 = 9;
/// Maximum bench size.
pub const MAX_BENCH: usize = 5;

/// Complete state for a single player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    /// Player ID (0-7).
    pub id: u8,
    /// Current gold.
    pub gold: u32,
    /// Current hit points.
    pub hp: f32,
    /// Owned heroes (by name).
    pub heroes: Vec<String>,
    /// Owned abilities: name → level (copies purchased).
    pub abilities: HashMap<String, u32>,
    /// Equipped abilities per hero: hero_name → vec of ability names.
    pub equipped: HashMap<String, Vec<String>>,
    /// Bench: unequipped abilities (max 5 slots).
    pub bench: Vec<String>,
    /// Chosen god.
    pub god: Option<God>,
    /// Hero name selected for god buff (e.g. Paladin's Radiant Shield target).
    pub god_buff_target: Option<String>,
    /// Shop state.
    pub shop: ShopState,
    /// Whether this player is still alive.
    pub alive: bool,
    /// Hero positions on the player's half of the arena (1000x2000 bottom half).
    /// Maps hero name to position. X: 0-2000, Y: 0-1000.
    pub hero_positions: HashMap<String, (f32, f32)>,
}

/// Mirror a position from player's perspective to opponent's top half.
/// Flips both X and Y: (x, y) -> (2000 - x, 2000 - y).
pub fn mirror_position(x: f32, y: f32) -> (f32, f32) {
    (2000.0 - x, 2000.0 - y)
}

impl PlayerState {
    /// Create a new player with starting values.
    pub fn new(id: u8) -> Self {
        Self {
            id,
            gold: 0,
            hp: crate::damage::STARTING_HP,
            heroes: Vec::new(),
            abilities: HashMap::new(),
            equipped: HashMap::new(),
            bench: Vec::new(),
            god: None,
            god_buff_target: None,
            shop: ShopState::new(),
            alive: true,
            hero_positions: HashMap::new(),
        }
    }

    /// Check if the player can afford to buy an ability.
    pub fn can_buy(&self) -> bool {
        self.gold >= BUY_COST
    }

    /// Buy an ability. If new, goes to bench. If duplicate, levels up (no bench slot).
    pub fn buy_ability(&mut self, name: &str, pool: &mut AbilityPool, bench_cap: usize) -> Result<(), &'static str> {
        if self.gold < BUY_COST {
            return Err("not enough gold");
        }
        let already_owned = self.abilities.contains_key(name);
        if already_owned {
            let level = self.abilities.get(name).copied().unwrap_or(0);
            if level >= MAX_ABILITY_LEVEL {
                return Err("ability at max level");
            }
        } else if self.bench.len() >= bench_cap {
            return Err("bench is full");
        }
        if !pool.take(name) {
            return Err("ability depleted from pool");
        }
        self.gold -= BUY_COST;
        if already_owned {
            *self.abilities.get_mut(name).expect("checked above") += 1;
        } else {
            self.abilities.insert(name.to_string(), 1);
            self.bench.push(name.to_string());
        }
        Ok(())
    }

    /// Sell an ability. Refunds 2g * level, returns all copies to pool.
    pub fn sell_ability(&mut self, name: &str, pool: &mut AbilityPool) -> Result<(), &'static str> {
        let level = self.abilities.remove(name).ok_or("ability not owned")?;
        self.gold += 2 * level;
        pool.return_copies(name, level);
        // Remove from bench
        if let Some(pos) = self.bench.iter().position(|a| a == name) {
            self.bench.remove(pos);
        } else {
            // Remove from equipped hero
            for abilities in self.equipped.values_mut() {
                if let Some(pos) = abilities.iter().position(|a| a == name) {
                    abilities.remove(pos);
                    break;
                }
            }
        }
        Ok(())
    }

    /// Equip an ability to a hero. Free (no gold cost).
    pub fn equip_ability(
        &mut self,
        ability_name: &str,
        hero_name: &str,
        ultimates: &HashSet<String>,
        config: &GameConfig,
    ) -> Result<(), &'static str> {
        if !self.abilities.contains_key(ability_name) {
            return Err("ability not owned");
        }
        if !self.heroes.contains(&hero_name.to_string()) {
            return Err("hero not owned");
        }
        let bench_pos = self.bench.iter().position(|a| a == ability_name)
            .ok_or("ability not on bench")?;
        let hero_abilities = self.equipped.entry(hero_name.to_string()).or_default();
        if hero_abilities.len() >= config.ability_slots_per_hero as usize {
            return Err("no free slots");
        }
        if ultimates.contains(ability_name) && hero_abilities.iter().any(|a| ultimates.contains(a.as_str())) {
            return Err("hero already has an ultimate");
        }
        self.bench.remove(bench_pos);
        hero_abilities.push(ability_name.to_string());
        Ok(())
    }

    /// Unequip an ability from a hero. Costs 1 gold.
    pub fn unequip_ability(&mut self, ability_name: &str, hero_name: &str) -> Result<(), &'static str> {
        if self.gold < UNEQUIP_COST {
            return Err("not enough gold");
        }
        if self.bench.len() >= MAX_BENCH {
            return Err("bench is full");
        }
        let hero_abilities = self.equipped.get_mut(hero_name)
            .ok_or("hero has no equipped abilities")?;
        let pos = hero_abilities.iter().position(|a| a == ability_name)
            .ok_or("ability not equipped on hero")?;
        hero_abilities.remove(pos);
        self.gold -= UNEQUIP_COST;
        self.bench.push(ability_name.to_string());
        Ok(())
    }

    /// Reroll (replace) an existing hero. Costs 2g.
    /// Discards the named hero and returns 3 new choices (1 STR/AGI/INT) from all tiers.
    /// The player MUST pick one of the new choices (old hero is gone).
    pub fn reroll_hero(
        &mut self,
        hero_to_discard: &str,
        available_heroes: &[&aa2_data::HeroDef],
        rng: &mut impl rand::Rng,
    ) -> Result<[Option<String>; 3], &'static str> {
        if self.gold < crate::economy::HERO_REROLL_COST {
            return Err("not enough gold");
        }
        if !self.heroes.contains(&hero_to_discard.to_string()) {
            return Err("you don't own that hero");
        }
        self.gold -= crate::economy::HERO_REROLL_COST;
        // Remove hero from roster
        self.heroes.retain(|h| h != hero_to_discard);
        // Move equipped abilities back to bench
        if let Some(abilities) = self.equipped.remove(hero_to_discard) {
            for ability in abilities {
                if self.bench.len() < MAX_BENCH {
                    self.bench.push(ability);
                }
            }
        }
        // Remove position
        self.hero_positions.remove(hero_to_discard);
        // Generate new choices from all available heroes (excluding owned)
        let choices = crate::draft::generate_reroll_choices(available_heroes, rng);
        Ok(choices)
    }

    /// Reroll the shop offerings.
    pub fn reroll_shop(
        &mut self,
        pool: &mut AbilityPool,
        ultimates: &HashSet<String>,
        ultimate_unlock_level: u32,
        size_bonus: u32,
        reroll_cost: u32,
        rng: &mut impl rand::Rng,
    ) -> Result<(), &'static str> {
        if self.shop.locked {
            return Err("shop is locked");
        }
        if self.gold < reroll_cost {
            return Err("not enough gold");
        }
        self.gold -= reroll_cost;
        self.shop.roll(pool, ultimates, ultimate_unlock_level, size_bonus, rng);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_pool() -> AbilityPool {
        let mut counts = HashMap::new();
        counts.insert("fireball".to_string(), 10);
        counts.insert("heal".to_string(), 10);
        counts.insert("ult_strike".to_string(), 10);
        AbilityPool::from_counts(counts)
    }

    #[test]
    fn test_new_player() {
        let p = PlayerState::new(0);
        assert_eq!(p.id, 0);
        assert_eq!(p.hp, 200.0);
        assert!(p.alive);
        assert!(p.heroes.is_empty());
    }

    #[test]
    fn test_can_buy() {
        let mut p = PlayerState::new(0);
        assert!(!p.can_buy());
        p.gold = 3;
        assert!(p.can_buy());
    }

    #[test]
    fn test_buy_ability_new() {
        let mut p = PlayerState::new(0);
        p.gold = 10;
        let mut pool = test_pool();

        assert!(p.buy_ability("fireball", &mut pool, MAX_BENCH).is_ok());
        assert_eq!(p.gold, 7);
        assert_eq!(p.abilities["fireball"], 1);
        assert_eq!(p.bench, vec!["fireball".to_string()]);
        assert_eq!(pool.counts["fireball"], 9);
    }

    #[test]
    fn test_buy_ability_duplicate_levels_up() {
        let mut p = PlayerState::new(0);
        p.gold = 10;
        let mut pool = test_pool();

        p.buy_ability("fireball", &mut pool, MAX_BENCH).unwrap();
        p.buy_ability("fireball", &mut pool, MAX_BENCH).unwrap();
        assert_eq!(p.abilities["fireball"], 2);
        // Bench should still only have one entry
        assert_eq!(p.bench.len(), 1);
        assert_eq!(p.gold, 4);
    }

    #[test]
    fn test_buy_ability_bench_full() {
        let mut p = PlayerState::new(0);
        p.gold = 100;
        let mut pool = test_pool();
        // Fill bench with 5 different abilities
        for i in 0..5 {
            let name = format!("ability_{i}");
            pool.counts.insert(name.clone(), 10);
            p.buy_ability(&name, &mut pool, MAX_BENCH).unwrap();
        }
        assert_eq!(p.bench.len(), 5);
        // New ability should be rejected
        let result = p.buy_ability("fireball", &mut pool, MAX_BENCH);
        assert_eq!(result, Err("bench is full"));
    }

    #[test]
    fn test_buy_ability_max_level() {
        let mut p = PlayerState::new(0);
        p.gold = 100;
        p.abilities.insert("fireball".to_string(), 9);
        p.bench.push("fireball".to_string());
        let mut pool = test_pool();

        let result = p.buy_ability("fireball", &mut pool, MAX_BENCH);
        assert_eq!(result, Err("ability at max level"));
    }

    #[test]
    fn test_sell_ability() {
        let mut p = PlayerState::new(0);
        p.gold = 0;
        p.abilities.insert("fireball".to_string(), 3);
        p.bench.push("fireball".to_string());
        let mut pool = test_pool();

        assert!(p.sell_ability("fireball", &mut pool).is_ok());
        assert_eq!(p.gold, 6); // 2 * 3
        assert!(!p.abilities.contains_key("fireball"));
        assert!(!p.bench.contains(&"fireball".to_string()));
        assert_eq!(pool.counts["fireball"], 13); // 10 + 3 returned
    }

    #[test]
    fn test_equip_ability() {
        let mut p = PlayerState::new(0);
        p.heroes.push("axe".to_string());
        p.abilities.insert("fireball".to_string(), 1);
        p.bench.push("fireball".to_string());
        let ultimates = HashSet::new();
        let config = GameConfig::default();

        assert!(p.equip_ability("fireball", "axe", &ultimates, &config).is_ok());
        assert!(p.bench.is_empty());
        assert_eq!(p.equipped["axe"], vec!["fireball".to_string()]);
    }

    #[test]
    fn test_equip_ability_full_slots() {
        let mut p = PlayerState::new(0);
        p.heroes.push("axe".to_string());
        let config = GameConfig { ability_slots_per_hero: 2, ..Default::default() };
        let ultimates = HashSet::new();

        // Equip 2 abilities
        for name in &["a", "b"] {
            p.abilities.insert(name.to_string(), 1);
            p.bench.push(name.to_string());
            p.equip_ability(name, "axe", &ultimates, &config).unwrap();
        }
        // Third should fail
        p.abilities.insert("c".to_string(), 1);
        p.bench.push("c".to_string());
        let result = p.equip_ability("c", "axe", &ultimates, &config);
        assert_eq!(result, Err("no free slots"));
    }

    #[test]
    fn test_equip_ability_duplicate_ultimate() {
        let mut p = PlayerState::new(0);
        p.heroes.push("axe".to_string());
        let mut ultimates = HashSet::new();
        ultimates.insert("ult1".to_string());
        ultimates.insert("ult2".to_string());
        let config = GameConfig::default();

        p.abilities.insert("ult1".to_string(), 1);
        p.bench.push("ult1".to_string());
        p.equip_ability("ult1", "axe", &ultimates, &config).unwrap();

        p.abilities.insert("ult2".to_string(), 1);
        p.bench.push("ult2".to_string());
        let result = p.equip_ability("ult2", "axe", &ultimates, &config);
        assert_eq!(result, Err("hero already has an ultimate"));
    }

    #[test]
    fn test_unequip_ability() {
        let mut p = PlayerState::new(0);
        p.gold = 5;
        p.heroes.push("axe".to_string());
        p.abilities.insert("fireball".to_string(), 1);
        p.equipped.insert("axe".to_string(), vec!["fireball".to_string()]);

        assert!(p.unequip_ability("fireball", "axe").is_ok());
        assert!(p.equipped["axe"].is_empty());
        assert_eq!(p.bench, vec!["fireball".to_string()]);
        assert_eq!(p.gold, 4);
    }

    #[test]
    fn test_unequip_ability_bench_full() {
        let mut p = PlayerState::new(0);
        p.gold = 5;
        p.heroes.push("axe".to_string());
        p.abilities.insert("fireball".to_string(), 1);
        p.equipped.insert("axe".to_string(), vec!["fireball".to_string()]);
        // Fill bench
        p.bench = vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string(), "e".to_string()];

        let result = p.unequip_ability("fireball", "axe");
        assert_eq!(result, Err("bench is full"));
    }
}
