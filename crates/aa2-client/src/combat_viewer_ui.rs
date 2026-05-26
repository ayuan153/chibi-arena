use std::collections::HashMap;

use godot::prelude::*;
use godot::classes::{Control, IControl, Label, ProgressBar};

use crate::game_manager::GameManager;

const ARENA_SIZE: f32 = 2000.0;

struct UnitVisual {
    node: Gd<Control>,
    target_pos: Vector2,
    speed: f32,
    hp_bar: Gd<ProgressBar>,
    max_hp: f32,
    cast_label: Option<Gd<Label>>,
    buff_label: Option<Gd<Label>>,
    death_timer: Option<f32>,
}

struct DamagePopup {
    label: Gd<Label>,
    time_remaining: f32,
    start_pos: Vector2,
}

struct Projectile {
    node: Gd<Control>,
    target_id: u32,
    speed: f32,
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
    projectiles: Vec<Projectile>,
}

impl CombatViewerUI {
    /// Convert game coordinates (2000x2000) to screen coordinates based on actual control size.
    fn game_to_screen(&self, x: f32, y: f32) -> Vector2 {
        let size = self.base().get_size();
        Vector2::new(
            (x / ARENA_SIZE) * size.x,
            (y / ARENA_SIZE) * size.y,
        )
    }

    /// Get the scale factor for speed calculations.
    fn scale_factor(&self) -> f32 {
        self.base().get_size().x / ARENA_SIZE
    }
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

        for event in &events_to_process {
            let event_type = event.get("type")
                .map(|v| v.to::<GString>().to_string())
                .unwrap_or_default();
            self.handle_event(&event_type, event);
        }

        // Interpolate unit positions
        let scale = self.scale_factor();
        for unit in self.units.values_mut() {
            let current = unit.node.get_position();
            let diff = unit.target_pos - current;
            if diff.length() > 1.0 && unit.speed > 0.0 {
                let step = unit.speed * scale * dt;
                let move_vec = diff.normalized() * step.min(diff.length());
                unit.node.set_position(current + move_vec);
            }
        }

        // Move projectiles toward targets
        self.projectiles.retain_mut(|proj| {
            let target_pos = self.units.get(&proj.target_id)
                .map(|u| u.node.get_position())
                .unwrap_or(proj.node.get_position());
            let current = proj.node.get_position();
            let diff = target_pos - current;
            if diff.length() < 5.0 {
                proj.node.clone().free();
                return false;
            }
            let step = proj.speed * scale * dt;
            let move_vec = diff.normalized() * step.min(diff.length());
            proj.node.set_position(current + move_vec);
            true
        });

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
        for proj in self.projectiles.drain(..) {
            proj.node.clone().free();
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
        self.base().get_node_or_null("/root/MainScene/GameManager")
            .map(|n| n.cast::<GameManager>())
    }

