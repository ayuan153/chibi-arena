//! Gold formulas and cost constants for the AA2 economy.

/// Cost to buy an ability from the shop.
pub const BUY_COST: u32 = 3;
/// Gold refunded when selling an ability.
pub const SELL_REFUND: u32 = 2;
/// Cost to reroll the shop offerings.
pub const REROLL_COST: u32 = 1;
/// Cost to unequip an ability from a hero.
pub const UNEQUIP_COST: u32 = 1;
/// Cost to reroll hero draft choices.
pub const HERO_REROLL_COST: u32 = 2;

/// Base costs to upgrade shop from level N to N+1. Index 0 = L1→L2.
pub const UPGRADE_BASE_COSTS: [u32; 4] = [10, 14, 17, 20];

/// Shop sizes per level. Index 0 = level 1.
pub const SHOP_SIZES: [u32; 5] = [4, 6, 6, 8, 10];

/// Calculate gold income for a given round.
/// Formula: min(6 + 2*(round-1), 20)
pub fn gold_for_round(round: u32) -> u32 {
    (6 + 2 * round.saturating_sub(1)).min(20)
}

/// Calculate the upgrade cost for a given level transition, accounting for decay.
/// `level` is the current shop level (1-4 can upgrade).
/// `rounds_at_level` is how many rounds spent without upgrading at this level.
pub fn upgrade_cost(level: u32, rounds_at_level: u32) -> Option<u32> {
    let idx = level.checked_sub(1)? as usize;
    let base = *UPGRADE_BASE_COSTS.get(idx)?;
    Some(base.saturating_sub(rounds_at_level))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gold_for_round() {
        assert_eq!(gold_for_round(1), 6);
        assert_eq!(gold_for_round(2), 8);
        assert_eq!(gold_for_round(3), 10);
        assert_eq!(gold_for_round(8), 20);
        assert_eq!(gold_for_round(10), 20);
        assert_eq!(gold_for_round(20), 20);
    }

    #[test]
    fn test_upgrade_cost_decay() {
        // Level 1→2 base cost is 10
        assert_eq!(upgrade_cost(1, 0), Some(10));
        assert_eq!(upgrade_cost(1, 3), Some(7));
        assert_eq!(upgrade_cost(1, 10), Some(0));
        // Level 4→5 base cost is 20
        assert_eq!(upgrade_cost(4, 0), Some(20));
        assert_eq!(upgrade_cost(4, 5), Some(15));
        // Invalid level
        assert_eq!(upgrade_cost(5, 0), None);
        assert_eq!(upgrade_cost(0, 0), None);
    }
}
