//! Deterministic game scenario test framework.
//!
//! Provides a declarative way to define and replay game scenarios with
//! fixed seeds, scripted actions, and assertions checked after specific rounds.

use std::collections::{HashMap, HashSet};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::god::God;
use crate::pool::AbilityPool;
use crate::{GameConfig, GameState};

/// A complete test scenario with deterministic replay.
pub struct GameScenario {
    /// RNG seed for all randomness.
    pub seed: u64,
    /// Number of active players (2-8).
    pub num_players: u8,
    /// Initial setup applied before round 1.
    pub setup: Vec<SetupAction>,
    /// Scripted player actions keyed by (round, player).
    pub actions: Vec<RoundActions>,
    /// Assertions checked after specific rounds.
    pub assertions: Vec<RoundAssertion>,
}

/// Actions applied during game setup (before round 1).
#[derive(Debug, Clone)]
pub enum SetupAction {
    /// Give a player a hero at a position.
    AddHero { player: u8, hero: String, x: f32, y: f32 },
    /// Give a player an ability at a specific level.
    AddAbility { player: u8, ability: String, level: u32 },
    /// Equip an ability on a hero (must already be added).
    Equip { player: u8, ability: String, hero: String },
    /// Set a player's god.
    SetGod { player: u8, god: God },
    /// Set a player's gold.
    SetGold { player: u8, gold: u32 },
    /// Set a player's shop level.
    SetShopLevel { player: u8, level: u32 },
    /// Set a player's alive status.
    SetAlive { player: u8, alive: bool },
}

/// Actions for a specific round and player.
pub struct RoundActions {
    /// Which round these actions apply to.
    pub round: u32,
    /// Which player performs these actions.
    pub player: u8,
    /// The actions to execute in order.
    pub actions: Vec<Action>,
}

/// A player action during the shop phase.
#[derive(Debug, Clone)]
pub enum Action {
    /// Buy from shop slot (0-indexed).
    Buy(usize),
    /// Sell ability by name.
    Sell(String),
    /// Equip ability to hero.
    Equip(String, String),
    /// Unequip ability from hero.
    Unequip(String, String),
    /// Reroll shop (costs gold).
    RerollShop,
    /// Upgrade shop level.
    UpgradeShop,
    /// Toggle shop lock.
    LockShop,
    /// Move hero to position.
    SetPosition(String, f32, f32),
    /// Set paladin god buff target.
    SetGodBuff(String),
    /// Select god during GodPick phase.
    PickGod(God),
    /// Pick hero by index (0-2) from draft choices.
    DraftHero(usize),
    /// Discard hero, get new draft choices.
    RerollHero(String),
    /// Swap two equipped ability slots on a hero (hero_name, slot_a, slot_b).
    SwapAbilities(String, usize, usize),
    /// Signal player is done with current phase.
    Ready,
}

/// An assertion checked after a specific round's combat.
pub struct RoundAssertion {
    /// Check this assertion after this round completes.
    pub after_round: u32,
    /// The check function. Returns Err(message) on failure.
    pub check: fn(&GameState) -> Result<(), String>,
}

