use godot::prelude::*;
use godot::classes::{Button, Control, IControl, Label, VBoxContainer};

use crate::game_manager::GameManager;

#[derive(GodotClass)]
#[class(init, base=Control)]
pub struct GodPickUI {
    base: Base<Control>,
}

#[godot_api]
impl IControl for GodPickUI {
    fn ready(&mut self) {
        let mut vbox = VBoxContainer::new_alloc();
        vbox.set_name("VBox");

        let mut title = Label::new_alloc();
        title.set_name("Title");
        title.set_text("Choose Your God");
        vbox.add_child(&title);

        if let Some(manager) = self.get_manager() {
            let gods = manager.bind().get_available_gods();
            for i in 0..gods.len() {
                let Some(dict) = gods.get(i) else { continue };
                let name = dict.get("name").unwrap_or_default().to::<GString>();
                let desc = dict.get("description").unwrap_or_default().to::<GString>();
                let mut btn = Button::new_alloc();
                let btn_name = format!("God{i}");
                btn.set_name(&btn_name);
                let text = format!("{name} — {desc}");
                btn.set_text(&text);
                let handler = format!("on_god_{i}");
                btn.connect("pressed", &self.base().callable(&handler));
                vbox.add_child(&btn);
            }
        }

        self.base_mut().add_child(&vbox);
    }

    fn process(&mut self, _delta: f64) {
        let Some(manager) = self.get_manager() else { return };
        let phase = manager.bind().get_phase();
        if phase != "GodPick" {
            self.base_mut().set_visible(false);
            return;
        }
        self.base_mut().set_visible(true);

        let god_name = manager.bind().get_player_god(0);
        if !god_name.is_empty() {
            if let Some(mut title) = self.try_get_node::<Label>("VBox/Title") {
                let text = format!("Selected: {god_name}");
                title.set_text(&text);
            }
            // Disable buttons
            for i in 0..2 {
                let path = format!("VBox/God{i}");
                if let Some(mut btn) = self.try_get_node::<Button>(&path) {
                    btn.set_disabled(true);
                }
            }
        }
    }
}

#[godot_api]
impl GodPickUI {
    fn get_manager(&self) -> Option<Gd<GameManager>> {
        self.base().get_node_or_null("/root/MainScene/GameManager")
            .map(|n| n.cast::<GameManager>())
    }

    fn try_get_node<T: GodotClass + Inherits<godot::classes::Node>>(&self, path: &str) -> Option<Gd<T>> {
        self.base().get_node_or_null(path).map(|n| n.cast::<T>())
    }

    fn pick_god(&mut self, idx: usize) {
        let Some(manager) = self.get_manager() else { return };
        let gods = manager.bind().get_available_gods();
        if let Some(dict) = gods.get(idx) {
            let name = dict.get("name").unwrap_or_default().to::<GString>();
            godot_print!("[AA2] Player picking god: {name}");
            if let Some(mut mgr) = self.get_manager() {
                let result = mgr.bind_mut().apply_player_action(0, "PickGod".into(), name.clone());
                godot_print!("[AA2] PickGod result: {result}");
            }
        }
    }

    #[func] fn on_god_0(&mut self) { self.pick_god(0); }
    #[func] fn on_god_1(&mut self) { self.pick_god(1); }
}
