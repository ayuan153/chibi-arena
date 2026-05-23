//! C FFI layer for aa2 game engine.
//! Thin translation layer — no game logic here.

use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString, c_char};
use std::panic::catch_unwind;
use std::path::Path;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use aa2_data::{AbilityDef, HeroDef};
use aa2_game::combat::CombatResult;
use aa2_game::pool::AbilityPool;
use aa2_game::scenario::Action;
use aa2_game::{GameConfig, GameState};

/// Opaque game context holding all state.
pub struct GameContext {
    game: GameState,
    hero_defs: HashMap<String, HeroDef>,
    ability_defs: HashMap<String, AbilityDef>,
    rng: StdRng,
    last_combat_results: Vec<CombatResult>,
}

/// Allocate a JSON string and return its pointer. Caller must free with `aa2_free_string`.
fn return_string(s: String) -> *const c_char {
    match CString::new(s) {
        Ok(cstr) => cstr.into_raw() as *const c_char,
        Err(_) => std::ptr::null(),
    }
}

/// Serialize a serde value to JSON and return as caller-owned C string.
fn return_json(value: &impl serde::Serialize) -> *const c_char {
    let json = serde_json::to_string(value).unwrap_or_else(|_| "null".to_string());
    return_string(json)
}

fn error_json(msg: &str) -> *const c_char {
    let json = serde_json::json!({"error": msg}).to_string();
    return_string(json)
}

fn ok_json() -> *const c_char {
    return_string(r#"{"ok":true}"#.to_string())
}

/// Parse a C string pointer to a Rust &str. Returns None on null/invalid UTF-8.
unsafe fn parse_cstr<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

/// Load hero defs from a directory of .ron files.
fn load_hero_defs(dir: &Path) -> Result<HashMap<String, HeroDef>, String> {
    let mut map = HashMap::new();
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{dir:?}: {e}"))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|ext| ext == "ron") {
            let def = aa2_data::load_hero_def(&path)?;
            map.insert(def.name.clone(), def);
        }
    }
    Ok(map)
}

/// Load ability defs from a directory of .ron files.
fn load_ability_defs(dir: &Path) -> Result<HashMap<String, AbilityDef>, String> {
    let mut map = HashMap::new();
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{dir:?}: {e}"))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|ext| ext == "ron") {
            let def = aa2_data::load_ability_def(&path)?;
            map.insert(def.name.clone(), def);
        }
    }
    Ok(map)
}

/// Parse an action JSON object into an Action enum.
fn parse_action(json: &serde_json::Value) -> Result<Action, String> {
    let action_type = json.get("type").and_then(|v| v.as_str()).ok_or("missing 'type'")?;
    match action_type {
        "Buy" => {
            let slot = json.get("slot").and_then(|v| v.as_u64()).ok_or("missing 'slot'")? as usize;
            Ok(Action::Buy(slot))
        }
        "Sell" => {
            let hero = json.get("hero").and_then(|v| v.as_str()).ok_or("missing 'hero'")?;
            Ok(Action::Sell(hero.to_string()))
        }
        "Equip" => {
            let ability = json.get("ability").and_then(|v| v.as_str()).ok_or("missing 'ability'")?;
            let hero = json.get("hero").and_then(|v| v.as_str()).ok_or("missing 'hero'")?;
            Ok(Action::Equip(ability.to_string(), hero.to_string()))
        }
        "Unequip" => {
            let ability = json.get("ability").and_then(|v| v.as_str()).ok_or("missing 'ability'")?;
            let hero = json.get("hero").and_then(|v| v.as_str()).ok_or("missing 'hero'")?;
            Ok(Action::Unequip(ability.to_string(), hero.to_string()))
        }
        "RerollShop" => Ok(Action::RerollShop),
        "UpgradeShop" => Ok(Action::UpgradeShop),
        "LockShop" => Ok(Action::LockShop),
        "SetPosition" => {
            let hero = json.get("hero").and_then(|v| v.as_str()).ok_or("missing 'hero'")?;
            let x = json.get("x").and_then(|v| v.as_f64()).ok_or("missing 'x'")? as f32;
            let y = json.get("y").and_then(|v| v.as_f64()).ok_or("missing 'y'")? as f32;
            Ok(Action::SetPosition(hero.to_string(), x, y))
        }
        "SetGodBuff" => {
            let hero = json.get("hero").and_then(|v| v.as_str()).ok_or("missing 'hero'")?;
            Ok(Action::SetGodBuff(hero.to_string()))
        }
        _ => Err(format!("unknown action type: {action_type}")),
    }
}

