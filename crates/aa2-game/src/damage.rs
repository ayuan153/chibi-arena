//! Player damage calculation after combat rounds.

/// Starting HP for all players.
pub const STARTING_HP: f32 = 200.0;

/// Calculate damage dealt to the losing player after combat.
/// Formula: round * 0.5 + (1.0 + round * 0.1) * surviving_heroes
pub fn calculate_damage(round: u32, surviving_heroes: u32) -> f32 {
    round as f32 * 0.5 + (1.0 + round as f32 * 0.1) * surviving_heroes as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_damage_formula() {
        // Round 1, 1 survivor: 0.5 + 1.1*1 = 1.6
        let d = calculate_damage(1, 1);
        assert!((d - 1.6).abs() < 0.001);

        // Round 1, 3 survivors: 0.5 + 1.1*3 = 3.8
        let d = calculate_damage(1, 3);
        assert!((d - 3.8).abs() < 0.001);

        // Round 5, 2 survivors: 2.5 + 1.5*2 = 5.5
        let d = calculate_damage(5, 2);
        assert!((d - 5.5).abs() < 0.001);

        // Round 10, 4 survivors: 5.0 + 2.0*4 = 13.0
        let d = calculate_damage(10, 4);
        assert!((d - 13.0).abs() < 0.001);

        // Round 0, 0 survivors: 0
        assert_eq!(calculate_damage(0, 0), 0.0);
    }
}
