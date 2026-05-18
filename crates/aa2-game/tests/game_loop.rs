//! Integration test: full game loop from start to finish.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use aa2_data::{AbilityDef, HeroDef};
use aa2_game::{AbilityPool, GameConfig, GameState};
use rand::rngs::StdRng;
use rand::SeedableRng;

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

#[test]
fn test_full_game_loop_terminates() {
    let (hero_defs, ability_defs) = load_defs();

    // Pick some heroes to distribute
    let hero_names: Vec<&str> = hero_defs.keys().map(|s| s.as_str()).take(5).collect();
    let ability_names: Vec<&str> = ability_defs.keys().map(|s| s.as_str()).take(4).collect();

    let pool_counts: HashMap<String, u32> = ability_names
        .iter()
        .map(|&n| (n.to_string(), 20))
        .collect();
    let pool = AbilityPool::from_counts(pool_counts);
    let mut game = GameState::new(pool, HashSet::new(), GameConfig::default());

    // Give each player 2-3 heroes and some abilities
    let mut rng = StdRng::seed_from_u64(12345);
    for (i, player) in game.players.iter_mut().enumerate() {
        let num_heroes = 2 + (i % 2); // 2 or 3
        for j in 0..num_heroes {
            let hero = hero_names[(i + j) % hero_names.len()].to_string();
            if !player.heroes.contains(&hero) {
                // Assign default position spread evenly
                let x = 400.0 + (j as f32) * 600.0;
                let y = 500.0;
                player.hero_positions.insert(hero.clone(), (x, y));
                player.heroes.push(hero.clone());
                player.equipped.entry(hero).or_default();
            }
        }
        // Give each player some abilities equipped on first hero
        if !player.heroes.is_empty() && !ability_names.is_empty() {
            let hero = player.heroes[0].clone();
            let ability = ability_names[i % ability_names.len()].to_string();
            player.abilities.insert(ability.clone(), 2);
            player.equipped.entry(hero).or_default().push(ability);
        }
    }

    // Run game loop
    game.start_round1();
    game.end_shop();

    let mut rounds = 0;
    let max_rounds = 100;

    loop {
        rounds += 1;
        if rounds > max_rounds {
            panic!("Game did not terminate within {max_rounds} rounds");
        }

        // Run combat
        let seed = 42 + rounds as u32;
        let _results = game.run_combat_round(&hero_defs, &ability_defs, seed, &mut rng);

        if game.alive_count() <= 1 {
            break;
        }

        // Transition through phases
        game.end_combat(false);
        game.end_grace_period();
        game.end_shop();
    }

    assert!(rounds > 1, "Game should last more than 1 round");
    assert!(game.alive_count() <= 1, "At most one player should survive");
    assert!(game.players.iter().filter(|p| !p.alive).count() >= 7);
}

#[test]
fn test_rounds_advance_and_players_eliminated() {
    let (hero_defs, ability_defs) = load_defs();

    let hero_names: Vec<&str> = hero_defs.keys().map(|s| s.as_str()).take(3).collect();

    let pool = AbilityPool::from_counts(HashMap::new());
    let mut game = GameState::new(pool, HashSet::new(), GameConfig::default());
    let mut rng = StdRng::seed_from_u64(99999);

    // Give each player 1 hero (minimal setup)
    for (i, player) in game.players.iter_mut().enumerate() {
        let hero = hero_names[i % hero_names.len()].to_string();
        player.hero_positions.insert(hero.clone(), (1000.0, 500.0));
        player.heroes.push(hero.clone());
        player.equipped.entry(hero).or_default();
    }

    game.start_round1();
    game.end_shop();

    let initial_alive = game.alive_count();
    let mut any_eliminated = false;

    for round in 0..50 {
        let seed = 100 + round;
        game.run_combat_round(&hero_defs, &ability_defs, seed, &mut rng);

        if game.alive_count() < initial_alive {
            any_eliminated = true;
        }
        if game.alive_count() <= 1 {
            break;
        }

        game.end_combat(false);
        game.end_grace_period();
        game.end_shop();
    }

    assert!(any_eliminated, "Players should get eliminated over time");
}
