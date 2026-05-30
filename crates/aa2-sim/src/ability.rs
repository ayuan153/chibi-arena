//! Ability execution engine: resolves ability effects when a cast completes.

use aa2_data::{AbilityDef, DamageType, Effect, TargetType};
use crate::aoe::find_aoe_targets;
use crate::buff::{active_status, apply_buff, Buff, DispelType, StackBehavior, StatusFlags};
use crate::combat::{apply_armor, apply_magic_resistance};
use crate::pending::{PendingEffect, PendingEffectKind};
use crate::unit::Unit;
use crate::vec2::Vec2;
use crate::CombatEvent;

/// Execute an ability's effects when cast completes.
/// Returns a list of combat events generated.
#[allow(clippy::too_many_arguments)]
pub fn execute_ability(
    ability: &AbilityDef,
    level: u8,
    caster_id: u32,
    caster_team: u8,
    caster_pos: Vec2,
    target_id: Option<u32>,
    target_pos: Option<Vec2>,
    units: &mut [Unit],
    tick: u32,
    pending_effects: &mut Vec<PendingEffect>,
) -> Vec<CombatEvent> {
    // Composable path: if effect_specs is present, use the generic resolver and skip legacy.
    if let Some(specs) = ability.effect_specs.as_ref() {
        return crate::effect_spec::run_cast_effect_specs(
            specs, &ability.name, level, caster_id, caster_team, caster_pos,
            target_id, target_pos,
            units, tick, pending_effects,
        );
    }

    let mut events = Vec::new();

    // Determine target indices based on targeting type
    let target_indices: Vec<usize> = match &ability.targeting {
        TargetType::PointAoE => {
            let Some(shape) = &ability.aoe_shape else { return events };
            let origin = target_pos.unwrap_or(caster_pos);
            let direction = (origin - caster_pos).normalize();
            // Default to facing right if origin == caster_pos
            let direction = if direction.length() < 1e-6 { Vec2::new(1.0, 0.0) } else { direction };
            // Damage effects hit enemies, heal effects hit allies — use first effect to decide
            let hit_enemies = ability.effects.first().is_none_or(|e| !matches!(e, Effect::Heal { .. }));
            find_aoe_targets(shape, origin, direction, units, caster_id, caster_team, hit_enemies)
        }
        TargetType::NoTarget => {
            // No target needed — effects handled in the second loop (DarkPact, etc.)
            vec![]
        }
        _ => {
            // Single-target (SingleEnemy, SingleAlly, SingleAllyHG): resolve target
            match target_id {
                Some(tid) => match units.iter().position(|u| u.id == tid && u.is_alive()) {
                    Some(idx) => vec![idx],
                    None => return events,
                },
                None => return events,
            }
        }
    };

    for &idx in &target_indices {
        for effect in &ability.effects {
            match effect {
                Effect::Damage { kind, base } => {
                    let raw = value_at_level(base, level);
                    let actual = match kind {
                        DamageType::Physical => apply_armor(raw, units[idx].armor),
                        DamageType::Magical => {
                            if active_status(&units[idx].buffs).magic_immune {
                                0.0
                            } else {
                                apply_magic_resistance(raw, units[idx].magic_resistance)
                            }
                        }
                        DamageType::Pure => raw,
                    };
                    if actual > 0.0 {
                        units[idx].hp -= actual;
                        events.push(CombatEvent::AbilityDamage {
                            tick,
                            caster_id,
                            target_id: units[idx].id,
                            ability_name: ability.name.clone(),
                            damage: actual,
                            damage_type: kind.clone(),
                        });
                    }
                }
                Effect::Heal { base } => {
                    let raw = value_at_level(base, level);
                    let before = units[idx].hp;
                    units[idx].hp = (units[idx].hp + raw).min(units[idx].max_hp);
                    let healed = units[idx].hp - before;
                    events.push(CombatEvent::Heal {
                        tick,
                        target_id: units[idx].id,
                        amount: healed,
                    });
                }
                Effect::ApplyBuff { name, duration } => {
                    let is_debuff = units[idx].team != caster_team;
                    // Skip non-piercing debuffs on magic immune units
                    if is_debuff && active_status(&units[idx].buffs).magic_immune {
                        continue;
                    }
                    let buff = Buff {
                        name: name.clone(),
                        remaining_ticks: (*duration * 30.0) as u32,
                        tick_effect: None,
                        stacking: StackBehavior::RefreshDuration,
                        dispel_type: DispelType::BasicDispel,
                        status: StatusFlags::default(),
                        stat_modifier: None,
                        source_id: caster_id,
                        is_debuff,
                        pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
                    };
                    apply_buff(&mut units[idx].buffs, buff);
                    events.push(CombatEvent::BuffApplied {
                        tick,
                        target_id: units[idx].id,
                        name: name.clone(),
                    });
                }
                Effect::Summon { .. } => {}
                Effect::DarkPact { .. } => {
                    // These are handled outside the per-target loop
                }
                Effect::FurySwipes { .. } | Effect::ChaosStrike { .. } | Effect::EssenceShift { .. } => {
                    // Attack modifiers are handled in the attack pipeline, not ability execution
                }
                Effect::GlaivesOfWisdom { .. } => {
                    // Attack modifier — handled in the attack pipeline
                }
                Effect::Burrowstrike { .. } => {
                    // Handled outside the per-target loop
                }
                Effect::SpearOfMars { .. } => {
                    // Handled outside the per-target loop
                }
                Effect::SpiritLance { .. } => {
                    // Handled outside the per-target loop
                }
            }
        }
    }

    // Handle effects that don't iterate over targets
    for effect in &ability.effects {
        match effect {
            Effect::DarkPact {
                kind, total_damage, radius, self_damage_pct,
                delay, pulse_count, pulse_interval, dispel_self, non_lethal,
            } => {
                let dmg_total = value_at_level(total_damage, level);
                let r = value_at_level(radius, level);
                let interval_ticks = (*pulse_interval * 30.0) as u32;
                pending_effects.push(PendingEffect {
                    caster_id,
                    caster_team,
                    ability_name: ability.name.clone(),
                    kind: PendingEffectKind::DarkPactPulse {
                        damage_per_pulse: dmg_total / *pulse_count as f32,
                        radius: r,
                        self_damage_pct: *self_damage_pct,
                        damage_type: kind.clone(),
                        dispel_self: *dispel_self,
                        non_lethal: *non_lethal,
                        pulses_remaining: *pulse_count,
                        pulse_interval_ticks: interval_ticks,
                        ticks_until_next_pulse: 0,
                    },
                    delay_ticks_remaining: (*delay * 30.0) as u32,
                });
            }
            Effect::Burrowstrike {
                damage, stun_duration, range, width, travel_speed,
                caustic_finale_damage, caustic_finale_radius,
            } => {
                let line_length = value_at_level(range, level);
                let end_point = if let Some(tpos) = target_pos {
                    let dir = (tpos - caster_pos).normalize();
                    let dir = if dir.length() < 1e-6 { Vec2::new(1.0, 0.0) } else { dir };
                    caster_pos + dir.scale(line_length)
                } else {
                    caster_pos + Vec2::new(line_length, 0.0)
                };

                let travel_time_secs = line_length / *travel_speed;
                let travel_ticks = (travel_time_secs * 30.0) as u32;

                // Apply invulnerable buff to caster during travel
                if let Some(caster) = units.iter_mut().find(|u| u.id == caster_id) {
                    let invuln_buff = Buff {
                        name: "burrowstrike_invuln".to_string(),
                        remaining_ticks: travel_ticks + 1,
                        tick_effect: None,
                        stacking: StackBehavior::RefreshDuration,
                        dispel_type: DispelType::Undispellable,
                        status: StatusFlags { invulnerable: true, stunned: true, ..StatusFlags::default() },
                        stat_modifier: None,
                        source_id: caster_id,
                        is_debuff: false,
                        pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
                    };
                    apply_buff(&mut caster.buffs, invuln_buff);
                }

                let cf_dmg = value_at_level(caustic_finale_damage, level);

                pending_effects.push(PendingEffect {
                    caster_id,
                    caster_team,
                    ability_name: ability.name.clone(),
                    kind: PendingEffectKind::BurrowstrikeTravel {
                        start_pos: caster_pos,
                        end_pos: end_point,
                        travel_speed: *travel_speed,
                        current_distance: 0.0,
                        max_distance: line_length,
                        width: *width,
                        damage: value_at_level(damage, level),
                        stun_duration_secs: value_at_level(stun_duration, level),
                        caustic_finale_damage: cf_dmg,
                        caustic_finale_radius: *caustic_finale_radius,
                        caustic_finale_duration_secs: 4.5,
                        already_hit: Vec::new(),
                        pending_damage: Vec::new(),
                    },
                    delay_ticks_remaining: 0,
                });
            }
            Effect::SpearOfMars {
                damage, stun_duration, range, travel_speed, width,
                fire_trail_dps, fire_trail_slow, fire_trail_duration, wall_bounces,
            } => {
                let direction = if let Some(tpos) = target_pos {
                    let d = (tpos - caster_pos).normalize();
                    if d.length() < 1e-6 { Vec2::new(1.0, 0.0) } else { d }
                } else {
                    Vec2::new(1.0, 0.0)
                };
                let wb = if !wall_bounces.is_empty() {
                    let idx = (level.saturating_sub(1) as usize).min(wall_bounces.len().saturating_sub(1));
                    wall_bounces[idx]
                } else {
                    0
                };
                pending_effects.push(PendingEffect {
                    caster_id,
                    caster_team,
                    ability_name: ability.name.clone(),
                    kind: PendingEffectKind::SpearOfMarsTravel {
                        start_pos: caster_pos,
                        direction,
                        travel_speed: *travel_speed,
                        max_range: value_at_level(range, level),
                        current_distance: 0.0,
                        width: *width,
                        damage: value_at_level(damage, level),
                        stun_duration_secs: value_at_level(stun_duration, level),
                        impaled_unit: None,
                        pass_through_hit: Vec::new(),
                        fire_trail_dps: value_at_level(fire_trail_dps, level),
                        fire_trail_slow: value_at_level(fire_trail_slow, level),
                        fire_trail_duration_secs: value_at_level(fire_trail_duration, level),
                        bounces_remaining: wb,
                        fire_trail_positions: Vec::new(),
                    },
                    delay_ticks_remaining: 0,
                });
            }
            Effect::SpiritLance {
                damage, slow_pct, slow_duration, projectile_speed,
                illusion_damage_dealt, illusion_damage_taken, illusion_duration,
                bounce_radius, bounce_count,
            } => {
                if let Some(tid) = target_id {
                    let dmg = value_at_level(damage, level);
                    let slow = value_at_level(slow_pct, level);
                    let slow_dur = value_at_level(slow_duration, level);
                    let ill_dealt = value_at_level(illusion_damage_dealt, level);
                    let ill_taken = *illusion_damage_taken;
                    let ill_dur_ticks = (value_at_level(illusion_duration, level) * 30.0) as u32;
                    let br = value_at_level(bounce_radius, level);
                    let bc = {
                        let idx = (level.saturating_sub(1) as usize).min(bounce_count.len().saturating_sub(1));
                        bounce_count[idx]
                    };
                    pending_effects.push(PendingEffect {
                        caster_id,
                        caster_team,
                        ability_name: ability.name.clone(),
                        kind: PendingEffectKind::SpiritLanceProjectile {
                            target_id: tid,
                            caster_id,
                            caster_team,
                            position: caster_pos,
                            speed: *projectile_speed,
                            damage: dmg,
                            slow_pct: slow,
                            slow_duration_secs: slow_dur,
                            illusion_damage_dealt_pct: ill_dealt,
                            illusion_damage_taken_pct: ill_taken,
                            illusion_duration_ticks: ill_dur_ticks,
                            bounce_radius: br,
                            bounces_remaining: bc,
                            already_hit: vec![tid],
                        },
                        delay_ticks_remaining: 0,
                    });
                }
            }
            _ => {}
        }
    }

    events
}

