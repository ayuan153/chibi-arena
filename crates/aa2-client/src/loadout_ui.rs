use godot::prelude::*;
use godot::classes::{Button, HBoxContainer, IVBoxContainer, Label, VBoxContainer};

use crate::game_manager::GameManager;
use crate::ui_helpers::{attribute_stylebox, format_ability_tooltip, ultimate_stylebox};

const MAX_HEROES: usize = 5;
const MAX_SLOTS: usize = 4;
const MAX_BENCH: usize = 5;

/// Loadout grid showing hero rows with equipped abilities and bench slots.
/// Click bench ability then click hero slot to equip.
#[derive(GodotClass)]
#[class(init, base=VBoxContainer)]
pub struct LoadoutUi {
    base: Base<VBoxContainer>,
    /// Currently selected bench ability for equip flow
    #[init(val = String::new())]
    selected_ability: String,
    /// Source slot for ability swap: (hero_idx, slot_idx)
    #[init(val = None)]
    swap_source: Option<(usize, usize)>,
}

#[godot_api]
impl IVBoxContainer for LoadoutUi {
    fn ready(&mut self) {
        godot_print!("[AA2] LoadoutUi ready");

        // Create hero rows
        for i in 0..MAX_HEROES {
            let mut row = HBoxContainer::new_alloc();
            row.set_name(&format!("HeroRow{i}"));
            row.set_visible(false);

            let mut portrait = Button::new_alloc();
            portrait.set_name("HeroPortrait");
            portrait.set_custom_minimum_size(Vector2::new(80.0, 80.0));
            portrait.set_text("?");
            portrait.connect("pressed", &self.base().callable(&format!("on_hero_{i}")));
            row.add_child(&portrait);

            for s in 0..MAX_SLOTS {
                let mut slot = Button::new_alloc();
                slot.set_name(&format!("AbilitySlot{s}"));
                slot.set_custom_minimum_size(Vector2::new(64.0, 64.0));
                slot.set_text("--");
                slot.connect("pressed", &self.base().callable(&format!("on_equip_{i}_{s}")));
                row.add_child(&slot);
            }

            let mut reroll_btn = Button::new_alloc();
            reroll_btn.set_name("RerollHeroBtn");
            reroll_btn.set_text("Reroll 2");
            reroll_btn.connect("pressed", &self.base().callable(&format!("on_reroll_hero_{i}")));
            row.add_child(&reroll_btn);

            self.base_mut().add_child(&row);
        }

        // Bench row
        let mut bench_row = HBoxContainer::new_alloc();
        bench_row.set_name("BenchRow");

        let mut bench_label = Label::new_alloc();
        bench_label.set_text("Bench: ");
        bench_row.add_child(&bench_label);

        for i in 0..MAX_BENCH {
            let mut btn = Button::new_alloc();
            btn.set_name(&format!("BenchSlot{i}"));
            btn.set_custom_minimum_size(Vector2::new(64.0, 64.0));
            btn.set_text("--");
            btn.set_visible(false);
            btn.connect("pressed", &self.base().callable(&format!("on_bench_{i}")));
            bench_row.add_child(&btn);
        }
        self.base_mut().add_child(&bench_row);

        // Connect sell bin button
        if let Some(node) = self.base().get_node_or_null("/root/MainScene/PersistentChrome/GodPortrait/SellBin") {
            let mut btn: Gd<Button> = node.cast();
            btn.connect("pressed", &self.base().callable("on_sell_pressed"));
        }

        // Wire drag-and-drop forwarding for bench slots
        for i in 0..MAX_BENCH {
            let path = format!("BenchRow/BenchSlot{i}");
            if let Some(node) = self.base().get_node_or_null(&path) {
                let mut btn: Gd<Button> = node.cast();
                let iv = (i as i64).to_variant();
                btn.set_drag_forwarding(
                    &self.base().callable("forward_bench_drag").bind(std::slice::from_ref(&iv)),
                    &self.base().callable("forward_bench_can_drop").bind(std::slice::from_ref(&iv)),
                    &self.base().callable("forward_bench_drop").bind(std::slice::from_ref(&iv)),
                );
            }
        }

        // Wire drag-and-drop forwarding for ability slots
        for h in 0..MAX_HEROES {
            for s in 0..MAX_SLOTS {
                let path = format!("HeroRow{h}/AbilitySlot{s}");
                if let Some(node) = self.base().get_node_or_null(&path) {
                    let mut btn: Gd<Button> = node.cast();
                    let hv = (h as i64).to_variant();
                    let sv = (s as i64).to_variant();
                    btn.set_drag_forwarding(
                        &self.base().callable("forward_slot_drag").bind(&[hv.clone(), sv.clone()]),
                        &self.base().callable("forward_slot_can_drop").bind(&[hv.clone(), sv.clone()]),
                        &self.base().callable("forward_slot_drop").bind(&[hv, sv]),
                    );
                }
            }
        }

        // Wire drag-and-drop forwarding for sell bin (drop-only)
        if let Some(node) = self.base().get_node_or_null("/root/MainScene/PersistentChrome/GodPortrait/SellBin") {
            let mut btn: Gd<Button> = node.cast();
            btn.set_drag_forwarding(
                &self.base().callable("forward_no_drag"),
                &self.base().callable("forward_sell_can_drop"),
                &self.base().callable("forward_sell_drop"),
            );
        }
    }

