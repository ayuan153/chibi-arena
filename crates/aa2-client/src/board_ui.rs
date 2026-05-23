use godot::prelude::*;
use godot::classes::{Button, Control, IControl, InputEvent, InputEventMouseButton, Label, Panel};
use godot::global::MouseButton;

use crate::game_manager::GameManager;

const BOARD_W: f32 = 600.0;
const BOARD_H: f32 = 300.0;
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
        let mut panel = Panel::new_alloc();
        panel.set_name("BoardPanel");
        panel.set_custom_minimum_size(Vector2::new(BOARD_W, BOARD_H));
        panel.set_size(Vector2::new(BOARD_W, BOARD_H));
        self.base_mut().add_child(&panel);

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
        status.set_position(Vector2::new(0.0, BOARD_H + 5.0));
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
            let gx = (pos.x / BOARD_W) * 2000.0;
            let gy = (pos.y / BOARD_H) * 1000.0;
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

        for i in 0..MAX_HEROES {
            let path = format!("Hero{i}");
            let Some(mut btn) = self.try_get_node::<Button>(&path) else { continue };
            if (i as i32) < heroes.len() as i32 {
                let name = heroes.get(i).map(|g| g.to_string()).unwrap_or_default();
                btn.set_visible(true);
                btn.set_text(&name);
                let pos = manager.bind().get_hero_position(0, GString::from(name.as_str()));
                let px = (pos.x / 2000.0) * BOARD_W;
                let py = (pos.y / 1000.0) * BOARD_H;
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