/// Get value from a per-level array. Level is 1-indexed (level 1 = base[0]).
fn value_at_level(base: &[f32], level: u8) -> f32 {
    let idx = (level.saturating_sub(1) as usize).min(base.len().saturating_sub(1));
    base[idx]
}

#[cfg(test)]
mod tests {
    use super::*;
    use aa2_data::{AbilityDef, Attribute, DamageType, Effect, HeroDef, TargetType};
    use crate::unit::Unit;
    use crate::vec2::Vec2;

    fn make_test_hero() -> HeroDef {
        HeroDef {
            name: "TestHero".to_string(),
            primary_attribute: Attribute::Strength,
            base_str: 20.0,
            base_agi: 20.0,
            base_int: 20.0,
            str_gain: 2.0,
            agi_gain: 2.0,
            int_gain: 2.0,
            base_attack_time: 1.7,
            attack_range: 150.0,
            attack_point: 0.5,
            move_speed: 300.0,
            turn_rate: 0.6,
            collision_radius: 24.0,
            tier: 1,
            is_melee: true,
            base_damage_min: 30.0,
            base_damage_max: 30.0,
            projectile_speed: None,
        }
    }

    fn make_ability(effects: Vec<Effect>) -> AbilityDef {
        AbilityDef {
            name: "TestAbility".to_string(),
            cooldown: vec![10.0],
            mana_cost: vec![100.0],
            cast_point: 0.3,
            targeting: TargetType::SingleEnemy,
            effects,
            description: String::new(), is_ultimate: false,
            aoe_shape: None,
            cast_range: 600.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None, effect_specs: None,
        }
    }