    fn process(&mut self, _delta: f64) {
        self.refresh_ui();
    }
}

#[godot_api]
impl LoadoutUi {
    #[func]
    fn refresh_ui(&mut self) {
        let Some(manager) = self.get_manager() else { return };
        let heroes = manager.bind().get_heroes(0);
        let bench = manager.bind().get_bench(0);

        // Update hero rows
        for i in 0..MAX_HEROES {
            let row_path = format!("HeroRow{i}");
            let Some(node) = self.base().get_node_or_null(&row_path) else { continue };
            let mut row: Gd<HBoxContainer> = node.cast();
            if i < heroes.len() {
                let hero_name = heroes.get(i).map(|g| g.to_string()).unwrap_or_default();
                row.set_visible(true);

                if let Some(node) = self.base().get_node_or_null(&format!("{row_path}/HeroPortrait")) {
                    let mut btn: Gd<Button> = node.cast();
                    btn.set_text(&hero_name);
                    let info = manager.bind().get_hero_info(GString::from(hero_name.as_str()));
                    let attr = info.get("attribute").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
                    let style = attribute_stylebox(&attr);
                    btn.add_theme_stylebox_override("normal", &style);
                }

                let equipped = manager.bind().get_equipped_abilities(0, GString::from(hero_name.as_str()));
                for s in 0..MAX_SLOTS {
                    let slot_path = format!("{row_path}/AbilitySlot{s}");
                    if let Some(node) = self.base().get_node_or_null(&slot_path) {
                        let mut btn: Gd<Button> = node.cast();
                        if s < equipped.len() {
                            let aname = equipped.get(s).map(|g| g.to_string()).unwrap_or_default();
                            let level = manager.bind().get_ability_level(0, GString::from(aname.as_str()));
                            let display = if level > 0 { format!("{aname} Lv{level}") } else { aname.clone() };
                            btn.set_text(&display);
                            btn.set_visible(true);
                            // Set tooltip
                            let info = manager.bind().get_ability_info(GString::from(aname.as_str()));
                            if !info.is_empty() {
                                btn.set_tooltip_text(&format_ability_tooltip(&info));
                            }
                            if manager.bind().get_ability_is_ultimate(GString::from(aname.as_str())) {
                                btn.add_theme_stylebox_override("normal", &ultimate_stylebox());
                            } else {
                                btn.remove_theme_stylebox_override("normal");
                            }
                        } else {
                            btn.set_text("[+]");
                            btn.set_visible(true);
                            btn.set_tooltip_text("");
                            btn.remove_theme_stylebox_override("normal");
                        }
                    }
                }
            } else {
                row.set_visible(false);
            }
        }

        // Update bench
        for i in 0..MAX_BENCH {
            let path = format!("BenchRow/BenchSlot{i}");
            if let Some(node) = self.base().get_node_or_null(&path) {
                let mut btn: Gd<Button> = node.cast();
                if i < bench.len() {
                    let name = bench.get(i).map(|g| g.to_string()).unwrap_or_default();
                    let level = manager.bind().get_ability_level(0, GString::from(name.as_str()));
                    let display = if level > 0 { format!("{name} Lv{level}") } else { name.clone() };
                    btn.set_text(&display);
                    btn.set_visible(true);
                    // Set tooltip
                    let info = manager.bind().get_ability_info(GString::from(name.as_str()));
                    if !info.is_empty() {
                        btn.set_tooltip_text(&format_ability_tooltip(&info));
                    }
                } else {
                    btn.set_visible(false);
                    btn.set_tooltip_text("");
                }
            }
        }
    }

