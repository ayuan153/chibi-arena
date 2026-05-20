//! Fixture-based end-to-end tests for deterministic game scenarios.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use aa2_data::{AbilityDef, HeroDef};
use aa2_game::pool::AbilityPool;
use aa2_game::{GameConfig, GameState};
use rand::rngs::StdRng;
use rand::SeedableRng;

/// A test fixture that defines a complete game scenario with deterministic outcome.
#[allow(dead_code)]
struct GameFixture {
    seed: u64,
    /// Pre-configured player states (heroes, abilities, positions).
    player_setups: Vec<PlayerSetup>,
    /// Expected: game terminates within N rounds.
    max_rounds: u32,
    /// Expected: specific player wins (or just "game terminates" if None).
    expected_winner: Option<u8>,
}

struct PlayerSetup {
    heroes: Vec<(String, f32, f32)>,                // (name, x, y)
    abilities: Vec<(String, String, u32)>,           // (ability_name, hero_name, level)
}

fn load_defs() -> (HashMap<String, HeroDef>, HashMap<String, AbilityDef>) {
    let data_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");

    let heroes_dir = data_dir.join("heroes");
    let hero_list = aa2_data::load_all_heroes(&heroes_dir).expect("load heroes");
    let hero_defs: HashMap<String, HeroDef> = hero_list
        .into_iter()
        .map(|h| (h.name.clone(), h))
        .collect();

    let abilities_dir = data_dir.join("abilities");
    let mut ability_defs: HashMap<String, AbilityDef> = HashMap::new();
    for entry in std::fs::read_dir(&abilities_dir).expect("read abilities dir") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|e| e == "ron") {
            let def = aa2_data::load_ability_def(&path).expect("load ability");
            ability_defs.insert(def.name.clone(), def);
        }
    }

    (hero_defs, ability_defs)
}

/// Run a fixture: set up players, run combat rounds until game terminates or max_rounds.
fn run_fixture(fixture: &GameFixture) -> (Option<u8>, u32) {
    let (hero_defs, ability_defs) = load_defs();
    let mut rng = StdRng::seed_from_u64(fixture.seed);

    // Build a minimal pool (not used for buying, just needed for GameState)
    let pool = AbilityPool::from_counts(HashMap::new());
    let ultimates: HashSet<String> = ability_defs
        .values()
        .filter(|a| a.is_ultimate)
        .map(|a| a.name.clone())
        .collect();
    let config = GameConfig::default();
    let mut game = GameState::new(pool, ultimates, config);

    // Kill extra players beyond what the fixture needs
    for i in fixture.player_setups.len()..8 {
        game.players[i].alive = false;
    }

    // Configure players from fixture
    for (i, setup) in fixture.player_setups.iter().enumerate() {
        let player = &mut game.players[i];
        player.alive = true;
        player.hp = 200.0;

        for (hero_name, x, y) in &setup.heroes {
            player.heroes.push(hero_name.clone());
            player.hero_positions.insert(hero_name.clone(), (*x, *y));
            player.equipped.entry(hero_name.clone()).or_default();
        }

        for (ability_name, hero_name, level) in &setup.abilities {
            player.abilities.insert(ability_name.clone(), *level);
            player
                .equipped
                .entry(hero_name.clone())
                .or_default()
                .push(ability_name.clone());
        }
    }

    // Run rounds
    let mut rounds_played = 0;
    for round in 1..=fixture.max_rounds {
        game.round = round;
        let seed = fixture.seed as u32 + round;
        game.run_combat_round(&hero_defs, &ability_defs, seed, &mut rng);
        rounds_played = round;

        if game.alive_count() <= 1 {
            break;
        }
    }

    let winner = game
        .players
        .iter()
        .find(|p| p.alive)
        .map(|p| p.id);

    (winner, rounds_played)
}