/// Run a game scenario to completion (or until all assertions are checked).
///
/// Panics on assertion failure with a descriptive message including the round number.
pub fn run_scenario(
    scenario: GameScenario,
    hero_defs: &HashMap<String, aa2_data::HeroDef>,
    ability_defs: &HashMap<String, aa2_data::AbilityDef>,
) {
    let mut rng = StdRng::seed_from_u64(scenario.seed);

    // Build pool and ultimates from ability defs
    let ultimates: HashSet<String> = ability_defs
        .iter()
        .filter(|(_, def)| def.is_ultimate)
        .map(|(name, _)| name.clone())
        .collect();
    let pool_counts: HashMap<String, u32> = ability_defs
        .keys()
        .map(|n| (n.clone(), 20))
        .collect();
    let pool = AbilityPool::from_counts(pool_counts);
    let config = GameConfig {
        auto_advance: false,
        ..GameConfig::default()
    };
    let mut game = GameState::new(pool, ultimates.clone(), config);

    // Mark extra players as dead
    for i in scenario.num_players as usize..8 {
        game.players[i].alive = false;
    }

    // Apply setup actions
    for action in &scenario.setup {
        apply_setup(&mut game, action);
    }

    // Determine how many rounds to run
    let max_round = scenario
        .assertions
        .iter()
        .map(|a| a.after_round)
        .max()
        .unwrap_or(20);

    let mut round_seed: u32 = rng.r#gen();

    for round in 1..=max_round {
        game.round = round;
        game.start_round();

        // Re-apply gold overrides from setup (start_round resets gold)
        if round == 1 {
            for action in &scenario.setup {
                if let SetupAction::SetGold { player, gold } = action {
                    game.players[*player as usize].gold = *gold;
                }
            }
        }

        // Roll shops for alive players
        for i in 0..8 {
            if game.players[i].alive {
                game.players[i].shop.roll(
                    &mut game.pool,
                    &ultimates,
                    game.config.ultimate_unlock_level,
                    game.config.shop_size_bonus,
                    &mut rng,
                );
            }
        }

        // Execute scripted actions for this round
        let round_actions: Vec<&RoundActions> = scenario
            .actions
            .iter()
            .filter(|ra| ra.round == round)
            .collect();
        for ra in &round_actions {
            for action in &ra.actions {
                execute_action(&mut game, ra.player, action, &mut rng, hero_defs);
            }
        }

        // Simple AI for unscripted alive players
        let scripted_players: HashSet<u8> = round_actions.iter().map(|ra| ra.player).collect();
        for i in 0..scenario.num_players {
            if !scripted_players.contains(&i) && game.players[i as usize].alive {
                ai_actions(&mut game, i as usize, &ultimates, &mut rng);
            }
        }

        // Run combat
        let _results = game.run_combat_round(hero_defs, ability_defs, round_seed, &mut rng);
        round_seed = round_seed.wrapping_add(1);

        // Check assertions for this round
        for assertion in &scenario.assertions {
            if assertion.after_round == round
                && let Err(msg) = (assertion.check)(&game)
            {
                panic!("Assertion failed after round {round}: {msg}");
            }
        }

        // Check if game is over
        if game.alive_count() <= 1 {
            break;
        }
    }
}

fn apply_setup(game: &mut GameState, action: &SetupAction) {
    match action {
        SetupAction::AddHero { player, hero, x, y } => {
            let p = &mut game.players[*player as usize];
            p.heroes.push(hero.clone());
            p.equipped.entry(hero.clone()).or_default();
            p.hero_positions.insert(hero.clone(), (*x, *y));
        }
        SetupAction::AddAbility { player, ability, level } => {
            let p = &mut game.players[*player as usize];
            p.abilities.insert(ability.clone(), *level);
            if !p.bench.contains(ability) {
                p.bench.push(ability.clone());
            }
        }
        SetupAction::Equip { player, ability, hero } => {
            let p = &mut game.players[*player as usize];
            p.bench.retain(|a| a != ability);
            p.equipped.entry(hero.clone()).or_default().push(ability.clone());
        }
        SetupAction::SetGod { player, god } => {
            game.players[*player as usize].god = Some(god.clone());
        }
        SetupAction::SetGold { player, gold } => {
            game.players[*player as usize].gold = *gold;
        }
        SetupAction::SetShopLevel { player, level } => {
            game.players[*player as usize].shop.level = *level;
        }
        SetupAction::SetAlive { player, alive } => {
            game.players[*player as usize].alive = *alive;
        }
    }
}

