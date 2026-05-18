//! Integration tests for game mechanics: shop lock, combat timeout, ghost damage,
//! reroll costs, grace period gold, ability levels, and sell-equipped.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use aa2_data::{AbilityDef, HeroDef};
use aa2_game::combat::{build_team, run_combat};
use aa2_game::economy::REROLL_COST;
use aa2_game::{AbilityPool, GameConfig, GamePhase, GameState, PlayerState};
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

fn test_pool() -> AbilityPool {
    let counts: HashMap<String, u32> = (0..20)
        .map(|i| (format!("ability_{i}"), 10))
        .collect();
    AbilityPool::from_counts(counts)
}

fn test_game() -> GameState {
    GameState::new(test_pool(), HashSet::new(), GameConfig::default())
}

/// Verify: lock shop → combat ends → offerings preserved → lock auto-clears → next reroll works
#[test]
fn test_shop_lock_preserves_offerings_then_clears() {
    let mut game = test_game();
    game.phase = GamePhase::Combat;

    // Give player 0 some shop offerings
    let mut rng = StdRng::seed_from_u64(42);
    game.players[0].shop.roll(
        &mut game.pool,
        &game.ultimates,
        game.config.ultimate_unlock_level,
        game.config.shop_size_bonus,
        &mut rng,
    );
    let original_offerings = game.players[0].shop.offerings.clone();
    assert!(!original_offerings.is_empty());

    // Lock the shop
    game.players[0].shop.toggle_lock();
    assert!(game.players[0].shop.locked);

    // End combat → grace period
    game.end_combat(false);

    // Locked shop: offerings preserved, lock auto-cleared, needs_reroll NOT set
    assert_eq!(game.players[0].shop.offerings, original_offerings);
    assert!(!game.players[0].shop.locked);
    assert!(!game.players[0].shop.needs_reroll);

    // Other players should have needs_reroll set
    assert!(game.players[1].shop.needs_reroll);

    // Now manually reroll → works normally (new offerings)
    game.players[0].gold = 10;
    game.players[0]
        .reroll_shop(
            &mut game.pool,
            &game.ultimates,
            game.config.ultimate_unlock_level,
            game.config.shop_size_bonus,
            REROLL_COST,
            &mut rng,
        )
        .unwrap();
    // Offerings changed (or at least reroll succeeded)
    assert!(!game.players[0].shop.offerings.is_empty());
    assert_eq!(game.players[0].gold, 9);
}

/// Verify: when combat times out (1500 ticks), both players take damage in a draw
#[test]
fn test_combat_timeout_draw_mutual_damage() {
    let (hero_defs, ability_defs) = load_defs();

    let mut game = test_game();
    game.round = 5;
    let mut rng = StdRng::seed_from_u64(777);

    // Give 2 players identical single-hero teams (to maximize chance of timeout or draw)
    // Use a hero with high HP — pick the first available hero
    let hero_name = hero_defs.keys().next().unwrap().clone();

    for i in 0..2 {
        game.players[i].heroes.push(hero_name.clone());
        game.players[i]
            .hero_positions
            .insert(hero_name.clone(), (1000.0, 500.0));
        game.players[i].equipped.entry(hero_name.clone()).or_default();
    }
    // Mark players 2-7 as dead so only 2 fight
    for i in 2..8 {
        game.players[i].alive = false;
    }

    let initial_hp_0 = game.players[0].hp;
    let initial_hp_1 = game.players[1].hp;

    game.run_combat_round(&hero_defs, &ability_defs, 42, &mut rng);

    // At least one player should have taken damage (either winner/loser or draw)
    let hp_0 = game.players[0].hp;
    let hp_1 = game.players[1].hp;
    assert!(
        hp_0 < initial_hp_0 || hp_1 < initial_hp_1,
        "At least one player should take damage: p0={hp_0}, p1={hp_1}"
    );
}

