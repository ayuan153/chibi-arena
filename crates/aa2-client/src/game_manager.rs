use std::collections::{HashMap, HashSet};

use godot::prelude::*;
use godot::classes::{Node, INode};
use rand::rngs::StdRng;
use rand::SeedableRng;

use aa2_game::{GameConfig, GamePhase, GameState};
use aa2_game::combat::CombatResult;
use aa2_game::god::all_gods;
use aa2_game::scenario::Action;
use aa2_game::pool::AbilityPool;
use aa2_data::{AbilityDef, HeroDef};

#[derive(GodotClass)]
#[class(init, base=Node)]
pub struct GameManager {
    base: Base<Node>,
    game: Option<GameState>,
    hero_defs: HashMap<String, HeroDef>,
    ability_defs: HashMap<String, AbilityDef>,
    rng: Option<StdRng>,
    last_combat_results: Vec<CombatResult>,
}

#[godot_api]
impl INode for GameManager {}

#[godot_api]
impl GameManager {
    #[func]
    pub fn init_game(&mut self, seed: i64, _num_players: i32, data_path: GString) {
        let data_path_str = data_path.to_string();
        let data_dir = std::path::Path::new(&data_path_str);

        // Load hero defs
        if let Ok(heroes) = aa2_data::load_all_heroes(&data_dir.join("heroes")) {
            for h in heroes {
                self.hero_defs.insert(h.name.clone(), h);
            }
        }

        // Load ability defs
        if let Ok(entries) = std::fs::read_dir(data_dir.join("abilities")) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "ron")
                    && let Ok(def) = aa2_data::load_ability_def(&path)
                {
                    self.ability_defs.insert(def.name.clone(), def);
                }
            }
        }

        // Build pool and ultimates
        let ultimates: HashSet<String> = self.ability_defs.iter()
            .filter(|(_, d)| d.is_ultimate)
            .map(|(n, _)| n.clone())
            .collect();
        let pool_counts: HashMap<String, u32> = self.ability_defs.keys()
            .map(|n| (n.clone(), 20))
            .collect();
        let pool = AbilityPool::from_counts(pool_counts);

        let config = GameConfig {
            auto_advance: false,
            ..GameConfig::default()
        };
        self.game = Some(GameState::new(pool, ultimates, config));
        self.rng = Some(StdRng::seed_from_u64(seed as u64));
    }

    #[func]
    pub fn apply_player_action(&mut self, player_id: i32, action_type: GString, param: GString) -> GString {
        let Some(game) = &mut self.game else { return "no game".into() };
        let Some(rng) = &mut self.rng else { return "no rng".into() };

        let action_str = action_type.to_string();
        let param_str = param.to_string();

        let action = match action_str.as_str() {
            "Buy" => {
                let slot: usize = param_str.parse().unwrap_or(0);
                Action::Buy(slot)
            }
            "Sell" => Action::Sell(param_str),
            "RerollShop" => Action::RerollShop,
            "UpgradeShop" => Action::UpgradeShop,
            "LockShop" => Action::LockShop,
            "SetPosition" => {
                let parts: Vec<&str> = param_str.splitn(3, ',').collect();
                if parts.len() != 3 { return "bad params".into(); }
                let name = parts[0].to_string();
                let x: f32 = parts[1].parse().unwrap_or(1000.0);
                let y: f32 = parts[2].parse().unwrap_or(500.0);
                Action::SetPosition(name, x, y)
            }
            "Equip" => {
                let parts: Vec<&str> = param_str.splitn(2, ',').collect();
                if parts.len() != 2 { return "bad params".into(); }
                Action::Equip(parts[0].to_string(), parts[1].to_string())
            }
            "Unequip" => {
                let parts: Vec<&str> = param_str.splitn(2, ',').collect();
                if parts.len() != 2 { return "bad params".into(); }
                Action::Unequip(parts[0].to_string(), parts[1].to_string())
            }
            "PickGod" => {
                let gods = all_gods();
                match gods.into_iter().find(|g| g.name == param_str) {
                    Some(god) => Action::PickGod(god),
                    None => return GString::from("unknown god"),
                }
            }
            "DraftHero" => {
                let idx: usize = param_str.parse().unwrap_or(0);
                Action::DraftHero(idx)
            }
            "Ready" => Action::Ready,
            _ => return GString::from(format!("unknown action: {action_str}").as_str()),
        };

        match game.apply_action(player_id as u8, action, rng) {
            Ok(()) => "ok".into(),
            Err(e) => GString::from(e.as_str()),
        }
    }

    #[func]
    pub fn get_gold(&self, player_id: i32) -> i32 {
        self.game.as_ref()
            .and_then(|g| g.players.get(player_id as usize))
            .map(|p| p.gold as i32)
            .unwrap_or(0)
    }

    #[func]
    pub fn get_shop_level(&self, player_id: i32) -> i32 {
        self.game.as_ref()
            .and_then(|g| g.players.get(player_id as usize))
            .map(|p| p.shop.level as i32)
            .unwrap_or(1)
    }

    #[func]
    pub fn get_shop_offerings(&self, player_id: i32) -> PackedStringArray {
        let mut arr = PackedStringArray::new();
        if let Some(game) = &self.game
            && let Some(player) = game.players.get(player_id as usize)
        {
            for slot in &player.shop.offerings {
                arr.push(&GString::from(slot.as_deref().unwrap_or("")));
            }
        }
        arr
    }

    #[func]
    pub fn get_shop_locked(&self, player_id: i32) -> bool {
        self.game.as_ref()
            .and_then(|g| g.players.get(player_id as usize))
            .map(|p| p.shop.locked)
            .unwrap_or(false)
    }

    #[func]
    pub fn get_upgrade_cost(&self, player_id: i32) -> i32 {
        self.game.as_ref()
            .and_then(|g| g.players.get(player_id as usize))
            .and_then(|p| p.shop.upgrade_cost())
            .map(|c| c as i32)
            .unwrap_or(-1)
    }

    #[func]
    pub fn get_phase(&self) -> GString {
        self.game.as_ref()
            .map(|g| match &g.phase {
                GamePhase::GodPick => "GodPick",
                GamePhase::Combat => "Combat",
                GamePhase::GracePeriod => "GracePeriod",
                GamePhase::Shop => "Shop",
                GamePhase::Finished => "Finished",
            })
            .unwrap_or("None")
            .into()
    }

    #[func]
    pub fn tick(&mut self, dt: f32) {
        if let (Some(game), Some(rng)) = (&mut self.game, &mut self.rng) {
            game.tick(dt, rng);
        }
    }

    #[func]
    pub fn get_heroes(&self, player_id: i32) -> PackedStringArray {
        let mut arr = PackedStringArray::new();
        if let Some(game) = &self.game
            && let Some(player) = game.players.get(player_id as usize)
        {
            for h in &player.heroes {
                arr.push(&GString::from(h.as_str()));
            }
        }
        arr
    }

    #[func]
    pub fn get_hero_position(&self, player_id: i32, hero_name: GString) -> Vector2 {
        if let Some(game) = &self.game
            && let Some(player) = game.players.get(player_id as usize)
        {
            let name = hero_name.to_string();
            if let Some(&(x, y)) = player.hero_positions.get(&name) {
                return Vector2::new(x, y);
            }
        }
        Vector2::new(1000.0, 500.0)
    }

    #[func]
    pub fn get_bench(&self, player_id: i32) -> PackedStringArray {
        let mut arr = PackedStringArray::new();
        if let Some(game) = &self.game
            && let Some(player) = game.players.get(player_id as usize)
        {
            for a in &player.bench {
                arr.push(&GString::from(a.as_str()));
            }
        }
        arr
    }

    #[func]
    pub fn get_equipped_abilities(&self, player_id: i32, hero_name: GString) -> PackedStringArray {
        let mut arr = PackedStringArray::new();
        if let Some(game) = &self.game
            && let Some(player) = game.players.get(player_id as usize)
        {
            let name = hero_name.to_string();
            if let Some(abilities) = player.equipped.get(&name) {
                for a in abilities {
                    arr.push(&GString::from(a.as_str()));
                }
            }
        }
        arr
    }

    #[func]
    pub fn get_ability_level(&self, player_id: i32, ability_name: GString) -> i32 {
        if let Some(game) = &self.game
            && let Some(player) = game.players.get(player_id as usize)
        {
            let name = ability_name.to_string();
            if let Some(&level) = player.abilities.get(&name) {
                return level as i32;
            }
        }
        0
    }

    #[func]
    pub fn run_combat(&mut self) -> bool {
        let (Some(game), Some(rng)) = (&mut self.game, &mut self.rng) else { return false };
        use rand::RngCore;
        let seed = rng.next_u32();
        let results = game.run_combat_round(&self.hero_defs, &self.ability_defs, seed, rng);
        self.last_combat_results = results;
        !self.last_combat_results.is_empty()
    }

    #[func]
    pub fn get_combat_event_count(&self, matchup_index: i32) -> i32 {
        self.last_combat_results
            .get(matchup_index as usize)
            .map(|r| r.combat_log.len() as i32)
            .unwrap_or(0)
    }

    #[func]
    pub fn get_combat_event(&self, matchup_index: i32, event_index: i32) -> VarDictionary {
        let Some(result) = self.last_combat_results.get(matchup_index as usize) else {
            return VarDictionary::new();
        };
        let Some(event) = result.combat_log.get(event_index as usize) else {
            return VarDictionary::new();
        };
        combat_event_to_dict(event)
    }

    #[func]
    pub fn get_combat_result(&self, matchup_index: i32) -> VarDictionary {
        let Some(result) = self.last_combat_results.get(matchup_index as usize) else {
            return VarDictionary::new();
        };
        let mut d = VarDictionary::new();
        d.set("winner", result.winner.map(|w| w as i32).unwrap_or(-1));
        d.set("survivors_a", result.survivors_a as i32);
        d.set("survivors_b", result.survivors_b as i32);
        d
    }

    #[func]
    pub fn get_combat_matchup_count(&self) -> i32 {
        self.last_combat_results.len() as i32
    }

    #[func]
    pub fn get_available_gods(&self) -> Array<VarDictionary> {
        let mut arr = Array::new();
        for god in all_gods() {
            let mut d = VarDictionary::new();
            d.set("name", &GString::from(god.name.as_str()));
            d.set("description", &GString::from(god.description.as_str()));
            arr.push(&d);
        }
        arr
    }

    #[func]
    pub fn get_player_god(&self, player_id: i32) -> GString {
        self.game.as_ref()
            .and_then(|g| g.players.get(player_id as usize))
            .and_then(|p| p.god.as_ref())
            .map(|g| GString::from(g.name.as_str()))
            .unwrap_or_default()
    }

    #[func]
    pub fn get_draft_choices(&self, player_id: i32) -> PackedStringArray {
        let _ = player_id;
        // Draft choices are managed externally in the dev binary;
        // return empty for now (client will populate via signals)
        PackedStringArray::new()
    }

    #[func]
    pub fn is_draft_active(&self) -> bool {
        self.game.as_ref().map(|g| g.draft_pending).unwrap_or(false)
    }

    #[func]
    pub fn get_player_count(&self) -> i32 {
        self.game.as_ref().map(|g| g.players.len() as i32).unwrap_or(0)
    }

    #[func]
    pub fn get_player_hp(&self, player_id: i32) -> f32 {
        self.game.as_ref()
            .and_then(|g| g.players.get(player_id as usize))
            .map(|p| p.hp)
            .unwrap_or(0.0)
    }

    #[func]
    pub fn get_player_alive(&self, player_id: i32) -> bool {
        self.game.as_ref()
            .and_then(|g| g.players.get(player_id as usize))
            .map(|p| p.alive)
            .unwrap_or(false)
    }

    #[func]
    pub fn get_round(&self) -> i32 {
        self.game.as_ref().map(|g| g.round as i32).unwrap_or(0)
    }
}

