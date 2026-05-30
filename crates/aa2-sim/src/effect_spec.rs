//! Composable effect resolver: generic dispatch for data-driven ability effects.
//!
//! Resolves `EffectSpec` (trigger + targeting + delivery + payloads) into concrete
//! combat events, replacing bespoke per-ability match arms one ability at a time.

use aa2_data::{BuffDef, DamageType, Delivery, EffectSpec, Payload, TargetingSpec, Trigger};
use crate::buff::{active_status, apply_buff, dispel, Buff, TickEffect};
use crate::combat::{apply_armor, apply_magic_resistance};
use crate::pending::{PendingEffect, PendingEffectKind};
use crate::unit::Unit;
use crate::vec2::Vec2;
use crate::{CombatEvent, TICK_RATE};

/// Maximum recursion depth for `Payload::Chain` sub-effects.
pub const MAX_EFFECT_CHAIN_DEPTH: usize = 2;

/// Outcome of applying a single payload to a unit.
///
/// Returned by `apply_payload_to_unit` so callers can emit the appropriate event
/// (AbilityDamage for Instant, aggregated WaveHit for ExpandingWave) without
/// duplicating mutation logic.
pub enum PayloadOutcome {
    /// Damage was dealt (after armor/MR/magic-immunity gating). `amount` may be 0 if gated.
    Damage { amount: f32, damage_type: DamageType },
    /// A buff/debuff was applied. `duration_secs` is the actual (status-resistance-adjusted) duration.
    BuffApplied { name: String, duration_secs: f32 },
    /// A dispel was performed.
    Dispel,
    /// Payload was skipped (magic-immune gate on debuff, empty vec, chain scaffold).
    Skipped,
}

/// Construct a runtime `Buff` from a data-driven `BuffDef`.
///
/// Picks the duration for the given ability level (1-indexed, clamped to last element).
/// Converts seconds to ticks via `TICK_RATE` (30 Hz), truncating to match the sim-wide
/// `(secs * 30.0) as u32` convention used by every other buff/pending duration.
pub fn buff_from_def(def: &BuffDef, level: u8, source_id: u32) -> Buff {
    debug_assert!(!def.duration.is_empty(), "BuffDef.duration must not be empty");
    if def.duration.is_empty() {
        // Safe no-op: zero-duration buff that will expire immediately
        return Buff {
            name: def.name.clone(),
            remaining_ticks: 0,
            tick_effect: None,
            stacking: def.stacking.clone(),
            dispel_type: def.dispel_type,
            status: def.status,
            stat_modifier: def.stat_modifier.as_ref().map(|s| s.resolve(level)),
            source_id,
            is_debuff: def.is_debuff,
            pierces_magic_immunity: def.pierces_magic_immunity,
            damage_reflection_pct: def.damage_reflection_pct,
        };
    }
    let idx = (level.saturating_sub(1) as usize).min(def.duration.len().saturating_sub(1));
    let duration_secs = def.duration[idx];
    let remaining_ticks = (duration_secs * TICK_RATE) as u32;
    let tick_effect = def.tick_effect.as_ref().map(|te| TickEffect {
        damage: te.damage,
        damage_type: te.damage_type.clone(),
        interval_ticks: te.interval_ticks,
        ticks_until_next: te.interval_ticks,
    });
    Buff {
        name: def.name.clone(),
        remaining_ticks,
        tick_effect,
        stacking: def.stacking.clone(),
        dispel_type: def.dispel_type,
        status: def.status,
        stat_modifier: def.stat_modifier.as_ref().map(|s| s.resolve(level)),
        source_id,
        is_debuff: def.is_debuff,
        pierces_magic_immunity: def.pierces_magic_immunity,
        damage_reflection_pct: def.damage_reflection_pct,
    }
}

