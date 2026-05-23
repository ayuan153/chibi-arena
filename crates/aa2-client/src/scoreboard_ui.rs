use godot::prelude::*;
use godot::classes::{Control, IControl, Label, VBoxContainer};

use crate::game_manager::GameManager;

#[derive(GodotClass)]
#[class(init, base=Control)]
pub struct ScoreboardUI {
    base: Base<Control>,
}

#[godot_api]
impl IControl for ScoreboardUI {
    fn ready(&mut self) {
        let mut vbox = VBoxContainer::new_alloc();
        vbox.set_name("VBox");

        let mut title = Label::new_alloc();
        title.set_name("Title");
        title.set_text("Scoreboard");
        vbox.add_child(&title);

        for i in 0..8 {
            let mut label = Label::new_alloc();
            let name = format!("Player{i}");
            label.set_name(&name);
            label.set_text("—");
            vbox.add_child(&label);
        }

        self.base_mut().add_child(&vbox);
    }

    fn process(&mut self, _delta: f64) {
        let Some(manager) = self.get_manager() else { return };
        let count = manager.bind().get_player_count();

        for i in 0..8 {
            let path = format!("VBox/Player{i}");
            let Some(mut label) = self.try_get_node::<Label>(&path) else { continue };

            if i >= count {
                label.set_visible(false);
                continue;
            }
            label.set_visible(true);

            let hp = manager.bind().get_player_hp(i);
            let god = manager.bind().get_player_god(i);
            let alive = manager.bind().get_player_alive(i);
            let heroes = manager.bind().get_heroes(i);

            let hero_list: Vec<String> = (0..heroes.len())
                .filter_map(|idx| heroes.get(idx).map(|g| g.to_string()))
                .collect();
            let heroes_str = hero_list.join(", ");

            let god_str = if god.is_empty() { "—".to_string() } else { god.to_string() };
            let status = if alive { "ALIVE" } else { "DEAD" };
            let text = format!("P{i}: {hp:.0}HP | {god_str} | Heroes: {heroes_str} | {status}");
            label.set_text(&text);
        }
    }
}

#[godot_api]
impl ScoreboardUI {
    fn get_manager(&self) -> Option<Gd<GameManager>> {
        self.base().get_node_or_null("/root/MainScene/GameManager")
            .map(|n| n.cast::<GameManager>())
    }

    fn try_get_node<T: GodotClass + Inherits<godot::classes::Node>>(&self, path: &str) -> Option<Gd<T>> {
        self.base().get_node_or_null(path).map(|n| n.cast::<T>())
    }
}