fn combat_event_to_dict(event: &aa2_sim::CombatEvent) -> VarDictionary {
    use aa2_sim::CombatEvent;
    let mut d = VarDictionary::new();
    match event {
        CombatEvent::Attack { tick, attacker_id, target_id, damage } => {
            d.set("type", "Attack");
            d.set("tick", *tick as i32);
            d.set("attacker_id", *attacker_id as i32);
            d.set("target_id", *target_id as i32);
            d.set("damage", *damage);
        }
        CombatEvent::Death { tick, unit_id } => {
            d.set("type", "Death");
            d.set("tick", *tick as i32);
            d.set("unit_id", *unit_id as i32);
        }
        CombatEvent::ProjectileHit { tick, target_id, damage } => {
            d.set("type", "ProjectileHit");
            d.set("tick", *tick as i32);
            d.set("target_id", *target_id as i32);
            d.set("damage", *damage);
        }
        CombatEvent::ProjectileSpawn { tick, attacker_id, target_id } => {
            d.set("type", "ProjectileSpawn");
            d.set("tick", *tick as i32);
            d.set("attacker_id", *attacker_id as i32);
            d.set("target_id", *target_id as i32);
        }
        CombatEvent::CastStart { tick, caster_id, ability_name } => {
            d.set("type", "CastStart");
            d.set("tick", *tick as i32);
            d.set("caster_id", *caster_id as i32);
            d.set("ability_name", ability_name.as_str());
        }
        CombatEvent::CastComplete { tick, caster_id, ability_name } => {
            d.set("type", "CastComplete");
            d.set("tick", *tick as i32);
            d.set("caster_id", *caster_id as i32);
            d.set("ability_name", ability_name.as_str());
        }
        CombatEvent::AbilityDamage { tick, caster_id, target_id, ability_name, damage, .. } => {
            d.set("type", "AbilityDamage");
            d.set("tick", *tick as i32);
            d.set("caster_id", *caster_id as i32);
            d.set("target_id", *target_id as i32);
            d.set("ability_name", ability_name.as_str());
            d.set("damage", *damage);
        }
        CombatEvent::Heal { tick, target_id, amount } => {
            d.set("type", "Heal");
            d.set("tick", *tick as i32);
            d.set("target_id", *target_id as i32);
            d.set("amount", *amount);
        }
        CombatEvent::RoundEnd { tick, winning_team } => {
            d.set("type", "RoundEnd");
            d.set("tick", *tick as i32);
            d.set("winning_team", *winning_team as i32);
        }
        CombatEvent::UnitSpawn { tick, unit_id, team, name, x, y, max_hp } => {
            d.set("type", "UnitSpawn");
            d.set("tick", *tick as i32);
            d.set("unit_id", *unit_id as i32);
            d.set("team", *team as i32);
            d.set("name", name.as_str());
            d.set("x", *x);
            d.set("y", *y);
            d.set("max_hp", *max_hp);
        }
        CombatEvent::MoveTo { tick, unit_id, x, y, speed } => {
            d.set("type", "MoveTo");
            d.set("tick", *tick as i32);
            d.set("unit_id", *unit_id as i32);
            d.set("x", *x);
            d.set("y", *y);
            d.set("speed", *speed);
        }
        _ => {
            d.set("type", "Other");
        }
    }
    d
}
