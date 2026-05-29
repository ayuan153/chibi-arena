use godot::prelude::*;
use godot::classes::{Button, ColorRect, Control, IControl, InputEvent, InputEventMouseButton, Label};
use godot::global::MouseButton;

use crate::game_manager::GameManager;
use crate::ui_helpers::attribute_stylebox;

const MAX_HEROES: usize = 5;

#[derive(GodotClass)]
#[class(init, base=Control)]
pub struct BoardUI {
    base: Base<Control>,
    #[init(val = None)]
    selected_hero: Option<String>,
}

#[godot_api]
impl IControl for BoardUI {
    fn ready(&mut self) {
        // Arena background
        let mut bg = ColorRect::new_alloc();
        bg.set_name("ArenaBg");
        bg.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        bg.set_color(Color::from_rgba(0.06, 0.08, 0.15, 1.0)); // dark navy
        bg.set_mouse_filter(godot::classes::control::MouseFilter::IGNORE);
        self.base_mut().add_child(&bg);

        // Center dividing line (drawn in process based on actual size)
        let mut line = ColorRect::new_alloc();
        line.set_name("CenterLine");
        line.set_color(Color::from_rgba(0.3, 0.3, 0.5, 0.6));
        line.set_mouse_filter(godot::classes::control::MouseFilter::IGNORE);
        self.base_mut().add_child(&line);

        // Create hero buttons
        for i in 0..MAX_HEROES {
            let mut btn = Button::new_alloc();
            let name = format!("Hero{i}");
            btn.set_name(&name);
            btn.set_visible(false);
            btn.set_size(Vector2::new(80.0, 30.0));
            let handler = format!("on_hero_{i}");
            btn.connect("pressed", &self.base().callable(&handler));
            self.base_mut().add_child(&btn);
        }

        let mut status = Label::new_alloc();
        status.set_name("StatusLabel");
        status.set_text("");
        self.base_mut().add_child(&status);

        // Wire drag-and-drop forwarding for hero buttons
        for i in 0..MAX_HEROES {
            let path = format!("Hero{i}");
            if let Some(node) = self.base().get_node_or_null(&path) {
                let mut btn: Gd<Button> = node.cast();
                let iv = (i as i64).to_variant();
                btn.set_drag_forwarding(
                    &self.base().callable("forward_hero_drag").bind(std::slice::from_ref(&iv)),
                    &self.base().callable("forward_hero_can_drop").bind(std::slice::from_ref(&iv)),
                    &self.base().callable("forward_hero_drop").bind(std::slice::from_ref(&iv)),
                );
            }
        }
    }

    fn process(&mut self, _delta: f64) {
        self.refresh();
    }

    fn gui_input(&mut self, event: Gd<InputEvent>) {
        if let Ok(mb) = event.try_cast::<InputEventMouseButton>()
            && mb.is_pressed() && mb.get_button_index() == MouseButton::LEFT
            && let Some(hero) = self.selected_hero.take()
        {
            let pos = mb.get_position();
            let size = self.base().get_size();
            let gx = ((pos.x / size.x) * 2000.0).clamp(0.0, 2000.0);
            // Constrain to bottom half of arena (y: 1000-2000)
            let gy = ((pos.y / size.y) * 2000.0).clamp(1000.0, 2000.0);
            let param = format!("{hero},{gx},{gy}");
            if let Some(mut mgr) = self.get_manager() {
                mgr.bind_mut().apply_player_action(0, "SetPosition".into(), GString::from(param.as_str()));
            }
            if let Some(mut lbl) = self.try_get_node::<Label>("StatusLabel") {
                lbl.set_text("");
            }
            // Hide unit info panel
            if let Some(node) = self.base().get_node_or_null("/root/MainScene/UnitInfo") {
                let mut ctrl: Gd<Control> = node.cast();
                ctrl.set_visible(false);
            }
        }
    }

