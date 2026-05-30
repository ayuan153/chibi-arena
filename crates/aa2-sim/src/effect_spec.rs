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

/// Construct a runtime `Buff` from a data-driven `BuffDef`.
///
/// Picks the duration for the given ability level (1-indexed, clamped to last element).
/// Converts seconds to ticks via `TICK_RATE` (30 Hz), truncating to match the sim-wide
/// `(secs * 30.0) as u32` convention used by every other buff/pending duration.
pub fn buff_from_def(def: &BuffDef, level: u8, source_id: u32) -> Buff {
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
        stat_modifier: def.stat_modifier.clone(),
        source_id,
        is_debuff: def.is_debuff,
        pierces_magic_immunity: def.pierces_magic_immunity,
        damage_reflection_pct: def.damage_reflection_pct,
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
            units, tick, pending_effects, &mut events,
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
        match payload {
            Payload::Damage { kind, base } => {
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
                    events.push(CombatEvent::AbilityDamage {
                        tick,
                        caster_id,
                        target_id: units[target_idx].id,
                        ability_name: ability_name.to_string(),
                        damage: actual,
                        damage_type: kind.clone(),
                    });
                }
            }
            Payload::ApplyBuff(def) => {
                let is_debuff = def.is_debuff;
                // Skip non-piercing debuffs on magic immune units
                if is_debuff && !def.pierces_magic_immunity && active_status(&units[target_idx].buffs).magic_immune {
                    continue;
                }
                let buff = buff_from_def(def, level, caster_id);
                // Apply status resistance to debuff duration
                let buff = if is_debuff && units[target_idx].status_resistance > 0.0 {
                    let actual_ticks = (buff.remaining_ticks as f32 * (1.0 - units[target_idx].status_resistance)) as u32;
                    Buff { remaining_ticks: actual_ticks, ..buff }
                } else {
                    buff
                };
                let name = buff.name.clone();
                apply_buff(&mut units[target_idx].buffs, buff);
                events.push(CombatEvent::BuffApplied {
                    tick,
                    target_id: units[target_idx].id,
                    name,
                });
            }
            Payload::Dispel { strength } => {
                dispel(&mut units[target_idx].buffs, *strength);
            }
            Payload::Chain(child_spec) => {
                if depth + 1 > MAX_EFFECT_CHAIN_DEPTH {
                    continue;
                }
                // Chain is a scaffold — Rage/Ravage don't use it.
                let _ = child_spec;
            }
        }
    }
}