    #[test]
    fn test_ability_damage_physical() {
        let def = make_test_hero();
        let mut units = vec![
            Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0)),
            Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0)),
        ];
        let ability = make_ability(vec![Effect::Damage {
            kind: DamageType::Physical,
            base: vec![100.0, 150.0, 200.0],
        }]);

        let hp_before = units[1].hp;
        let armor = units[1].armor;
        let events = execute_ability(&ability, 1, 0, 0, Vec2::new(0.0, 0.0), Some(1), None, &mut units, 10, &mut Vec::new());

        let expected_dmg = apply_armor(100.0, armor);
        assert!((hp_before - units[1].hp - expected_dmg).abs() < 0.01);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], CombatEvent::AbilityDamage { damage, damage_type: DamageType::Physical, .. } if (*damage - expected_dmg).abs() < 0.01));
    }

    #[test]
    fn test_ability_damage_magical() {
        let def = make_test_hero();
        let mut units = vec![
            Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0)),
            Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0)),
        ];
        let ability = make_ability(vec![Effect::Damage {
            kind: DamageType::Magical,
            base: vec![200.0],
        }]);

        let hp_before = units[1].hp;
        let mr = units[1].magic_resistance; // 0.25
        execute_ability(&ability, 1, 0, 0, Vec2::new(0.0, 0.0), Some(1), None, &mut units, 10, &mut Vec::new());

        let expected_dmg = apply_magic_resistance(200.0, mr);
        assert!((hp_before - units[1].hp - expected_dmg).abs() < 0.01);
        // 25% magic resistance -> 150 damage
        assert!((expected_dmg - 150.0).abs() < 0.01);
    }

    #[test]
    fn test_ability_damage_pure() {
        let def = make_test_hero();
        let mut units = vec![
            Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0)),
            Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0)),
        ];
        let ability = make_ability(vec![Effect::Damage {
            kind: DamageType::Pure,
            base: vec![100.0],
        }]);

        let hp_before = units[1].hp;
        execute_ability(&ability, 1, 0, 0, Vec2::new(0.0, 0.0), Some(1), None, &mut units, 10, &mut Vec::new());

        assert!((hp_before - units[1].hp - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_ability_heal() {
        let def = make_test_hero();
        let mut units = vec![
            Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0)),
            Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0)),
        ];
        // Damage the target first
        units[1].hp = 100.0;
        let ability = make_ability(vec![Effect::Heal { base: vec![50.0, 75.0, 100.0] }]);

        let events = execute_ability(&ability, 2, 0, 0, Vec2::new(0.0, 0.0), Some(1), None, &mut units, 10, &mut Vec::new());

        assert!((units[1].hp - 175.0).abs() < 0.01);
        assert!(matches!(&events[0], CombatEvent::Heal { amount, .. } if (*amount - 75.0).abs() < 0.01));

        // Test cap at max_hp
        units[1].hp = units[1].max_hp - 10.0;
        let events = execute_ability(&ability, 2, 0, 0, Vec2::new(0.0, 0.0), Some(1), None, &mut units, 20, &mut Vec::new());
        assert!((units[1].hp - units[1].max_hp).abs() < 0.01);
        assert!(matches!(&events[0], CombatEvent::Heal { amount, .. } if (*amount - 10.0).abs() < 0.01));
    }

    #[test]
    fn test_ability_apply_buff() {
        let def = make_test_hero();
        let mut units = vec![
            Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0)),
            Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0)),
        ];
        let ability = make_ability(vec![Effect::ApplyBuff {
            name: "slow".to_string(),
            duration: 3.0,
        }]);

        let events = execute_ability(&ability, 1, 0, 0, Vec2::new(0.0, 0.0), Some(1), None, &mut units, 10, &mut Vec::new());

        assert_eq!(units[1].buffs.len(), 1);
        assert_eq!(units[1].buffs[0].name, "slow");
        assert_eq!(units[1].buffs[0].remaining_ticks, 90); // 3.0 * 30
        assert!(matches!(&events[0], CombatEvent::BuffApplied { name, .. } if name == "slow"));
    }

    #[test]
    fn test_rage_blocks_magical_damage() {
        let def = make_test_hero();
        let mut units = vec![
            Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0)),
            Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0)),
        ];
        // Give unit 1 magic immunity
        use crate::buff::{Buff, StackBehavior, DispelType, StatusFlags};
        units[1].buffs.push(Buff {
            name: "rage".to_string(),
            remaining_ticks: 90,
            tick_effect: None,
            stacking: StackBehavior::RefreshDuration,
            dispel_type: DispelType::Undispellable,
            status: StatusFlags { magic_immune: true, ..StatusFlags::default() },
            stat_modifier: None,
            source_id: 1,
            is_debuff: false,
            pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
        });

        let hp_before = units[1].hp;
        let ability = make_ability(vec![Effect::Damage {
            kind: DamageType::Magical,
            base: vec![200.0],
        }]);
        execute_ability(&ability, 1, 0, 0, Vec2::new(0.0, 0.0), Some(1), None, &mut units, 10, &mut Vec::new());
        // Magic immune unit takes 0 magical damage
        assert!((units[1].hp - hp_before).abs() < 0.01);
    }

    #[test]
    fn test_rage_allows_physical_damage() {
        let def = make_test_hero();
        let mut units = vec![
            Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0)),
            Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0)),
        ];
        use crate::buff::{Buff, StackBehavior, DispelType, StatusFlags};
        units[1].buffs.push(Buff {
            name: "rage".to_string(),
            remaining_ticks: 90,
            tick_effect: None,
            stacking: StackBehavior::RefreshDuration,
            dispel_type: DispelType::Undispellable,
            status: StatusFlags { magic_immune: true, ..StatusFlags::default() },
            stat_modifier: None,
            source_id: 1,
            is_debuff: false,
            pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
        });

        let hp_before = units[1].hp;
        let ability = make_ability(vec![Effect::Damage {
            kind: DamageType::Physical,
            base: vec![100.0],
        }]);
        execute_ability(&ability, 1, 0, 0, Vec2::new(0.0, 0.0), Some(1), None, &mut units, 10, &mut Vec::new());
        // Physical damage still applies
        assert!(units[1].hp < hp_before);
    }

    #[test]
    fn test_rage_prevents_spell_targeting() {
        use crate::ai::try_find_cast;
        use crate::cast::AbilityState;
        use crate::buff::{Buff, StackBehavior, DispelType, StatusFlags};

        let def = make_test_hero();
        let mut u0 = Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0));
        let mut u1 = Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0));

        u0.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Fireball".to_string(),
                cooldown: vec![10.0],
                mana_cost: vec![50.0],
                cast_point: 0.3,
                targeting: TargetType::SingleEnemy,
                effects: vec![Effect::Damage { kind: DamageType::Magical, base: vec![100.0] }],
                description: String::new(), is_ultimate: false,
                aoe_shape: None,
                cast_range: 600.0,
                cast_behavior: aa2_data::CastBehavior::default(),
                max_charges: None,
                effect_specs: None,
            },
            cooldown_remaining: 0.0,
            level: 1,
            casts: 0,
            charges: None,
        });

        // Make target magic immune
        u1.buffs.push(Buff {
            name: "rage".to_string(),
            remaining_ticks: 90,
            tick_effect: None,
            stacking: StackBehavior::RefreshDuration,
            dispel_type: DispelType::Undispellable,
            status: StatusFlags { magic_immune: true, ..StatusFlags::default() },
            stat_modifier: None,
            source_id: 1,
            is_debuff: false,
            pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
        });

        let units = vec![u0.clone(), u1];
        let result = try_find_cast(&units[0], &units);
        // AI should not find a target (magic immune)
        assert!(result.is_none());
    }

    #[test]
    fn test_rage_dispels_on_cast() {
        let def = make_test_hero();
        let mut units = vec![
            Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0)),
            Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0)),
        ];
        // Apply a debuff to caster
        use crate::buff::{Buff, StackBehavior, DispelType, StatusFlags};
        units[0].buffs.push(Buff {
            name: "slow".to_string(),
            remaining_ticks: 90,
            tick_effect: None,
            stacking: StackBehavior::RefreshDuration,
            dispel_type: DispelType::BasicDispel,
            status: StatusFlags::default(),
            stat_modifier: None,
            source_id: 1,
            is_debuff: true,
            pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
        });
        assert_eq!(units[0].buffs.len(), 1);

        let ability = AbilityDef {
            name: "Rage".to_string(),
            cooldown: vec![18.0],
            mana_cost: vec![80.0],
            cast_point: 0.0,
            targeting: TargetType::NoTarget,
            effects: vec![],
            description: String::new(), is_ultimate: false,
            aoe_shape: None,
            cast_range: 0.0,
            cast_behavior: aa2_data::CastBehavior::default(),
            max_charges: None,
            effect_specs: Some(vec![aa2_data::EffectSpec {
                trigger: aa2_data::Trigger::OnCast,
                targeting: aa2_data::TargetingSpec::Caster,
                delivery: aa2_data::Delivery::Instant,
                payload: vec![
                    aa2_data::Payload::Dispel { strength: aa2_data::DispelType::BasicDispel },
                    aa2_data::Payload::ApplyBuff(Box::new(aa2_data::BuffDef {
                        name: "rage".to_string(),
                        duration: vec![4.0],
                        status: aa2_data::StatusFlags { magic_immune: true, ..Default::default() },
                        stat_modifier: None,
                        tick_effect: None,
                        stacking: aa2_data::StackBehavior::RefreshDuration,
                        dispel_type: aa2_data::DispelType::Undispellable,
                        is_debuff: false,
                        pierces_magic_immunity: false,
                        damage_reflection_pct: 0.0,
                    })),
                ],
            }]),
        };
        execute_ability(&ability, 1, 0, 0, Vec2::new(0.0, 0.0), None, None, &mut units, 10, &mut Vec::new());

        // Debuff should be dispelled, rage buff should be applied
        assert_eq!(units[0].buffs.len(), 1);
        assert_eq!(units[0].buffs[0].name, "rage");
        assert!(active_status(&units[0].buffs).magic_immune);
    }

    #[test]
    fn test_essence_shift_pierces_rage() {
        use crate::attack_modifier::post_attack_effects;
        use crate::cast::AbilityState;
        use crate::buff::{Buff, StackBehavior, DispelType, StatusFlags};

        let def = make_test_hero();
        let mut attacker = Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0));
        let mut target = Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0));

        // Give target magic immunity
        target.buffs.push(Buff {
            name: "rage".to_string(),
            remaining_ticks: 90,
            tick_effect: None,
            stacking: StackBehavior::RefreshDuration,
            dispel_type: DispelType::Undispellable,
            status: StatusFlags { magic_immune: true, ..StatusFlags::default() },
            stat_modifier: None,
            source_id: 1,
            is_debuff: false,
            pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
        });

        attacker.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Essence Shift".to_string(),
                cooldown: vec![0.0],
                mana_cost: vec![0.0],
                cast_point: 0.0,
                targeting: TargetType::Passive,
                effects: vec![Effect::EssenceShift {
                    str_steal: vec![1.0],
                    agi_steal: vec![1.0],
                    int_steal: vec![1.0],
                    agi_gain: vec![3.0],
                    duration: vec![30.0],
                }],
                description: String::new(), is_ultimate: false,
                aoe_shape: None,
                cast_range: 0.0,
                cast_behavior: aa2_data::CastBehavior::default(),
                max_charges: None,
                effect_specs: None,
            },
            cooldown_remaining: 0.0,
            level: 1,
            casts: 0,
            charges: None,
        });

        post_attack_effects(&mut attacker, &mut target, 50.0, 0.0, 0);

        // ES debuff should still apply (pierces magic immunity)
        let es_debuffs: Vec<_> = target.buffs.iter().filter(|b| b.name == "essence_shift_debuff").collect();
        assert_eq!(es_debuffs.len(), 1);
        assert!(es_debuffs[0].pierces_magic_immunity);
    }
}
