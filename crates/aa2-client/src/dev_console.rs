use godot::prelude::*;
use godot::classes::{Control, IControl, LineEdit, RichTextLabel, VBoxContainer, PanelContainer};
use godot::builtin::Side;

use crate::game_manager::GameManager;

#[derive(GodotClass)]
#[class(init, base=Control)]
pub struct DevConsole {
    base: Base<Control>,
    #[init(val = String::new())]
    last_phase: String,
}

#[godot_api]
impl IControl for DevConsole {
    fn ready(&mut self) {
        let mut panel = PanelContainer::new_alloc();
        panel.set_name("Panel");
        panel.set_anchor(Side::LEFT, 1.0);
        panel.set_anchor(Side::RIGHT, 1.0);
        panel.set_anchor(Side::TOP, 0.0);
        panel.set_anchor(Side::BOTTOM, 1.0);
        panel.set_offset(Side::LEFT, -300.0);
        panel.set_offset(Side::RIGHT, 0.0);
        panel.set_offset(Side::TOP, 0.0);
        panel.set_offset(Side::BOTTOM, 0.0);

        let mut vbox = VBoxContainer::new_alloc();
        vbox.set_name("VBox");
        vbox.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);

        let mut output = RichTextLabel::new_alloc();
        output.set_name("Output");
        output.set_scroll_active(true);
        output.set_use_bbcode(true);
        output.set_selection_enabled(true);
        output.set_v_size_flags(godot::classes::control::SizeFlags::EXPAND_FILL);
        vbox.add_child(&output);

        let mut input = LineEdit::new_alloc();
        input.set_name("Input");
        input.set_placeholder("Enter command...");
        input.connect("text_submitted", &self.base().callable("on_command_submitted"));
        vbox.add_child(&input);

        panel.add_child(&vbox);
        self.base_mut().add_child(&panel);

        self.log("[DevConsole] Ready. Type 'help' for commands.");
    }

    fn process(&mut self, _delta: f64) {
        if let Some(manager) = self.get_manager() {
            let phase = manager.bind().get_phase().to_string();
            if phase != self.last_phase {
                let msg = format!("[Phase] {phase}");
                self.last_phase = phase;
                self.log(&msg);
            }
        }
    }
}

#[godot_api]
impl DevConsole {
    #[func]
    fn on_command_submitted(&mut self, text: GString) {
        let cmd = text.to_string();
        if cmd.is_empty() {
            return;
        }

        if let Some(input) = self.base().get_node_or_null("Panel/VBox/Input") {
            let mut line_edit: Gd<LineEdit> = input.cast();
            line_edit.set_text("");
        }

        self.log(&format!("> {cmd}"));
        let result = self.execute_command(&cmd);
        self.log(&result);
    }
}

impl DevConsole {
    pub fn log(&mut self, msg: &str) {
        if let Some(node) = self.base().get_node_or_null("Panel/VBox/Output") {
            let mut output: Gd<RichTextLabel> = node.cast();
            let text = format!("{msg}\n");
            output.append_text(&text);
        }
    }

    fn execute_command(&mut self, cmd: &str) -> String {
        let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();
        let command = parts[0];
        let arg = parts.get(1).copied().unwrap_or("");

        let Some(mut manager) = self.get_manager() else {
            return "No GameManager found".to_string();
        };

        match command {
            "gold" => {
                if let Ok(n) = arg.parse::<i32>() {
                    manager.bind_mut().set_gold(0, n);
                    format!("Set gold to {n}")
                } else {
                    "Usage: gold <amount>".to_string()
                }
            }
            "hp" => {
                if let Ok(n) = arg.parse::<f32>() {
                    manager.bind_mut().set_hp(0, n);
                    format!("Set HP to {n}")
                } else {
                    "Usage: hp <amount>".to_string()
                }
            }
            "phase" => format!("Phase: {}", manager.bind().get_phase()),
            "round" => format!("Round: {}", manager.bind().get_round()),
            "state" => {
                let b = manager.bind();
                let gold = b.get_gold(0);
                let hp = b.get_player_hp(0);
                let phase = b.get_phase();
                let round = b.get_round();
                let heroes = b.get_heroes(0);
                format!("Gold:{gold} HP:{hp} Phase:{phase} Round:{round} Heroes:{heroes}")
            }
            "buy" => {
                let slot = arg.to_string();
                let r = manager.bind_mut().apply_player_action(0, "Buy".into(), GString::from(slot.as_str()));
                format!("Buy: {r}")
            }
            "reroll" => {
                let r = manager.bind_mut().apply_player_action(0, "RerollShop".into(), "".into());
                format!("Reroll: {r}")
            }
            "combat" => {
                let r = manager.bind_mut().run_combat();
                format!("Combat ran: {r}")
            }
            "ready" => {
                let r = manager.bind_mut().apply_player_action(0, "Ready".into(), "".into());
                format!("Ready: {r}")
            }
            "help" => "Commands: gold <n>, hp <n>, phase, round, state, buy <slot>, reroll, combat, ready, help".to_string(),
            _ => format!("Unknown command: {command}. Type 'help'."),
        }
    }

    fn get_manager(&self) -> Option<Gd<GameManager>> {
        let root = self.base().get_tree().get_root()?;
        root.get_node_or_null("MainScene/GameManager")
            .map(|n| n.cast::<GameManager>())
    }
}
