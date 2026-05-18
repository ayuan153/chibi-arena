//! Matchup pairing using round-robin with ghost seat.

use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Represents a matchup for one round.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Matchup {
    /// First player (always a real player).
    pub player_a: u8,
    /// Second player (real or ghost).
    pub player_b: u8,
    /// If true, player_b is a ghost (clone of another player's loadout).
    pub ghost: bool,
    /// If ghost, which real player's loadout to clone.
    pub ghost_source: Option<u8>,
}

/// Generate matchups for a round using round-robin with ghost seat.
///
/// Uses the circle method: fix rotation[0], rotate the rest.
/// `rotation` is the current shuffled order of alive players (+ ghost seat if odd).
/// `cycle_round` is which round within the current cycle (0-indexed).
pub fn generate_matchups(
    alive_players: &[u8],
    rotation: &[u8],
    cycle_round: usize,
    rng: &mut impl Rng,
) -> Vec<Matchup> {
    let n = alive_players.len();
    if n < 2 {
        return Vec::new();
    }

    // Build the rotated order for this round using circle method
    let len = rotation.len(); // even (ghost seat added if odd)
    let mut order: Vec<u8> = Vec::with_capacity(len);
    order.push(rotation[0]); // fixed pivot
    // Rotate the rest by cycle_round positions
    for i in 1..len {
        let idx = 1 + (i - 1 + cycle_round) % (len - 1);
        order.push(rotation[idx]);
    }

    // Pair: first with last, second with second-to-last, etc.
    let half = len / 2;
    let mut matchups = Vec::with_capacity(half);
    let ghost_seat = u8::MAX; // sentinel for ghost

    for i in 0..half {
        let a = order[i];
        let b = order[len - 1 - i];

        if a == ghost_seat {
            // Player b fights a ghost
            let source = pick_ghost_source(b, alive_players, rng);
            matchups.push(Matchup {
                player_a: b,
                player_b: b, // placeholder
                ghost: true,
                ghost_source: Some(source),
            });
        } else if b == ghost_seat {
            // Player a fights a ghost
            let source = pick_ghost_source(a, alive_players, rng);
            matchups.push(Matchup {
                player_a: a,
                player_b: a, // placeholder
                ghost: true,
                ghost_source: Some(source),
            });
        } else {
            matchups.push(Matchup {
                player_a: a,
                player_b: b,
                ghost: false,
                ghost_source: None,
            });
        }
    }

    matchups
}

/// Create a new shuffled rotation for a cycle.
/// If odd number of alive players, adds a ghost seat (u8::MAX).
pub fn new_rotation(alive_players: &[u8], rng: &mut impl Rng) -> Vec<u8> {
    let mut rotation: Vec<u8> = alive_players.to_vec();
    rotation.shuffle(rng);
    if !rotation.len().is_multiple_of(2) {
        rotation.push(u8::MAX); // ghost seat
    }
    rotation
}

/// Number of rounds in a full cycle for the given rotation.
pub fn cycle_length(rotation: &[u8]) -> usize {
    if rotation.len() <= 1 {
        1
    } else {
        rotation.len() - 1
    }
}

/// Pick a ghost source player (someone other than the fighter).
fn pick_ghost_source(fighter: u8, alive_players: &[u8], rng: &mut impl Rng) -> u8 {
    let candidates: Vec<u8> = alive_players.iter().copied().filter(|&p| p != fighter).collect();
    if candidates.is_empty() {
        return fighter; // fallback: shouldn't happen with 2+ players
    }
    candidates[rng.gen_range(0..candidates.len())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_rotation_even() {
        let mut rng = rand::thread_rng();
        let rotation = new_rotation(&[0, 1, 2, 3], &mut rng);
        assert_eq!(rotation.len(), 4);
    }

    #[test]
    fn test_new_rotation_odd_adds_ghost() {
        let mut rng = rand::thread_rng();
        let rotation = new_rotation(&[0, 1, 2], &mut rng);
        assert_eq!(rotation.len(), 4);
        assert!(rotation.contains(&u8::MAX));
    }

    #[test]
    fn test_generate_matchups_4_players() {
        let mut rng = rand::thread_rng();
        let alive = [0, 1, 2, 3];
        let rotation = new_rotation(&alive, &mut rng);
        let matchups = generate_matchups(&alive, &rotation, 0, &mut rng);
        assert_eq!(matchups.len(), 2);
        // All matchups should be PvP (no ghost with even players)
        for m in &matchups {
            assert!(!m.ghost);
        }
    }

    #[test]
    fn test_generate_matchups_3_players_has_ghost() {
        let mut rng = rand::thread_rng();
        let alive = [0, 1, 2];
        let rotation = new_rotation(&alive, &mut rng);
        let matchups = generate_matchups(&alive, &rotation, 0, &mut rng);
        assert_eq!(matchups.len(), 2);
        let ghost_count = matchups.iter().filter(|m| m.ghost).count();
        assert_eq!(ghost_count, 1);
    }

    #[test]
    fn test_cycle_length() {
        assert_eq!(cycle_length(&[0, 1, 2, 3]), 3);
        assert_eq!(cycle_length(&[0, 1, 2, 3, 4, u8::MAX]), 5);
    }
}
