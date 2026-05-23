use godot::prelude::*;
use godot::classes::{Button, Control, HBoxContainer, IControl, Label, VBoxContainer};

use crate::game_manager::GameManager;

const MAX_SHOP_SLOTS: usize = 10;

#[derive(GodotClass)]
#[class(init, base=Control)]
pub struct ShopUI {
    base: Base<Control>,
}

#[godot_api]
impl IControl for ShopUI {
    fn ready(&mut self) {
        let mut vbox = VBoxContainer::new_alloc();
        vbox.set_name("VBox");

        // Top bar
        let mut top_bar = HBoxContainer::new_alloc();
        top_bar.set_name("TopBar");

        let mut gold_label = Label::new_alloc();
        gold_label.set_name("GoldLabel");
        gold_label.set_text("Gold: 0");
        top_bar.add_child(&gold_label);

        let mut level_label = Label::new_alloc();
        level_label.set_name("LevelLabel");
        level_label.set_text("Shop Lv: 1");
        top_bar.add_child(&level_label);

        let mut reroll_btn = Button::new_alloc();
        reroll_btn.set_name("RerollBtn");
        reroll_btn.set_text("Reroll (1g)");
        reroll_btn.connect("pressed", &self.base().callable("on_reroll"));
        top_bar.add_child(&reroll_btn);

        let mut upgrade_btn = Button::new_alloc();
        upgrade_btn.set_name("UpgradeBtn");
        upgrade_btn.set_text("Upgrade");
        upgrade_btn.connect("pressed", &self.base().callable("on_upgrade"));
        top_bar.add_child(&upgrade_btn);

        let mut lock_btn = Button::new_alloc();
        lock_btn.set_name("LockBtn");
        lock_btn.set_text("Lock");
        lock_btn.connect("pressed", &self.base().callable("on_lock"));
        top_bar.add_child(&lock_btn);

        vbox.add_child(&top_bar);

        // Shop slots
        let mut slots_hbox = HBoxContainer::new_alloc();
        slots_hbox.set_name("Slots");
        for i in 0..MAX_SHOP_SLOTS {
            let mut btn = Button::new_alloc();
            let name = format!("Slot{i}");
            btn.set_name(&name);
            btn.set_text("\u{2014}"); // em-dash
            btn.set_visible(false);
            let handler = format!("on_slot_{i}");
            btn.connect("pressed", &self.base().callable(&handler));
            slots_hbox.add_child(&btn);
        }
        vbox.add_child(&slots_hbox);

        self.base_mut().add_child(&vbox);
    }

    fn process(&mut self, _delta: f64) {
        self.refresh();
    }
}

#[godot_api]
impl ShopUI {
    #[func]
    fn refresh(&mut self) {
        let Some(manager) = self.get_manager() else { return };
        let gold = manager.bind().get_gold(0);
        let level = manager.bind().get_shop_level(0);
        let offerings = manager.bind().get_shop_offerings(0);
        let locked = manager.bind().get_shop_locked(0);
        let upgrade_cost = manager.bind().get_upgrade_cost(0);

        // Update labels
        if let Some(mut label) = self.try_get_node::<Label>("VBox/TopBar/GoldLabel") {
            let text = format!("Gold: {gold}");
            label.set_text(&text);
        }
        if let Some(mut label) = self.try_get_node::<Label>("VBox/TopBar/LevelLabel") {
            let text = format!("Shop Lv: {level}");
            label.set_text(&text);
        }
        if let Some(mut btn) = self.try_get_node::<Button>("VBox/TopBar/UpgradeBtn") {
            let text = if upgrade_cost >= 0 {
                format!("Upgrade ({upgrade_cost}g)")
            } else {
                "MAX".to_string()
            };
            btn.set_text(&text);
        }
        if let Some(mut btn) = self.try_get_node::<Button>("VBox/TopBar/LockBtn") {
            btn.set_text(if locked { "Unlock" } else { "Lock" });
        }

        // Update slots
        for i in 0..MAX_SHOP_SLOTS {
            let path = format!("VBox/Slots/Slot{i}");
            if let Some(mut btn) = self.try_get_node::<Button>(&path) {
                if (i as i32) < offerings.len() as i32 {
                    let name_str = offerings.get(i)
                        .map(|g| g.to_string())
                        .unwrap_or_default();
                    btn.set_visible(true);
                    if name_str.is_empty() {
                        btn.set_text("(sold)");
                        btn.set_disabled(true);
                    } else {
                        btn.set_text(&name_str);
                        btn.set_disabled(false);
                    }
                } else {
                    btn.set_visible(false);
                }
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

    fn buy_slot(&mut self, slot: usize) {
        if let Some(mut manager) = self.get_manager() {
            let param = format!("{slot}");
            manager.bind_mut().apply_player_action(0, "Buy".into(), GString::from(param.as_str()));
        }
    }

    #[func] fn on_reroll(&mut self) {
        if let Some(mut manager) = self.get_manager() {
            manager.bind_mut().apply_player_action(0, "RerollShop".into(), "".into());
        }
    }

    #[func] fn on_upgrade(&mut self) {
        if let Some(mut manager) = self.get_manager() {
            manager.bind_mut().apply_player_action(0, "UpgradeShop".into(), "".into());
        }
    }

    #[func] fn on_lock(&mut self) {
        if let Some(mut manager) = self.get_manager() {
            manager.bind_mut().apply_player_action(0, "LockShop".into(), "".into());
        }
    }

    #[func] fn on_slot_0(&mut self) { self.buy_slot(0); }
    #[func] fn on_slot_1(&mut self) { self.buy_slot(1); }
    #[func] fn on_slot_2(&mut self) { self.buy_slot(2); }
    #[func] fn on_slot_3(&mut self) { self.buy_slot(3); }
    #[func] fn on_slot_4(&mut self) { self.buy_slot(4); }
    #[func] fn on_slot_5(&mut self) { self.buy_slot(5); }
    #[func] fn on_slot_6(&mut self) { self.buy_slot(6); }
    #[func] fn on_slot_7(&mut self) { self.buy_slot(7); }
    #[func] fn on_slot_8(&mut self) { self.buy_slot(8); }
    #[func] fn on_slot_9(&mut self) { self.buy_slot(9); }
}
