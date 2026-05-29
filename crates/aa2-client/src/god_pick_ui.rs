use godot::prelude::*;
use godot::classes::{
    Button, ColorRect, Control, GridContainer, HBoxContainer, IControl, Label,
    ProgressBar, VBoxContainer,
};

use crate::game_manager::GameManager;

const MAX_GODS: usize = 20;

/// Full-screen god pick overlay with a 10-column grid of god portrait buttons,
/// a right-side preview panel, confirm/discard buttons, and a timer bar.
#[derive(GodotClass)]
#[class(init, base=Control)]
pub struct GodPickUI {
    base: Base<Control>,
    selected_god: Option<String>,
    god_names: Vec<String>,
}

#[godot_api]
impl IControl for GodPickUI {
    fn ready(&mut self) {
        // Full-screen overlay
        self.base_mut().set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);

        // Semi-transparent background
        let mut bg = ColorRect::new_alloc();
        bg.set_name("Background");
        bg.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        bg.set_color(Color::from_rgba(0.1, 0.1, 0.15, 0.9));
        self.base_mut().add_child(&bg);

        // Main vertical layout
        let mut root_vbox = VBoxContainer::new_alloc();
        root_vbox.set_name("RootVBox");
        root_vbox.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);

        // Title
        let mut title = Label::new_alloc();
        title.set_name("Title");
        title.set_text("Draft Your God");
        root_vbox.add_child(&title);

        // Main content: HBox with grid (left) and preview (right)
        let mut content = HBoxContainer::new_alloc();
        content.set_name("Content");
        content.set_v_size_flags(godot::classes::control::SizeFlags::EXPAND_FILL);

        // LEFT: Grid of god buttons (populated lazily in process() after init_game)
        let mut grid = GridContainer::new_alloc();
        grid.set_name("GodGrid");
        grid.set_columns(10);
        grid.set_h_size_flags(godot::classes::control::SizeFlags::EXPAND_FILL);
        content.add_child(&grid);

        // RIGHT: Preview panel
        let mut preview = VBoxContainer::new_alloc();
        preview.set_name("Preview");
        preview.set_custom_minimum_size(Vector2::new(250.0, 0.0));

        let mut name_label = Label::new_alloc();
        name_label.set_name("GodName");
        name_label.set_text("Select a god");
        preview.add_child(&name_label);

        let mut desc_label = Label::new_alloc();
        desc_label.set_name("GodDesc");
        desc_label.set_text("");
        desc_label.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
        preview.add_child(&desc_label);

        let mut hp_label = Label::new_alloc();
        hp_label.set_name("GodHP");
        hp_label.set_text("\u{2764} 200");
        preview.add_child(&hp_label);

        let mut confirm_btn = Button::new_alloc();
        confirm_btn.set_name("ConfirmBtn");
        confirm_btn.set_text("Confirm");
        confirm_btn.connect("pressed", &self.base().callable("on_confirm"));
        preview.add_child(&confirm_btn);

        let mut discard_btn = Button::new_alloc();
        discard_btn.set_name("DiscardBtn");
        discard_btn.set_text("Discard");
        discard_btn.connect("pressed", &self.base().callable("on_discard"));
        preview.add_child(&discard_btn);

        content.add_child(&preview);
        root_vbox.add_child(&content);

        // BOTTOM: Timer bar (visual only)
        let mut timer = ProgressBar::new_alloc();
        timer.set_name("TimerBar");
        timer.set_custom_minimum_size(Vector2::new(0.0, 20.0));
        timer.set_value(100.0);
        root_vbox.add_child(&timer);

        self.base_mut().add_child(&root_vbox);
    }

    fn process(&mut self, _delta: f64) {
        let Some(manager) = self.get_manager() else { return };
        let phase = manager.bind().get_phase();
        if phase != "GodPick" {
            self.base_mut().set_visible(false);
            return;
        }
        self.base_mut().set_visible(true);

        // Lazy-populate grid once gods are available (after init_game)
        if self.god_names.is_empty() {
            let gods = manager.bind().get_available_gods();
            if gods.is_empty() {
                return;
            }
            let Some(mut grid) = self.try_get_node::<GridContainer>("RootVBox/Content/GodGrid") else { return };
            for i in 0..gods.len().min(MAX_GODS) {
                let Some(dict) = gods.get(i) else { continue };
                let name = dict.get("name").unwrap_or_default().to::<GString>().to_string();
                self.god_names.push(name.clone());

                let mut btn = Button::new_alloc();
                btn.set_name(&format!("GodBtn{i}"));
                btn.set_custom_minimum_size(Vector2::new(80.0, 80.0));
                btn.set_text(&name);
                let handler = format!("on_god_{i}");
                btn.connect("pressed", &self.base().callable(&handler));
                grid.add_child(&btn);
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

    fn select_god(&mut self, idx: usize) {
        let Some(name) = self.god_names.get(idx).cloned() else { return };
        self.selected_god = Some(name.clone());

        // Update preview
        if let Some(mut label) = self.try_get_node::<Label>("RootVBox/Content/Preview/GodName") {
            label.set_text(&name);
        }
        if let Some(manager) = self.get_manager() {
            let gods = manager.bind().get_available_gods();
            if let Some(dict) = gods.get(idx) {
                let desc = dict.get("description").unwrap_or_default().to::<GString>();
                if let Some(mut label) = self.try_get_node::<Label>("RootVBox/Content/Preview/GodDesc") {
                    label.set_text(&desc.to_string());
                }
            }
        }

        // Highlight selected button
        for i in 0..self.god_names.len() {
            let path = format!("RootVBox/Content/GodGrid/GodBtn{i}");
            if let Some(mut btn) = self.try_get_node::<Button>(&path) {
                if i == idx {
                    btn.set_flat(false);
                    btn.set_disabled(false);
                } else {
                    btn.set_flat(true);
                }
            }
        }
    }

    #[func]
    fn on_confirm(&mut self) {
        let Some(god_name) = self.selected_god.clone() else { return };
        godot_print!("[AA2] Player picking god: {god_name}");
        if let Some(mut mgr) = self.get_manager() {
            let result = mgr.bind_mut().apply_player_action(
                0,
                "PickGod".into(),
                GString::from(god_name.as_str()),
            );
            godot_print!("[AA2] PickGod result: {result}");
        }
    }

    #[func]
    fn on_discard(&mut self) {
        self.selected_god = None;
        if let Some(mut label) = self.try_get_node::<Label>("RootVBox/Content/Preview/GodName") {
            label.set_text("Select a god");
        }
        if let Some(mut label) = self.try_get_node::<Label>("RootVBox/Content/Preview/GodDesc") {
            label.set_text("");
        }
        // Un-highlight all buttons
        for i in 0..self.god_names.len() {
            let path = format!("RootVBox/Content/GodGrid/GodBtn{i}");
            if let Some(mut btn) = self.try_get_node::<Button>(&path) {
                btn.set_flat(false);
            }
        }
    }

    // TODO: collapse into single handler when gdext supports Callable::bindv with i32 arg
    #[func] fn on_god_0(&mut self) { self.select_god(0); }
    #[func] fn on_god_1(&mut self) { self.select_god(1); }
    #[func] fn on_god_2(&mut self) { self.select_god(2); }
    #[func] fn on_god_3(&mut self) { self.select_god(3); }
    #[func] fn on_god_4(&mut self) { self.select_god(4); }
    #[func] fn on_god_5(&mut self) { self.select_god(5); }
    #[func] fn on_god_6(&mut self) { self.select_god(6); }
    #[func] fn on_god_7(&mut self) { self.select_god(7); }
    #[func] fn on_god_8(&mut self) { self.select_god(8); }
    #[func] fn on_god_9(&mut self) { self.select_god(9); }
    #[func] fn on_god_10(&mut self) { self.select_god(10); }
    #[func] fn on_god_11(&mut self) { self.select_god(11); }
    #[func] fn on_god_12(&mut self) { self.select_god(12); }
    #[func] fn on_god_13(&mut self) { self.select_god(13); }
    #[func] fn on_god_14(&mut self) { self.select_god(14); }
    #[func] fn on_god_15(&mut self) { self.select_god(15); }
    #[func] fn on_god_16(&mut self) { self.select_god(16); }
    #[func] fn on_god_17(&mut self) { self.select_god(17); }
    #[func] fn on_god_18(&mut self) { self.select_god(18); }
    #[func] fn on_god_19(&mut self) { self.select_god(19); }
}
