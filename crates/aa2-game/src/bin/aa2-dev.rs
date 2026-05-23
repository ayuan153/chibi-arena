//! AA2 Dev Mode — interactive single-player CLI game.

use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};
use std::path::Path;

use rand::seq::SliceRandom;
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

use aa2_data::{AbilityDef, HeroDef, load_ability_def, load_all_heroes};
use aa2_game::*;
use aa2_game::draft::{DraftState, generate_draft_choices, tier_for_draft_round};
use aa2_game::god::{self, all_gods};
use aa2_game::economy::{BUY_COST, REROLL_COST, HERO_REROLL_COST};
use aa2_game::player::MAX_HEROES;

fn main() {
    if let Err(e) = run() {
        eprintln!("Fatal error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let data_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let heroes = load_all_heroes(&data_dir.join("heroes"))?;
    let abilities = load_all_abilities(&data_dir.join("abilities"))?;

    let hero_defs: HashMap<String, HeroDef> = heroes.iter().map(|h| (h.name.clone(), h.clone())).collect();
    let ability_defs: HashMap<String, AbilityDef> = abilities.iter().map(|a| (a.name.clone(), a.clone())).collect();
    let ultimates: HashSet<String> = abilities.iter().filter(|a| a.is_ultimate).map(|a| a.name.clone()).collect();

    let seed: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(rand::random);
    let mut rng = StdRng::seed_from_u64(seed);
    eprintln!("  [seed: {}]", seed);

    // Build ability pool: all abilities with 20 copies each
    let roster: Vec<String> = abilities.iter().map(|a| a.name.clone()).collect();
    let pool = AbilityPool::new(&roster, roster.len(), 20, &mut rng);

    let config = GameConfig { auto_advance: false, ..GameConfig::default() };
    let mut game = GameState::new(pool, ultimates.clone(), config);

    // God pick phase
    god_pick_phase(&mut game, &mut rng)?;

    // Start round 1
    game.start_round1();

    // Generate initial draft choices for all players
    let mut drafts: Vec<Option<DraftState>> = Vec::new();
    for i in 0..8 {
        let available = available_heroes_for_player(&heroes, &game.players[i]);
        let tier = tier_for_draft_round(1).unwrap_or(0);
        let choices = generate_draft_choices(&available, tier, &mut rng);
        drafts.push(Some(DraftState { choices, round_tier: tier }));
    }

    // Roll initial shops
    for player in &mut game.players {
        player.shop.roll(&mut game.pool, &ultimates, game.config.ultimate_unlock_level, game.config.shop_size_bonus, &mut rng);
    }

    let mut round_seed: u32 = rng.r#gen();
    let mut placements: Vec<(u8, u32)> = Vec::new(); // (player_id, round_eliminated)
    let mut last_combat_log: Option<(u32, u8, Vec<aa2_sim::CombatEvent>)> = None; // (round, opponent_id, log)
    let mut pending_reroll_draft: Option<[Option<String>; 3]> = None;

    println!("\n=== GAME START ===\n");

    loop {
        if game.alive_count() <= 1 {
            break;
        }

        // Archmage sorcery at shop start (all players)
        for i in 0..8 {
            if !game.players[i].alive { continue; }
            if let Some(name) = god::maybe_trigger_sorcery(&mut game.players[i], &mut rng)
                && i == 0
            {
                let lv = game.players[0].abilities.get(&name).copied().unwrap_or(1);
                println!("  ✨ Sorcery! {} upgraded to Lv {}", name, lv);
            }
        }

        // Display status
        println!("\n=== ROUND {} | SHOP PHASE | Gold: {} | HP: {:.0} ===",
            game.round, game.players[0].gold, game.players[0].hp);

        if game.draft_pending
            && let Some(ref draft) = drafts[0]
        {
            display_draft(draft, &hero_defs);
        }
        if let Some(ref choices) = pending_reroll_draft {
            let draft = DraftState { choices: choices.clone(), round_tier: 0 };
            display_draft(&draft, &hero_defs);
            println!("  Pick one with: draft <1|2|3>");
        }

        display_shop(&game.players[0], &game);
        display_heroes(&game.players[0], &game, &hero_defs);

        // Player command loop
        let stdin = io::stdin();
        loop {
            print!("> [ready | shop | heroes | bench | status | help] ");
            io::stdout().flush().ok();
            let mut line = String::new();
            if stdin.lock().read_line(&mut line).is_err() || line.is_empty() {
                return Ok(());
            }
            let line = line.trim().to_lowercase();
            if line.is_empty() { continue; }

            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts[0] {
                "ready" => break,
                "help" => print_help(),
                "status" => println!("  Round: {} | Gold: {} | HP: {:.0} | Phase: Shop",
                    game.round, game.players[0].gold, game.players[0].hp),
                "shop" => display_shop(&game.players[0], &game),
                "heroes" => display_heroes(&game.players[0], &game, &hero_defs),
                "bench" => display_bench(&game.players[0]),
                "board" => display_board(&game.players[0]),
                "god" => display_god(&game.players[0]),
                "players" => display_players(&game),
                "log" => display_combat_log(&last_combat_log),
                "lock" => {
                    game.players[0].shop.toggle_lock();
                    println!("  Shop lock: {}", if game.players[0].shop.locked { "ON" } else { "OFF" });
                }
                "buy" => {
                    if parts.len() < 2 {
                        println!("  Usage: buy <index>");
                    } else if let Ok(idx) = parts[1].parse::<usize>() {
                        handle_buy(&mut game, 0, idx, &ultimates);
                    } else {
                        println!("  Invalid index");
                    }
                }
                "sell" => {
                    if parts.len() < 2 {
                        println!("  Usage: sell <ability_name>");
                    } else {
                        let name = parts[1..].join("_");
                        handle_sell(&mut game, 0, &name);
                    }
                }
                "reroll" => {
                    let cost = game.config.reroll_cost_override.unwrap_or(REROLL_COST);
                    match game.players[0].reroll_shop(
                        &mut game.pool, &ultimates,
                        game.config.ultimate_unlock_level,
                        game.config.shop_size_bonus, cost, &mut rng,
                    ) {
                        Ok(()) => {
                            println!("  Rerolled! (-{}g)", cost);
                            display_shop(&game.players[0], &game);
                        }
                        Err(e) => println!("  Cannot reroll: {e}"),
                    }
                }
                "upgrade" => {
                    let mut gold = game.players[0].gold;
                    if let Some(cost) = game.players[0].shop.upgrade(&mut gold) {
                        game.players[0].gold = gold;
                        println!("  Upgraded shop to level {}! (-{}g)", game.players[0].shop.level, cost);
                        // Archmage guaranteed sorcery on upgrade
                        if god::is_archmage(&game.players[0])
                            && let Some(name) = god::trigger_sorcery(&mut game.players[0], &mut rng)
                        {
                            let lv = game.players[0].abilities.get(&name).copied().unwrap_or(1);
                            println!("  ✨ Sorcery! {} upgraded to Lv {}", name, lv);
                        }
                    } else {
                        let cost_str = game.players[0].shop.upgrade_cost()
                            .map(|c| format!("(costs {}g)", c))
                            .unwrap_or_else(|| "max level".to_string());
                        println!("  Cannot upgrade: {}", cost_str);
                    }
                }
                "equip" => {
                    if parts.len() < 3 {
                        println!("  Usage: equip <ability> <hero>");
                    } else {
                        let ability = parts[1].to_string();
                        let hero = parts[2..].join("_");
                        handle_equip(&mut game, 0, &ability, &hero, &ultimates);
                    }
                }
                "unequip" => {
                    if parts.len() < 3 {
                        println!("  Usage: unequip <ability> <hero>");
                    } else {
                        let ability_input = parts[1].to_string();
                        let hero_input = parts[2..].join("_");
                        let ability_slug_val = slug(&ability_input);
                        let hero_slug_val = slug(&hero_input);
                        let actual_ability = game.players[0].equipped.values().flatten()
                            .find(|a| slug(a) == ability_slug_val)
                            .cloned();
                        let actual_hero = game.players[0].heroes.iter()
                            .find(|h| slug(h) == hero_slug_val)
                            .cloned();
                        match (actual_ability, actual_hero) {
                            (Some(a), Some(h)) => {
                                match game.players[0].unequip_ability(&a, &h) {
                                    Ok(()) => println!("  Unequipped {} from {}", a, h),
                                    Err(e) => println!("  Cannot unequip: {e}"),
                                }
                            }
                            (None, _) => println!("  Ability not equipped: {}", ability_input),
                            (_, None) => println!("  Hero not owned: {}", hero_input),
                        }
                    }
                }
                "draft" => {
                    if parts.len() < 2 {
                        println!("  Usage: draft <1|2|3>");
                    } else if let Ok(idx) = parts[1].parse::<usize>() {
                        if let Some(ref choices) = pending_reroll_draft {
                            if !(1..=3).contains(&idx) {
                                println!("  Pick 1, 2, or 3");
                            } else if let Some(Some(hero_name)) = choices.get(idx - 1) {
                                let hero_name = hero_name.clone();
                                game.players[0].heroes.push(hero_name.clone());
                                game.players[0].hero_positions.insert(hero_name.clone(), (1000.0, 500.0));
                                println!("  Drafted {}!", hero_name);
                                pending_reroll_draft = None;
                            } else {
                                println!("  No hero available at that slot");
                            }
                        } else {
                            handle_draft(&mut game, &mut drafts, 0, idx, &hero_defs);
                        }
                    } else {
                        println!("  Invalid index");
                    }
                }
                "reroll-hero" => {
                    if parts.len() < 2 {
                        println!("  Usage: reroll-hero <hero_name>");
                    } else {
                        let hero_input = parts[1..].join("_");
                        let hero_slug_val = slug(&hero_input);
                        let actual_hero = game.players[0].heroes.iter()
                            .find(|h| slug(h) == hero_slug_val)
                            .cloned();
                        match actual_hero {
                            Some(h) => {
                                let available = available_heroes_for_player(&heroes, &game.players[0]);
                                match game.players[0].reroll_hero(&h, &available, &mut rng) {
                                    Ok(choices) => {
                                        println!("  Discarded {}! (-{}g)", h, HERO_REROLL_COST);
                                        pending_reroll_draft = Some(choices.clone());
                                        let draft = DraftState { choices, round_tier: 0 };
                                        display_draft(&draft, &hero_defs);
                                        println!("  Pick one with: draft <1|2|3>");
                                    }
                                    Err(e) => println!("  Cannot reroll: {e}"),
                                }
                            }
                            None => println!("  Hero not owned: {}", hero_input),
                        }
                    }
                }
                "position" => {
                    if parts.len() < 4 {
                        println!("  Usage: position <hero> <x> <y>");
                    } else {
                        let hero_input = parts[1].to_string();
                        let hero_slug_val = slug(&hero_input);
                        if let (Ok(x), Ok(y)) = (parts[2].parse::<f32>(), parts[3].parse::<f32>()) {
                            let actual_hero = game.players[0].heroes.iter()
                                .find(|h| slug(h) == hero_slug_val)
                                .cloned();
                            match actual_hero {
                                Some(h) => {
                                    game.players[0].hero_positions.insert(h.clone(), (x, y));
                                    println!("  {} positioned at ({}, {})", h, x, y);
                                }
                                None => println!("  Hero not owned: {}", hero_input),
                            }
                        } else {
                            println!("  Invalid coordinates");
                        }
                    }
                }
                "buff" => {
                    if parts.len() < 2 {
                        println!("  Usage: buff <hero>");
                    } else {
                        let hero_input = parts[1..].join("_");
                        let hero_slug_val = slug(&hero_input);
                        let actual_hero = game.players[0].heroes.iter()
                            .find(|h| slug(h) == hero_slug_val)
                            .cloned();
                        match actual_hero {
                            Some(h) => {
                                game.players[0].god_buff_target = Some(h.clone());
                                println!("  God buff target set to: {}", h);
                            }
                            None => println!("  Hero not owned: {}", hero_input),
                        }
                    }
                }
                _ => println!("  Unknown command. Type 'help' for commands."),
            }
        }

        // AI takes actions
        ai_take_actions(&mut game, &mut drafts, &hero_defs, &ultimates, &mut rng);

        // AI sorcery on upgrade (silently)
        // (AI upgrades happen inside ai_take_actions — sorcery already handled there if needed)

        // Combat phase
        game.end_shop();
        println!("\n=== ROUND {} | COMBAT ===", game.round);

        let prev_alive: Vec<u8> = game.players.iter().filter(|p| p.alive).map(|p| p.id).collect();
        let results = game.run_combat_round(&hero_defs, &ability_defs, round_seed, &mut rng);
        round_seed = round_seed.wrapping_add(1);

        // Store combat log for player 0's matchup
        for result in &results {
            if result.matchup.player_a == 0 || result.matchup.player_b == 0 {
                let opponent = if result.matchup.player_a == 0 {
                    result.matchup.player_b
                } else {
                    result.matchup.player_a
                };
                last_combat_log = Some((game.round, opponent, result.combat_log.clone()));
                break;
            }
        }

        display_combat_results(&results, &game);
        println!("  Type 'log' to see detailed combat log");

        // Check eliminations
        for pid in &prev_alive {
            if !game.players[*pid as usize].alive {
                println!("  *** Player {} has been ELIMINATED! ***", pid);
                placements.push((*pid, game.round));
            }
        }

        if game.alive_count() <= 1 {
            break;
        }

        // Advance to next round
        game.round += 1;
        game.start_round();

        // Roll shops for all alive players
        for player in &mut game.players {
            if player.alive {
                if !player.shop.locked {
                    player.shop.roll(&mut game.pool, &ultimates, game.config.ultimate_unlock_level, game.config.shop_size_bonus, &mut rng);
                } else {
                    player.shop.locked = false;
                }
            }
        }

        // Generate draft if needed
        if draft::is_draft_round(game.round) {
            game.draft_pending = true;
            #[allow(clippy::needless_range_loop)]
            for i in 0..8 {
                if game.players[i].alive {
                    let available = available_heroes_for_player(&heroes, &game.players[i]);
                    let tier = tier_for_draft_round(game.round).unwrap_or(0);
                    let choices = generate_draft_choices(&available, tier, &mut rng);
                    drafts[i] = Some(DraftState { choices, round_tier: tier });
                }
            }
        } else {
            game.draft_pending = false;
        }
    }

    // Game over
    println!("\n=== GAME OVER ===");
    // Add surviving player
    if let Some(winner) = game.players.iter().find(|p| p.alive) {
        placements.push((winner.id, game.round));
    }
    placements.reverse();
    println!("\nFinal Placements:");
    for (place, (pid, round)) in placements.iter().enumerate() {
        let label = if *pid == 0 { " (YOU)" } else { "" };
        println!("  {}. Player {}{} (eliminated round {})", place + 1, pid, label, round);
    }
    if game.players[0].alive {
        println!("\n  *** YOU WIN! ***");
    } else {
        println!("\n  You placed #{}", placements.iter().position(|(id, _)| *id == 0).unwrap_or(7) + 1);
    }

    Ok(())
}

// --- Data Loading ---

fn load_all_abilities(dir: &Path) -> Result<Vec<AbilityDef>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{dir:?}: {e}"))?;
    let mut abilities = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| format!("{dir:?}: {e}"))?.path();
        if path.extension().is_some_and(|ext| ext == "ron") {
            abilities.push(load_ability_def(&path)?);
        }
    }
    Ok(abilities)
}