/// Execute an action on the game state (mirrors scenario.rs dispatch).
fn execute_action(ctx: &mut GameContext, player_id: u8, action: &Action) -> Result<(), String> {
    let p_idx = player_id as usize;
    if p_idx >= ctx.game.players.len() {
        return Err("invalid player_id".to_string());
    }
    match action {
        Action::Buy(slot) => {
            let offering = ctx.game.players[p_idx]
                .shop
                .offerings
                .get(*slot)
                .cloned()
                .flatten();
            if let Some(name) = offering {
                ctx.game.players[p_idx]
                    .buy_ability(&name, &mut ctx.game.pool)
                    .map_err(|e| e.to_string())?;
                ctx.game.players[p_idx].shop.offerings[*slot] = None;
                Ok(())
            } else {
                Err("empty shop slot".to_string())
            }
        }
        Action::Sell(name) => ctx.game.players[p_idx]
            .sell_ability(name, &mut ctx.game.pool)
            .map_err(|e| e.to_string()),
        Action::Equip(ability, hero) => ctx.game.players[p_idx]
            .equip_ability(ability, hero, &ctx.game.ultimates, &ctx.game.config)
            .map_err(|e| e.to_string()),
        Action::Unequip(ability, hero) => ctx.game.players[p_idx]
            .unequip_ability(ability, hero)
            .map_err(|e| e.to_string()),
        Action::RerollShop => {
            let reroll_cost = ctx.game.config.reroll_cost_override
                .unwrap_or(aa2_game::economy::REROLL_COST);
            ctx.game.players[p_idx]
                .reroll_shop(
                    &mut ctx.game.pool,
                    &ctx.game.ultimates,
                    ctx.game.config.ultimate_unlock_level,
                    ctx.game.config.shop_size_bonus,
                    reroll_cost,
                    &mut ctx.rng,
                )
                .map_err(|e| e.to_string())
        }
        Action::UpgradeShop => {
            let p = &mut ctx.game.players[p_idx];
            p.shop.upgrade(&mut p.gold);
            Ok(())
        }
        Action::LockShop => {
            ctx.game.players[p_idx].shop.toggle_lock();
            Ok(())
        }
        Action::SetPosition(hero, x, y) => {
            ctx.game.players[p_idx]
                .hero_positions
                .insert(hero.clone(), (*x, *y));
            Ok(())
        }
        Action::SetGodBuff(hero) => {
            ctx.game.players[p_idx].god_buff_target = Some(hero.clone());
            Ok(())
        }
    }
}

