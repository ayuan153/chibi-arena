use std::collections::HashMap;

use godot::prelude::*;
use godot::classes::{Control, IControl, Label, ProgressBar};

use crate::game_manager::GameManager;

const ARENA_SIZE: f32 = 2000.0;
const PANEL_SIZE: f32 = 600.0;

fn game_to_screen(x: f32, y: f32) -> Vector2 {
    Vector2::new(
        (x / ARENA_SIZE) * PANEL_SIZE,
        (y / ARENA_SIZE) * PANEL_SIZE,
    )
}

struct UnitVisual {
    node: Gd<Control>,
    target_pos: Vector2,
    speed: f32,
    hp_bar: Gd<ProgressBar>,
    max_hp: f32,
    cast_label: Option<Gd<Label>>,
    /// If Some, unit is fading out. Counts down from 0.5.
    death_timer: Option<f32>,
}

struct DamagePopup {
    label: Gd<Label>,
    time_remaining: f32,
    start_pos: Vector2,
}

#[derive(GodotClass)]
#[class(init, base=Control)]
pub struct CombatViewerUI {
    base: Base<Control>,
    playing: bool,
    playback_time: f32,
    next_event_index: usize,
    matchup_index: i32,
    units: HashMap<u32, UnitVisual>,
    popups: Vec<DamagePopup>,
}

#[godot_api]
impl IControl for CombatViewerUI {
    fn process(&mut self, delta: f64) {
        if !self.playing {
            return;
        }

        let dt = delta as f32;
        self.playback_time += dt;
        let current_tick = (self.playback_time * 30.0) as i32;

        // Process events up to current tick
        let Some(manager) = self.get_manager() else { return };
        let mgr = manager.bind();
        let event_count = mgr.get_combat_event_count(self.matchup_index) as usize;

        // Collect events to process
        let mut events_to_process = Vec::new();
        while self.next_event_index < event_count {
            let event = mgr.get_combat_event(self.matchup_index, self.next_event_index as i32);
            let tick = event.get("tick").map(|v| v.to::<i32>()).unwrap_or(0);
            if tick > current_tick {
                break;
            }
            self.next_event_index += 1;
            events_to_process.push(event);
        }
        drop(mgr);
        drop(manager);

        // Handle collected events
        for event in &events_to_process {
            let event_type = event.get("type")
                .map(|v| v.to::<GString>().to_string())
                .unwrap_or_default();
            self.handle_event(&event_type, event);
        }

        // Interpolate unit positions
        for unit in self.units.values_mut() {
            let current = unit.node.get_position();
            let diff = unit.target_pos - current;
            if diff.length() > 1.0 && unit.speed > 0.0 {
                let step = (unit.speed / ARENA_SIZE) * PANEL_SIZE * dt;
                let move_vec = diff.normalized() * step.min(diff.length());
                unit.node.set_position(current + move_vec);
            }
        }

        // Update death fades
        let mut dead_ids = Vec::new();
        for (&id, unit) in self.units.iter_mut() {
            if let Some(ref mut timer) = unit.death_timer {
                *timer -= dt;
                let alpha = (*timer / 0.5).max(0.0);
                unit.node.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, alpha));
                if *timer <= 0.0 {
                    dead_ids.push(id);
                }
            }
        }
        for id in dead_ids {
            if let Some(unit) = self.units.remove(&id) {
                unit.node.clone().free();
            }
        }

        // Update damage popups
        self.popups.retain_mut(|popup| {
            popup.time_remaining -= dt;
            if popup.time_remaining <= 0.0 {
                popup.label.clone().free();
                return false;
            }
            let progress = 1.0 - (popup.time_remaining / 1.0);
            let y_offset = progress * 30.0;
            popup.label.set_position(popup.start_pos + Vector2::new(0.0, -y_offset));
            popup.label.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, popup.time_remaining));
            true
        });

        // Stop at round end
        if self.next_event_index >= event_count && event_count > 0 {
            self.playing = false;
        }
    }
}

#[godot_api]
impl CombatViewerUI {
    #[func]
    pub fn start_playback(&mut self, matchup_index: i32) {
        for (_, unit) in self.units.drain() {
            unit.node.clone().free();
        }
        for popup in self.popups.drain(..) {
            popup.label.clone().free();
        }
        self.matchup_index = matchup_index;
        self.playback_time = 0.0;
        self.next_event_index = 0;
        self.playing = true;
    }

    #[func]
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    fn get_manager(&self) -> Option<Gd<GameManager>> {
        self.base().get_node_or_null("/root/GameManager")
            .map(|n| n.cast::<GameManager>())
    }