/// Apply a single payload to a target unit, performing the mutation (HP subtraction,
/// buff application, dispel) and returning a `PayloadOutcome` describing what happened.
///
/// This is the single source of truth for payload math — both the Instant delivery path
/// and the ExpandingWave path call this, differing only in how they emit events from the
/// returned outcome.
///
/// Magic-immunity gating: Magical damage is zeroed; non-piercing debuffs are skipped.
/// Status resistance: debuff duration is reduced by `(1 - status_resistance)`.
/// Rounding: `(secs * 30.0) as u32` truncation (sim-wide convention).
pub fn apply_payload_to_unit(
    payload: &Payload,
    level: u8,
    caster_id: u32,
    units: &mut [Unit],
    target_idx: usize,
) -> PayloadOutcome {
    match payload {
        Payload::Damage { kind, base } => {
            debug_assert!(!base.is_empty(), "Payload::Damage base must not be empty");
            if base.is_empty() {
                return PayloadOutcome::Damage { amount: 0.0, damage_type: kind.clone() };
            }
            let idx = (level.saturating_sub(1) as usize).min(base.len().saturating_sub(1));
            let raw = base[idx];
            let actual = match kind {
                DamageType::Physical => apply_armor(raw, units[target_idx].armor),
                DamageType::Magical => {
                    if active_status(&units[target_idx].buffs).magic_immune {
                        0.0
                    } else {
                        apply_magic_resistance(raw, units[target_idx].magic_resistance)
                    }
                }
                DamageType::Pure => raw,
            };
            if actual > 0.0 {
                units[target_idx].hp -= actual;
            }
            PayloadOutcome::Damage { amount: actual, damage_type: kind.clone() }
        }
        Payload::ApplyBuff(def) => {
            let is_debuff = def.is_debuff;
            // Skip non-piercing debuffs on magic immune units
            if is_debuff && !def.pierces_magic_immunity && active_status(&units[target_idx].buffs).magic_immune {
                return PayloadOutcome::Skipped;
            }
            let buff = buff_from_def(def, level, caster_id);
            // Persistent status_resistance: add to unit field on apply (no reversal on expiry)
            if let Some(ref m) = buff.stat_modifier
                && m.status_resistance != 0.0
            {
                units[target_idx].status_resistance += m.status_resistance;
            }
            // Apply status resistance to debuff duration
            let buff = if is_debuff && units[target_idx].status_resistance > 0.0 {
                let actual_ticks = (buff.remaining_ticks as f32 * (1.0 - units[target_idx].status_resistance)) as u32;
                Buff { remaining_ticks: actual_ticks, ..buff }
            } else {
                buff
            };
            let name = buff.name.clone();
            let duration_secs = buff.remaining_ticks as f32 / TICK_RATE;
            apply_buff(&mut units[target_idx].buffs, buff);
            PayloadOutcome::BuffApplied { name, duration_secs }
        }
        Payload::Dispel { strength } => {
            dispel(&mut units[target_idx].buffs, *strength);
            PayloadOutcome::Dispel
        }
        Payload::Chain(_child_spec) => {
            // Chain is a scaffold — Rage/Ravage don't use it.
            PayloadOutcome::Skipped
        }
        Payload::SelfDamage { .. } => {
            // Self-damage is handled by the ComposablePulse step, not per-target.
            PayloadOutcome::Skipped
        }
    }
}

/// Entry point: run all `OnCast`-triggered effect specs for an ability.
///
/// For each spec whose trigger is `OnCast`, resolves targeting → delivery → payloads
/// and collects events in deterministic order (specs in Vec order, targets in index order).
#[allow(clippy::too_many_arguments)]
pub fn run_cast_effect_specs(
    specs: &[EffectSpec],
    ability_name: &str,
    level: u8,
    caster_id: u32,
    caster_team: u8,
    caster_pos: Vec2,
    target_id: Option<u32>,
    _target_pos: Option<Vec2>,
    units: &mut [Unit],
    tick: u32,
    pending_effects: &mut Vec<PendingEffect>,
) -> Vec<CombatEvent> {
    let mut events = Vec::new();
    for spec in specs {
        if spec.trigger != Trigger::OnCast {
            continue;
        }
        resolve_spec(
            spec, ability_name, level, caster_id, caster_team, caster_pos,
            target_id, units, tick, pending_effects, &mut events,
        );
    }
    events
}

