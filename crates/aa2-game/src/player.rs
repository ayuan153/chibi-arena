//! Player state management — inventory, heroes, abilities, gold.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::economy::{BUY_COST, SELL_REFUND, UNEQUIP_COST};
use crate::pool::AbilityPool;
use crate::shop::ShopState;

/// Maximum number of heroes a player can have.
pub const MAX_HEROES: usize = 5;
/// Maximum ability level (copies purchased).
pub const MAX_ABILITY_LEVEL: u32 = 9;

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
    /// Owned abilities: name → copies/level.
    pub abilities: HashMap<String, u32>,
    /// Equipped abilities per hero: hero_name → vec of ability names.
    pub equipped: HashMap<String, Vec<String>>,
    /// Bench heroes (by name).
    pub bench: Vec<String>,
    /// Chosen god.
    pub god: Option<String>,
    /// Shop state.
    pub shop: ShopState,
    /// Whether this player is still alive.
    pub alive: bool,
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
            shop: ShopState::new(),
            alive: true,
        }
    }

    /// Check if the player can afford to buy an ability.
    pub fn can_buy(&self) -> bool {
        self.gold >= BUY_COST
    }

    /// Buy an ability from the shop at the given index.
    /// Removes from offerings, deducts from pool, adds to inventory.
    pub fn buy_ability(&mut self, index: usize, pool: &mut AbilityPool) -> Result<(), &'static str> {
        if self.gold < BUY_COST {
            return Err("not enough gold");
        }
        let name = self.shop.offerings.get(index).ok_or("invalid shop index")?.clone();
        if !pool.take(&name) {
            return Err("ability depleted from pool");
        }
        self.gold -= BUY_COST;
        let level = self.abilities.entry(name.clone()).or_insert(0);
        if *level >= MAX_ABILITY_LEVEL {
            // Return to pool since we can't hold more
            pool.return_ability(&name);
            return Err("ability at max level");
        }
        *level += 1;
        self.shop.offerings.remove(index);
        Ok(())
    }

    /// Sell an ability back to the pool.
    pub fn sell_ability(&mut self, name: &str, pool: &mut AbilityPool) -> Result<(), &'static str> {
        let level = self.abilities.get_mut(name).ok_or("ability not owned")?;
        if *level == 0 {
            return Err("ability not owned");
        }
        *level -= 1;
        if *level == 0 {
            self.abilities.remove(name);
            // Unequip from all heroes
            for abilities in self.equipped.values_mut() {
                abilities.retain(|a| a != name);
            }
        }
        pool.return_ability(name);
        self.gold += SELL_REFUND;
        Ok(())
    }

    /// Equip an ability to a hero.
    pub fn equip(
        &mut self,
        hero: &str,
        ability: &str,
        slots_per_hero: u32,
        ultimates: &HashSet<String>,
    ) -> Result<(), &'static str> {
        if !self.heroes.contains(&hero.to_string()) {
            return Err("hero not owned");
        }
        if !self.abilities.contains_key(ability) {
            return Err("ability not owned");
        }
        let hero_abilities = self.equipped.entry(hero.to_string()).or_default();
        if hero_abilities.len() >= slots_per_hero as usize {
            return Err("no free slots");
        }
        // Max 1 ultimate per hero
        if ultimates.contains(ability) && hero_abilities.iter().any(|a| ultimates.contains(a)) {
            return Err("hero already has an ultimate");
        }
        hero_abilities.push(ability.to_string());
        Ok(())
    }

    /// Unequip an ability from a hero. Costs gold.
    pub fn unequip(&mut self, hero: &str, ability: &str) -> Result<(), &'static str> {
        if self.gold < UNEQUIP_COST {
            return Err("not enough gold");
        }
        let hero_abilities = self.equipped.get_mut(hero).ok_or("hero has no equipped abilities")?;
        let pos = hero_abilities.iter().position(|a| a == ability).ok_or("ability not equipped on hero")?;
        hero_abilities.remove(pos);
        self.gold -= UNEQUIP_COST;
        Ok(())
    }

    /// Reroll the shop offerings.
    pub fn reroll_shop(
        &mut self,
        pool: &AbilityPool,
        ultimates: &HashSet<String>,
        ultimate_unlock_level: u32,
        size_bonus: u32,
        reroll_cost: u32,
        rng: &mut impl rand::Rng,
    ) -> Result<(), &'static str> {
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
    fn test_buy_ability() {
        let mut p = PlayerState::new(0);
        p.gold = 10;
        p.shop.offerings = vec!["fireball".to_string(), "heal".to_string()];
        let mut counts = HashMap::new();
        counts.insert("fireball".to_string(), 5);
        counts.insert("heal".to_string(), 5);
        let mut pool = AbilityPool::from_counts(counts);

        assert!(p.buy_ability(0, &mut pool).is_ok());
        assert_eq!(p.gold, 7);
        assert_eq!(p.abilities["fireball"], 1);
        assert_eq!(pool.counts["fireball"], 4);
        assert_eq!(p.shop.offerings.len(), 1);
    }

    #[test]
    fn test_sell_ability() {
        let mut p = PlayerState::new(0);
        p.gold = 0;
        p.abilities.insert("fireball".to_string(), 2);
        let mut counts = HashMap::new();
        counts.insert("fireball".to_string(), 3);
        let mut pool = AbilityPool::from_counts(counts);

        assert!(p.sell_ability("fireball", &mut pool).is_ok());
        assert_eq!(p.gold, SELL_REFUND);
        assert_eq!(p.abilities["fireball"], 1);
        assert_eq!(pool.counts["fireball"], 4);
    }

    #[test]
    fn test_equip_unequip() {
        let mut p = PlayerState::new(0);
        p.gold = 5;
        p.heroes.push("axe".to_string());
        p.abilities.insert("fireball".to_string(), 1);
        let ultimates = HashSet::new();

        assert!(p.equip("axe", "fireball", 4, &ultimates).is_ok());
        assert_eq!(p.equipped["axe"], vec!["fireball".to_string()]);

        assert!(p.unequip("axe", "fireball").is_ok());
        assert!(p.equipped["axe"].is_empty());
        assert_eq!(p.gold, 4);
    }
}
