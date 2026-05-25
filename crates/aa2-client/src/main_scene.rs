use godot::prelude::*;
use godot::classes::{Control, IControl, Label};

use crate::game_manager::GameManager;

#[derive(GodotClass)]
#[class(init, base=Control)]
pub struct MainScene {
    base: Base<Control>,
    #[init(val = String::new())]
    current_phase: String,
    #[init(val = false)]
    combat_started: bool,
}

#[godot_api]
impl IControl for MainScene {
    fn ready(&mut self) {
        godot_print!("[AA2] MainScene ready - initializing game...");

        // GameManager is already in the scene tree via main.tscn
        if let Some(mut manager) = self.get_manager() {
            manager.bind_mut().init_game(42, 2, "../data".into());
            godot_print!("[AA2] GameManager initialized");
        }

        // Connect ready button signal
        if let Some(btn) = self.base().get_node_or_null("ReadyButton") {
            let mut button: Gd<godot::classes::Button> = btn.cast();
            button.connect("pressed", &self.base().callable("on_ready_pressed"));
        }

        // Connect sell bin button
        if let Some(btn) = self.base().get_node_or_null("PersistentChrome/GodPortrait/SellBin") {
            let mut button: Gd<godot::classes::Button> = btn.cast();
            button.connect("pressed", &self.base().callable("on_sell_pressed"));
        }

        self.switch_to_phase("GodPick");
    }

    fn process(&mut self, delta: f64) {
        let Some(mut manager) = self.get_manager() else { return };

        manager.bind_mut().tick(delta as f32);

        let phase = manager.bind().get_phase().to_string();

        // AI player auto-actions
        if phase == "GodPick" {
            let ai_god = manager.bind().get_player_god(1).to_string();
            if ai_god.is_empty() {
                let gods = manager.bind().get_available_gods();
                if let Some(dict) = gods.get(0) {
                    let name = dict.get("name").unwrap_or_default().to::<GString>();
                    godot_print!("[AA2] AI picking god: {name}");
                    manager.bind_mut().apply_player_action(1, "PickGod".into(), name);
                }
            }
            // Auto-ready both players once both have picked gods
            let p0_god = manager.bind().get_player_god(0).to_string();
            let p1_god = manager.bind().get_player_god(1).to_string();
            if !p0_god.is_empty() && !p1_god.is_empty() {
                godot_print!("[AA2] All gods picked, advancing...");
                manager.bind_mut().apply_player_action(0, "Ready".into(), "".into());
                manager.bind_mut().apply_player_action(1, "Ready".into(), "".into());
            }
        }

        if phase == self.current_phase {
            // Update top bar info even without phase change
            self.update_top_bar(&phase);
            return;
        }

        godot_print!("[AA2] Phase transition: {} -> {}", self.current_phase, phase);
        self.current_phase = phase.clone();
        self.switch_to_phase(&phase);

        if phase == "Shop" || phase == "GracePeriod" {
            self.combat_started = false;
            manager.bind_mut().apply_player_action(1, "Ready".into(), "".into());
        }

        if phase == "Combat" && !self.combat_started {
            manager.bind_mut().run_combat();
            self.combat_started = true;
        }
    }
}

#[godot_api]
impl MainScene {
    #[func]
    fn on_ready_pressed(&mut self) {
        if let Some(mut manager) = self.get_manager() {
            godot_print!("[AA2] Player pressed Ready");
            manager.bind_mut().apply_player_action(0, "Ready".into(), "".into());
            manager.bind_mut().apply_player_action(1, "Ready".into(), "".into());
        }
    }

    #[func]
    fn on_sell_pressed(&mut self) {
        godot_print!("[AA2] Sell bin clicked (not yet implemented)");
    }
}

impl MainScene {
    fn switch_to_phase(&mut self, phase: &str) {
        godot_print!("[AA2] Switching to phase: {phase}");

        // Phase visibility matrix
        let show_god_pick = phase == "GodPick";
        let show_bottom = phase == "Shop" || phase == "GracePeriod";
        let show_combat = phase == "Combat";
        let show_scoreboard = phase == "Finished";

        self.set_screen_visible("GodPickUI", show_god_pick);
        self.set_screen_visible("DraftUI", show_bottom); // draft overlay during shop
        self.set_screen_visible("BottomPanel", show_bottom);
        self.set_screen_visible("ArenaRegion", !show_god_pick);
        self.set_screen_visible("PersistentChrome/PlayerList", !show_god_pick);
        self.set_screen_visible("PersistentChrome/GodPortrait", !show_god_pick);
        self.set_screen_visible("CombatViewerUI", show_combat);
        self.set_screen_visible("ScoreboardUI", show_scoreboard);
        self.set_screen_visible("ReadyButton", show_bottom);

        self.update_top_bar(phase);
    }

    fn update_top_bar(&self, phase: &str) {
        if let Some(node) = self.base().get_node_or_null("PersistentChrome/TopBar/GameInfo") {
            let mut label: Gd<Label> = node.cast();
            if let Some(manager) = self.get_manager() {
                let round = manager.bind().get_round();
                let text = format!("Round {round} \u{00b7} {phase}");
                label.set_text(&text);
            }
        }
    }

    fn set_screen_visible(&self, path: &str, visible: bool) {
        if let Some(node) = self.base().get_node_or_null(path) {
            let mut ctrl: Gd<Control> = node.cast();
            ctrl.set_visible(visible);
        }
    }

    fn get_manager(&self) -> Option<Gd<GameManager>> {
        self.base().get_node_or_null("GameManager")
            .map(|n| n.cast::<GameManager>())
    }
}
