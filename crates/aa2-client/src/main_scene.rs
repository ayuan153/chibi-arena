use godot::prelude::*;
use godot::classes::{Control, IControl};

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
            manager.bind_mut().init_game(42, 2, "data".into());
            godot_print!("[AA2] GameManager initialized");
        }

        // Connect ready button signal
        if let Some(btn) = self.base().get_node_or_null("ReadyButton") {
            let mut button: Gd<godot::classes::Button> = btn.cast();
            button.connect("pressed", &self.base().callable("on_ready_pressed"));
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
        }

        if phase == self.current_phase {
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
}

impl MainScene {
    fn switch_to_phase(&mut self, phase: &str) {
        let (god, draft, shop, board, bench, combat, score) = match phase {
            "GodPick" => (true, false, false, false, false, false, false),
            "Shop" | "GracePeriod" => (false, true, true, true, true, false, false),
            "Combat" => (false, false, false, false, false, true, false),
            "Finished" => (false, false, false, false, false, false, true),
            _ => return,
        };
        self.set_screen_visible("GodPickUI", god);
        self.set_screen_visible("DraftUI", draft);
        self.set_screen_visible("BottomCenter/ShopUI", shop);
        self.set_screen_visible("Arena/BoardUI", board);
        self.set_screen_visible("BottomCenter/BenchUI", bench);
        self.set_screen_visible("CombatViewerUI", combat);
        self.set_screen_visible("ScoreboardUI", score);

        // Show/hide layout regions based on phase
        self.set_screen_visible("PlayerList", phase != "GodPick");
        self.set_screen_visible("Arena", phase != "GodPick");
        self.set_screen_visible("MyGod", phase != "GodPick");
        self.set_screen_visible("BottomCenter", phase == "Shop" || phase == "GracePeriod");
        self.set_screen_visible("UnitInfo", false); // placeholder, shown on select later
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
