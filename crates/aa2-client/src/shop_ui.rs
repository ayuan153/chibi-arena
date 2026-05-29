use godot::prelude::*;
use godot::classes::{Button, HBoxContainer, IHBoxContainer, Label};

use crate::game_manager::GameManager;
use crate::ui_helpers::format_ability_tooltip;

const MAX_SHOP_SLOTS: usize = 10;

/// Shop row UI: gold display, upgrade/reroll/lock buttons, and ability slot buttons.
/// Builds its own children in ready() and refreshes every frame.
#[derive(GodotClass)]
#[class(init, base=HBoxContainer)]
pub struct ShopUi {
    base: Base<HBoxContainer>,
}

#[godot_api]
impl IHBoxContainer for ShopUi {
    fn ready(&mut self) {
        godot_print!("[AA2] ShopUi ready");

        let mut gold_label = Label::new_alloc();
        gold_label.set_name("GoldLabel");
        gold_label.set_text("Gold: 0");
        self.base_mut().add_child(&gold_label);

        let mut upgrade_btn = Button::new_alloc();
        upgrade_btn.set_name("UpgradeBtn");
        upgrade_btn.set_text("Upgrade");
        upgrade_btn.connect("pressed", &self.base().callable("on_upgrade"));
        self.base_mut().add_child(&upgrade_btn);

        // Ability slots container
        let mut slots = HBoxContainer::new_alloc();
        slots.set_name("AbilitySlots");
        for i in 0..MAX_SHOP_SLOTS {
            let mut btn = Button::new_alloc();
            btn.set_name(&format!("ShopSlot{i}"));
            btn.set_custom_minimum_size(Vector2::new(64.0, 64.0));
            btn.set_text("\u{2014}");
            btn.set_visible(false);
            btn.connect("pressed", &self.base().callable(&format!("on_slot_{i}")));
            slots.add_child(&btn);
        }
        self.base_mut().add_child(&slots);

        let mut reroll_btn = Button::new_alloc();
        reroll_btn.set_name("RerollBtn");
        reroll_btn.set_text("Reroll (1g)");
        reroll_btn.connect("pressed", &self.base().callable("on_reroll"));
        self.base_mut().add_child(&reroll_btn);

        let mut lock_btn = Button::new_alloc();
        lock_btn.set_name("LockBtn");
        lock_btn.set_text("Lock");
        lock_btn.connect("pressed", &self.base().callable("on_lock"));
        self.base_mut().add_child(&lock_btn);
    }

    fn process(&mut self, _delta: f64) {
        self.refresh();
    }
}

#[godot_api]
impl ShopUi {
    #[func]
    fn refresh(&mut self) {
        let Some(manager) = self.get_manager() else { return };
        let gold = manager.bind().get_gold(0);
        let level = manager.bind().get_shop_level(0);
        let offerings = manager.bind().get_shop_offerings(0);
        let locked = manager.bind().get_shop_locked(0);
        let upgrade_cost = manager.bind().get_upgrade_cost(0);

        // Update labels
        if let Some(node) = self.base().get_node_or_null("GoldLabel") {
            let mut label: Gd<Label> = node.cast();
            let text = format!("Gold: {gold}");
            label.set_text(&text);
        }
        if let Some(node) = self.base().get_node_or_null("UpgradeBtn") {
            let mut btn: Gd<Button> = node.cast();
            let text = if upgrade_cost >= 0 {
                format!("Upgrade ({upgrade_cost}g) Lv{level}")
            } else {
                "MAX".to_string()
            };
            btn.set_text(&text);
        }
        if let Some(node) = self.base().get_node_or_null("LockBtn") {
            let mut btn: Gd<Button> = node.cast();
            btn.set_text(if locked { "Unlock" } else { "Lock" });
        }

        // Update slots
        for i in 0..MAX_SHOP_SLOTS {
            let path = format!("AbilitySlots/ShopSlot{i}");
            if let Some(node) = self.base().get_node_or_null(&path) {
                let mut btn: Gd<Button> = node.cast();
                if (i as i32) < offerings.len() as i32 {
                    let name_str = offerings.get(i).map(|g| g.to_string()).unwrap_or_default();
                    btn.set_visible(true);
                    if name_str.is_empty() {
                        btn.set_text("(sold)");
                        btn.set_disabled(true);
                        btn.set_tooltip_text("");
                    } else {
                        btn.set_text(&name_str);
                        btn.set_disabled(false);
                        let info = manager.bind().get_ability_info(GString::from(name_str.as_str()));
                        if !info.is_empty() {
                            btn.set_tooltip_text(&format_ability_tooltip(&info));
                        }
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

    fn buy_slot(&mut self, slot: usize) {
        godot_print!("[AA2] Buy slot {slot} clicked");
        if let Some(mut manager) = self.get_manager() {
            let param = format!("{slot}");
            let result = manager.bind_mut().apply_player_action(0, "Buy".into(), GString::from(param.as_str()));
            godot_print!("[AA2] Buy result: {result}");
        }
    }

    #[func] fn on_reroll(&mut self) {
        godot_print!("[AA2] Reroll clicked");
        if let Some(mut manager) = self.get_manager() {
            manager.bind_mut().apply_player_action(0, "RerollShop".into(), "".into());
        }
    }

    #[func] fn on_upgrade(&mut self) {
        godot_print!("[AA2] Upgrade clicked");
        if let Some(mut manager) = self.get_manager() {
            manager.bind_mut().apply_player_action(0, "UpgradeShop".into(), "".into());
        }
    }

    #[func] fn on_lock(&mut self) {
        godot_print!("[AA2] Lock clicked");
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
