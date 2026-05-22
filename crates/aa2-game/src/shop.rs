//! Shop state, levels, offerings, and upgrade mechanics.

use std::collections::HashSet;

use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::economy::{self, SHOP_SIZES};
use crate::game::GameConfig;
use crate::pool::AbilityPool;

/// Shop state for a single player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopState {
    /// Current shop level (1-5).
    pub level: u32,
    /// Current ability offerings in the shop. None = sold/empty slot.
    pub offerings: Vec<Option<String>>,
    /// Rounds spent at each level without upgrading. Index 0 = level 1.
    pub decay_tracker: [u32; 5],
    /// Whether the shop is locked (won't auto-reroll at combat end).
    pub locked: bool,
    /// Whether this shop needs a reroll (set by game state, consumed by roll_shop).
    pub needs_reroll: bool,
}

impl ShopState {
    /// Create a new shop at level 1 with no offerings.
    pub fn new() -> Self {
        Self {
            level: 1,
            offerings: Vec::new(),
            decay_tracker: [0; 5],
            locked: false,
            needs_reroll: false,
        }
    }

    /// Toggle shop lock. Free action.
    pub fn toggle_lock(&mut self) {
        self.locked = !self.locked;
    }

    /// Get the shop size for the current level, plus any bonus.
    pub fn size(&self, bonus: u32) -> u32 {
        let base = SHOP_SIZES.get(self.level.saturating_sub(1) as usize).copied().unwrap_or(4);
        base + bonus
    }

    /// Roll new shop offerings from the pool.
    /// Returns unbought offerings back to pool, then samples new ones.
    pub fn roll_shop(
        &mut self,
        pool: &mut AbilityPool,
        config: &GameConfig,
        rng: &mut impl rand::Rng,
    ) {
        // Return current offerings to pool (only Some values)
        for name in self.offerings.drain(..).flatten() {
            pool.return_ability(&name);
        }
        let size = self.size(config.shop_size_bonus) as usize;
        let ultimates = HashSet::new();
        let available = pool.available_for_shop(self.level, &ultimates, config.ultimate_unlock_level);
        let mut available_vec = available;
        available_vec.shuffle(rng);
        self.offerings = available_vec.into_iter().take(size).map(Some).collect();
        // Take each from pool
        for name in self.offerings.iter().flatten() {
            pool.take(name);
        }
    }

    /// Roll new shop offerings with an explicit ultimates set.
    pub fn roll(
        &mut self,
        pool: &mut AbilityPool,
        ultimates: &HashSet<String>,
        ultimate_unlock_level: u32,
        size_bonus: u32,
        rng: &mut impl rand::Rng,
    ) {
        // Return current offerings to pool (only Some values)
        for name in self.offerings.drain(..).flatten() {
            pool.return_ability(&name);
        }
        let size = self.size(size_bonus) as usize;
        let available = pool.available_for_shop(self.level, ultimates, ultimate_unlock_level);
        let mut available_vec = available;
        available_vec.shuffle(rng);
        self.offerings = available_vec.into_iter().take(size).map(Some).collect();
        // Take each from pool
        for name in self.offerings.iter().flatten() {
            pool.take(name);
        }
    }

    /// Buy an ability from the shop at the given index.
    /// Returns the ability name. Does NOT return to pool (it's purchased).
    pub fn buy_from_shop(&mut self, index: usize) -> Option<String> {
        if index < self.offerings.len() {
            self.offerings[index].take()
        } else {
            None
        }
    }

    /// Get the current upgrade cost (with decay applied). Returns None if already max level.
    pub fn upgrade_cost(&self) -> Option<u32> {
        if self.level >= 5 {
            return None;
        }
        let rounds_at_level = self.decay_tracker[self.level.saturating_sub(1) as usize];
        economy::upgrade_cost(self.level, rounds_at_level)
    }

    /// Increment decay trackers for all levels at or below current.
    pub fn tick_decay(&mut self) {
        for i in 0..self.level as usize {
            if i < self.decay_tracker.len() {
                self.decay_tracker[i] += 1;
            }
        }
    }

    /// Upgrade the shop level. Returns the cost paid, or None if can't upgrade.
    pub fn upgrade(&mut self, gold: &mut u32) -> Option<u32> {
        let cost = self.upgrade_cost()?;
        if *gold < cost {
            return None;
        }
        *gold -= cost;
        self.level += 1;
        Some(cost)
    }
}