    fn select_bench(&mut self, idx: usize) {
        let Some(manager) = self.get_manager() else { return };
        let bench = manager.bind().get_bench(0);
        if let Some(name) = bench.get(idx) {
            self.selected_ability = name.to_string();
            godot_print!("[AA2] Selected bench ability: {}", self.selected_ability);
        }
    }

    fn equip_to_slot(&mut self, hero_idx: usize, slot_idx: usize) {
        if !self.selected_ability.is_empty() {
            // Equip from bench
            let Some(manager) = self.get_manager() else { return };
            let heroes = manager.bind().get_heroes(0);
            let hero = heroes.get(hero_idx).map(|g| g.to_string()).unwrap_or_default();
            if hero.is_empty() { return; }
            let param = format!("{},{}", self.selected_ability, hero);
            godot_print!("[AA2] Equip: {param}");
            if let Some(mut mgr) = self.get_manager() {
                mgr.bind_mut().apply_player_action(0, "Equip".into(), GString::from(param.as_str()));
            }
            self.selected_ability.clear();
            self.swap_source = None;
            return;
        }

        // No bench ability selected — handle swap or unequip
        if let Some((src_hero, src_slot)) = self.swap_source
            && src_hero == hero_idx && src_slot != slot_idx
        {
                // Swap abilities within same hero
                let Some(manager) = self.get_manager() else { return };
                let heroes = manager.bind().get_heroes(0);
                let hero = heroes.get(hero_idx).map(|g| g.to_string()).unwrap_or_default();
                let param = format!("{hero},{src_slot},{slot_idx}");
                godot_print!("[AA2] Swap: {param}");
                if let Some(mut mgr) = self.get_manager() {
                    mgr.bind_mut().apply_player_action(0, "SwapAbilities".into(), GString::from(param.as_str()));
                }
                self.swap_source = None;
                return;
        }

        // First click on an equipped slot — check if it has an ability to start swap
        let Some(manager) = self.get_manager() else { return };
        let heroes = manager.bind().get_heroes(0);
        let hero = heroes.get(hero_idx).map(|g| g.to_string()).unwrap_or_default();
        let equipped = manager.bind().get_equipped_abilities(0, GString::from(hero.as_str()));
        if slot_idx < equipped.len() && !equipped.get(slot_idx).map(|g| g.to_string()).unwrap_or_default().is_empty() {
            // Start swap — select this slot as source
            self.swap_source = Some((hero_idx, slot_idx));
            godot_print!("[AA2] Swap source: hero {hero_idx} slot {slot_idx}");
        } else {
            // Empty slot or out of range — unequip (legacy behavior)
            self.swap_source = None;
            self.unequip_slot(hero_idx, slot_idx);
        }
    }

    fn unequip_slot(&mut self, hero_idx: usize, slot_idx: usize) {
        let Some(manager) = self.get_manager() else { return };
        let heroes = manager.bind().get_heroes(0);
        let hero = heroes.get(hero_idx).map(|g| g.to_string()).unwrap_or_default();
        let equipped = manager.bind().get_equipped_abilities(0, GString::from(hero.as_str()));
        let ability = equipped.get(slot_idx).map(|g| g.to_string()).unwrap_or_default();
        if ability.is_empty() || hero.is_empty() { return; }
        let param = format!("{ability},{hero}");
        godot_print!("[AA2] Unequip: {param}");
        if let Some(mut mgr) = self.get_manager() {
            mgr.bind_mut().apply_player_action(0, "Unequip".into(), GString::from(param.as_str()));
        }
    }

