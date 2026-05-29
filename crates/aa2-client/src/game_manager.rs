use std::collections::{HashMap, HashSet};

use godot::prelude::*;
use godot::classes::{Node, INode};
use rand::rngs::StdRng;
use rand::SeedableRng;

use aa2_game::{GameConfig, GamePhase, GameState};
use aa2_game::combat::CombatResult;
use aa2_game::god;
use aa2_game::scenario::Action;
use aa2_game::pool::AbilityPool;
use aa2_data::{AbilityDef, God, HeroDef};
use aa2_net::ClientMsg;

use crate::net_client;

#[derive(GodotClass)]
#[class(init, base=Node)]
pub struct GameManager {
    base: Base<Node>,
    game: Option<GameState>,
    hero_defs: HashMap<String, HeroDef>,
    ability_defs: HashMap<String, AbilityDef>,
    gods: Vec<God>,
    rng: Option<StdRng>,
    last_combat_results: Vec<CombatResult>,
    last_phase: String,
    net: Option<net_client::NetClient>,
    net_state: net_client::NetState,
}

#[godot_api]
impl INode for GameManager {}

#[godot_api]
impl GameManager {
    #[func]
    pub fn init_game(&mut self, seed: i64, num_players: i32, data_path: GString) {
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

        // Load gods
        self.gods = aa2_data::load_all_gods(&data_dir.join("gods")).unwrap_or_else(|_| god::all_gods());

        let config = GameConfig {
            auto_advance: false,
            ..GameConfig::default()
        };
        let mut game = GameState::new(pool, ultimates, config);
        game.gods = self.gods.clone();
        self.game = Some(game);
        self.rng = Some(StdRng::seed_from_u64(seed as u64));
        self.last_phase.clear();

        // Mark extra players as dead
        if let Some(ref mut game) = self.game {
            for i in num_players as usize..8 {
                game.players[i].alive = false;
            }
        }
    }

    fn networked(&self) -> bool {
        self.net.is_some()
    }

    /// Public accessor for networked state (used by MainScene to gate local-only logic).
    #[func]
    pub fn is_networked(&self) -> bool {
        self.networked()
    }

    /// Request the server to start the game. No-op if not networked.
    #[func]
    pub fn start_game(&mut self) {
        if let Some(nc) = &self.net {
            nc.send(ClientMsg::Start);
        }
    }

    /// Connect to a remote game server via WebSocket. Sends a Join message immediately.
    #[func]
    pub fn connect_to_server(&mut self, url: GString) {
        let nc = net_client::NetClient::connect(url.to_string());
        nc.send(ClientMsg::Join { name: "Player".into() });
        self.net = Some(nc);
    }

    #[func]
    pub fn get_my_player_id(&self) -> i32 {
        if self.networked() {
            self.net_state.my_player_id() as i32
        } else {
            0
        }
    }

    #[func]
    pub fn apply_player_action(&mut self, player_id: i32, action_type: GString, param: GString) -> GString {
        if self.networked() {
            self.net.as_ref().unwrap().send(ClientMsg::Action {
                action_type: action_type.to_string(),
                param: param.to_string(),
            });
            return "ok".into();
        }
        let Some(game) = &mut self.game else { return "no game".into() };
        let Some(rng) = &mut self.rng else { return "no rng".into() };

        let action_str = action_type.to_string();
        let param_str = param.to_string();

        let action = match aa2_game::scenario::parse_action(&action_str, &param_str, &self.gods) {
            Ok(a) => a,
            Err(e) => return GString::from(e.as_str()),
        };

        match game.apply_action(player_id as u8, action.clone(), &self.hero_defs, rng) {
            Ok(()) => {
                // Log shop state for debugging after Ready
                if matches!(action, Action::Ready) && game.phase == GamePhase::Shop {
                    let offerings: Vec<_> = game.players[0].shop.offerings.iter()
                        .filter_map(|o| o.as_ref())
                        .collect();
                    godot_print!("[AA2] Shop offerings: {:?}", offerings);
                    if let Some(choices) = game.draft_choices.get(&0) {
                        godot_print!("[AA2] Draft choices: {:?}", choices);
                    }
                }
                "ok".into()
            }
            Err(e) => GString::from(e.as_str()),
        }
    }