/// Parse a string-based action (from FFI or network) into a typed Action.
///
/// This is the single source of truth for action_type/param string decoding.
/// The `gods` slice is used to resolve god names for PickGod.
pub fn parse_action(action_type: &str, param: &str, gods: &[aa2_data::God]) -> Result<Action, String> {
    match action_type {
        "Buy" => {
            let slot: usize = param.parse().unwrap_or(0);
            Ok(Action::Buy(slot))
        }
        "Sell" => Ok(Action::Sell(param.to_string())),
        "RerollShop" => Ok(Action::RerollShop),
        "UpgradeShop" => Ok(Action::UpgradeShop),
        "LockShop" => Ok(Action::LockShop),
        "SetPosition" => {
            let parts: Vec<&str> = param.splitn(3, ',').collect();
            if parts.len() != 3 { return Err("bad params".to_string()); }
            let name = parts[0].to_string();
            let x: f32 = parts[1].parse().unwrap_or(1000.0);
            let y: f32 = parts[2].parse().unwrap_or(500.0);
            Ok(Action::SetPosition(name, x, y))
        }
        "Equip" => {
            let parts: Vec<&str> = param.splitn(2, ',').collect();
            if parts.len() != 2 { return Err("bad params".to_string()); }
            Ok(Action::Equip(parts[0].to_string(), parts[1].to_string()))
        }
        "Unequip" => {
            let parts: Vec<&str> = param.splitn(2, ',').collect();
            if parts.len() != 2 { return Err("bad params".to_string()); }
            Ok(Action::Unequip(parts[0].to_string(), parts[1].to_string()))
        }
        "PickGod" => {
            match gods.iter().find(|g| g.name == param).cloned() {
                Some(god) => Ok(Action::PickGod(god)),
                None => Err("unknown god".to_string()),
            }
        }
        "SwapAbilities" => {
            // param: "hero_name,slot_a,slot_b"
            let parts: Vec<&str> = param.splitn(3, ',').collect();
            if parts.len() != 3 { return Err("invalid params".to_string()); }
            let hero_name = parts[0].to_string();
            let slot_a: usize = parts[1].parse().unwrap_or(0);
            let slot_b: usize = parts[2].parse().unwrap_or(0);
            Ok(Action::SwapAbilities(hero_name, slot_a, slot_b))
        }
        "DraftHero" => {
            let idx: usize = param.parse().unwrap_or(0);
            Ok(Action::DraftHero(idx))
        }
        "RerollHero" => {
            Ok(Action::RerollHero(param.to_string()))
        }
        "Ready" => {
            Ok(Action::Ready)
        }
        _ => Err(format!("unknown action: {action_type}")),
    }
}

fn execute_action(
    game: &mut GameState,
    player: u8,
    action: &Action,
    rng: &mut impl Rng,
    hero_defs: &HashMap<String, aa2_data::HeroDef>,
) {
    match action {
        // These are handled by GameState::apply_action in the game loop, not in scenarios.
        Action::PickGod(_) | Action::DraftHero(_) | Action::RerollHero(_) | Action::SwapAbilities(..) | Action::Ready => {}
        _ => {
            let _ = game.apply_action(player, action.clone(), hero_defs, rng);
        }
    }
}

