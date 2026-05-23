use godot::prelude::*;
use godot::classes::{Button, Control, HBoxContainer, IControl, Label, VBoxContainer};

use crate::game_manager::GameManager;

const MAX_BENCH: usize = 5;
const MAX_HEROES: usize = 5;
const MAX_SLOTS: usize = 4;

#[derive(GodotClass)]
#[class(init, base=Control)]
pub struct BenchUI {
    base: Base<Control>,
}

#[godot_api]
impl IControl for BenchUI {
    fn ready(&mut self) {
        let mut vbox = VBoxContainer::new_alloc();
        vbox.set_name("VBox");

        // Bench section
        let mut bench_label = Label::new_alloc();
        bench_label.set_name("BenchLabel");
        bench_label.set_text("Bench");
        vbox.add_child(&bench_label);

        let mut bench_box = HBoxContainer::new_alloc();
        bench_box.set_name("BenchSlots");
        for i in 0..MAX_BENCH {
            let mut btn = Button::new_alloc();
            let name = format!("Bench{i}");
            btn.set_name(&name);
            btn.set_visible(false);
            let handler = format!("on_bench_{i}");
            btn.connect("pressed", &self.base().callable(&handler));
            bench_box.add_child(&btn);
        }
        vbox.add_child(&bench_box);

        // Heroes section
        let mut heroes_label = Label::new_alloc();
        heroes_label.set_name("HeroesLabel");
        heroes_label.set_text("Equipped");
        vbox.add_child(&heroes_label);

        let mut heroes_box = VBoxContainer::new_alloc();
        heroes_box.set_name("HeroesBox");
        for i in 0..MAX_HEROES {
            let mut hbox = HBoxContainer::new_alloc();
            let name = format!("HeroRow{i}");
            hbox.set_name(&name);
            hbox.set_visible(false);

            let mut lbl = Label::new_alloc();
            lbl.set_name("Name");
            hbox.add_child(&lbl);

            for s in 0..MAX_SLOTS {
                let mut btn = Button::new_alloc();
                let slot_name = format!("Slot{s}");
                btn.set_name(&slot_name);
                btn.set_visible(false);
                let handler = format!("on_unequip_{i}_{s}");
                btn.connect("pressed", &self.base().callable(&handler));
                hbox.add_child(&btn);
            }
            heroes_box.add_child(&hbox);
        }
        vbox.add_child(&heroes_box);

        // Status
        let mut status = Label::new_alloc();
        status.set_name("Status");
        status.set_text("");
        vbox.add_child(&status);

        self.base_mut().add_child(&vbox);
    }

    fn process(&mut self, _delta: f64) {
        self.refresh();
    }
}

#[godot_api]
impl BenchUI {
    fn refresh(&mut self) {
        let Some(manager) = self.get_manager() else { return };
        let bench = manager.bind().get_bench(0);
        let heroes = manager.bind().get_heroes(0);

        // Update bench slots
        for i in 0..MAX_BENCH {
            let path = format!("VBox/BenchSlots/Bench{i}");
            let Some(mut btn) = self.try_get_node::<Button>(&path) else { continue };
            if (i as i32) < bench.len() as i32 {
                let name = bench.get(i).map(|g| g.to_string()).unwrap_or_default();
                let level = manager.bind().get_ability_level(0, GString::from(name.as_str()));
                let text = format!("{name} Lv.{level}");
                btn.set_text(&text);
                btn.set_visible(true);
            } else {
                btn.set_visible(false);
            }
        }

        // Update hero rows
        for i in 0..MAX_HEROES {
            let row_path = format!("VBox/HeroesBox/HeroRow{i}");
            let Some(mut row) = self.try_get_node::<HBoxContainer>(&row_path) else { continue };
            if (i as i32) < heroes.len() as i32 {
                let hero_name = heroes.get(i).map(|g| g.to_string()).unwrap_or_default();
                row.set_visible(true);
                let lbl_path = format!("{row_path}/Name");
                if let Some(mut lbl) = self.try_get_node::<Label>(&lbl_path) {
                    lbl.set_text(&hero_name);
                }
                let equipped = manager.bind().get_equipped_abilities(0, GString::from(hero_name.as_str()));
                for s in 0..MAX_SLOTS {
                    let slot_path = format!("{row_path}/Slot{s}");
                    let Some(mut btn) = self.try_get_node::<Button>(&slot_path) else { continue };
                    if (s as i32) < equipped.len() as i32 {
                        let aname = equipped.get(s).map(|g| g.to_string()).unwrap_or_default();
                        let text = format!("[X] {aname}");
                        btn.set_text(&text);
                        btn.set_visible(true);
                    } else {
                        btn.set_visible(false);
                    }
                }
            } else {
                row.set_visible(false);
            }
        }
    }

