use godot::prelude::*;
use godot::classes::{Button, Control, IControl, Label, VBoxContainer};

use crate::game_manager::GameManager;

#[derive(GodotClass)]
#[class(init, base=Control)]
pub struct DraftUI {
    base: Base<Control>,
}

#[godot_api]
impl IControl for DraftUI {
    fn ready(&mut self) {
        let mut vbox = VBoxContainer::new_alloc();
        vbox.set_name("VBox");

        let mut title = Label::new_alloc();
        title.set_name("Title");
        title.set_text("Draft a Hero");
        vbox.add_child(&title);

        for i in 0..3 {
            let mut btn = Button::new_alloc();
            let name = format!("Choice{i}");
            btn.set_name(&name);
            btn.set_text("—");
            let handler = format!("on_choice_{i}");
            btn.connect("pressed", &self.base().callable(&handler));
            vbox.add_child(&btn);
        }

        self.base_mut().add_child(&vbox);
    }

    fn process(&mut self, _delta: f64) {
        let Some(manager) = self.get_manager() else { return };
        let active = manager.bind().is_draft_active();
        if !active {
            self.base_mut().set_visible(false);
            return;
        }
        self.base_mut().set_visible(true);

        let choices = manager.bind().get_draft_choices(0);
        for i in 0..3 {
            let path = format!("VBox/Choice{i}");
            if let Some(mut btn) = self.try_get_node::<Button>(&path) {
                let text = choices.get(i)
                    .map(|g| g.to_string())
                    .unwrap_or_default();
                if text.is_empty() {
                    btn.set_text("—");
                    btn.set_disabled(true);
                } else {
                    btn.set_text(&text);
                    btn.set_disabled(false);
                }
            }
        }
    }
}

#[godot_api]
impl DraftUI {
    fn get_manager(&self) -> Option<Gd<GameManager>> {
        self.base().get_node_or_null("/root/GameManager")
            .map(|n| n.cast::<GameManager>())
    }

    fn try_get_node<T: GodotClass + Inherits<godot::classes::Node>>(&self, path: &str) -> Option<Gd<T>> {
        self.base().get_node_or_null(path).map(|n| n.cast::<T>())
    }

    fn draft_choice(&mut self, idx: usize) {
        if let Some(mut manager) = self.get_manager() {
            let param = format!("{idx}");
            manager.bind_mut().apply_player_action(0, "DraftHero".into(), GString::from(param.as_str()));
        }
    }

    #[func] fn on_choice_0(&mut self) { self.draft_choice(0); }
    #[func] fn on_choice_1(&mut self) { self.draft_choice(1); }
    #[func] fn on_choice_2(&mut self) { self.draft_choice(2); }
}
