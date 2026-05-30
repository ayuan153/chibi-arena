//! Pending effects system: delayed or over-time effects that fire independently of caster actions.

use aa2_data::{DamageType, Payload};
use crate::vec2::Vec2;

/// The kind of pending effect currently active.
#[derive(Debug, Clone)]
pub enum PendingEffectKind {
    /// Spirit Lance: homing projectile that damages, slows, and spawns illusion.
    SpiritLanceProjectile {
        target_id: u32,
        caster_id: u32,
        caster_team: u8,
        position: Vec2,
        speed: f32,
        damage: f32,
        slow_pct: f32,
        slow_duration_secs: f32,
        illusion_damage_dealt_pct: f32,
        illusion_damage_taken_pct: f32,
        illusion_duration_ticks: u32,
        bounce_radius: f32,
        bounces_remaining: u32,
        already_hit: Vec<u32>,
    },
    /// Generic composable pulsing AoE: fires multiple pulses from caster position,
    /// applying data-driven payloads to enemies in radius and self-damage/dispel to caster.
    ComposablePulse {
        /// Payloads to apply per pulse.
        payload: Vec<Payload>,
        /// Ability level (for per-level payload values).
        level: u8,
        /// AoE radius around caster.
        radius: f32,
        /// Damage type used for self-damage calculation.
        damage_type: DamageType,
        /// Number of pulses remaining.
        pulses_remaining: u32,
        /// Ticks between pulses.
        pulse_interval_ticks: u32,
        /// Ticks until the next pulse fires.
        ticks_until_next_pulse: u32,
    },
    /// Spear of Mars: traveling projectile that impales first hero and drags to wall.
    SpearOfMarsTravel {
        start_pos: Vec2,
        direction: Vec2,
        travel_speed: f32,
        max_range: f32,
        current_distance: f32,
        width: f32,
        damage: f32,
        stun_duration_secs: f32,
        impaled_unit: Option<u32>,
        pass_through_hit: Vec<u32>,
        fire_trail_dps: f32,
        fire_trail_slow: f32,
        fire_trail_duration_secs: f32,
        bounces_remaining: u32,
        fire_trail_positions: Vec<Vec2>,
    },
    /// Generic composable expanding wave: applies data-driven payloads to units as wave reaches them.
    Composable {
        /// Origin position (caster pos at cast time).
        origin: Vec2,
        /// Current radius of the wave.
        current_radius: f32,
        /// Maximum radius the wave expands to.
        max_radius: f32,
        /// Speed of wave expansion in units/sec.
        speed: f32,
        /// Unit IDs already hit by this wave.
        already_hit: Vec<u32>,
        /// Payloads to apply to each hit unit.
        payload: Vec<Payload>,
        /// Ability level (for per-level payload values).
        level: u8,
    },
    /// Composable caster-travel: caster moves along a line, hitting enemies
    /// within capsule width as the wave front reaches them. Payloads applied
    /// per-hit with a configurable damage delay.
    ComposableCasterTravel {
        /// Start position of the travel line.
        start_pos: Vec2,
        /// End position of the travel line.
        end_pos: Vec2,
        /// Travel speed in units/sec.
        travel_speed: f32,
        /// Current distance traveled.
        current_distance: f32,
        /// Maximum travel distance.
        max_distance: f32,
        /// Capsule half-width for hit detection.
        width: f32,
        /// Unit IDs already hit by this travel.
        already_hit: Vec<u32>,
        /// Pending damage: (unit_id, ticks_remaining, damage_amount).
        pending_damage: Vec<(u32, u32, f32)>,
        /// Payloads to apply to each hit unit.
        payload: Vec<Payload>,
        /// Ability level (for per-level payload values).
        level: u8,
    },
}

/// A pending effect that fires after a delay or over time.
#[derive(Debug, Clone)]
pub struct PendingEffect {
    /// ID of the caster who created this effect.
    pub caster_id: u32,
    /// Team of the caster.
    pub caster_team: u8,
    /// Name of the ability that created this effect.
    pub ability_name: String,
    /// The specific kind of pending effect.
    pub kind: PendingEffectKind,
    /// Ticks before the effect starts processing.
    pub delay_ticks_remaining: u32,
}