fn slug(name: &str) -> String {
    name.to_lowercase().replace(' ', "_")
}

fn available_heroes_for_player<'a>(all_heroes: &'a [HeroDef], player: &PlayerState) -> Vec<&'a HeroDef> {
    all_heroes.iter().filter(|h| !player.heroes.contains(&h.name)).collect()
}

// --- God Pick Phase ---

fn god_pick_phase(game: &mut GameState, rng: &mut StdRng) -> Result<(), String> {
    let gods = all_gods();
    println!("\n=== GOD PICK ===\n");
    for (i, god) in gods.iter().enumerate() {
        println!("  {}. {} - {}", i + 1, god.name, god.description);
    }
    println!();
    let stdin = io::stdin();
    loop {
        print!("Pick your god (1-{}): ", gods.len());
        io::stdout().flush().ok();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_err() {
            return Err("Failed to read input".to_string());
        }
        if let Ok(idx) = line.trim().parse::<usize>()
            && idx >= 1 && idx <= gods.len()
        {
            game.players[0].god = Some(gods[idx - 1].clone());
            println!("  You chose: {}\n", gods[idx - 1].name);
            break;
        }
        println!("  Invalid choice.");
    }
    // AI picks randomly
    for i in 1..8 {
        let god = gods.choose(rng).expect("gods not empty").clone();
        game.players[i].god = Some(god);
    }
    Ok(())
}