/// Resolve a single EffectSpec: targeting → delivery → payloads.
#[allow(clippy::too_many_arguments)]
fn resolve_spec(
    spec: &EffectSpec,
    ability_name: &str,
    level: u8,
    caster_id: u32,
    caster_team: u8,
    caster_pos: Vec2,
    target_id: Option<u32>,
    units: &mut [Unit],
    tick: u32,
    pending_effects: &mut Vec<PendingEffect>,
    events: &mut Vec<CombatEvent>,
) {
    // Resolve targeting
    let target_indices: Vec<usize> = match &spec.targeting {
        TargetingSpec::Caster => {
            match units.iter().position(|u| u.id == caster_id) {
                Some(idx) => vec![idx],
                None => return,
            }
        }
        TargetingSpec::EnemiesInDelivery => {
            // For Instant delivery, collect all alive enemies in index order.
            // For ExpandingWave, the delivery layer handles per-unit hit timing.
            units.iter().enumerate()
                .filter(|(_, u)| u.team != caster_team && u.is_alive())
                .map(|(i, _)| i)
                .collect()
        }
        TargetingSpec::TargetAndCaster => {
            let caster_idx = units.iter().position(|u| u.id == caster_id);
            let target_idx = target_id.and_then(|tid| units.iter().position(|u| u.id == tid && u.is_alive()));
            match (target_idx, caster_idx) {
                (Some(ti), Some(ci)) if ti != ci => vec![ti, ci],
                (Some(ti), _) => vec![ti],
                (None, Some(ci)) => vec![ci],
                _ => return,
            }
        }
    };

    // Resolve delivery
    match &spec.delivery {
        Delivery::Instant => {
            for &idx in &target_indices {
                apply_payloads(
                    &spec.payload, ability_name, level, caster_id, caster_team,
                    units, idx, tick, events, 0,
                );
            }
        }
        Delivery::ExpandingWave { max_radius, speed } => {
            debug_assert!(!max_radius.is_empty(), "ExpandingWave max_radius must not be empty");
            if max_radius.is_empty() {
                return;
            }
            let radius_idx = (level.saturating_sub(1) as usize).min(max_radius.len().saturating_sub(1));
            let mr = max_radius[radius_idx];
            pending_effects.push(PendingEffect {
                caster_id,
                caster_team,
                ability_name: ability_name.to_string(),
                kind: PendingEffectKind::Composable {
                    origin: caster_pos,
                    current_radius: 0.0,
                    max_radius: mr,
                    speed: *speed,
                    already_hit: Vec::new(),
                    payload: spec.payload.clone(),
                    level,
                },
                delay_ticks_remaining: 0,
            });
        }
        Delivery::DelayedPulse { delay, pulse_count, pulse_interval, radius } => {
            debug_assert!(!radius.is_empty(), "DelayedPulse radius must not be empty");
            if radius.is_empty() {
                return;
            }
            let radius_idx = (level.saturating_sub(1) as usize).min(radius.len().saturating_sub(1));
            let r = radius[radius_idx];
            // Determine damage type from the first Damage payload (for self-damage calc).
            let damage_type = spec.payload.iter().find_map(|p| {
                if let Payload::Damage { kind, .. } = p { Some(kind.clone()) } else { None }
            }).unwrap_or(DamageType::Pure);
            let interval_ticks = (*pulse_interval * TICK_RATE) as u32;
            pending_effects.push(PendingEffect {
                caster_id,
                caster_team,
                ability_name: ability_name.to_string(),
                kind: PendingEffectKind::ComposablePulse {
                    payload: spec.payload.clone(),
                    level,
                    radius: r,
                    damage_type,
                    pulses_remaining: *pulse_count,
                    pulse_interval_ticks: interval_ticks,
                    ticks_until_next_pulse: 0,
                },
                delay_ticks_remaining: (*delay * TICK_RATE) as u32,
            });
        }
    }
}

/// Apply a list of payloads to a single target unit.
///
/// Handles damage (with armor/MR/magic-immunity), buff application (with magic-immunity
/// gating for debuffs), dispel, and bounded Chain recursion.
#[allow(clippy::too_many_arguments)]
pub fn apply_payloads(
    payloads: &[Payload],
    ability_name: &str,
    level: u8,
    caster_id: u32,
    _caster_team: u8,
    units: &mut [Unit],
    target_idx: usize,
    tick: u32,
    events: &mut Vec<CombatEvent>,
    depth: usize,
) {
    for payload in payloads {
        if let Payload::Chain(_child_spec) = payload {
            if depth + 1 > MAX_EFFECT_CHAIN_DEPTH {
                continue;
            }
            // Chain is a scaffold — Rage/Ravage don't use it.
            continue;
        }
        let outcome = apply_payload_to_unit(payload, level, caster_id, units, target_idx);
        match outcome {
            PayloadOutcome::Damage { amount, damage_type } => {
                if amount > 0.0 {
                    events.push(CombatEvent::AbilityDamage {
                        tick,
                        caster_id,
                        target_id: units[target_idx].id,
                        ability_name: ability_name.to_string(),
                        damage: amount,
                        damage_type,
                    });
                }
            }
            PayloadOutcome::BuffApplied { name, .. } => {
                events.push(CombatEvent::BuffApplied {
                    tick,
                    target_id: units[target_idx].id,
                    name,
                });
            }
            PayloadOutcome::Dispel | PayloadOutcome::Skipped => {}
        }
    }
}