    #[func]
    pub fn get_gold(&self, player_id: i32) -> i32 {
        if self.networked() {
            return self.net_state.gold(player_id as usize) as i32;
        }
        self.game.as_ref()
            .and_then(|g| g.players.get(player_id as usize))
            .map(|p| p.gold as i32)
            .unwrap_or(0)
    }

    #[func]
    pub fn get_shop_level(&self, player_id: i32) -> i32 {
        if self.networked() {
            return self.net_state.shop_level(player_id as usize) as i32;
        }
        self.game.as_ref()
            .and_then(|g| g.players.get(player_id as usize))
            .map(|p| p.shop.level as i32)
            .unwrap_or(1)
    }

    #[func]
    pub fn get_shop_offerings(&self, player_id: i32) -> PackedStringArray {
        if self.networked() {
            let mut arr = PackedStringArray::new();
            for slot in self.net_state.shop_offerings(player_id as usize) {
                arr.push(&GString::from(slot.as_deref().unwrap_or("")));
            }
            return arr;
        }
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
        if self.networked() {
            return self.net_state.shop_locked(player_id as usize);
        }
        self.game.as_ref()
            .and_then(|g| g.players.get(player_id as usize))
            .map(|p| p.shop.locked)
            .unwrap_or(false)
    }

    #[func]
    pub fn get_upgrade_cost(&self, player_id: i32) -> i32 {
        if self.networked() {
            return self.net_state.upgrade_cost(player_id as usize).map(|c| c as i32).unwrap_or(-1);
        }
        self.game.as_ref()
            .and_then(|g| g.players.get(player_id as usize))
            .and_then(|p| p.shop.upgrade_cost())
            .map(|c| c as i32)
            .unwrap_or(-1)
    }

    #[func]
    pub fn get_phase(&self) -> GString {
        if self.networked() {
            return GString::from(self.net_state.phase().as_str());
        }
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
        if self.networked() {
            while let Some(msg) = self.net.as_ref().unwrap().try_recv() {
                self.net_state.apply(&msg);
            }
            return;
        }
        let mut should_generate_draft = false;

        if let (Some(game), Some(rng)) = (&mut self.game, &mut self.rng) {
            game.tick(dt, rng);

            let phase = format!("{:?}", game.phase);
            if phase != self.last_phase {
                self.last_phase = phase;
                if game.phase == GamePhase::Shop && game.draft_pending && !game.draft_choices.contains_key(&0) {
                    should_generate_draft = true;
                }
            }
        }

        if should_generate_draft {
            self.do_generate_draft();
        }
    }

    fn do_generate_draft(&mut self) {
        use aa2_game::draft::{generate_draft_choices, tier_for_draft_round};
        let Some(game) = &mut self.game else { return };
        let Some(rng) = &mut self.rng else { return };

        let tier = tier_for_draft_round(game.round).unwrap_or(0);
        let mut all_heroes: Vec<&HeroDef> = self.hero_defs.values().collect();
        all_heroes.sort_by_key(|h| &h.name);
        for i in 0..game.players.len() {
            if game.players[i].alive {
                let owned: Vec<&str> = game.players[i].heroes.iter().map(|s| s.as_str()).collect();
                let available: Vec<&HeroDef> = all_heroes.iter()
                    .filter(|h| !owned.contains(&h.name.as_str()))
                    .copied()
                    .collect();
                let choices = generate_draft_choices(&available, tier, rng);
                game.draft_choices.insert(game.players[i].id, choices);
            }
        }
    }

    #[func]
    pub fn get_heroes(&self, player_id: i32) -> PackedStringArray {
        if self.networked() {
            let mut arr = PackedStringArray::new();
            for h in self.net_state.heroes(player_id as usize) {
                arr.push(&GString::from(h.as_str()));
            }
            return arr;
        }
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
        if self.networked() {
            let (x, y) = self.net_state.hero_position(player_id as usize, &hero_name.to_string());
            return Vector2::new(x, y);
        }
        if let Some(game) = &self.game
            && let Some(player) = game.players.get(player_id as usize)
        {
            let name = hero_name.to_string();
            if let Some(&(x, y)) = player.hero_positions.get(&name) {
                return Vector2::new(x, y);
            }
        }
        Vector2::new(500.0, 1500.0)
    }