    fn handle_event(&mut self, event_type: &str, event: &VarDictionary) {
        match event_type {
            "Attack" | "ProjectileHit" | "AbilityDamage" => {
                let target_id = event.get("target_id").map(|v| v.to::<u32>()).unwrap_or(0);
                let damage = event.get("damage").map(|v| v.to::<f32>()).unwrap_or(0.0);
                if let Some(unit) = self.units.get_mut(&target_id) {
                    let new_val = (unit.hp_bar.get_value() - damage as f64).max(0.0);
                    unit.hp_bar.set_value(new_val);
                }
                if let Some(unit) = self.units.get(&target_id) {
                    self.spawn_damage_popup(unit.node.get_position(), damage, false);
                }
            }
            "ProjectileSpawn" => {
                let attacker_id = event.get("attacker_id").map(|v| v.to::<u32>()).unwrap_or(0);
                let target_id = event.get("target_id").map(|v| v.to::<u32>()).unwrap_or(0);
                let start_pos = self.units.get(&attacker_id)
                    .map(|u| u.node.get_position())
                    .unwrap_or(Vector2::ZERO);
                // Small dot representing projectile
                let mut node = Control::new_alloc();
                node.set_position(start_pos);
                let mut dot = Label::new_alloc();
                dot.set_text("•");
                dot.add_theme_color_override("font_color", Color::from_rgb(1.0, 1.0, 0.5));
                node.add_child(&dot);
                self.base_mut().add_child(&node);
                self.projectiles.push(Projectile {
                    node: node.upcast(),
                    target_id,
                    speed: 900.0, // default projectile speed
                });
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
                    label.set_text(&format!("⚡{ability_name}"));
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
            "BuffApplied" => {
                let target_id = event.get("target_id").map(|v| v.to::<u32>()).unwrap_or(0);
                let name = event.get("name").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
                if let Some(unit) = self.units.get_mut(&target_id) {
                    // Show buff indicator below HP bar
                    if let Some(old) = unit.buff_label.take() {
                        old.clone().free();
                    }
                    let mut label = Label::new_alloc();
                    label.set_text(&format!("↑{name}"));
                    label.set_position(Vector2::new(0.0, 24.0));
                    label.add_theme_color_override("font_color", Color::from_rgb(0.5, 1.0, 0.5));
                    unit.node.add_child(&label);
                    unit.buff_label = Some(label);
                }
            }
            "BuffExpired" => {
                let target_id = event.get("target_id").map(|v| v.to::<u32>()).unwrap_or(0);
                if let Some(unit) = self.units.get_mut(&target_id)
                    && let Some(label) = unit.buff_label.take()
                {
                    label.clone().free();
                }
            }
            "DarkPactPulse" => {
                let caster_id = event.get("caster_id").map(|v| v.to::<u32>()).unwrap_or(0);
                let self_damage = event.get("self_damage").map(|v| v.to::<f32>()).unwrap_or(0.0);
                if let Some(unit) = self.units.get_mut(&caster_id) {
                    // Self-damage
                    let new_val = (unit.hp_bar.get_value() - self_damage as f64).max(0.0);
                    unit.hp_bar.set_value(new_val);
                }
                if let Some(unit) = self.units.get(&caster_id) {
                    self.spawn_damage_popup(unit.node.get_position(), self_damage, false);
                }
            }
            "WaveHit" => {
                let target_id = event.get("target_id").map(|v| v.to::<u32>()).unwrap_or(0);
                let damage = event.get("damage").map(|v| v.to::<f32>()).unwrap_or(0.0);
                if let Some(unit) = self.units.get_mut(&target_id) {
                    let new_val = (unit.hp_bar.get_value() - damage as f64).max(0.0);
                    unit.hp_bar.set_value(new_val);
                }
                if let Some(unit) = self.units.get(&target_id) {
                    // Show stun indicator
                    self.spawn_damage_popup(unit.node.get_position(), damage, false);
                }
            }
            "UnitSpawn" => {
                let unit_id = event.get("unit_id").map(|v| v.to::<u32>()).unwrap_or(0);
                let name = event.get("name").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
                let x = event.get("x").map(|v| v.to::<f32>()).unwrap_or(0.0);
                let y = event.get("y").map(|v| v.to::<f32>()).unwrap_or(0.0);
                let team = event.get("team").map(|v| v.to::<i32>()).unwrap_or(0);
                let max_hp = event.get("max_hp").map(|v| v.to::<f32>()).unwrap_or(100.0);
                let pos = self.game_to_screen(x, y);

                let mut container = Control::new_alloc();
                container.set_position(pos);

                // Name label with team color
                let mut label = Label::new_alloc();
                label.set_text(&name);
                label.set_position(Vector2::ZERO);
                let team_color = if team == 0 {
                    Color::from_rgb(0.4, 0.8, 1.0) // blue for player
                } else {
                    Color::from_rgb(1.0, 0.4, 0.4) // red for enemy
                };
                label.add_theme_color_override("font_color", team_color);
                container.add_child(&label);

                // HP bar
                let mut hp_bar = ProgressBar::new_alloc();
                hp_bar.set_min(0.0);
                hp_bar.set_max(max_hp as f64);
                hp_bar.set_value(max_hp as f64);
                hp_bar.set_position(Vector2::new(0.0, 16.0));
                hp_bar.set_size(Vector2::new(60.0, 6.0));
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
                    buff_label: None,
                    death_timer: None,
                });
            }
            "MoveTo" => {
                let unit_id = event.get("unit_id").map(|v| v.to::<u32>()).unwrap_or(0);
                let x = event.get("x").map(|v| v.to::<f32>()).unwrap_or(0.0);
                let y = event.get("y").map(|v| v.to::<f32>()).unwrap_or(0.0);
                let speed = event.get("speed").map(|v| v.to::<f32>()).unwrap_or(0.0);
                let screen_pos = self.game_to_screen(x, y);
                if let Some(unit) = self.units.get_mut(&unit_id) {
                    unit.target_pos = screen_pos;
                    unit.speed = speed;
                }
            }
            "RoundEnd" => {
                self.playing = false;
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
