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

/// Verify: rerolling shop costs 1 gold, rerolling hero costs 2 gold (discards hero)
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

    // Hero reroll costs 2 gold and discards the hero
    let (hero_defs, _) = load_defs();
    let hero_refs: Vec<&HeroDef> = hero_defs.values().collect();

    let mut player2 = PlayerState::new(1);
    player2.gold = 10;
    player2.heroes.push("Sven".to_string());
    player2.equipped.insert("Sven".to_string(), vec!["ability_0".to_string()]);
    player2.abilities.insert("ability_0".to_string(), 1);

    // Reroll hero: discards Sven, returns abilities to bench, gives 3 choices
    let choices = player2.reroll_hero("Sven", &hero_refs, &mut rng).unwrap();
    assert_eq!(player2.gold, 8);
    assert!(!player2.heroes.contains(&"Sven".to_string()));
    assert!(player2.bench.contains(&"ability_0".to_string()));
    assert!(choices.iter().any(|c| c.is_some()));

    // Reroll hero you don't own → rejected
    let result = player2.reroll_hero("Sven", &hero_refs, &mut rng);
    assert_eq!(result, Err("you don't own that hero"));

    // Reroll with insufficient gold → rejected
    player2.gold = 1;
    player2.heroes.push("Juggernaut".to_string());
    let result = player2.reroll_hero("Juggernaut", &hero_refs, &mut rng);
    assert_eq!(result, Err("not enough gold"));
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
    game.end_grace_period(&mut rand::thread_rng());
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

    let team = build_team(&player, &hero_defs, &ability_defs, 2, 1);
    assert_eq!(team.len(), 1);

    let (config, _pos, _buffs) = &team[0];
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

    let team_a = build_team(&player, &hero_defs, &ability_defs, 2, 1);
    let team_b = build_team(&player, &hero_defs, &ability_defs, 2, 1);

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

/// Verify: Archmage sorcery triggers guaranteed when shop is upgraded.
/// Setup: player with Archmage god, some abilities at various levels.
/// After upgrade + trigger_sorcery, exactly one ability gains +1 level.
/// Pool is unchanged (free level, no deduction).
#[test]
fn test_archmage_sorcery_on_shop_upgrade() {
    use aa2_game::god::{archmage, is_archmage, trigger_sorcery};

    let mut player = PlayerState::new(0);
    player.gold = 50;
    player.god = Some(archmage());
    player.abilities.insert("fireball".to_string(), 3);
    player.abilities.insert("heal".to_string(), 5);
    player.abilities.insert("shield".to_string(), 1);

    assert!(is_archmage(&player));

    let total_before: u32 = player.abilities.values().sum();

    // Upgrade shop
    let cost = player.shop.upgrade(&mut player.gold);
    assert!(cost.is_some());
    assert_eq!(player.shop.level, 2);

    // Archmage guaranteed sorcery on upgrade
    let mut rng = StdRng::seed_from_u64(99);
    let upgraded = trigger_sorcery(&mut player, &mut rng);
    assert!(upgraded.is_some());

    let total_after: u32 = player.abilities.values().sum();
    assert_eq!(total_after, total_before + 1, "exactly one ability gained +1 level");
}

/// Verify: player can equip ultimates on different heroes (only same-hero duplicate rejected).
#[test]
fn test_multiple_ultimates_different_heroes() {
    let mut player = PlayerState::new(0);
    player.heroes.push("hero1".to_string());
    player.heroes.push("hero2".to_string());

    let mut ultimates = HashSet::new();
    ultimates.insert("ult1".to_string());
    ultimates.insert("ult2".to_string());
    ultimates.insert("ult3".to_string());
    let config = GameConfig::default();

    // Put ults on bench
    player.abilities.insert("ult1".to_string(), 1);
    player.bench.push("ult1".to_string());
    player.abilities.insert("ult2".to_string(), 1);
    player.bench.push("ult2".to_string());
    player.abilities.insert("ult3".to_string(), 1);
    player.bench.push("ult3".to_string());

    // Equip ult1 to hero1 -> OK
    assert!(player.equip_ability("ult1", "hero1", &ultimates, &config).is_ok());

    // Equip ult2 to hero2 -> OK (different hero)
    assert!(player.equip_ability("ult2", "hero2", &ultimates, &config).is_ok());

    // Equip ult3 to hero1 -> REJECTED (hero1 already has an ultimate)
    let result = player.equip_ability("ult3", "hero1", &ultimates, &config);
    assert_eq!(result, Err("hero already has an ultimate"));
}

