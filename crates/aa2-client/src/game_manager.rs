use std::collections::{HashMap, HashSet};

use godot::prelude::*;
use godot::classes::{Node, INode};
use rand::rngs::StdRng;
use rand::SeedableRng;

use aa2_game::{GameConfig, GamePhase, GameState};
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
}