/// Serialize CombatEvent to a JSON value (no Serialize derive available).
fn combat_event_to_json(event: &aa2_sim::CombatEvent) -> serde_json::Value {
    use aa2_sim::CombatEvent::*;
    match event {
        Attack { tick, attacker_id, target_id, damage } => serde_json::json!({
            "type": "Attack", "tick": tick, "attacker_id": attacker_id, "target_id": target_id, "damage": damage
        }),
        ProjectileSpawn { tick, attacker_id, target_id } => serde_json::json!({
            "type": "ProjectileSpawn", "tick": tick, "attacker_id": attacker_id, "target_id": target_id
        }),
        ProjectileHit { tick, target_id, damage } => serde_json::json!({
            "type": "ProjectileHit", "tick": tick, "target_id": target_id, "damage": damage
        }),
        Death { tick, unit_id } => serde_json::json!({
            "type": "Death", "tick": tick, "unit_id": unit_id
        }),
        RoundEnd { tick, winning_team } => serde_json::json!({
            "type": "RoundEnd", "tick": tick, "winning_team": winning_team
        }),
        BuffApplied { tick, target_id, name } => serde_json::json!({
            "type": "BuffApplied", "tick": tick, "target_id": target_id, "name": name
        }),
        BuffExpired { tick, target_id, name } => serde_json::json!({
            "type": "BuffExpired", "tick": tick, "target_id": target_id, "name": name
        }),
        CastStart { tick, caster_id, ability_name } => serde_json::json!({
            "type": "CastStart", "tick": tick, "caster_id": caster_id, "ability_name": ability_name
        }),
        CastComplete { tick, caster_id, ability_name } => serde_json::json!({
            "type": "CastComplete", "tick": tick, "caster_id": caster_id, "ability_name": ability_name
        }),
        AbilityDamage { tick, caster_id, target_id, ability_name, damage, damage_type } => serde_json::json!({
            "type": "AbilityDamage", "tick": tick, "caster_id": caster_id, "target_id": target_id,
            "ability_name": ability_name, "damage": damage, "damage_type": format!("{damage_type:?}")
        }),
        Heal { tick, target_id, amount } => serde_json::json!({
            "type": "Heal", "tick": tick, "target_id": target_id, "amount": amount
        }),
        DarkPactPulse { tick, caster_id, enemies_hit, self_damage } => serde_json::json!({
            "type": "DarkPactPulse", "tick": tick, "caster_id": caster_id, "enemies_hit": enemies_hit, "self_damage": self_damage
        }),
        WaveHit { tick, target_id, damage, stun_duration } => serde_json::json!({
            "type": "WaveHit", "tick": tick, "target_id": target_id, "damage": damage, "stun_duration": stun_duration
        }),
    }
}