    fn handle_event(&mut self, event_type: &str, event: &VarDictionary) {
        match event_type {
            "Attack" | "ProjectileHit" | "AbilityDamage" => {
                let target_id = event.get("target_id").map(|v| v.to::<u32>()).unwrap_or(0);
                let damage = event.get("damage").map(|v| v.to::<f32>()).unwrap_or(0.0);
                if let Some(unit) = self.units.get_mut(&target_id) {
                    // Update HP bar
                    let new_val = (unit.hp_bar.get_value() - damage as f64).max(0.0);
                    unit.hp_bar.set_value(new_val);
                }
                if let Some(unit) = self.units.get(&target_id) {
                    self.spawn_damage_popup(unit.node.get_position(), damage, false);
                }
            }
            "Heal" => {
                let target_id = event.get("target_id").map(|v| v.to::<u32>()).unwrap_or(0);
                let amount = event.get("amount").map(|v| v.to::<f32>()).unwrap_or(0.0);
                if let Some(unit) = self.units.get_mut(&target_id) {
                    let new_val = (unit.hp_bar.get_value() + amount as f64).min(unit.max_hp as f64);
                    unit.hp_bar.set_value(new_val);
                }
                if let Some(unit) = self.units.get(&target_id) {
                    self.spawn_damage_popup(unit.node.get_position(), amount, true);
                }
            }
            "Death" => {
                let unit_id = event.get("unit_id").map(|v| v.to::<u32>()).unwrap_or(0);
                if let Some(unit) = self.units.get_mut(&unit_id) {
                    unit.death_timer = Some(0.5);
                }
            }
            "CastStart" => {
                let caster_id = event.get("caster_id").map(|v| v.to::<u32>()).unwrap_or(0);
                let ability_name = event.get("ability_name")
                    .map(|v| v.to::<GString>().to_string())
                    .unwrap_or_default();
                if let Some(unit) = self.units.get_mut(&caster_id) {
                    let mut label = Label::new_alloc();
                    label.set_text(&format!("Casting: {ability_name}"));
                    label.set_position(Vector2::new(0.0, -20.0));
                    label.add_theme_color_override("font_color", Color::from_rgb(0.3, 0.8, 1.0));
                    unit.node.add_child(&label);
                    unit.cast_label = Some(label);
                }
            }
            "CastComplete" => {
                let caster_id = event.get("caster_id").map(|v| v.to::<u32>()).unwrap_or(0);
                if let Some(unit) = self.units.get_mut(&caster_id)
                    && let Some(label) = unit.cast_label.take()
                {
                    label.clone().free();
                }
            }
            "UnitSpawn" => {
                let unit_id = event.get("unit_id").map(|v| v.to::<u32>()).unwrap_or(0);
                let name = event.get("name").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
                let x = event.get("x").map(|v| v.to::<f32>()).unwrap_or(0.0);
                let y = event.get("y").map(|v| v.to::<f32>()).unwrap_or(0.0);
                let team = event.get("team").map(|v| v.to::<i32>()).unwrap_or(0);
                let max_hp = event.get("max_hp").map(|v| v.to::<f32>()).unwrap_or(100.0);
                let pos = game_to_screen(x, y);

                // Container control for unit
                let mut container = Control::new_alloc();
                container.set_position(pos);

                // Name label
                let mut label = Label::new_alloc();
                label.set_text(&format!("{name} [T{team}]"));
                label.set_position(Vector2::ZERO);
                container.add_child(&label);

                // HP bar below name
                let mut hp_bar = ProgressBar::new_alloc();
                hp_bar.set_min(0.0);
                hp_bar.set_max(max_hp as f64);
                hp_bar.set_value(max_hp as f64);
                hp_bar.set_position(Vector2::new(0.0, 16.0));
                hp_bar.set_size(Vector2::new(40.0, 6.0));
                hp_bar.set_show_percentage(false);
                container.add_child(&hp_bar);

                self.base_mut().add_child(&container);
                let control: Gd<Control> = container.upcast();
                self.units.insert(unit_id, UnitVisual {
                    node: control,
                    target_pos: pos,
                    speed: 0.0,
                    hp_bar,
                    max_hp,
                    cast_label: None,
                    death_timer: None,
                });
            }
            "MoveTo" => {
                let unit_id = event.get("unit_id").map(|v| v.to::<u32>()).unwrap_or(0);
                let x = event.get("x").map(|v| v.to::<f32>()).unwrap_or(0.0);
                let y = event.get("y").map(|v| v.to::<f32>()).unwrap_or(0.0);
                let speed = event.get("speed").map(|v| v.to::<f32>()).unwrap_or(0.0);
                if let Some(unit) = self.units.get_mut(&unit_id) {
                    unit.target_pos = game_to_screen(x, y);
                    unit.speed = speed;
                }
            }
            _ => {}
        }
    }

    fn spawn_damage_popup(&mut self, pos: Vector2, value: f32, is_heal: bool) {
        let mut label = Label::new_alloc();
        let text = if is_heal {
            format!("+{:.0}", value)
        } else {
            format!("-{:.0}", value)
        };
        label.set_text(&text);
        let start_pos = pos + Vector2::new(20.0, -10.0);
        label.set_position(start_pos);
        let color = if is_heal {
            Color::from_rgb(0.2, 1.0, 0.2)
        } else {
            Color::from_rgb(1.0, 0.2, 0.2)
        };
        label.add_theme_color_override("font_color", color);
        self.base_mut().add_child(&label);
        self.popups.push(DamagePopup {
            label,
            time_remaining: 1.0,
            start_pos,
        });
    }
}
