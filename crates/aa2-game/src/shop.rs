//! Shop state, levels, offerings, and upgrade mechanics.

use std::collections::HashSet;

use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::economy::{self, SHOP_SIZES};
use crate::pool::AbilityPool;

/// Shop state for a single player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopState {
    /// Current shop level (1-5).
    pub level: u32,
    /// Current ability offerings in the shop.
    pub offerings: Vec<String>,
    /// Rounds spent at each level without upgrading. Index 0 = level 1.
    pub decay_tracker: [u32; 5],
}

impl ShopState {
    /// Create a new shop at level 1 with no offerings.
    pub fn new() -> Self {
        Self {
            level: 1,
            offerings: Vec::new(),
            decay_tracker: [0; 5],
        }
    }

    /// Get the shop size for the current level, plus any bonus.
    pub fn size(&self, bonus: u32) -> u32 {
        let base = SHOP_SIZES.get(self.level.saturating_sub(1) as usize).copied().unwrap_or(4);
        base + bonus
    }

    /// Roll new shop offerings from the pool.
    pub fn roll(
        &mut self,
        pool: &AbilityPool,
        ultimates: &HashSet<String>,
        ultimate_unlock_level: u32,
        size_bonus: u32,
        rng: &mut impl rand::Rng,
    ) {
        let size = self.size(size_bonus) as usize;
        let available = pool.available_for_shop(self.level, ultimates, ultimate_unlock_level);
        let mut available_ref: Vec<&String> = available.iter().collect();
        available_ref.shuffle(rng);
        self.offerings = available_ref.into_iter().take(size).cloned().collect();
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
    use super::*;

    #[test]
    fn test_shop_sizes() {
        let shop = ShopState::new();
        assert_eq!(shop.size(0), 4); // level 1
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
        // Level 1→2 costs 10
        let cost = shop.upgrade(&mut gold);
        assert_eq!(cost, Some(10));
        assert_eq!(shop.level, 2);
        assert_eq!(gold, 10);
    }

    #[test]
    fn test_upgrade_with_decay() {
        let mut shop = ShopState::new();
        shop.tick_decay(); // 1 round at level 1
        shop.tick_decay(); // 2 rounds at level 1
        // Base cost 10 - 2 decay = 8
        assert_eq!(shop.upgrade_cost(), Some(8));
    }

    #[test]
    fn test_upgrade_max_level() {
        let mut shop = ShopState::new();
        shop.level = 5;
        assert_eq!(shop.upgrade_cost(), None);
    }
}
