use godot::prelude::*;
use godot::classes::{Button, ColorRect, Control, HBoxContainer, IControl, Label, VBoxContainer};

use crate::game_manager::GameManager;

#[derive(GodotClass)]
#[class(init, base=Control)]
pub struct EndgameUI {
    base: Base<Control>,
}

#[godot_api]
impl IControl for EndgameUI {
    fn ready(&mut self) {
        self.base_mut().set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        self.base_mut().set_visible(false);

        // Dark background
        let mut bg = ColorRect::new_alloc();
        bg.set_name("Background");
        bg.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        bg.set_color(Color::from_rgba(0.02, 0.02, 0.08, 0.9));
        self.base_mut().add_child(&bg);

        // Main layout
        let mut root_vbox = VBoxContainer::new_alloc();
        root_vbox.set_name("RootVBox");
        root_vbox.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);

        // Placement title
        let mut title = Label::new_alloc();
        title.set_name("PlacementTitle");
        title.set_text("Game Over");
        title.set_horizontal_alignment(godot::global::HorizontalAlignment::CENTER);
        root_vbox.add_child(&title);

        // Player rows
        let mut rows_vbox = VBoxContainer::new_alloc();
        rows_vbox.set_name("PlayerRows");
        rows_vbox.set_v_size_flags(godot::classes::control::SizeFlags::EXPAND_FILL);

        for i in 0..8 {
            let mut row = HBoxContainer::new_alloc();
            row.set_name(&format!("Row{i}"));

            let mut rank_label = Label::new_alloc();
            rank_label.set_name("Rank");
            rank_label.set_custom_minimum_size(Vector2::new(40.0, 0.0));
            row.add_child(&rank_label);

            let mut name_label = Label::new_alloc();
            name_label.set_name("Name");
            name_label.set_custom_minimum_size(Vector2::new(100.0, 0.0));
            row.add_child(&name_label);

            let mut god_label = Label::new_alloc();
            god_label.set_name("God");
            god_label.set_custom_minimum_size(Vector2::new(120.0, 0.0));
            row.add_child(&god_label);

            let mut heroes_label = Label::new_alloc();
            heroes_label.set_name("Heroes");
            heroes_label.set_h_size_flags(godot::classes::control::SizeFlags::EXPAND_FILL);
            row.add_child(&heroes_label);

            rows_vbox.add_child(&row);
        }

        root_vbox.add_child(&rows_vbox);

        // Buttons
        let mut btn_row = HBoxContainer::new_alloc();
        btn_row.set_name("Buttons");
        btn_row.set_alignment(godot::classes::box_container::AlignmentMode::CENTER);

        let mut spectate_btn = Button::new_alloc();
        spectate_btn.set_name("SpectateBtn");
        spectate_btn.set_text("SPECTATE MATCH");
        spectate_btn.connect("pressed", &Callable::from_object_method(&self.base(), "on_spectate_pressed"));
        btn_row.add_child(&spectate_btn);

        let mut disconnect_btn = Button::new_alloc();
        disconnect_btn.set_name("DisconnectBtn");
        disconnect_btn.set_text("DISCONNECT");
        btn_row.add_child(&disconnect_btn);

        root_vbox.add_child(&btn_row);
        self.base_mut().add_child(&root_vbox);
    }

    fn process(&mut self, _delta: f64) {
        if !self.base().is_visible() {
            return;
        }

        let Some(manager) = self.get_manager() else { return };

        // Update placement title
        let placement = manager.bind().get_player_placement(0);
        let suffix = match placement {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        };
        let title_text = format!("You placed {placement}{suffix}!");
        if let Some(mut title) = self.try_get_node::<Label>("RootVBox/PlacementTitle") {
            title.set_text(&title_text);
        }

        // Update player rows
        let count = manager.bind().get_player_count();
        for i in 0..8 {
            let path = format!("RootVBox/PlayerRows/Row{i}");
            let Some(mut row) = self.try_get_node::<HBoxContainer>(&path) else { continue };

            if i >= count {
                row.set_visible(false);
                continue;
            }
            row.set_visible(true);

            let alive = manager.bind().get_player_alive(i);
            let god = manager.bind().get_player_god(i);
            let heroes = manager.bind().get_heroes(i);
            let hero_list: Vec<String> = (0..heroes.len())
                .filter_map(|idx| heroes.get(idx).map(|g| g.to_string()))
                .collect();

            let rank = if alive { "—".to_string() } else {
                let p = manager.bind().get_player_placement(i);
                format!("#{p}")
            };

            if let Some(mut l) = self.try_get_node::<Label>(&format!("{path}/Rank")) {
                l.set_text(&rank);
            }
            if let Some(mut l) = self.try_get_node::<Label>(&format!("{path}/Name")) {
                l.set_text(&format!("Player {}", i + 1));
            }
            if let Some(mut l) = self.try_get_node::<Label>(&format!("{path}/God")) {
                l.set_text(&if god.is_empty() { "—".into() } else { god.to_string() });
            }
            if let Some(mut l) = self.try_get_node::<Label>(&format!("{path}/Heroes")) {
                l.set_text(&hero_list.join(", "));
            }
        }
    }
}

#[godot_api]
impl EndgameUI {
    #[func]
    fn on_spectate_pressed(&mut self) {
        self.base_mut().set_visible(false);
    }

    fn get_manager(&self) -> Option<Gd<GameManager>> {
        self.base().get_node_or_null("/root/MainScene/GameManager")
            .map(|n| n.cast::<GameManager>())
    }

    fn try_get_node<T: GodotClass + Inherits<godot::classes::Node>>(&self, path: &str) -> Option<Gd<T>> {
        self.base().get_node_or_null(path).map(|n| n.cast::<T>())
    }
}