// --- Display Functions ---

fn display_combat_results(results: &[aa2_game::CombatResult], game: &GameState) {
    for result in results {
        let a = result.matchup.player_a;
        let b = result.matchup.player_b;
        let ghost_str = if result.matchup.ghost { " (ghost)" } else { "" };
        match result.winner {
            Some(w) => {
                let loser = if w == a { b } else { a };
                println!("  Player {} vs Player {}{}: Player {} wins! ({} survivors)",
                    a, b, ghost_str, w,
                    if w == a { result.survivors_a } else { result.survivors_b });
                if !result.matchup.ghost || w != b {
                    println!("    Player {} HP: {:.0}", loser, game.players[loser as usize].hp);
                }
            }
            None => {
                println!("  Player {} vs Player {}{}: DRAW", a, b, ghost_str);
            }
        }
    }
}

fn display_shop(player: &PlayerState, game: &GameState) {
    let shop = &player.shop;
    println!("\n--- SHOP (Level {}, Size {}) ---", shop.level, shop.size(game.config.shop_size_bonus));
    if shop.offerings.is_empty() {
        println!("  (empty)");
    }
    for (i, slot) in shop.offerings.iter().enumerate() {
        match slot {
            Some(name) => {
                let level = player.abilities.get(name).map(|l| format!(" <- you own Lv {}", l)).unwrap_or_default();
                let ult = if game.ultimates.contains(name) { " [ULT]" } else { "" };
                println!("  {}. {}{}{}", i + 1, name, ult, level);
            }
            None => println!("  {}. [SOLD]", i + 1),
        }
    }
    if let Some(cost) = shop.upgrade_cost() {
        println!("  [Upgrade to Lv {} costs {}g]", shop.level + 1, cost);
    }
    println!("  Lock: {} | Gold: {} | Buy: {}g | Reroll: {}g",
        if shop.locked { "ON" } else { "OFF" },
        player.gold, BUY_COST,
        game.config.reroll_cost_override.unwrap_or(REROLL_COST));
    let size = shop.offerings.len();
    println!("  Commands: buy <1-{}>, reroll, lock, upgrade, sell <name>", size);
}