    fn get_manager(&self) -> Option<Gd<GameManager>> {
        self.base().get_node_or_null("/root/MainScene/GameManager")
            .map(|n| n.cast::<GameManager>())
    }

    // Hero select handlers
    #[func] fn on_hero_0(&mut self) { godot_print!("[AA2] Hero 0 selected"); }
    #[func] fn on_hero_1(&mut self) { godot_print!("[AA2] Hero 1 selected"); }
    #[func] fn on_hero_2(&mut self) { godot_print!("[AA2] Hero 2 selected"); }
    #[func] fn on_hero_3(&mut self) { godot_print!("[AA2] Hero 3 selected"); }
    #[func] fn on_hero_4(&mut self) { godot_print!("[AA2] Hero 4 selected"); }

    // Bench select handlers
    #[func] fn on_bench_0(&mut self) { self.select_bench(0); }
    #[func] fn on_bench_1(&mut self) { self.select_bench(1); }
    #[func] fn on_bench_2(&mut self) { self.select_bench(2); }
    #[func] fn on_bench_3(&mut self) { self.select_bench(3); }
    #[func] fn on_bench_4(&mut self) { self.select_bench(4); }

    // Equip slot handlers (hero_idx, slot_idx)
    #[func] fn on_equip_0_0(&mut self) { self.equip_to_slot(0, 0); }
    #[func] fn on_equip_0_1(&mut self) { self.equip_to_slot(0, 1); }
    #[func] fn on_equip_0_2(&mut self) { self.equip_to_slot(0, 2); }
    #[func] fn on_equip_0_3(&mut self) { self.equip_to_slot(0, 3); }
    #[func] fn on_equip_1_0(&mut self) { self.equip_to_slot(1, 0); }
    #[func] fn on_equip_1_1(&mut self) { self.equip_to_slot(1, 1); }
    #[func] fn on_equip_1_2(&mut self) { self.equip_to_slot(1, 2); }
    #[func] fn on_equip_1_3(&mut self) { self.equip_to_slot(1, 3); }
    #[func] fn on_equip_2_0(&mut self) { self.equip_to_slot(2, 0); }
    #[func] fn on_equip_2_1(&mut self) { self.equip_to_slot(2, 1); }
    #[func] fn on_equip_2_2(&mut self) { self.equip_to_slot(2, 2); }
    #[func] fn on_equip_2_3(&mut self) { self.equip_to_slot(2, 3); }
    #[func] fn on_equip_3_0(&mut self) { self.equip_to_slot(3, 0); }
    #[func] fn on_equip_3_1(&mut self) { self.equip_to_slot(3, 1); }
    #[func] fn on_equip_3_2(&mut self) { self.equip_to_slot(3, 2); }
    #[func] fn on_equip_3_3(&mut self) { self.equip_to_slot(3, 3); }
    #[func] fn on_equip_4_0(&mut self) { self.equip_to_slot(4, 0); }
    #[func] fn on_equip_4_1(&mut self) { self.equip_to_slot(4, 1); }
    #[func] fn on_equip_4_2(&mut self) { self.equip_to_slot(4, 2); }
    #[func] fn on_equip_4_3(&mut self) { self.equip_to_slot(4, 3); }

    // Reroll hero handlers
    #[func] fn on_reroll_hero_0(&mut self) { self.reroll_hero(0); }
    #[func] fn on_reroll_hero_1(&mut self) { self.reroll_hero(1); }
    #[func] fn on_reroll_hero_2(&mut self) { self.reroll_hero(2); }
    #[func] fn on_reroll_hero_3(&mut self) { self.reroll_hero(3); }
    #[func] fn on_reroll_hero_4(&mut self) { self.reroll_hero(4); }