    fn can_drop_data(&self, _at_position: Vector2, data: Variant) -> bool {
        let Ok(dict) = data.try_to::<VarDictionary>() else { return false };
        let kind = dict.get("kind").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
        kind == "hero"
    }

    fn drop_data(&mut self, _at_position: Vector2, data: Variant) {
        let Ok(dict) = data.try_to::<VarDictionary>() else { return };
        let hero = dict.get("hero").map(|v| v.to::<GString>()).unwrap_or_default();
        let local = self.base().get_local_mouse_position();
        let size = self.base().get_size();
        let gx = ((local.x / size.x) * 2000.0).clamp(0.0, 2000.0);
        let gy = ((local.y / size.y) * 2000.0).clamp(1000.0, 2000.0);
        self.reposition_hero(hero, gx, gy);
    }
}

#[godot_api]
impl BoardUI {
    fn refresh(&mut self) {
        let Some(manager) = self.get_manager() else { return };
        let heroes = manager.bind().get_heroes(0);
        let size = self.base().get_size();

        // Position center dividing line
        if let Some(mut line) = self.try_get_node::<ColorRect>("CenterLine") {
            line.set_position(Vector2::new(0.0, size.y * 0.5 - 1.0));
            line.set_size(Vector2::new(size.x, 2.0));
        }

        for i in 0..MAX_HEROES {
            let path = format!("Hero{i}");
            let Some(mut btn) = self.try_get_node::<Button>(&path) else { continue };
            if (i as i32) < heroes.len() as i32 {
                let name = heroes.get(i).map(|g| g.to_string()).unwrap_or_default();
                btn.set_visible(true);
                btn.set_text(&name);
                let info = manager.bind().get_hero_info(GString::from(name.as_str()));
                let attr = info.get("attribute").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
                btn.add_theme_stylebox_override("normal", &attribute_stylebox(&attr));
                let pos = manager.bind().get_hero_position(0, GString::from(name.as_str()));
                let px = (pos.x / 2000.0) * size.x;
                let py = (pos.y / 2000.0) * size.y;
                btn.set_position(Vector2::new(px, py));
            } else {
                btn.set_visible(false);
            }
        }
    }

    fn select_hero(&mut self, idx: usize) {
        let Some(manager) = self.get_manager() else { return };
        let heroes = manager.bind().get_heroes(0);
        if let Some(name) = heroes.get(idx).map(|g| g.to_string()) {
            self.selected_hero = Some(name.clone());
            if let Some(mut lbl) = self.try_get_node::<Label>("StatusLabel") {
                let text = format!("Selected: {name} — click board to move");
                lbl.set_text(&text);
            }
            // Show unit info panel
            let info = manager.bind().get_hero_info(GString::from(name.as_str()));
            self.update_unit_info(&info);
        }
    }