fn display_heroes(player: &PlayerState, game: &GameState, hero_defs: &HashMap<String, HeroDef>) {
    println!("\n--- HEROES ({}/{}) ---", player.heroes.len(), MAX_HEROES);
    if player.heroes.is_empty() {
        println!("  (none - pick from draft!)");
        return;
    }
    let level = 1.0 + game.round as f32;
    for (i, hero_name) in player.heroes.iter().enumerate() {
        let equipped = player.equipped.get(hero_name);
        let slots = game.config.ability_slots_per_hero as usize;
        let mut ability_strs: Vec<String> = Vec::new();
        if let Some(abilities) = equipped {
            for a in abilities {
                let lv = player.abilities.get(a).copied().unwrap_or(1);
                let ult = if game.ultimates.contains(a) { " [ULT]" } else { "" };
                ability_strs.push(format!("{}{} (Lv {})", a, ult, lv));
            }
        }
        let filled = ability_strs.len();
        for _ in filled..slots {
            ability_strs.push("[empty]".to_string());
        }
        let pos = player.hero_positions.get(hero_name).copied().unwrap_or((1000.0, 500.0));

        if let Some(h) = hero_defs.get(hero_name) {
            let attr_str = match h.primary_attribute {
                aa2_data::Attribute::Strength => "STR",
                aa2_data::Attribute::Agility => "AGI",
                aa2_data::Attribute::Intelligence => "INT",
                aa2_data::Attribute::Universal => "UNI",
            };
            let lvl = level as u32;
            let str_total = h.base_str + h.str_gain * (level - 1.0);
            let agi_total = h.base_agi + h.agi_gain * (level - 1.0);
            let int_total = h.base_int + h.int_gain * (level - 1.0);
            let primary = match h.primary_attribute {
                aa2_data::Attribute::Strength => str_total,
                aa2_data::Attribute::Agility => agi_total,
                aa2_data::Attribute::Intelligence => int_total,
                aa2_data::Attribute::Universal => 0.0,
            };
            let hp = 120.0 + 22.0 * str_total;
            let mana = 75.0 + 12.0 * int_total;
            let armor = agi_total / 6.0;
            let atk_speed = 100.0 + agi_total;
            let dmg_min = h.base_damage_min + primary;
            let dmg_max = h.base_damage_max + primary;

            println!("  {}. {} [{}] [{}] Lv {} @ ({:.0}, {:.0})", i + 1, hero_name, slug(hero_name), attr_str, lvl, pos.0, pos.1);
            println!("     HP: {:.0}  Mana: {:.0}  Armor: {:.1}  MS: {:.0}", hp, mana, armor, h.move_speed);
            println!("     Damage: {:.0}-{:.0}  BAT: {:.1}  AS: {:.0}  Range: {:.0}", dmg_min, dmg_max, h.base_attack_time, atk_speed, h.attack_range);
            println!("     STR: {:.0}+{:.1}  AGI: {:.0}+{:.1}  INT: {:.0}+{:.1}", h.base_str, h.str_gain, h.base_agi, h.agi_gain, h.base_int, h.int_gain);
        } else {
            println!("  {}. {} [{}] [???] @ ({:.0}, {:.0})", i + 1, hero_name, slug(hero_name), pos.0, pos.1);
        }
        println!("     Abilities: {}", ability_strs.join(", "));
    }
    println!("  Commands: equip <ability> <hero>, unequip <ability> <hero>, reroll-hero <hero> ({}g), position <hero> <x> <y>", HERO_REROLL_COST);
}