    /// Sells the currently-selected ability (bench or equipped slot), refunding gold.
    #[func]
    fn on_sell_pressed(&mut self) {
        let name = if !self.selected_ability.is_empty() {
            self.selected_ability.clone()
        } else if let Some((hero_idx, slot_idx)) = self.swap_source {
            let Some(manager) = self.get_manager() else { return };
            let heroes = manager.bind().get_heroes(0);
            let hero = heroes.get(hero_idx).map(|g| g.to_string()).unwrap_or_default();
            let equipped = manager.bind().get_equipped_abilities(0, GString::from(hero.as_str()));
            let ability = equipped.get(slot_idx).map(|g| g.to_string()).unwrap_or_default();
            if ability.is_empty() {
                godot_print!("[AA2] Sell: no ability selected");
                return;
            }
            ability
        } else {
            godot_print!("[AA2] Sell: no ability selected");
            return;
        };
        if let Some(mut mgr) = self.get_manager() {
            mgr.bind_mut().apply_player_action(0, "Sell".into(), GString::from(name.as_str()));
        }
        godot_print!("[AA2] Sold: {name}");
        self.selected_ability.clear();
        self.swap_source = None;
    }

    fn reroll_hero(&mut self, hero_idx: usize) {
        let param = format!("{hero_idx}");
        godot_print!("[AA2] Reroll hero {hero_idx}");
        if let Some(mut mgr) = self.get_manager() {
            let result = mgr.bind_mut().apply_player_action(0, "RerollHero".into(), GString::from(param.as_str()));
            godot_print!("[AA2] Reroll result: {result}");
        }
    }

    // === Drag-and-drop glue methods ===

    /// Build drag payload for a bench ability at `idx`. Returns empty Dictionary if invalid.
    #[func]
    fn make_bench_payload(&self, idx: i32) -> VarDictionary {
        let mut dict = VarDictionary::new();
        let Some(manager) = self.get_manager() else { return dict };
        let bench = manager.bind().get_bench(0);
        if idx < 0 || (idx as usize) >= bench.len() { return dict; }
        let name = bench.get(idx as usize).map(|g| g.to_string()).unwrap_or_default();
        if name.is_empty() { return dict; }
        dict.set("kind", "ability");
        dict.set("ability", &Variant::from(GString::from(name.as_str())));
        dict.set("src", "bench");
        dict
    }

    /// Build drag payload for an equipped ability at `hero_idx`/`slot_idx`. Returns empty Dictionary if invalid.
    #[func]
    fn make_slot_payload(&self, hero_idx: i32, slot_idx: i32) -> VarDictionary {
        let mut dict = VarDictionary::new();
        let Some(manager) = self.get_manager() else { return dict };
        let heroes = manager.bind().get_heroes(0);
        if hero_idx < 0 || (hero_idx as usize) >= heroes.len() { return dict; }
        let hero = heroes.get(hero_idx as usize).map(|g| g.to_string()).unwrap_or_default();
        let equipped = manager.bind().get_equipped_abilities(0, GString::from(hero.as_str()));
        if slot_idx < 0 || (slot_idx as usize) >= equipped.len() { return dict; }
        let ability = equipped.get(slot_idx as usize).map(|g| g.to_string()).unwrap_or_default();
        if ability.is_empty() { return dict; }
        dict.set("kind", "ability");
        dict.set("ability", &Variant::from(GString::from(ability.as_str())));
        dict.set("src", "equipped");
        dict.set("hero", &Variant::from(GString::from(hero.as_str())));
        dict
    }

    /// Equip a bench ability to the hero at `hero_idx`. Returns true on success.
    #[func]
    fn drop_equip(&mut self, ability: GString, hero_idx: i32) -> bool {
        let Some(manager) = self.get_manager() else { return false };
        let heroes = manager.bind().get_heroes(0);
        if hero_idx < 0 || (hero_idx as usize) >= heroes.len() { return false; }
        let hero = heroes.get(hero_idx as usize).map(|g| g.to_string()).unwrap_or_default();
        if hero.is_empty() { return false; }
        let param = format!("{ability},{hero}");
        godot_print!("[AA2] DnD Equip: {param}");
        if let Some(mut mgr) = self.get_manager() {
            mgr.bind_mut().apply_player_action(0, "Equip".into(), GString::from(param.as_str()));
        }
        true
    }