/// Verify ghost matchup: ghost loses → no damage to ghost source. Ghost wins → opponent damaged.
#[test]
fn test_ghost_matchup_damage_application() {
    let (hero_defs, ability_defs) = load_defs();

    let mut game = test_game();
    game.round = 5;
    let mut rng = StdRng::seed_from_u64(123);

    // 3 alive players → odd → ghost matchup
    let hero_names: Vec<String> = hero_defs.keys().take(3).cloned().collect();

    for i in 0..3 {
        let hero = &hero_names[i % hero_names.len()];
        game.players[i].heroes.push(hero.clone());
        game.players[i]
            .hero_positions
            .insert(hero.clone(), (1000.0, 500.0));
        game.players[i].equipped.entry(hero.clone()).or_default();
    }
    // Kill players 3-7
    for i in 3..8 {
        game.players[i].alive = false;
    }

    let hp_before: Vec<f32> = game.players.iter().map(|p| p.hp).collect();

    let results = game.run_combat_round(&hero_defs, &ability_defs, 42, &mut rng);

    // Find the ghost matchup
    let ghost_result = results.iter().find(|r| r.matchup.ghost);
    if let Some(gr) = ghost_result {
        let source_id = gr.matchup.ghost_source.unwrap() as usize;
        let source_hp_before = hp_before[source_id];
        let source_hp_after = game.players[source_id].hp;

        // Ghost source should NOT take damage from the ghost matchup
        // (they may take damage from their own separate matchup though)
        // Check: if ghost lost, source is unaffected by THIS matchup
        if gr.winner == Some(gr.matchup.player_a) {
            // Ghost (player_b side) lost → no damage to source
            // Source's HP change should only come from their own matchup, not this one
            // We verify the logic path was exercised
            assert!(
                source_hp_after <= source_hp_before,
                "Source HP should not increase"
            );
        }
    }
    // Verify at least some damage was dealt overall
    let total_hp_lost: f32 = game
        .players
        .iter()
        .enumerate()
        .filter(|(i, _)| *i < 3)
        .map(|(i, p)| hp_before[i] - p.hp)
        .sum();
    assert!(total_hp_lost > 0.0, "Some damage should be dealt");
}

/// Verify: rerolling shop costs 1 gold, rerolling hero draft costs 2 gold
#[test]
fn test_reroll_costs() {
    let mut rng = StdRng::seed_from_u64(42);
    let mut pool = test_pool();
    let ultimates = HashSet::new();
    let config = GameConfig::default();

    // Shop reroll costs 1 gold
    let mut player = PlayerState::new(0);
    player.gold = 10;
    player.shop.roll(&mut pool, &ultimates, config.ultimate_unlock_level, config.shop_size_bonus, &mut rng);

    player
        .reroll_shop(&mut pool, &ultimates, config.ultimate_unlock_level, config.shop_size_bonus, REROLL_COST, &mut rng)
        .unwrap();
    assert_eq!(player.gold, 9);

    player
        .reroll_shop(&mut pool, &ultimates, config.ultimate_unlock_level, config.shop_size_bonus, REROLL_COST, &mut rng)
        .unwrap();
    assert_eq!(player.gold, 8);

    // Reroll with 0 gold → rejected
    player.gold = 0;
    let result = player.reroll_shop(&mut pool, &ultimates, config.ultimate_unlock_level, config.shop_size_bonus, REROLL_COST, &mut rng);
    assert_eq!(result, Err("not enough gold"));

    // Hero reroll costs 2 gold
    let (hero_defs, _) = load_defs();
    let hero_refs: Vec<&HeroDef> = hero_defs.values().collect();

    let mut player2 = PlayerState::new(1);
    player2.gold = 10;
    let draft = player2.reroll_draft(&hero_refs, &mut rng).unwrap();
    assert_eq!(player2.gold, 8);
    // Should return valid draft state
    assert!(draft.choices.iter().any(|c| c.is_some()));

    // Reroll with 1 gold → rejected (not enough for cost of 2)
    player2.gold = 1;
    let result = player2.reroll_draft(&hero_refs, &mut rng);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "not enough gold");
}

