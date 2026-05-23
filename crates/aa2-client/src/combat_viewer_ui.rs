use std::collections::HashMap;

use godot::prelude::*;
use godot::classes::{Control, IControl, Label};

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
            "Attack" => {
                let target_id = event.get("target_id").map(|v| v.to::<u32>()).unwrap_or(0);
                let damage = event.get("damage").map(|v| v.to::<f32>()).unwrap_or(0.0);
                if let Some(unit) = self.units.get(&target_id) {
                    self.spawn_damage_number(unit.node.get_position(), damage);
                }
            }
            "ProjectileHit" => {
                let target_id = event.get("target_id").map(|v| v.to::<u32>()).unwrap_or(0);
                let damage = event.get("damage").map(|v| v.to::<f32>()).unwrap_or(0.0);
                if let Some(unit) = self.units.get(&target_id) {
                    self.spawn_damage_number(unit.node.get_position(), damage);
                }
            }
            "Death" => {
                let unit_id = event.get("unit_id").map(|v| v.to::<u32>()).unwrap_or(0);
                if let Some(unit) = self.units.get(&unit_id) {
                    unit.node.clone().set_visible(false);
                }
            }
            "UnitSpawn" => {
                let unit_id = event.get("unit_id").map(|v| v.to::<u32>()).unwrap_or(0);
                let name = event.get("name").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
                let x = event.get("x").map(|v| v.to::<f32>()).unwrap_or(0.0);
                let y = event.get("y").map(|v| v.to::<f32>()).unwrap_or(0.0);
                let team = event.get("team").map(|v| v.to::<i32>()).unwrap_or(0);
                let pos = game_to_screen(x, y);
                let mut label = Label::new_alloc();
                label.set_text(&format!("{name} [T{team}]"));
                label.set_position(pos);
                let control: Gd<Control> = label.clone().upcast();
                self.base_mut().add_child(&label);
                self.units.insert(unit_id, UnitVisual { node: control, target_pos: pos, speed: 0.0 });
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

    fn spawn_damage_number(&mut self, pos: Vector2, damage: f32) {
        let mut label = Label::new_alloc();
        let text = format!("-{:.0}", damage);
        label.set_text(&text);
        label.set_position(pos + Vector2::new(0.0, -20.0));
        label.add_theme_color_override("font_color", Color::from_rgb(1.0, 0.2, 0.2));
        self.base_mut().add_child(&label);
    }
}