impl Default for ShopState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_shop_sizes() {
        let shop = ShopState::new();
        assert_eq!(shop.size(0), 4);
        let mut shop2 = shop.clone();
        shop2.level = 2;
        assert_eq!(shop2.size(0), 6);
        shop2.level = 3;
        assert_eq!(shop2.size(0), 6);
        shop2.level = 4;
        assert_eq!(shop2.size(0), 8);
        shop2.level = 5;
        assert_eq!(shop2.size(0), 10);
    }

    #[test]
    fn test_shop_size_bonus() {
        let mut shop = ShopState::new();
        shop.level = 1;
        assert_eq!(shop.size(2), 6);
    }

    #[test]
    fn test_upgrade_mechanics() {
        let mut shop = ShopState::new();
        let mut gold = 20;
        let cost = shop.upgrade(&mut gold);
        assert_eq!(cost, Some(10));
        assert_eq!(shop.level, 2);
        assert_eq!(gold, 10);
    }

    #[test]
    fn test_upgrade_with_decay() {
        let mut shop = ShopState::new();
        shop.tick_decay();
        shop.tick_decay();
        assert_eq!(shop.upgrade_cost(), Some(8));
    }

    #[test]
    fn test_upgrade_max_level() {
        let mut shop = ShopState::new();
        shop.level = 5;
        assert_eq!(shop.upgrade_cost(), None);
    }

    #[test]
    fn test_roll_shop_correct_count() {
        let mut counts = HashMap::new();
        for i in 0..20 {
            counts.insert(format!("ability_{i}"), 5);
        }
        let mut pool = AbilityPool::from_counts(counts);
        let mut shop = ShopState::new(); // level 1 = size 4
        let config = GameConfig::default();
        let mut rng = rand::thread_rng();

        shop.roll_shop(&mut pool, &config, &mut rng);
        assert_eq!(shop.offerings.len(), 4);
    }

    #[test]
    fn test_roll_shop_distinct_offerings() {
        let mut counts = HashMap::new();
        for i in 0..20 {
            counts.insert(format!("ability_{i}"), 5);
        }
        let mut pool = AbilityPool::from_counts(counts);
        let mut shop = ShopState::new();
        let config = GameConfig::default();
        let mut rng = rand::thread_rng();

        shop.roll_shop(&mut pool, &config, &mut rng);
        let set: HashSet<&String> = shop.offerings.iter().filter_map(|o| o.as_ref()).collect();
        assert_eq!(set.len(), shop.offerings.len());
    }

    #[test]
    fn test_roll_shop_returns_unbought() {
        let mut counts = HashMap::new();
        for i in 0..20 {
            counts.insert(format!("ability_{i}"), 5);
        }
        let mut pool = AbilityPool::from_counts(counts);
        let mut shop = ShopState::new();
        let config = GameConfig::default();
        let mut rng = rand::thread_rng();

        shop.roll_shop(&mut pool, &config, &mut rng);
        let first_offerings: Vec<String> = shop.offerings.iter().filter_map(|o| o.clone()).collect();
        // Total pool count should be reduced by 4 (offerings taken)
        let total: u32 = pool.counts.values().sum();
        assert_eq!(total, 20 * 5 - 4);

        // Reroll: old offerings returned, new ones taken
        shop.roll_shop(&mut pool, &config, &mut rng);
        let total_after: u32 = pool.counts.values().sum();
        assert_eq!(total_after, 20 * 5 - 4); // same net reduction
        // First offerings should have been returned
        for name in &first_offerings {
            if !shop.offerings.contains(&Some(name.clone())) {
                assert!(pool.counts[name] >= 5); // returned
            }
        }
    }

    #[test]
    fn test_buy_from_shop() {
        let mut shop = ShopState::new();
        shop.offerings = vec![Some("a".to_string()), Some("b".to_string()), Some("c".to_string())];

        let bought = shop.buy_from_shop(1);
        assert_eq!(bought, Some("b".to_string()));
        assert_eq!(shop.offerings, vec![Some("a".to_string()), None, Some("c".to_string())]);
        assert_eq!(shop.buy_from_shop(1), None); // already sold
        assert_eq!(shop.buy_from_shop(10), None);
    }

    #[test]
    fn test_locked_initialized_false() {
        let shop = ShopState::new();
        assert!(!shop.locked);
        assert!(!shop.needs_reroll);
    }

    #[test]
    fn test_toggle_lock() {
        let mut shop = ShopState::new();
        assert!(!shop.locked);
        shop.toggle_lock();
        assert!(shop.locked);
        shop.toggle_lock();
        assert!(!shop.locked);
    }

    #[test]
    fn test_ultimates_filtered_by_level() {
        let mut counts = HashMap::new();
        counts.insert("basic".to_string(), 5);
        counts.insert("ult".to_string(), 5);
        let mut pool = AbilityPool::from_counts(counts);
        let mut ultimates = HashSet::new();
        ultimates.insert("ult".to_string());

        let mut shop = ShopState::new();
        shop.level = 2; // below ultimate_unlock_level=3
        let mut rng = rand::thread_rng();

        shop.roll(&mut pool, &ultimates, 3, 0, &mut rng);
        assert!(!shop.offerings.contains(&Some("ult".to_string())));

        // Return offerings and try at level 3
        for slot in shop.offerings.drain(..) {
            if let Some(name) = slot {
                pool.return_ability(&name);
            }
        }
        shop.level = 3;
        // Roll many times to check ult can appear
        let mut found_ult = false;
        for _ in 0..20 {
            shop.roll(&mut pool, &ultimates, 3, 0, &mut rng);
            if shop.offerings.contains(&Some("ult".to_string())) {
                found_ult = true;
                break;
            }
        }
        assert!(found_ult);
    }
}
