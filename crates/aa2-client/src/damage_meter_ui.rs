use godot::prelude::*;
use godot::classes::{Control, HBoxContainer, IControl, Label, ProgressBar, VBoxContainer};

use crate::game_manager::GameManager;

/// Right-side panel showing total damage dealt per unit, grouped by team.
#[derive(GodotClass)]
#[class(init, base=Control)]
pub struct DamageMeter {
    base: Base<Control>,
    /// Signature to avoid per-frame rebuilds.
    #[init(val = -1)]
    last_sig: i64,
}

#[godot_api]
impl IControl for DamageMeter {
    fn ready(&mut self) {
        let mut root = VBoxContainer::new_alloc();
        root.set_name("Root");
        root.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        self.base_mut().add_child(&root);

        let mut title = Label::new_alloc();
        title.set_name("Title");
        title.set_text("DAMAGE DEALT");
        title.set_horizontal_alignment(godot::global::HorizontalAlignment::CENTER);
        root.add_child(&title);

        let mut list = VBoxContainer::new_alloc();
        list.set_name("List");
        root.add_child(&list);
    }

    fn process(&mut self, _delta: f64) {
        let Some(manager) = self.get_manager() else { return };
        let matchup_count = manager.bind().get_combat_matchup_count();

        if matchup_count == 0 {
            if self.last_sig != -1 {
                self.clear_list();
                self.last_sig = -1;
            }
            return;
        }

        let summary = manager.bind().get_damage_summary(0);
        let mut sig: i64 = matchup_count as i64 * 1_000_003;
        sig += summary.len() as i64;
        for i in 0..summary.len() {
            if let Some(d) = summary.get(i) {
                sig += d.get("damage").unwrap_or_default().to::<i64>();
            }
        }

        if sig == self.last_sig {
            return;
        }
        self.last_sig = sig;

        self.clear_list();

        // Find max damage for progress bar scaling
        let mut max_damage: i32 = 1;
        for i in 0..summary.len() {
            if let Some(d) = summary.get(i) {
                let dmg = d.get("damage").unwrap_or_default().to::<i32>();
                if dmg > max_damage {
                    max_damage = dmg;
                }
            }
        }

        let Some(list_node) = self.base().get_node_or_null("Root/List") else { return };
        let mut list: Gd<VBoxContainer> = list_node.cast();

        // Collect entries by team
        let mut enemy_entries = Vec::new();
        let mut own_entries = Vec::new();
        for i in 0..summary.len() {
            if let Some(d) = summary.get(i) {
                let team = d.get("team").unwrap_or_default().to::<i32>();
                if team == 1 {
                    enemy_entries.push(d.clone());
                } else {
                    own_entries.push(d.clone());
                }
            }
        }

        // Enemy group first
        if !enemy_entries.is_empty() {
            let mut header = Label::new_alloc();
            header.set_text("ENEMY");
            header.add_theme_color_override("font_color", Color::from_rgb(1.0, 0.3, 0.3));
            list.add_child(&header);
            for entry in &enemy_entries {
                self.add_row(&mut list, entry, max_damage, Color::from_rgb(0.8, 0.2, 0.2));
            }
        }

        // Own group
        if !own_entries.is_empty() {
            let mut header = Label::new_alloc();
            header.set_text("YOU");
            header.add_theme_color_override("font_color", Color::from_rgb(0.3, 1.0, 0.3));
            list.add_child(&header);
            for entry in &own_entries {
                self.add_row(&mut list, entry, max_damage, Color::from_rgb(0.2, 0.4, 0.8));
            }
        }
    }
}

impl DamageMeter {
    fn get_manager(&self) -> Option<Gd<GameManager>> {
        self.base().get_node_or_null("/root/MainScene/GameManager")
            .map(|n| n.cast::<GameManager>())
    }

    fn clear_list(&self) {
        let Some(list_node) = self.base().get_node_or_null("Root/List") else { return };
        let list: Gd<VBoxContainer> = list_node.cast();
        let children = list.get_children();
        for i in 0..children.len() {
            if let Some(mut child) = children.get(i) {
                child.queue_free();
            }
        }
    }

    fn add_row(&self, list: &mut Gd<VBoxContainer>, entry: &VarDictionary, max_damage: i32, bar_color: Color) {
        let name = entry.get("name").unwrap_or_default().to::<GString>();
        let damage = entry.get("damage").unwrap_or_default().to::<i32>();

        let mut row = HBoxContainer::new_alloc();

        let mut name_label = Label::new_alloc();
        name_label.set_text(&name);
        name_label.set_custom_minimum_size(Vector2::new(90.0, 0.0));
        row.add_child(&name_label);

        let mut bar = ProgressBar::new_alloc();
        bar.set_min(0.0);
        bar.set_max(max_damage as f64);
        bar.set_value(damage as f64);
        bar.set_custom_minimum_size(Vector2::new(80.0, 0.0));
        bar.set_modulate(bar_color);
        bar.set_show_percentage(false);
        row.add_child(&bar);

        let mut dmg_label = Label::new_alloc();
        dmg_label.set_text(&GString::from(damage.to_string().as_str()));
        dmg_label.set_horizontal_alignment(godot::global::HorizontalAlignment::RIGHT);
        row.add_child(&dmg_label);

        list.add_child(&row);
    }
}