// ─── Exported C API ───────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
/// # Safety
/// `config_json` must be a valid null-terminated C string or null.
pub unsafe extern "C" fn aa2_create_game(config_json: *const c_char) -> *mut GameContext {
    let result = catch_unwind(std::panic::AssertUnwindSafe(|| {
        let json_str = unsafe { parse_cstr(config_json) }?;
        let config: serde_json::Value = serde_json::from_str(json_str).ok()?;

        let seed = config.get("seed")?.as_u64()?;
        let num_players = config.get("num_players")?.as_u64()? as u8;
        let data_path = config.get("data_path")?.as_str()?;

        let data_dir = Path::new(data_path);
        let hero_defs = load_hero_defs(&data_dir.join("heroes")).ok()?;
        let ability_defs = load_ability_defs(&data_dir.join("abilities")).ok()?;

        let ultimates: HashSet<String> = ability_defs
            .iter()
            .filter(|(_, def)| def.is_ultimate)
            .map(|(name, _)| name.clone())
            .collect();
        let pool_counts: HashMap<String, u32> = ability_defs.keys().map(|n| (n.clone(), 20)).collect();
        let pool = AbilityPool::from_counts(pool_counts);

        let game_config = GameConfig {
            auto_advance: false,
            ..GameConfig::default()
        };
        let mut game = GameState::new(pool, ultimates, game_config);

        // Mark extra players as dead
        for i in num_players as usize..8 {
            game.players[i].alive = false;
        }

        let rng = StdRng::seed_from_u64(seed);

        Some(Box::into_raw(Box::new(GameContext {
            game,
            hero_defs,
            ability_defs,
            rng,
            last_combat_results: Vec::new(),
        })))
    }));
    result.unwrap_or(None).unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` must be a valid pointer from `aa2_create_game` or null.
pub unsafe extern "C" fn aa2_destroy_game(ctx: *mut GameContext) {
    if !ctx.is_null() {
        unsafe { drop(Box::from_raw(ctx)) };
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` must be a valid pointer from `aa2_create_game` or null.
pub unsafe extern "C" fn aa2_tick(ctx: *mut GameContext, dt: f32) -> *const c_char {
    if ctx.is_null() {
        return std::ptr::null();
    }
    let result = catch_unwind(std::panic::AssertUnwindSafe(|| {
        let ctx = unsafe { &mut *ctx };
        let events = ctx.game.tick(dt, &mut ctx.rng);
        return_json(&events)
    }));
    result.unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` must be a valid pointer from `aa2_create_game` or null.
/// `action_json` must be a valid null-terminated C string or null.
pub unsafe extern "C" fn aa2_player_action(
    ctx: *mut GameContext,
    player_id: u8,
    action_json: *const c_char,
) -> *const c_char {
    if ctx.is_null() {
        return std::ptr::null();
    }
    let result = catch_unwind(std::panic::AssertUnwindSafe(|| {
        let ctx = unsafe { &mut *ctx };
        let json_str = match unsafe { parse_cstr(action_json) } {
            Some(s) => s,
            None => return error_json("invalid action string"),
        };
        let value: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(e) => return error_json(&e.to_string()),
        };
        let action = match parse_action(&value) {
            Ok(a) => a,
            Err(e) => return error_json(&e),
        };
        match execute_action(ctx, player_id, &action) {
            Ok(()) => ok_json(),
            Err(e) => error_json(&e),
        }
    }));
    result.unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` must be a valid pointer from `aa2_create_game` or null.
pub unsafe extern "C" fn aa2_run_combat(ctx: *mut GameContext) -> *const c_char {
    if ctx.is_null() {
        return std::ptr::null();
    }
    let result = catch_unwind(std::panic::AssertUnwindSafe(|| {
        let ctx = unsafe { &mut *ctx };
        let GameContext { game, hero_defs, ability_defs, rng, last_combat_results, .. } = ctx;
        let seed: u32 = rng.r#gen();
        let results = game.run_combat_round(hero_defs, ability_defs, seed, rng);

        let summary: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "player_a": r.matchup.player_a,
                    "player_b": r.matchup.player_b,
                    "winner": r.winner,
                    "survivors_a": r.survivors_a,
                    "survivors_b": r.survivors_b,
                })
            })
            .collect();

        *last_combat_results = results;
        return_json(&summary)
    }));
    result.unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` must be a valid pointer from `aa2_create_game` or null.
pub unsafe extern "C" fn aa2_get_player_view(ctx: *mut GameContext, player_id: u8) -> *const c_char {
    if ctx.is_null() {
        return std::ptr::null();
    }
    let result = catch_unwind(std::panic::AssertUnwindSafe(|| {
        let ctx = unsafe { &mut *ctx };
        let p_idx = player_id as usize;
        if p_idx >= ctx.game.players.len() {
            return error_json("invalid player_id");
        }
        let json = serde_json::to_string(&ctx.game.players[p_idx]).unwrap_or_else(|_| "null".to_string());
        return_string(json)
    }));
    result.unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` must be a valid pointer from `aa2_create_game` or null.
pub unsafe extern "C" fn aa2_get_combat_replay(ctx: *mut GameContext, matchup_index: u8) -> *const c_char {
    if ctx.is_null() {
        return std::ptr::null();
    }
    let result = catch_unwind(std::panic::AssertUnwindSafe(|| {
        let ctx = unsafe { &mut *ctx };
        let idx = matchup_index as usize;
        if idx >= ctx.last_combat_results.len() {
            return error_json("invalid matchup_index");
        }
        let events: Vec<serde_json::Value> = ctx.last_combat_results[idx]
            .combat_log
            .iter()
            .map(combat_event_to_json)
            .collect();
        let replay = serde_json::json!({ "events": events });
        return_json(&replay)
    }));
    result.unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
/// # Safety
/// `ptr` must be a pointer previously returned by an aa2_* function, or null.
pub unsafe extern "C" fn aa2_free_string(ptr: *const c_char) {
    if !ptr.is_null() {
        unsafe { drop(CString::from_raw(ptr as *mut c_char)) };
    }
}