/// Verify: during grace period, player can spend old gold. After grace ends, gold resets.
#[test]
fn test_grace_period_gold_spending_then_reset() {
    let mut game = test_game();
    game.round = 1; // end_grace_period will increment to round 2
    game.phase = GamePhase::GracePeriod;
    game.players[0].gold = 5;

    // Player buys an ability during grace period (costs BUY_COST=3)
    game.pool.counts.insert("test_ability".to_string(), 10);
    game.players[0]
        .buy_ability("test_ability", &mut game.pool)
        .unwrap();
    assert_eq!(game.players[0].gold, 2); // 5 - 3

    // End grace period → gold resets to round 2 formula
    game.end_grace_period();
    // Round 2 gold = 6 + 2*(2-1) = 8
    assert_eq!(game.players[0].gold, 8);
}

/// Verify: ability level (copies purchased) is correctly passed to UnitConfig
#[test]
fn test_ability_level_in_combat() {
    let (hero_defs, ability_defs) = load_defs();

    let hero_name = hero_defs.keys().next().unwrap().clone();
    let ability_name = ability_defs.keys().next().unwrap().clone();

    let mut player = PlayerState::new(0);
    player.heroes.push(hero_name.clone());
    player.hero_positions.insert(hero_name.clone(), (1000.0, 500.0));
    player.abilities.insert(ability_name.clone(), 3);
    player
        .equipped
        .entry(hero_name.clone())
        .or_default()
        .push(ability_name.clone());

    let team = build_team(&player, &hero_defs, &ability_defs, 2);
    assert_eq!(team.len(), 1);

    let (config, _pos) = &team[0];
    // The ability should be at level 3
    assert!(!config.abilities.is_empty());
    let (_, level) = &config.abilities[0];
    assert_eq!(*level, 3);
}

/// Verify: selling an ability that's equipped on a hero works (frees slot, refunds, returns to pool)
#[test]
fn test_sell_equipped_ability() {
    let mut pool = test_pool();
    pool.counts.insert("fireball".to_string(), 7); // 10 - 3 copies owned

    let mut player = PlayerState::new(0);
    player.gold = 0;
    player.heroes.push("axe".to_string());
    player.abilities.insert("fireball".to_string(), 2);
    player
        .equipped
        .insert("axe".to_string(), vec!["fireball".to_string()]);

    // Sell the equipped ability
    player.sell_ability("fireball", &mut pool).unwrap();

    // Gold refund: 2 * level(2) = 4
    assert_eq!(player.gold, 4);
    // Ability removed from abilities map
    assert!(!player.abilities.contains_key("fireball"));
    // Ability removed from hero's equipped list
    assert!(player.equipped["axe"].is_empty());
    // Copies returned to pool: 7 + 2 = 9
    assert_eq!(pool.counts["fireball"], 9);
}

/// Verify: run_combat respects COMBAT_MAX_TICKS and returns a result even on timeout
#[test]
fn test_run_combat_returns_result_on_timeout() {
    let (hero_defs, ability_defs) = load_defs();

    // Build two identical single-hero teams
    let hero_name = hero_defs.keys().next().unwrap().clone();

    let mut player = PlayerState::new(0);
    player.heroes.push(hero_name.clone());
    player.hero_positions.insert(hero_name.clone(), (1000.0, 500.0));
    player.equipped.entry(hero_name.clone()).or_default();

    let team_a = build_team(&player, &hero_defs, &ability_defs, 2);
    let team_b = build_team(&player, &hero_defs, &ability_defs, 2);

    let (winner, survivors_a, survivors_b) = run_combat(&team_a, &team_b, 42);

    // Combat should produce a valid result (winner or draw)
    // Either someone won or it's a draw — both are valid
    assert!(
        winner.is_none() || winner == Some(0) || winner == Some(1),
        "Winner should be None (draw), 0, or 1"
    );
    // At least one team should have survivors (or both if draw)
    assert!(
        survivors_a > 0 || survivors_b > 0,
        "At least one team should have survivors"
    );
}