    fn equip_bench(&mut self, idx: usize) {
        let Some(manager) = self.get_manager() else { return };
        let bench = manager.bind().get_bench(0);
        let heroes = manager.bind().get_heroes(0);
        let ability = bench.get(idx).map(|g| g.to_string()).unwrap_or_default();
        if ability.is_empty() { return; }

        // Find first hero with a free slot
        for i in 0..heroes.len() {
            let hero = heroes.get(i).map(|g| g.to_string()).unwrap_or_default();
            let equipped = manager.bind().get_equipped_abilities(0, GString::from(hero.as_str()));
            if equipped.len() < MAX_SLOTS {
                let param = format!("{ability},{hero}");
                if let Some(mut mgr) = self.get_manager() {
                    mgr.bind_mut().apply_player_action(0, "Equip".into(), GString::from(param.as_str()));
                }
                return;
            }
        }
        if let Some(mut lbl) = self.try_get_node::<Label>("VBox/Status") {
            lbl.set_text("No free slots!");
        }
    }

    fn unequip_slot(&mut self, hero_idx: usize, slot_idx: usize) {
        let Some(manager) = self.get_manager() else { return };
        let heroes = manager.bind().get_heroes(0);
        let hero = heroes.get(hero_idx).map(|g| g.to_string()).unwrap_or_default();
        let equipped = manager.bind().get_equipped_abilities(0, GString::from(hero.as_str()));
        let ability = equipped.get(slot_idx).map(|g| g.to_string()).unwrap_or_default();
        if ability.is_empty() || hero.is_empty() { return; }
        let param = format!("{ability},{hero}");
        if let Some(mut mgr) = self.get_manager() {
            mgr.bind_mut().apply_player_action(0, "Unequip".into(), GString::from(param.as_str()));
        }
    }

    fn get_manager(&self) -> Option<Gd<GameManager>> {
        self.base().get_node_or_null("/root/MainScene/GameManager")
            .map(|n| n.cast::<GameManager>())
    }

    fn try_get_node<T: GodotClass + Inherits<godot::classes::Node>>(&self, path: &str) -> Option<Gd<T>> {
        self.base().get_node_or_null(path).map(|n| n.cast::<T>())
    }

    #[func] fn on_bench_0(&mut self) { self.equip_bench(0); }
    #[func] fn on_bench_1(&mut self) { self.equip_bench(1); }
    #[func] fn on_bench_2(&mut self) { self.equip_bench(2); }
    #[func] fn on_bench_3(&mut self) { self.equip_bench(3); }
    #[func] fn on_bench_4(&mut self) { self.equip_bench(4); }

    #[func] fn on_unequip_0_0(&mut self) { self.unequip_slot(0, 0); }
    #[func] fn on_unequip_0_1(&mut self) { self.unequip_slot(0, 1); }
    #[func] fn on_unequip_0_2(&mut self) { self.unequip_slot(0, 2); }
    #[func] fn on_unequip_0_3(&mut self) { self.unequip_slot(0, 3); }
    #[func] fn on_unequip_1_0(&mut self) { self.unequip_slot(1, 0); }
    #[func] fn on_unequip_1_1(&mut self) { self.unequip_slot(1, 1); }
    #[func] fn on_unequip_1_2(&mut self) { self.unequip_slot(1, 2); }
    #[func] fn on_unequip_1_3(&mut self) { self.unequip_slot(1, 3); }
    #[func] fn on_unequip_2_0(&mut self) { self.unequip_slot(2, 0); }
    #[func] fn on_unequip_2_1(&mut self) { self.unequip_slot(2, 1); }
    #[func] fn on_unequip_2_2(&mut self) { self.unequip_slot(2, 2); }
    #[func] fn on_unequip_2_3(&mut self) { self.unequip_slot(2, 3); }
    #[func] fn on_unequip_3_0(&mut self) { self.unequip_slot(3, 0); }
    #[func] fn on_unequip_3_1(&mut self) { self.unequip_slot(3, 1); }
    #[func] fn on_unequip_3_2(&mut self) { self.unequip_slot(3, 2); }
    #[func] fn on_unequip_3_3(&mut self) { self.unequip_slot(3, 3); }
    #[func] fn on_unequip_4_0(&mut self) { self.unequip_slot(4, 0); }
    #[func] fn on_unequip_4_1(&mut self) { self.unequip_slot(4, 1); }
    #[func] fn on_unequip_4_2(&mut self) { self.unequip_slot(4, 2); }
    #[func] fn on_unequip_4_3(&mut self) { self.unequip_slot(4, 3); }
}
