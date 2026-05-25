use godot::prelude::*;
use godot::classes::{IVBoxContainer, Label, VBoxContainer};

use crate::game_manager::GameManager;

const MAX_PLAYERS: usize = 8;

/// Left sidebar showing all players with HP and god name.
#[derive(GodotClass)]
#[class(init, base=VBoxContainer)]
pub struct PlayerListUi {
    base: Base<VBoxContainer>,
}

#[godot_api]
impl IVBoxContainer for PlayerListUi {
    fn ready(&mut self) {
        godot_print!("[AA2] PlayerListUi ready");
        for i in 0..MAX_PLAYERS {
            let mut label = Label::new_alloc();
            label.set_name(&format!("Player{i}"));
            label.set_text(&format!("P{} --", i + 1));
            label.set_visible(false);
            self.base_mut().add_child(&label);
        }
    }

    fn process(&mut self, _delta: f64) {
        self.refresh_ui();
    }
}

#[godot_api]
impl PlayerListUi {
    #[func]
    fn refresh_ui(&mut self) {
        let Some(manager) = self.get_manager() else { return };
        let count = manager.bind().get_player_count();

        for i in 0..MAX_PLAYERS {
            let path = format!("Player{i}");
            if let Some(node) = self.base().get_node_or_null(&path) {
                let mut label: Gd<Label> = node.cast();
                if (i as i32) < count {
                    let hp = manager.bind().get_player_hp(i as i32);
                    let god = manager.bind().get_player_god(i as i32).to_string();
                    let alive = manager.bind().get_player_alive(i as i32);
                    let status = if alive { "" } else { " [DEAD]" };
                    let god_display = if god.is_empty() { "?".to_string() } else { god };
                    let text = format!("P{} {god_display} HP:{hp:.0}{status}", i + 1);
                    label.set_text(&text);
                    label.set_visible(true);
                } else {
                    label.set_visible(false);
                }
            }
        }
    }

    fn get_manager(&self) -> Option<Gd<GameManager>> {
        self.base().get_node_or_null("/root/MainScene/GameManager")
            .map(|n| n.cast::<GameManager>())
    }
}