fn display_bench(player: &PlayerState) {
    println!("\n--- BENCH ({}/{}) ---", player.bench.len(), aa2_game::player::MAX_BENCH);
    if player.bench.is_empty() {
        println!("  (empty)");
    } else {
        for name in &player.bench {
            let lv = player.abilities.get(name).copied().unwrap_or(1);
            println!("  {} [{}] (Lv {})", name, slug(name), lv);
        }
    }
}

fn display_board(player: &PlayerState) {
    println!("\n--- BOARD ---");
    for hero in &player.heroes {
        let pos = player.hero_positions.get(hero).copied().unwrap_or((1000.0, 500.0));
        println!("  {} at ({:.0}, {:.0})", hero, pos.0, pos.1);
    }
}

fn display_god(player: &PlayerState) {
    match &player.god {
        Some(god) => println!("  God: {} - {}", god.name, god.description),
        None => println!("  No god selected"),
    }
    if let Some(ref target) = player.god_buff_target {
        println!("  Buff target: {}", target);
    }
}

fn display_players(game: &GameState) {
    println!("\n--- PLAYERS ---");
    for p in &game.players {
        let status = if p.alive { format!("HP: {:.0}", p.hp) } else { "DEAD".to_string() };
        let you = if p.id == 0 { " (YOU)" } else { "" };
        println!("  Player {}{}: {} | Heroes: {}", p.id, you, status, p.heroes.len());
    }
}

