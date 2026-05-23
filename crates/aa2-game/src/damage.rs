//! Player damage calculation after combat rounds.

/// Starting HP for all players.
pub const STARTING_HP: f32 = 200.0;

/// Calculate player damage after losing a combat round.
/// Formula: 5.5 * survivors + 0.05 * round * survivors + 0.5 * round
/// Calibrated: round 1, 1 survivor = 6 damage. Round 30, 5 survivors = 50 damage.
pub fn calculate_damage(round: u32, surviving_heroes: u32) -> f32 {
    let r = round as f32;
    let s = surviving_heroes as f32;
    5.5 * s + 0.05 * r * s + 0.5 * r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_damage_formula() {
        // Round 1, 1 survivor: 5.5 + 0.05 + 0.5 = 6.05
        let d = calculate_damage(1, 1);
        assert!((d - 6.05).abs() < 0.001);

        // Round 30, 5 survivors: 27.5 + 7.5 + 15.0 = 50.0
        let d = calculate_damage(30, 5);
        assert!((d - 50.0).abs() < 0.001);

        // Round 1, 0 survivors: just base round damage = 0.5
        let d = calculate_damage(1, 0);
        assert!((d - 0.5).abs() < 0.001);

        // Round 0, 0 survivors: 0
        assert_eq!(calculate_damage(0, 0), 0.0);
    }
}