    /// Sell an ability via drag-and-drop. Returns true.
    #[func]
    fn drop_sell(&mut self, ability: GString) -> bool {
        godot_print!("[AA2] DnD Sell: {ability}");
        if let Some(mut mgr) = self.get_manager() {
            mgr.bind_mut().apply_player_action(0, "Sell".into(), ability);
        }
        true
    }

    /// Unequip an ability from a hero via drag-and-drop. Returns true.
    #[func]
    fn drop_unequip(&mut self, ability: GString, hero: GString) -> bool {
        let param = format!("{ability},{hero}");
        godot_print!("[AA2] DnD Unequip: {param}");
        if let Some(mut mgr) = self.get_manager() {
            mgr.bind_mut().apply_player_action(0, "Unequip".into(), GString::from(param.as_str()));
        }
        true
    }

    // === Drag forwarding handlers ===

    /// Forwarding: start drag from bench slot (bound arg: idx).
    #[func]
    fn forward_bench_drag(&mut self, _pos: Vector2, idx: i64) -> Variant {
        let dict = self.make_bench_payload(idx as i32);
        if dict.is_empty() { return Variant::nil(); }
        dict.to_variant()
    }

    /// Forwarding: start drag from equipped ability slot (bound args: hero_idx, slot_idx).
    #[func]
    fn forward_slot_drag(&mut self, _pos: Vector2, hero_idx: i64, slot_idx: i64) -> Variant {
        let dict = self.make_slot_payload(hero_idx as i32, slot_idx as i32);
        if dict.is_empty() { return Variant::nil(); }
        dict.to_variant()
    }

    /// Forwarding: can drop onto ability slot? Only bench abilities.
    #[func]
    fn forward_slot_can_drop(&self, _pos: Vector2, data: Variant, _h: i64, _s: i64) -> bool {
        let Ok(dict) = data.try_to::<VarDictionary>() else { return false };
        let kind = dict.get("kind").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
        let src = dict.get("src").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
        kind == "ability" && src == "bench"
    }

    /// Forwarding: drop onto ability slot — equip from bench.
    #[func]
    fn forward_slot_drop(&mut self, _pos: Vector2, data: Variant, hero_idx: i64, _slot_idx: i64) {
        let Ok(dict) = data.try_to::<VarDictionary>() else { return };
        let ability = dict.get("ability").map(|v| v.to::<GString>()).unwrap_or_default();
        self.drop_equip(ability, hero_idx as i32);
    }

    /// Forwarding: can drop onto bench slot? Only equipped abilities (unequip).
    #[func]
    fn forward_bench_can_drop(&self, _pos: Vector2, data: Variant, _idx: i64) -> bool {
        let Ok(dict) = data.try_to::<VarDictionary>() else { return false };
        let kind = dict.get("kind").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
        let src = dict.get("src").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
        kind == "ability" && src == "equipped"
    }

    /// Forwarding: drop onto bench slot — unequip.
    #[func]
    fn forward_bench_drop(&mut self, _pos: Vector2, data: Variant, _idx: i64) {
        let Ok(dict) = data.try_to::<VarDictionary>() else { return };
        let ability = dict.get("ability").map(|v| v.to::<GString>()).unwrap_or_default();
        let hero = dict.get("hero").map(|v| v.to::<GString>()).unwrap_or_default();
        self.drop_unequip(ability, hero);
    }

    /// Forwarding: can drop onto sell bin? Any ability.
    #[func]
    fn forward_sell_can_drop(&self, _pos: Vector2, data: Variant) -> bool {
        let Ok(dict) = data.try_to::<VarDictionary>() else { return false };
        let kind = dict.get("kind").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
        kind == "ability"
    }

    /// Forwarding: drop onto sell bin — sell ability.
    #[func]
    fn forward_sell_drop(&mut self, _pos: Vector2, data: Variant) {
        let Ok(dict) = data.try_to::<VarDictionary>() else { return };
        let ability = dict.get("ability").map(|v| v.to::<GString>()).unwrap_or_default();
        self.drop_sell(ability);
    }

    /// Forwarding: no-drag for sell bin (drop-only target).
    #[func]
    fn forward_no_drag(&mut self, _pos: Vector2) -> Variant {
        Variant::nil()
    }
}