fn tier_letter(tier: u8) -> &'static str {
    match tier {
        0 => "D",
        1 => "C",
        2 => "B",
        3 => "A",
        4 => "S",
        _ => "?",
    }
}

fn display_draft(draft: &DraftState, hero_defs: &HashMap<String, HeroDef>) {
    println!("\n--- DRAFT (pick a hero) ---");
    let labels = ["STR", "AGI", "INT"];
    for (i, choice) in draft.choices.iter().enumerate() {
        match choice {
            Some(name) => {
                let tier = hero_defs.get(name).map(|h| h.tier).unwrap_or(0);
                println!("  {}. {} [{}] (Tier {})", i + 1, name, labels[i], tier_letter(tier));
            }
            None => println!("  {}. (none available)", i + 1),
        }
    }
    println!("  Commands: draft <1-3>");
}

fn print_help() {
    println!("
Commands:
  ready           - end shop phase
  shop            - show shop offerings
  buy <index>     - buy ability (1-indexed)
  sell <name>     - sell ability (use underscores for spaces)
  reroll          - reroll shop (1g)
  lock            - toggle shop lock
  upgrade         - upgrade shop level
  bench           - show bench
  heroes          - show heroes + equipped abilities
  equip <a> <h>   - equip ability to hero
  unequip <a> <h> - unequip ability (1g)
  draft <1|2|3>   - pick hero from draft
  reroll-hero <h> - discard hero, get 3 new choices (2g)
  position <h> <x> <y> - set hero position
  god             - show god info
  buff <hero>     - set paladin buff target
  log             - show last combat log
  status          - show gold, HP, round
  board           - show hero positions
  players         - show all players HP
  help            - show this help
");
}

fn display_combat_log(log: &Option<(u32, u8, Vec<aa2_sim::CombatEvent>)>) {
    let Some((round, opponent, events)) = log else {
        println!("  No combat log available yet.");
        return;
    };
    println!("\n=== COMBAT LOG (Round {}: You vs Player {}) ===", round, opponent);
    if events.is_empty() {
        println!("  (no events)");
        return;
    }
    let mut last_snapshot_tick: u32 = 0;
    for event in events {
        match event {
            aa2_sim::CombatEvent::Attack { tick, attacker_id, target_id, damage } => {
                println!("  [{:.1}s] Unit {} attacks Unit {} for {:.0} damage",
                    *tick as f32 / 30.0, attacker_id, target_id, damage);
            }
            aa2_sim::CombatEvent::ProjectileHit { tick, target_id, damage } => {
                println!("  [{:.1}s] Projectile hits Unit {} for {:.0} damage",
                    *tick as f32 / 30.0, target_id, damage);
            }
            aa2_sim::CombatEvent::Death { tick, unit_id } => {
                println!("  [{:.1}s] Unit {} dies", *tick as f32 / 30.0, unit_id);
            }
            aa2_sim::CombatEvent::CastStart { tick, caster_id, ability_name } => {
                println!("  [{:.1}s] Unit {} begins casting {}",
                    *tick as f32 / 30.0, caster_id, ability_name);
            }
            aa2_sim::CombatEvent::CastComplete { tick, caster_id, ability_name } => {
                println!("  [{:.1}s] Unit {} casts {}",
                    *tick as f32 / 30.0, caster_id, ability_name);
            }
            aa2_sim::CombatEvent::AbilityDamage { tick, caster_id, target_id, ability_name, damage, .. } => {
                println!("  [{:.1}s] {} (Unit {}) hits Unit {} for {:.0} damage",
                    *tick as f32 / 30.0, ability_name, caster_id, target_id, damage);
            }
            aa2_sim::CombatEvent::Heal { tick, target_id, amount } => {
                println!("  [{:.1}s] Unit {} healed for {:.0}",
                    *tick as f32 / 30.0, target_id, amount);
            }
            aa2_sim::CombatEvent::RoundEnd { tick, winning_team } => {
                println!("  [{:.1}s] Combat ends — Team {} wins",
                    *tick as f32 / 30.0, winning_team);
            }
            // Skip: ProjectileSpawn, BuffApplied, BuffExpired, DarkPactPulse, WaveHit
            _ => {
                // Periodic snapshot every 150 ticks (~5s)
                let tick_val = match event {
                    aa2_sim::CombatEvent::BuffApplied { tick, .. }
                    | aa2_sim::CombatEvent::BuffExpired { tick, .. }
                    | aa2_sim::CombatEvent::ProjectileSpawn { tick, .. }
                    | aa2_sim::CombatEvent::DarkPactPulse { tick, .. }
                    | aa2_sim::CombatEvent::WaveHit { tick, .. } => *tick,
                    _ => 0,
                };
                let _ = (tick_val, &mut last_snapshot_tick);
            }
        }
    }
}

// --- Action Handlers ---

fn handle_buy(game: &mut GameState, player_id: usize, index: usize, ultimates: &HashSet<String>) {
    if index == 0 || index > game.players[player_id].shop.offerings.len() {
        println!("  Invalid shop index");
        return;
    }
    let name = match &game.players[player_id].shop.offerings[index - 1] {
        Some(n) => n.clone(),
        None => {
            println!("  That slot is empty");
            return;
        }
    };
    // Check if ultimate is locked
    if ultimates.contains(&name) && game.players[player_id].shop.level < game.config.ultimate_unlock_level {
        println!("  Ultimates require shop level {}", game.config.ultimate_unlock_level);
        return;
    }
    match game.players[player_id].buy_ability(&name, &mut game.pool) {
        Ok(()) => {
            // Mark slot as sold
            game.players[player_id].shop.offerings[index - 1] = None;
            let lv = game.players[player_id].abilities.get(&name).copied().unwrap_or(1);
            println!("  Bought {} (now Lv {}) | Gold: {}", name, lv, game.players[player_id].gold);
            println!("  Ability on bench. Use 'equip <ability> <hero>' to equip.");
        }
        Err(e) => println!("  Cannot buy: {e}"),
    }
}

fn handle_sell(game: &mut GameState, player_id: usize, name: &str) {
    let input_slug = slug(name);
    let actual_name = game.players[player_id].abilities.keys()
        .find(|k| slug(k) == input_slug)
        .cloned();
    match actual_name {
        Some(n) => {
            match game.players[player_id].sell_ability(&n, &mut game.pool) {
                Ok(()) => println!("  Sold {} | Gold: {}", n, game.players[player_id].gold),
                Err(e) => println!("  Cannot sell: {e}"),
            }
        }
        None => println!("  Ability not owned: {}", name),
    }
}

fn handle_equip(game: &mut GameState, player_id: usize, ability: &str, hero: &str, ultimates: &HashSet<String>) {
    let ability_slug = slug(ability);
    let hero_slug = slug(hero);
    let actual_ability = game.players[player_id].bench.iter()
        .find(|a| slug(a) == ability_slug)
        .cloned();
    let actual_hero = game.players[player_id].heroes.iter()
        .find(|h| slug(h) == hero_slug)
        .cloned();

    match (actual_ability, actual_hero) {
        (Some(a), Some(h)) => {
            match game.players[player_id].equip_ability(&a, &h, ultimates, &game.config) {
                Ok(()) => println!("  Equipped {} on {}", a, h),
                Err(e) => println!("  Cannot equip: {e}"),
            }
        }
        (None, _) => println!("  Ability not on bench: {}", ability),
        (_, None) => println!("  Hero not owned: {}", hero),
    }
}

fn handle_draft(game: &mut GameState, drafts: &mut [Option<DraftState>], player_id: usize, index: usize, hero_defs: &HashMap<String, HeroDef>) {
    if !game.draft_pending || drafts[player_id].is_none() {
        println!("  No draft active this round");
        return;
    }
    if game.players[player_id].heroes.len() >= MAX_HEROES {
        println!("  Already at max heroes ({})", MAX_HEROES);
        return;
    }
    let draft = match &drafts[player_id] {
        Some(d) => d.clone(),
        None => { println!("  No draft available"); return; }
    };
    if !(1..=3).contains(&index) {
        println!("  Pick 1, 2, or 3");
        return;
    }
    match &draft.choices[index - 1] {
        Some(name) => {
            game.players[player_id].heroes.push(name.clone());
            // Set default position: center of player's half
            game.players[player_id].hero_positions.insert(
                name.clone(), (1000.0, 500.0)
            );
            println!("  Drafted {}!", name);
            if let Some(h) = hero_defs.get(name) {
                println!("    {:?} | Tier {} | {} | Range {:.0}",
                    h.primary_attribute, h.tier,
                    if h.is_melee { "Melee" } else { "Ranged" },
                    h.attack_range);
            }
            println!("  Use 'equip <ability> <hero>' to equip abilities");
            drafts[player_id] = None;
        }
        None => println!("  No hero available at that slot"),
    }
}

// --- AI Logic ---

fn ai_take_actions(
    game: &mut GameState,
    drafts: &mut [Option<DraftState>],
    _hero_defs: &HashMap<String, HeroDef>,
    ultimates: &HashSet<String>,
    rng: &mut StdRng,
) {
    #[allow(clippy::needless_range_loop)]
    for i in 1..8 {
        if !game.players[i].alive { continue; }

        // Draft: pick random available choice
        if game.draft_pending && game.players[i].heroes.len() < MAX_HEROES
            && let Some(ref draft) = drafts[i].clone()
        {
            let valid: Vec<usize> = draft.choices.iter().enumerate()
                .filter_map(|(idx, c)| c.as_ref().map(|_| idx))
                .collect();
            if let Some(&pick) = valid.choose(rng)
                && let Some(ref name) = draft.choices[pick]
            {
                game.players[i].heroes.push(name.clone());
                game.players[i].hero_positions.insert(
                    name.clone(), (1000.0, 500.0)
                );
            }
            drafts[i] = None;
        }

        // Buy: purchase random affordable abilities until can't
        let mut attempts = 0;
        while game.players[i].can_buy() && game.players[i].shop.offerings.iter().any(|s| s.is_some()) && attempts < 10 {
            attempts += 1;
            let shop_len = game.players[i].shop.offerings.len();
            let idx = rng.gen_range(0..shop_len);
            let name = match &game.players[i].shop.offerings[idx] {
                Some(n) => n.clone(),
                None => continue,
            };

            // Check bench space for new abilities
            let already_owned = game.players[i].abilities.contains_key(&name);
            if !already_owned && game.players[i].bench.len() >= aa2_game::player::MAX_BENCH {
                break;
            }

            if game.players[i].buy_ability(&name, &mut game.pool).is_ok() {
                game.players[i].shop.offerings[idx] = None;
            }
        }

        // Equip: fill empty hero slots from bench
        let heroes_clone = game.players[i].heroes.clone();
        for hero_name in &heroes_clone {
            let equipped_count = game.players[i].equipped.get(hero_name).map(|v| v.len()).unwrap_or(0);
            let slots = game.config.ability_slots_per_hero as usize;
            if equipped_count >= slots { continue; }
            if game.players[i].bench.is_empty() { break; }

            let bench_clone = game.players[i].bench.clone();
            for ability_name in &bench_clone {
                let equipped_count = game.players[i].equipped.get(hero_name).map(|v| v.len()).unwrap_or(0);
                if equipped_count >= slots { break; }
                // Try equip (ignore errors silently)
                let _ = game.players[i].equip_ability(ability_name, hero_name, ultimates, &game.config);
            }
        }
    }
}