    /// Get hero stats as a dictionary for the unit info panel.
    #[func]
    pub fn get_hero_info(&self, hero_name: GString) -> VarDictionary {
        let mut dict = VarDictionary::new();
        let name_str = hero_name.to_string();
        let Some(hero) = self.hero_defs.get(&name_str) else { return dict };
        let Some(game) = &self.game else { return dict };
        let level = game.hero_level() as f32;

        let attr_str = match hero.primary_attribute {
            aa2_data::Attribute::Strength => "STR",
            aa2_data::Attribute::Agility => "AGI",
            aa2_data::Attribute::Intelligence => "INT",
            aa2_data::Attribute::Universal => "UNI",
        };

        let str_total = hero.base_str + hero.str_gain * level;
        let agi_total = hero.base_agi + hero.agi_gain * level;
        let int_total = hero.base_int + hero.int_gain * level;

        let hp = 120.0 + str_total * 22.0;
        let mana = 75.0 + int_total * 12.0;
        let armor = agi_total * 0.167;
        let attack_speed = 100.0 + agi_total;

        let primary_bonus = match hero.primary_attribute {
            aa2_data::Attribute::Strength => str_total,
            aa2_data::Attribute::Agility => agi_total,
            aa2_data::Attribute::Intelligence => int_total,
            aa2_data::Attribute::Universal => (str_total + agi_total + int_total) * 0.7 / 3.0,
        };
        let dmg_min = hero.base_damage_min + primary_bonus;
        let dmg_max = hero.base_damage_max + primary_bonus;

        dict.set("name", &Variant::from(hero_name.clone()));
        dict.set("attribute", &Variant::from(GString::from(attr_str)));
        dict.set("str", str_total as i32);
        dict.set("agi", agi_total as i32);
        dict.set("int", int_total as i32);
        dict.set("hp", hp as i32);
        dict.set("mana", mana as i32);
        dict.set("armor", &Variant::from(GString::from(format!("{armor:.1}").as_str())));
        dict.set("attack_speed", attack_speed as i32);
        dict.set("damage", &Variant::from(GString::from(format!("{}-{}", dmg_min as i32, dmg_max as i32).as_str())));
        dict.set("move_speed", hero.move_speed as i32);
        dict.set("attack_range", hero.attack_range as i32);
        dict.set("is_melee", hero.is_melee);
        dict.set("bat", &Variant::from(GString::from(format!("{:.1}", hero.base_attack_time).as_str())));
        dict
    }

    #[func]
    pub fn get_ability_info(&self, name: GString) -> VarDictionary {
        let mut dict = VarDictionary::new();
        let Some(def) = self.ability_defs.get(&name.to_string()) else { return dict };

        let mana_str = def.mana_cost.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(" / ");
        let cd_str = def.cooldown.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(" / ");
        let targeting_str = match &def.targeting {
            aa2_data::TargetType::SingleEnemy => "Single Enemy",
            aa2_data::TargetType::SingleAlly => "Single Ally",
            aa2_data::TargetType::SingleAllyHG => "Single Ally (HG)",
            aa2_data::TargetType::PointAoE => "Point AoE",
            aa2_data::TargetType::NoTarget => "No Target",
            aa2_data::TargetType::Passive => "Passive",
        };

        dict.set("name", &Variant::from(GString::from(def.name.as_str())));
        dict.set("description", &Variant::from(GString::from(def.description.as_str())));
        dict.set("mana_cost", &Variant::from(GString::from(mana_str.as_str())));
        dict.set("cooldown", &Variant::from(GString::from(cd_str.as_str())));
        dict.set("cast_range", def.cast_range);
        dict.set("is_ultimate", def.is_ultimate);
        dict.set("targeting", &Variant::from(GString::from(targeting_str)));
        dict
    }