    fn update_unit_info(&self, info: &VarDictionary) {
        let Some(panel) = self.base().get_node_or_null("/root/MainScene/UnitInfo") else { return };
        let mut ctrl: Gd<Control> = panel.cast();
        ctrl.set_visible(true);

        let name = info.get("name").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
        let attr = info.get("attribute").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
        let hp = info.get("hp").map(|v| v.to::<i32>()).unwrap_or(0);
        let mana = info.get("mana").map(|v| v.to::<i32>()).unwrap_or(0);
        let str_val = info.get("str").map(|v| v.to::<i32>()).unwrap_or(0);
        let agi_val = info.get("agi").map(|v| v.to::<i32>()).unwrap_or(0);
        let int_val = info.get("int").map(|v| v.to::<i32>()).unwrap_or(0);
        let armor = info.get("armor").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
        let as_val = info.get("attack_speed").map(|v| v.to::<i32>()).unwrap_or(0);
        let dmg = info.get("damage").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
        let ms = info.get("move_speed").map(|v| v.to::<i32>()).unwrap_or(0);
        let range = info.get("attack_range").map(|v| v.to::<i32>()).unwrap_or(0);

        let text = format!(
            "{name} [{attr}]\nHP: {hp}  Mana: {mana}\nSTR: {str_val}  AGI: {agi_val}  INT: {int_val}\nArmor: {armor}  AS: {as_val}\nDmg: {dmg}  MS: {ms}  Range: {range}"
        );

        if let Some(node) = ctrl.get_node_or_null("InfoLabel") {
            let mut label: Gd<Label> = node.cast();
            label.set_text(&text);
        } else {
            // Create label on first use
            let mut label = Label::new_alloc();
            label.set_name("InfoLabel");
            label.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);
            label.set_text(&text);
            ctrl.add_child(&label);
        }
    }

    fn get_manager(&self) -> Option<Gd<GameManager>> {
        self.base().get_node_or_null("/root/MainScene/GameManager")
            .map(|n| n.cast::<GameManager>())
    }

    fn try_get_node<T: GodotClass + Inherits<godot::classes::Node>>(&self, path: &str) -> Option<Gd<T>> {
        self.base().get_node_or_null(path).map(|n| n.cast::<T>())
    }

    #[func] fn on_hero_0(&mut self) { self.select_hero(0); }
    #[func] fn on_hero_1(&mut self) { self.select_hero(1); }
    #[func] fn on_hero_2(&mut self) { self.select_hero(2); }
    #[func] fn on_hero_3(&mut self) { self.select_hero(3); }
    #[func] fn on_hero_4(&mut self) { self.select_hero(4); }

    // === Drag-and-drop glue methods ===

    /// Reposition a hero to grid coordinates via drag-and-drop. Returns true.
    #[func]
    fn reposition_hero(&mut self, hero: GString, gx: f32, gy: f32) -> bool {
        let param = format!("{hero},{gx},{gy}");
        godot_print!("[AA2] DnD Reposition: {param}");
        if let Some(mut mgr) = self.get_manager() {
            mgr.bind_mut().apply_player_action(0, "SetPosition".into(), GString::from(param.as_str()));
        }
        true
    }

    /// Build drag payload for a hero at `idx`. Returns empty Dictionary if invalid.
    #[func]
    fn make_hero_payload(&self, idx: i32) -> VarDictionary {
        let mut dict = VarDictionary::new();
        let Some(manager) = self.get_manager() else { return dict };
        let heroes = manager.bind().get_heroes(0);
        if idx < 0 || (idx as usize) >= heroes.len() { return dict; }
        let name = heroes.get(idx as usize).map(|g| g.to_string()).unwrap_or_default();
        if name.is_empty() { return dict; }
        dict.set("kind", "hero");
        dict.set("hero", &Variant::from(GString::from(name.as_str())));
        dict
    }

    // === Drag forwarding handlers ===

    /// Forwarding: start drag from hero button (bound arg: idx).
    #[func]
    fn forward_hero_drag(&mut self, _pos: Vector2, idx: i64) -> Variant {
        let dict = self.make_hero_payload(idx as i32);
        if dict.is_empty() { return Variant::nil(); }
        dict.to_variant()
    }

    /// Forwarding: can drop onto hero button? Only heroes (reposition).
    #[func]
    fn forward_hero_can_drop(&self, _pos: Vector2, data: Variant, _idx: i64) -> bool {
        let Ok(dict) = data.try_to::<VarDictionary>() else { return false };
        let kind = dict.get("kind").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
        kind == "hero"
    }

    /// Forwarding: drop onto hero button — reposition using local mouse position.
    #[func]
    fn forward_hero_drop(&mut self, _pos: Vector2, data: Variant, _idx: i64) {
        let Ok(dict) = data.try_to::<VarDictionary>() else { return };
        let hero = dict.get("hero").map(|v| v.to::<GString>()).unwrap_or_default();
        let local = self.base().get_local_mouse_position();
        let size = self.base().get_size();
        let gx = ((local.x / size.x) * 2000.0).clamp(0.0, 2000.0);
        let gy = ((local.y / size.y) * 2000.0).clamp(1000.0, 2000.0);
        self.reposition_hero(hero, gx, gy);
    }
}