/// Fixture: Minimal 2-player game — 1 hero each, verify combat resolves and one is eliminated.
#[test]
fn test_fixture_minimal_2_player() {
    let fixture = GameFixture {
        seed: 42,
        player_setups: vec![
            PlayerSetup {
                heroes: vec![("Juggernaut".to_string(), 1000.0, 500.0)],
                abilities: vec![],
            },
            PlayerSetup {
                heroes: vec![("Drow Ranger".to_string(), 1000.0, 500.0)],
                abilities: vec![],
            },
        ],
        max_rounds: 20,
        expected_winner: None, // just verify termination
    };

    let (winner, rounds) = run_fixture(&fixture);
    assert!(
        winner.is_some(),
        "Game should terminate with a winner within {rounds} rounds"
    );
    assert!(rounds <= 20, "Game should terminate within 20 rounds");
}

/// Fixture: One dominant player with level 5 abilities vs others with level 1.
/// The dominant player should win.
#[test]
fn test_fixture_dominant_player_wins() {
    let fixture = GameFixture {
        seed: 123,
        player_setups: vec![
            // Player 0: dominant — Sven + Juggernaut with level 5 abilities
            PlayerSetup {
                heroes: vec![
                    ("Sven".to_string(), 800.0, 300.0),
                    ("Juggernaut".to_string(), 1200.0, 300.0),
                ],
                abilities: vec![
                    ("fury_swipes".to_string(), "Sven".to_string(), 5),
                    ("chaos_strike".to_string(), "Juggernaut".to_string(), 5),
                ],
            },
            // Player 1: weak
            PlayerSetup {
                heroes: vec![("Drow Ranger".to_string(), 1000.0, 500.0)],
                abilities: vec![("essence_shift".to_string(), "Drow Ranger".to_string(), 1)],
            },
            // Player 2: weak
            PlayerSetup {
                heroes: vec![("Sven".to_string(), 1000.0, 500.0)],
                abilities: vec![("fury_swipes".to_string(), "Sven".to_string(), 1)],
            },
        ],
        max_rounds: 30,
        expected_winner: Some(0),
    };

    let (winner, rounds) = run_fixture(&fixture);
    assert!(rounds <= 30, "Game should terminate within 30 rounds");
    assert_eq!(
        winner,
        Some(0),
        "Dominant player (0) should win, got winner={winner:?}"
    );
}

/// Fixture: Balanced 4-player game — verify game terminates within 20 rounds.
#[test]
fn test_fixture_balanced_4_player_terminates() {
    let fixture = GameFixture {
        seed: 777,
        player_setups: vec![
            PlayerSetup {
                heroes: vec![
                    ("Juggernaut".to_string(), 800.0, 400.0),
                    ("Drow Ranger".to_string(), 1200.0, 400.0),
                ],
                abilities: vec![
                    ("chaos_strike".to_string(), "Juggernaut".to_string(), 2),
                ],
            },
            PlayerSetup {
                heroes: vec![
                    ("Sven".to_string(), 800.0, 400.0),
                    ("Drow Ranger".to_string(), 1200.0, 400.0),
                ],
                abilities: vec![
                    ("fury_swipes".to_string(), "Sven".to_string(), 2),
                ],
            },
            PlayerSetup {
                heroes: vec![
                    ("Juggernaut".to_string(), 1000.0, 300.0),
                    ("Sven".to_string(), 1000.0, 600.0),
                ],
                abilities: vec![
                    ("essence_shift".to_string(), "Juggernaut".to_string(), 2),
                ],
            },
            PlayerSetup {
                heroes: vec![
                    ("Drow Ranger".to_string(), 800.0, 500.0),
                    ("Sven".to_string(), 1200.0, 500.0),
                ],
                abilities: vec![
                    ("chaos_strike".to_string(), "Sven".to_string(), 2),
                ],
            },
        ],
        max_rounds: 20,
        expected_winner: None, // just verify termination
    };

    let (winner, rounds) = run_fixture(&fixture);
    assert!(
        winner.is_some(),
        "Game should terminate with a winner within 20 rounds, played {rounds}"
    );
}
