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

    let mut rng = StdRng::from_entropy();

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

    println!("\n=== GAME START ===\n");

    loop {
        if game.alive_count() <= 1 {
            break;
        }

        // Combat phase (skip round 1)
        if game.round > 1 {
            game.end_shop();
            println!("\n=== ROUND {} | COMBAT ===", game.round);

            let prev_alive: Vec<u8> = game.players.iter().filter(|p| p.alive).map(|p| p.id).collect();
            let results = game.run_combat_round(&hero_defs, &ability_defs, round_seed, &mut rng);
            round_seed = round_seed.wrapping_add(1);

            display_combat_results(&results, &game);

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

            // Grace period → new shop phase
            game.end_combat(false);
            game.end_grace_period(&mut rng);

            // Roll shops for players that need it
            for player in &mut game.players {
                if player.alive && player.shop.needs_reroll {
                    player.shop.roll(&mut game.pool, &ultimates, game.config.ultimate_unlock_level, game.config.shop_size_bonus, &mut rng);
                    player.shop.needs_reroll = false;
                }
            }

            // Generate draft if needed
            if game.draft_pending {
                #[allow(clippy::needless_range_loop)]
                for i in 0..8 {
                    if game.players[i].alive {
                        let available = available_heroes_for_player(&heroes, &game.players[i]);
                        let tier = tier_for_draft_round(game.round).unwrap_or(0);
                        let choices = generate_draft_choices(&available, tier, &mut rng);
                        drafts[i] = Some(DraftState { choices, round_tier: tier });
                    }
                }
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

        display_shop(&game.players[0], &game);
        display_heroes(&game.players[0], &game);

        // Player command loop
        let stdin = io::stdin();
        loop {
            print!("> ");
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
                "heroes" => display_heroes(&game.players[0], &game),
                "bench" => display_bench(&game.players[0]),
                "board" => display_board(&game.players[0]),
                "god" => display_god(&game.players[0]),
                "players" => display_players(&game),
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
                            println!("  Sorcery! {} leveled up!", name);
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
                        let ability = parts[1].to_string();
                        let hero = parts[2..].join("_");
                        match game.players[0].unequip_ability(&ability, &hero) {
                            Ok(()) => println!("  Unequipped {} from {}", ability, hero),
                            Err(e) => println!("  Cannot unequip: {e}"),
                        }
                    }
                }
                "draft" => {
                    if parts.len() < 2 {
                        println!("  Usage: draft <1|2|3>");
                    } else if let Ok(idx) = parts[1].parse::<usize>() {
                        handle_draft(&mut game, &mut drafts, 0, idx, &hero_defs);
                    } else {
                        println!("  Invalid index");
                    }
                }
                "reroll-hero" => {
                    if game.players[0].gold < HERO_REROLL_COST {
                        println!("  Not enough gold (need {}g)", HERO_REROLL_COST);
                    } else {
                        let available = available_heroes_for_player(&heroes, &game.players[0]);
                        match game.players[0].reroll_draft(&available, &mut rng) {
                            Ok(new_draft) => {
                                println!("  Rerolled draft! (-{}g)", HERO_REROLL_COST);
                                drafts[0] = Some(new_draft);
                                if let Some(ref d) = drafts[0] {
                                    display_draft(d, &hero_defs);
                                }
                            }
                            Err(e) => println!("  Cannot reroll: {e}"),
                        }
                    }
                }
                "position" => {
                    if parts.len() < 4 {
                        println!("  Usage: position <hero> <x> <y>");
                    } else {
                        let hero = parts[1].to_string();
                        if let (Ok(x), Ok(y)) = (parts[2].parse::<f32>(), parts[3].parse::<f32>()) {
                            if game.players[0].heroes.contains(&hero) {
                                game.players[0].hero_positions.insert(hero.clone(), (x, y));
                                println!("  {} positioned at ({}, {})", hero, x, y);
                            } else {
                                println!("  Hero not owned: {}", hero);
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
                        let hero = parts[1..].join("_");
                        if game.players[0].heroes.contains(&hero) {
                            game.players[0].god_buff_target = Some(hero.clone());
                            println!("  God buff target set to: {}", hero);
                        } else {
                            println!("  Hero not owned: {}", hero);
                        }
                    }
                }
                _ => println!("  Unknown command. Type 'help' for commands."),
            }
        }

        // AI takes actions
        ai_take_actions(&mut game, &mut drafts, &hero_defs, &ultimates, &mut rng);

        // After round 1 (no combat), manually advance to round 2
        if game.round == 1 {
            game.round = 1; // end_grace_period will increment to 2
            game.end_shop();
            game.end_combat(false);
            game.end_grace_period(&mut rng);
            // Roll shops
            for player in &mut game.players {
                if player.alive && player.shop.needs_reroll {
                    player.shop.roll(&mut game.pool, &ultimates, game.config.ultimate_unlock_level, game.config.shop_size_bonus, &mut rng);
                    player.shop.needs_reroll = false;
                }
            }
            if game.draft_pending {
                #[allow(clippy::needless_range_loop)]
                for i in 0..8 {
                    if game.players[i].alive {
                        let available = available_heroes_for_player(&heroes, &game.players[i]);
                        let tier = tier_for_draft_round(game.round).unwrap_or(0);
                        let choices = generate_draft_choices(&available, tier, &mut rng);
                        drafts[i] = Some(DraftState { choices, round_tier: tier });
                    }
                }
            }
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
    for (i, name) in shop.offerings.iter().enumerate() {
        let level = player.abilities.get(name).map(|l| format!(" <- you own Lv {}", l)).unwrap_or_default();
        let ult = if game.ultimates.contains(name) { " [ULT]" } else { "" };
        println!("  {}. {}{}{}", i + 1, name, ult, level);
    }
    if let Some(cost) = shop.upgrade_cost() {
        println!("  [Upgrade to Lv {} costs {}g]", shop.level + 1, cost);
    }
    println!("  Lock: {} | Gold: {} | Buy: {}g | Reroll: {}g",
        if shop.locked { "ON" } else { "OFF" },
        player.gold, BUY_COST,
        game.config.reroll_cost_override.unwrap_or(REROLL_COST));
}

fn display_heroes(player: &PlayerState, game: &GameState) {
    println!("\n--- HEROES ({}/{}) ---", player.heroes.len(), MAX_HEROES);
    if player.heroes.is_empty() {
        println!("  (none - pick from draft!)");
        return;
    }
    for hero_name in &player.heroes {
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
        println!("  {} - {}", hero_name, ability_strs.join(", "));
    }
}

fn display_bench(player: &PlayerState) {
    println!("\n--- BENCH ({}/{}) ---", player.bench.len(), aa2_game::player::MAX_BENCH);
    if player.bench.is_empty() {
        println!("  (empty)");
    } else {
        for name in &player.bench {
            let lv = player.abilities.get(name).copied().unwrap_or(1);
            println!("  {} (Lv {})", name, lv);
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

fn display_draft(draft: &DraftState, hero_defs: &HashMap<String, HeroDef>) {
    println!("\n--- DRAFT (pick a hero) ---");
    let labels = ["STR", "AGI", "INT"];
    for (i, choice) in draft.choices.iter().enumerate() {
        match choice {
            Some(name) => {
                let tier = hero_defs.get(name).map(|h| h.tier).unwrap_or(0);
                println!("  {}. {} [{}] (Tier {})", i + 1, name, labels[i], tier);
            }
            None => println!("  {}. (none available)", i + 1),
        }
    }
    println!("  Reroll cost: {}g", HERO_REROLL_COST);
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
  reroll-hero     - reroll draft choices (2g)
  position <h> <x> <y> - set hero position
  god             - show god info
  buff <hero>     - set paladin buff target
  status          - show gold, HP, round
  board           - show hero positions
  players         - show all players HP
  help            - show this help
");
}

// --- Action Handlers ---

fn handle_buy(game: &mut GameState, player_id: usize, index: usize, ultimates: &HashSet<String>) {
    if index == 0 || index > game.players[player_id].shop.offerings.len() {
        println!("  Invalid shop index");
        return;
    }
    let name = game.players[player_id].shop.offerings[index - 1].clone();
    // Check if ultimate is locked
    if ultimates.contains(&name) && game.players[player_id].shop.level < game.config.ultimate_unlock_level {
        println!("  Ultimates require shop level {}", game.config.ultimate_unlock_level);
        return;
    }
    match game.players[player_id].buy_ability(&name, &mut game.pool) {
        Ok(()) => {
            // Remove from shop offerings (already taken from pool by buy_ability... but shop still shows it)
            // Actually buy_ability takes from pool, but shop.offerings still has it. Remove it.
            if let Some(pos) = game.players[player_id].shop.offerings.iter().position(|n| n == &name) {
                game.players[player_id].shop.offerings.remove(pos);
            }
            let lv = game.players[player_id].abilities.get(&name).copied().unwrap_or(1);
            println!("  Bought {} (now Lv {}) | Gold: {}", name, lv, game.players[player_id].gold);
        }
        Err(e) => println!("  Cannot buy: {e}"),
    }
}

fn handle_sell(game: &mut GameState, player_id: usize, name: &str) {
    // Try exact match first, then case-insensitive
    let actual_name = game.players[player_id].abilities.keys()
        .find(|k| k.as_str() == name || k.to_lowercase() == name.to_lowercase())
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
    // Find actual ability name (case-insensitive)
    let actual_ability = game.players[player_id].bench.iter()
        .find(|a| a.as_str() == ability || a.to_lowercase() == ability.to_lowercase())
        .cloned();
    let actual_hero = game.players[player_id].heroes.iter()
        .find(|h| h.as_str() == hero || h.to_lowercase() == hero.to_lowercase())
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
    if !game.draft_pending {
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
            // Set default position
            let count = game.players[player_id].heroes.len() as f32;
            game.players[player_id].hero_positions.insert(
                name.clone(), (400.0 * count, 500.0)
            );
            println!("  Drafted {}!", name);
            if let Some(h) = hero_defs.get(name) {
                println!("    {:?} | Tier {} | {} | Range {:.0}",
                    h.primary_attribute, h.tier,
                    if h.is_melee { "Melee" } else { "Ranged" },
                    h.attack_range);
            }
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
                let count = game.players[i].heroes.len() as f32;
                game.players[i].hero_positions.insert(
                    name.clone(), (400.0 * count, 500.0)
                );
            }
            drafts[i] = None;
        }

        // Buy: purchase random affordable abilities until can't
        let mut attempts = 0;
        while game.players[i].can_buy() && !game.players[i].shop.offerings.is_empty() && attempts < 10 {
            attempts += 1;
            let shop_len = game.players[i].shop.offerings.len();
            let idx = rng.gen_range(0..shop_len);
            let name = game.players[i].shop.offerings[idx].clone();

            // Check bench space for new abilities
            let already_owned = game.players[i].abilities.contains_key(&name);
            if !already_owned && game.players[i].bench.len() >= aa2_game::player::MAX_BENCH {
                break;
            }

            if game.players[i].buy_ability(&name, &mut game.pool).is_ok() {
                game.players[i].shop.offerings.retain(|n| n != &name);
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

