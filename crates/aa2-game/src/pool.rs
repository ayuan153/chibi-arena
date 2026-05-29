//! Shared ability pool with depletion tracking.

use std::collections::{HashMap, HashSet};

use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

/// Shared pool of abilities available across all players.
/// Tracks remaining copies of each ability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbilityPool {
    /// Remaining copies per ability name.
    pub counts: HashMap<String, u32>,
}

impl AbilityPool {
    /// Initialize the pool by selecting `count` abilities from the full roster,
    /// each with `copies_each` copies.
    pub fn new(roster: &[String], count: usize, copies_each: u32, rng: &mut impl rand::Rng) -> Self {
        let mut available: Vec<&String> = roster.iter().collect();
        available.shuffle(rng);
        let selected = available.into_iter().take(count);
        let counts = selected.map(|name| (name.clone(), copies_each)).collect();
        Self { counts }
    }

    /// Create a pool from an explicit map (for testing or custom setups).
    pub fn from_counts(counts: HashMap<String, u32>) -> Self {
        Self { counts }
    }

    /// Take one copy of an ability from the pool. Returns false if depleted or not in pool.
    pub fn take(&mut self, name: &str) -> bool {
        if let Some(count) = self.counts.get_mut(name)
            && *count > 0
        {
            *count -= 1;
            return true;
        }
        false
    }

    /// Return one copy of an ability to the pool.
    pub fn return_ability(&mut self, name: &str) {
        if let Some(count) = self.counts.get_mut(name) {
            *count += 1;
        }
    }

    /// Return multiple copies of an ability to the pool.
    pub fn return_copies(&mut self, name: &str, copies: u32) {
        if let Some(count) = self.counts.get_mut(name) {
            *count += copies;
        }
    }

    /// Get abilities available for the shop at a given level.
    /// Filters out depleted abilities and ultimates if shop level < ultimate_unlock_level.
    pub fn available_for_shop(
        &self,
        shop_level: u32,
        ultimates: &HashSet<String>,
        ultimate_unlock_level: u32,
    ) -> Vec<String> {
        let mut available: Vec<String> = self
            .counts
            .iter()
            .filter(|(_, count)| **count > 0)
            .filter(|(name, _)| {
                if ultimates.contains(name.as_str()) {
                    shop_level >= ultimate_unlock_level
                } else {
                    true
                }
            })
            .map(|(name, _)| name.clone())
            .collect();
        // Sort for deterministic order: counts is a HashMap (random iteration order),
        // and the shop roll shuffles this list with a seeded RNG, so a stable input
        // order is required for reproducible offerings under a fixed seed.
        available.sort_unstable();
        available
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_init() {
        let roster: Vec<String> = (0..200).map(|i| format!("ability_{i}")).collect();
        let mut rng = rand::thread_rng();
        let pool = AbilityPool::new(&roster, 100, 20, &mut rng);
        assert_eq!(pool.counts.len(), 100);
        assert!(pool.counts.values().all(|&c| c == 20));
    }

    #[test]
    fn test_take_and_return() {
        let mut counts = HashMap::new();
        counts.insert("fireball".to_string(), 2);
        let mut pool = AbilityPool::from_counts(counts);

        assert!(pool.take("fireball"));
        assert_eq!(pool.counts["fireball"], 1);
        assert!(pool.take("fireball"));
        assert_eq!(pool.counts["fireball"], 0);
        assert!(!pool.take("fireball")); // depleted

        pool.return_ability("fireball");
        assert_eq!(pool.counts["fireball"], 1);
    }

    #[test]
    fn test_take_nonexistent() {
        let pool_map = HashMap::new();
        let mut pool = AbilityPool::from_counts(pool_map);
        assert!(!pool.take("nonexistent"));
    }

    #[test]
    fn test_available_for_shop() {
        let mut counts = HashMap::new();
        counts.insert("basic".to_string(), 5);
        counts.insert("ult".to_string(), 3);
        counts.insert("depleted".to_string(), 0);
        let pool = AbilityPool::from_counts(counts);

        let mut ultimates = HashSet::new();
        ultimates.insert("ult".to_string());

        // Level 2: no ultimates
        let avail = pool.available_for_shop(2, &ultimates, 3);
        assert!(avail.contains(&"basic".to_string()));
        assert!(!avail.contains(&"ult".to_string()));
        assert!(!avail.contains(&"depleted".to_string()));

        // Level 3: ultimates unlocked
        let avail = pool.available_for_shop(3, &ultimates, 3);
        assert!(avail.contains(&"basic".to_string()));
        assert!(avail.contains(&"ult".to_string()));
    }
}
