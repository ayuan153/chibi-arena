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

        // Connect Summary button to toggle ScoreboardUI
        if let Some(btn) = self.base().get_node_or_null("PersistentChrome/TopBar/SummaryButton") {
            let mut button: Gd<godot::classes::Button> = btn.cast();
            button.connect("pressed", &self.base().callable("toggle_summary"));
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

        // AI auto-draft: pick first choice immediately
        if phase == "Shop" {
            let ai_choices = manager.bind().get_draft_choices(1);
            if !ai_choices.is_empty() {
                manager.bind_mut().apply_player_action(1, "DraftHero".into(), "0".into());
            }
        }

        if phase == self.current_phase {
            // Check if combat viewer finished playing
            if phase == "Combat" && self.combat_started
                && let Some(viewer) = self.base().get_node_or_null("CombatViewerUI")
            {
                let viewer: Gd<crate::combat_viewer_ui::CombatViewerUI> = viewer.cast();
                if !viewer.bind().is_playing() {
                    godot_print!("[AA2] Combat playback finished, advancing...");
                    manager.bind_mut().end_combat();
                }
            }
            // Update top bar info even without phase change
            self.update_top_bar(&phase);
            return;
        }

        godot_print!("[AA2] Phase transition: {} -> {}", self.current_phase, phase);
        self.current_phase = phase.clone();
        self.switch_to_phase(&phase);

        if phase == "Shop" || phase == "GracePeriod" {
            self.combat_started = false;
            // AI auto-readies during shop
            if phase == "Shop" {
                manager.bind_mut().apply_player_action(1, "Ready".into(), "".into());
            }
        }

        if phase == "Combat" && !self.combat_started {
            manager.bind_mut().run_combat();
            self.combat_started = true;
            // Start combat viewer playback
            if let Some(viewer) = self.base().get_node_or_null("CombatViewerUI") {
                let mut viewer: Gd<crate::combat_viewer_ui::CombatViewerUI> = viewer.cast();
                viewer.bind_mut().start_playback(0);
            }
        }

        if phase == "GracePeriod" {
            // Skip grace period for now (no animation)
            manager.bind_mut().apply_player_action(0, "Ready".into(), "".into());
            manager.bind_mut().apply_player_action(1, "Ready".into(), "".into());
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
    fn toggle_summary(&mut self) {
        if let Some(node) = self.base().get_node_or_null("ScoreboardUI") {
            let mut ctrl: Gd<Control> = node.cast();
            let visible = ctrl.is_visible();
            ctrl.set_visible(!visible);
        }
    }
}

impl MainScene {
    fn switch_to_phase(&mut self, phase: &str) {
        godot_print!("[AA2] Switching to phase: {phase}");

        // Phase visibility matrix
        let show_god_pick = phase == "GodPick";
        let show_bottom = phase == "Shop" || phase == "GracePeriod";
        let show_combat = phase == "Combat";

        self.set_screen_visible("GodPickUI", show_god_pick);
        self.set_screen_visible("DraftUI", show_bottom); // draft overlay during shop
        self.set_screen_visible("BottomPanel", show_bottom);
        self.set_screen_visible("ArenaRegion", !show_god_pick);
        self.set_screen_visible("PersistentChrome/PlayerList", !show_god_pick);
        self.set_screen_visible("PersistentChrome/GodPortrait", !show_god_pick);
        self.set_screen_visible("CombatViewerUI", show_combat);
        self.set_screen_visible("DamageMeter", show_bottom || show_combat);
        self.set_screen_visible("ReadyButton", show_bottom);

        // ScoreboardUI: force-hide on GodPick, force-show on Finished, otherwise user-controlled
        if phase == "GodPick" {
            self.set_screen_visible("ScoreboardUI", false);
        } else if phase == "Finished" {
            self.set_screen_visible("ScoreboardUI", true);
        }

        // EndgameUI: show only on Finished
        self.set_screen_visible("EndgameUI", phase == "Finished");

        self.update_top_bar(phase);
    }

    fn update_top_bar(&self, phase: &str) {
        let Some(manager) = self.get_manager() else { return };
        if let Some(node) = self.base().get_node_or_null("PersistentChrome/TopBar/GameInfo") {
            let mut label: Gd<Label> = node.cast();
            let round = manager.bind().get_round();
            let text = format!("Round {round} \u{00b7} {phase}");
            label.set_text(&text);
        }
        // Update god portrait HP
        if let Some(node) = self.base().get_node_or_null("PersistentChrome/GodPortrait/HPLabel") {
            let mut label: Gd<Label> = node.cast();
            let hp = manager.bind().get_player_hp(0);
            let text = format!("HP: {}", hp as i32);
            label.set_text(&text);
        }
        // Update god portrait name
        if let Some(node) = self.base().get_node_or_null("PersistentChrome/GodPortrait/NameLabel") {
            let mut label: Gd<Label> = node.cast();
            let god = manager.bind().get_player_god(0);
            if !god.is_empty() {
                label.set_text(&god.to_string());
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
