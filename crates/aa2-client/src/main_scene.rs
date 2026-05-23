use godot::prelude::*;
use godot::classes::{Control, IControl};

use crate::bench_ui::BenchUI;
use crate::board_ui::BoardUI;
use crate::combat_viewer_ui::CombatViewerUI;
use crate::dev_console::DevConsole;
use crate::draft_ui::DraftUI;
use crate::game_manager::GameManager;
use crate::god_pick_ui::GodPickUI;
use crate::scoreboard_ui::ScoreboardUI;
use crate::shop_ui::ShopUI;

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
        // Add GameManager as our child (UIs find it via /root/MainScene/GameManager)
        let mut manager = GameManager::new_alloc();
        manager.set_name("GameManager");
        self.base_mut().add_child(&manager);
        manager.bind_mut().init_game(42, 2, "data".into());

        // Create UI screens as children
        self.add_control_child::<GodPickUI>(GodPickUI::new_alloc(), "GodPickUI");
        self.add_control_child::<DraftUI>(DraftUI::new_alloc(), "DraftUI");
        self.add_control_child::<ShopUI>(ShopUI::new_alloc(), "ShopUI");
        self.add_control_child::<BoardUI>(BoardUI::new_alloc(), "BoardUI");
        self.add_control_child::<BenchUI>(BenchUI::new_alloc(), "BenchUI");
        self.add_control_child::<CombatViewerUI>(CombatViewerUI::new_alloc(), "CombatViewerUI");
        self.add_control_child::<ScoreboardUI>(ScoreboardUI::new_alloc(), "ScoreboardUI");

        // DevConsole — always visible, not affected by phase changes
        let mut console = DevConsole::new_alloc();
        console.upcast_mut::<Node>().set_name("DevConsole");
        self.base_mut().add_child(&console);

        self.switch_to_phase("GodPick");
    }

    fn process(&mut self, delta: f64) {
        let Some(mut manager) = self.get_manager() else { return };

        manager.bind_mut().tick(delta as f32);

        let phase = manager.bind().get_phase().to_string();
        if phase == self.current_phase {
            return;
        }

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
            manager.bind_mut().apply_player_action(0, "Ready".into(), "".into());
        }
    }
}

impl MainScene {
    fn add_control_child<T: Inherits<Control> + Inherits<Node> + GodotClass>(&mut self, mut node: Gd<T>, name: &str) {
        node.upcast_mut::<Node>().set_name(name);
        node.upcast_mut::<Control>().set_visible(false);
        node.upcast_mut::<Control>().set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        self.base_mut().add_child(&node);
    }

    fn switch_to_phase(&mut self, phase: &str) {
        let (god, draft, shop, board, bench, combat, score) = match phase {
            "GodPick" => (true, false, false, false, false, false, false),
            "Shop" | "GracePeriod" => (false, true, true, true, true, false, true),
            "Combat" => (false, false, false, false, false, true, true),
            "Finished" => (false, false, false, false, false, false, true),
            _ => return,
        };
        self.set_screen_visible("GodPickUI", god);
        self.set_screen_visible("DraftUI", draft);
        self.set_screen_visible("ShopUI", shop);
        self.set_screen_visible("BoardUI", board);
        self.set_screen_visible("BenchUI", bench);
        self.set_screen_visible("CombatViewerUI", combat);
        self.set_screen_visible("ScoreboardUI", score);
    }

    fn set_screen_visible(&self, name: &str, visible: bool) {
        if let Some(node) = self.base().get_node_or_null(name) {
            let mut ctrl: Gd<Control> = node.cast();
            ctrl.set_visible(visible);
        }
    }

    fn get_manager(&self) -> Option<Gd<GameManager>> {
        self.base().get_node_or_null("GameManager")
            .map(|n| n.cast::<GameManager>())
    }
}
