use godot::prelude::*;
use godot::classes::{Button, Control, IControl, InputEvent, InputEventMouseButton, Label};
use godot::global::MouseButton;

use crate::game_manager::GameManager;

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
        }
    }
}

#[godot_api]
impl BoardUI {
    fn refresh(&mut self) {
        let Some(manager) = self.get_manager() else { return };
        let heroes = manager.bind().get_heroes(0);
        let size = self.base().get_size();

        for i in 0..MAX_HEROES {
            let path = format!("Hero{i}");
            let Some(mut btn) = self.try_get_node::<Button>(&path) else { continue };
            if (i as i32) < heroes.len() as i32 {
                let name = heroes.get(i).map(|g| g.to_string()).unwrap_or_default();
                btn.set_visible(true);
                btn.set_text(&name);
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
}