fn ai_actions(game: &mut GameState, player_idx: usize, ultimates: &HashSet<String>, rng: &mut impl Rng) {
    let bench_space = 5usize.saturating_sub(game.players[player_idx].bench.len());
    let mut buys = 0;
    while game.players[player_idx].gold >= 3 && buys < bench_space {
        let offerings: Vec<usize> = game.players[player_idx]
            .shop
            .offerings
            .iter()
            .enumerate()
            .filter_map(|(i, o)| o.as_ref().map(|_| i))
            .collect();
        if offerings.is_empty() {
            break;
        }
        let slot = offerings[rng.gen_range(0..offerings.len())];
        if let Some(Some(name)) = game.players[player_idx].shop.offerings.get(slot).cloned() {
            let bench_cap = game.config.bench_capacity as usize;
            let _ = game.players[player_idx].buy_ability(&name, &mut game.pool, bench_cap);
            game.players[player_idx].shop.offerings[slot] = None;
        }
        buys += 1;
    }

    // Equip bench abilities to heroes with empty slots
    let bench_clone: Vec<String> = game.players[player_idx].bench.clone();
    for ability in bench_clone {
        let heroes: Vec<String> = game.players[player_idx].heroes.clone();
        for hero in &heroes {
            let equipped_count = game.players[player_idx]
                .equipped
                .get(hero)
                .map(|v| v.len())
                .unwrap_or(0);
            if equipped_count < game.config.ability_slots_per_hero as usize {
                let _ = game.players[player_idx].equip_ability(&ability, hero, ultimates, &game.config);
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aa2_data::{God, GodPassive};

    fn test_gods() -> Vec<God> {
        vec![God {
            name: "Mars".to_string(),
            description: "God of War".to_string(),
            passive: GodPassive::Sorcery { trigger_chance: 0.5 },
        }]
    }

    #[test]
    fn parse_action_buy() {
        let gods = test_gods();
        let result = parse_action("Buy", "2", &gods).unwrap();
        assert!(matches!(result, Action::Buy(2)));
    }

    #[test]
    fn parse_action_sell() {
        let gods = test_gods();
        let result = parse_action("Sell", "Fireball", &gods).unwrap();
        assert!(matches!(result, Action::Sell(ref s) if s == "Fireball"));
    }

    #[test]
    fn parse_action_reroll_shop() {
        let gods = test_gods();
        let result = parse_action("RerollShop", "", &gods).unwrap();
        assert!(matches!(result, Action::RerollShop));
    }

    #[test]
    fn parse_action_upgrade_shop() {
        let gods = test_gods();
        let result = parse_action("UpgradeShop", "", &gods).unwrap();
        assert!(matches!(result, Action::UpgradeShop));
    }

    #[test]
    fn parse_action_lock_shop() {
        let gods = test_gods();
        let result = parse_action("LockShop", "", &gods).unwrap();
        assert!(matches!(result, Action::LockShop));
    }

    #[test]
    fn parse_action_set_position() {
        let gods = test_gods();
        let result = parse_action("SetPosition", "Sven,100.0,200.0", &gods).unwrap();
        assert!(matches!(result, Action::SetPosition(ref n, x, y) if n == "Sven" && x == 100.0 && y == 200.0));
    }

    #[test]
    fn parse_action_equip() {
        let gods = test_gods();
        let result = parse_action("Equip", "Fireball,Sven", &gods).unwrap();
        assert!(matches!(result, Action::Equip(ref a, ref h) if a == "Fireball" && h == "Sven"));
    }

    #[test]
    fn parse_action_unequip() {
        let gods = test_gods();
        let result = parse_action("Unequip", "Fireball,Sven", &gods).unwrap();
        assert!(matches!(result, Action::Unequip(ref a, ref h) if a == "Fireball" && h == "Sven"));
    }

    #[test]
    fn parse_action_pick_god() {
        let gods = test_gods();
        let result = parse_action("PickGod", "Mars", &gods).unwrap();
        assert!(matches!(result, Action::PickGod(ref g) if g.name == "Mars"));
    }

    #[test]
    fn parse_action_swap_abilities() {
        let gods = test_gods();
        let result = parse_action("SwapAbilities", "Sven,0,1", &gods).unwrap();
        assert!(matches!(result, Action::SwapAbilities(ref h, 0, 1) if h == "Sven"));
    }

    #[test]
    fn parse_action_draft_hero() {
        let gods = test_gods();
        let result = parse_action("DraftHero", "1", &gods).unwrap();
        assert!(matches!(result, Action::DraftHero(1)));
    }

    #[test]
    fn parse_action_reroll_hero() {
        let gods = test_gods();
        let result = parse_action("RerollHero", "0", &gods).unwrap();
        assert!(matches!(result, Action::RerollHero(ref s) if s == "0"));
    }

    #[test]
    fn parse_action_ready() {
        let gods = test_gods();
        let result = parse_action("Ready", "", &gods).unwrap();
        assert!(matches!(result, Action::Ready));
    }

    #[test]
    fn parse_action_unknown_type() {
        let gods = test_gods();
        let result = parse_action("Explode", "", &gods);
        assert_eq!(result.unwrap_err(), "unknown action: Explode");
    }

    #[test]
    fn parse_action_unknown_god() {
        let gods = test_gods();
        let result = parse_action("PickGod", "Zeus", &gods);
        assert_eq!(result.unwrap_err(), "unknown god");
    }

    #[test]
    fn parse_action_set_position_bad_params() {
        let gods = test_gods();
        let result = parse_action("SetPosition", "Sven,100.0", &gods);
        assert_eq!(result.unwrap_err(), "bad params");
    }

    #[test]
    fn parse_action_equip_bad_params() {
        let gods = test_gods();
        let result = parse_action("Equip", "Fireball", &gods);
        assert_eq!(result.unwrap_err(), "bad params");
    }

    #[test]
    fn parse_action_swap_abilities_bad_params() {
        let gods = test_gods();
        let result = parse_action("SwapAbilities", "Sven,0", &gods);
        assert_eq!(result.unwrap_err(), "invalid params");
    }
}