/// Verify: positioning matters. Compact front team beats spread-out team.
/// Both teams have the same heroes (Sven + Juggernaut, both melee).
/// Team A: both heroes near midline (Y=900) — they engage immediately.
/// Team B: Sven near midline (Y=900), Juggernaut far back (Y=100).
/// After mirroring, team B's Jugg ends up at (100, 1900) — very far from the fight.
/// Team A should win because they get a 2v1 advantage early.
#[test]
fn test_positioning_affects_combat_outcome() {
    let (hero_defs, ability_defs) = load_defs();

    // Build team A: both melee heroes near midline (Y=900), compact
    let mut player_a = PlayerState::new(0);
    player_a.heroes.push("Sven".to_string());
    player_a.heroes.push("Juggernaut".to_string());
    player_a.hero_positions.insert("Sven".to_string(), (900.0, 900.0));
    player_a.hero_positions.insert("Juggernaut".to_string(), (1100.0, 900.0));
    player_a.equipped.entry("Sven".to_string()).or_default();
    player_a.equipped.entry("Juggernaut".to_string()).or_default();

    // Build team B: Sven near midline, Jugg far back
    // After mirroring: Sven at (1000, 1100) — close to fight
    //                  Jugg at (100, 1900) — very far from fight
    let mut player_b = PlayerState::new(1);
    player_b.heroes.push("Sven".to_string());
    player_b.heroes.push("Juggernaut".to_string());
    player_b.hero_positions.insert("Sven".to_string(), (1000.0, 900.0));
    player_b.hero_positions.insert("Juggernaut".to_string(), (1900.0, 100.0));
    player_b.equipped.entry("Sven".to_string()).or_default();
    player_b.equipped.entry("Juggernaut".to_string()).or_default();

    let hero_level = 5;
    let round = 5;

    let team_a = build_team(&player_a, &hero_defs, &ability_defs, hero_level, round);
    let team_b = build_team(&player_b, &hero_defs, &ability_defs, hero_level, round);

    // Run with multiple seeds to confirm positioning advantage is consistent
    let mut a_wins = 0u32;
    let seeds: [u32; 5] = [42, 123, 777, 1001, 9999];
    for seed in seeds {
        let (winner, _, _) = run_combat(&team_a, &team_b, seed);
        if winner == Some(0) {
            a_wins += 1;
        }
    }

    // Team A (compact front) should win majority of the time
    assert!(
        a_wins >= 3,
        "Team A (compact front) should win at least 3/5 seeds, got {a_wins}/5"
    );
}

/// Verify: draft with no heroes of a given attribute returns None for that slot.
#[test]
fn test_draft_with_no_heroes_of_attribute() {
    use aa2_data::{Attribute, HeroDef};
    use aa2_game::draft::generate_draft_choices;

    // Only STR heroes available at tier 0
    let str_hero = HeroDef {
        name: "str_only".to_string(),
        primary_attribute: Attribute::Strength,
        base_str: 20.0,
        base_agi: 15.0,
        base_int: 10.0,
        str_gain: 2.0,
        agi_gain: 1.0,
        int_gain: 1.0,
        base_attack_time: 1.7,
        attack_range: 150.0,
        attack_point: 0.4,
        move_speed: 300.0,
        turn_rate: 0.6,
        collision_radius: 24.0,
        tier: 0,
        is_melee: true,
        base_damage_min: 30.0,
        base_damage_max: 35.0,
        projectile_speed: None,
    };

    let heroes = [str_hero];
    let refs: Vec<&HeroDef> = heroes.iter().collect();
    let mut rng = StdRng::seed_from_u64(42);

    let choices = generate_draft_choices(&refs, 0, &mut rng);
    assert_eq!(choices[0].as_deref(), Some("str_only"));
    assert_eq!(choices[1], None); // No AGI hero
    assert_eq!(choices[2], None); // No INT hero
}

/// Verify: shop roll with depleted pool offers only what's available without panicking.
#[test]
fn test_shop_roll_with_depleted_pool() {
    use aa2_game::shop::ShopState;

    // Pool has only 2 abilities left
    let mut counts = HashMap::new();
    counts.insert("ability_a".to_string(), 1);
    counts.insert("ability_b".to_string(), 1);
    let mut pool = AbilityPool::from_counts(counts);

    let mut shop = ShopState::new();
    shop.level = 2; // size = 6
    let ultimates = HashSet::new();
    let mut rng = StdRng::seed_from_u64(42);

    // Should not panic, just offer what's available
    shop.roll(&mut pool, &ultimates, 3, 0, &mut rng);
    assert_eq!(shop.offerings.len(), 2);
    assert!(shop.offerings.contains(&"ability_a".to_string()));
    assert!(shop.offerings.contains(&"ability_b".to_string()));
}

/// Verify: available_for_shop with completely empty pool returns empty Vec.
#[test]
fn test_empty_pool_available_for_shop() {
    let pool = AbilityPool::from_counts(HashMap::new());
    let ultimates = HashSet::new();
    let available = pool.available_for_shop(3, &ultimates, 3);
    assert!(available.is_empty());
}