    #[func]
    pub fn get_ability_is_ultimate(&self, name: GString) -> bool {
        self.ability_defs.get(&name.to_string()).is_some_and(|a| a.is_ultimate)
    }

    #[func]
    pub fn get_bench(&self, player_id: i32) -> PackedStringArray {
        if self.networked() {
            let mut arr = PackedStringArray::new();
            for a in self.net_state.bench(player_id as usize) {
                arr.push(&GString::from(a.as_str()));
            }
            return arr;
        }
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
        if self.networked() {
            let mut arr = PackedStringArray::new();
            for a in self.net_state.equipped(player_id as usize, &hero_name.to_string()) {
                arr.push(&GString::from(a.as_str()));
            }
            return arr;
        }
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
        if self.networked() {
            return self.net_state.ability_level(player_id as usize, &ability_name.to_string()) as i32;
        }
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

    /// End combat phase, transitioning to GracePeriod then Shop.
    #[func]
    pub fn end_combat(&mut self) {
        let Some(game) = &mut self.game else { return };
        game.end_combat(false);
    }

    #[func]
    pub fn get_combat_event_count(&self, matchup_index: i32) -> i32 {
        if self.networked() { return 0; }
        self.last_combat_results
            .get(matchup_index as usize)
            .map(|r| r.combat_log.len() as i32)
            .unwrap_or(0)
    }

    #[func]
    pub fn get_combat_event(&self, matchup_index: i32, event_index: i32) -> VarDictionary {
        if self.networked() { return VarDictionary::new(); }
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
        if self.networked() { return VarDictionary::new(); }
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
        if self.networked() { return 0; }
        self.last_combat_results.len() as i32
    }

    /// Per-unit damage summary for a matchup's last combat, sorted by damage descending.
    /// Each dict: { unit_id: i32, team: i32, name: GString, damage: i32 }.
    #[func]
    pub fn get_damage_summary(&self, matchup_index: i32) -> Array<VarDictionary> {
        if self.networked() { return Array::new(); }
        let mut arr = Array::new();
        let Some(result) = self.last_combat_results.get(matchup_index as usize) else {
            return arr;
        };
        let summary = aa2_sim::summarize_damage(&result.combat_log);
        for ud in &summary {
            let mut d = VarDictionary::new();
            d.set("unit_id", ud.unit_id as i32);
            d.set("team", ud.team as i32);
            d.set("name", &GString::from(ud.name.as_str()));
            d.set("damage", ud.damage.round() as i32);
            arr.push(&d);
        }
        arr
    }

    #[func]
    pub fn get_available_gods(&self) -> Array<VarDictionary> {
        let mut arr = Array::new();
        for god in &self.gods {
            let mut d = VarDictionary::new();
            d.set("name", &GString::from(god.name.as_str()));
            d.set("description", &GString::from(god.description.as_str()));
            arr.push(&d);
        }
        arr
    }

    #[func]
    pub fn get_player_god(&self, player_id: i32) -> GString {
        if self.networked() {
            return self.net_state.player_god(player_id as usize)
                .map(|g| GString::from(g.as_str()))
                .unwrap_or_default();
        }
        self.game.as_ref()
            .and_then(|g| g.players.get(player_id as usize))
            .and_then(|p| p.god.as_ref())
            .map(|g| GString::from(g.name.as_str()))
            .unwrap_or_default()
    }

    #[func]
    pub fn get_draft_choices(&self, player_id: i32) -> PackedStringArray {
        if self.networked() {
            let mut arr = PackedStringArray::new();
            for choice in self.net_state.draft_choices(player_id as usize) {
                arr.push(&GString::from(choice.as_deref().unwrap_or("")));
            }
            return arr;
        }
        let mut arr = PackedStringArray::new();
        if let Some(game) = &self.game
            && let Some(choices) = game.draft_choices.get(&(player_id as u8))
        {
            for choice in choices {
                arr.push(&GString::from(choice.as_deref().unwrap_or("")));
            }
        }
        arr
    }

    #[func]
    pub fn is_draft_active(&self) -> bool {
        if self.networked() {
            return self.net_state.is_draft_active(self.net_state.my_player_id() as usize);
        }
        // Show draft UI when player 0 has pending choices (round draft OR hero reroll)
        self.game.as_ref().is_some_and(|g| g.draft_choices.contains_key(&0))
    }

    #[func]
    pub fn get_player_count(&self) -> i32 {
        if self.networked() {
            return self.net_state.player_count() as i32;
        }
        self.game.as_ref().map(|g| g.players.len() as i32).unwrap_or(0)
    }

    #[func]
    pub fn get_player_hp(&self, player_id: i32) -> f32 {
        if self.networked() {
            return self.net_state.player_hp(player_id as usize);
        }
        self.game.as_ref()
            .and_then(|g| g.players.get(player_id as usize))
            .map(|p| p.hp)
            .unwrap_or(0.0)
    }

    #[func]
    pub fn get_player_alive(&self, player_id: i32) -> bool {
        if self.networked() {
            return self.net_state.player_alive(player_id as usize);
        }
        self.game.as_ref()
            .and_then(|g| g.players.get(player_id as usize))
            .map(|p| p.alive)
            .unwrap_or(false)
    }

    #[func]
    pub fn get_round(&self) -> i32 {
        if self.networked() {
            return self.net_state.round() as i32;
        }
        self.game.as_ref().map(|g| g.round as i32).unwrap_or(0)
    }

    #[func]
    pub fn set_gold(&mut self, player_id: i32, gold: i32) {
        if let Some(ref mut game) = self.game
            && let Some(p) = game.players.get_mut(player_id as usize)
        {
            p.gold = gold as u32;
        }
    }

    #[func]
    pub fn get_player_placement(&self, player_id: i32) -> i32 {
        if self.networked() {
            if let Some(ref placements) = self.net_state.placements {
                return placements.iter().position(|&id| id == player_id as u8)
                    .map(|i| (i + 1) as i32)
                    .unwrap_or(0);
            }
            if self.net_state.player_alive(player_id as usize) {
                return 1;
            }
            let alive_count = (0..self.net_state.player_count())
                .filter(|&i| self.net_state.player_alive(i))
                .count();
            return (alive_count + 1) as i32;
        }
        let Some(game) = &self.game else { return 0 };
        let Some(player) = game.players.get(player_id as usize) else { return 0 };
        if player.alive {
            // Alive = winner (or game still going). In 2-player: 1st place.
            1
        } else {
            // Dead player: placement = alive_count + 1 at time of query
            (game.alive_count() + 1) as i32
        }
    }

    #[func]
    pub fn set_hp(&mut self, player_id: i32, hp: f32) {
        if let Some(ref mut game) = self.game
            && let Some(p) = game.players.get_mut(player_id as usize)
        {
            p.hp = hp;
        }
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
            // Map remaining events
            match event {
                CombatEvent::BuffApplied { tick, target_id, name } => {
                    d.set("type", "BuffApplied");
                    d.set("tick", *tick as i32);
                    d.set("target_id", *target_id as i32);
                    d.set("name", name.as_str());
                }
                CombatEvent::BuffExpired { tick, target_id, name } => {
                    d.set("type", "BuffExpired");
                    d.set("tick", *tick as i32);
                    d.set("target_id", *target_id as i32);
                    d.set("name", name.as_str());
                }
                CombatEvent::DarkPactPulse { tick, caster_id, enemies_hit, self_damage } => {
                    d.set("type", "DarkPactPulse");
                    d.set("tick", *tick as i32);
                    d.set("caster_id", *caster_id as i32);
                    d.set("enemies_hit", *enemies_hit as i32);
                    d.set("self_damage", *self_damage);
                }
                CombatEvent::WaveHit { tick, target_id, damage, stun_duration } => {
                    d.set("type", "WaveHit");
                    d.set("tick", *tick as i32);
                    d.set("target_id", *target_id as i32);
                    d.set("damage", *damage);
                    d.set("stun_duration", *stun_duration);
                }
                _ => { d.set("type", "Other"); }
            }
        }
    }
    d
}
