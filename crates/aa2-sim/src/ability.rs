//! Ability execution engine: resolves ability effects when a cast completes.

use aa2_data::AbilityDef;
use crate::pending::PendingEffect;
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
    // All abilities dispatch through effect_specs (composable resolver).
    if let Some(specs) = ability.effect_specs.as_ref() {
        return crate::effect_spec::run_cast_effect_specs(
            specs, &ability.name, level, caster_id, caster_team, caster_pos,
            target_id, target_pos,
            units, tick, pending_effects,
        );
    }

    // No effect_specs → ability is a no-op (e.g. test fixtures with no specs).
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use aa2_data::{AbilityDef, Attribute, DamageType, HeroDef, TargetType};
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

    fn make_damage_ability(kind: DamageType, base: Vec<f32>) -> AbilityDef {
        AbilityDef {
            name: "TestAbility".to_string(),
            cooldown: vec![10.0],
            mana_cost: vec![100.0],
            cast_point: 0.3,
            targeting: TargetType::SingleEnemy,
            description: String::new(), is_ultimate: false,
            aoe_shape: None,
            cast_range: 600.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
            effect_specs: Some(vec![aa2_data::EffectSpec {
                trigger: aa2_data::Trigger::OnCast,
                targeting: aa2_data::TargetingSpec::EnemiesInDelivery,
                delivery: aa2_data::Delivery::Instant,
                payload: vec![aa2_data::Payload::Damage { kind, base }],
                illusion_interaction: aa2_data::IllusionInteraction::Disabled,
                mana_cost: vec![],
            }]),
        }
    }

    fn make_heal_ability(base: Vec<f32>) -> AbilityDef {
        AbilityDef {
            name: "TestAbility".to_string(),
            cooldown: vec![10.0],
            mana_cost: vec![100.0],
            cast_point: 0.3,
            targeting: TargetType::SingleEnemy,
            description: String::new(), is_ultimate: false,
            aoe_shape: None,
            cast_range: 600.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
            effect_specs: Some(vec![aa2_data::EffectSpec {
                trigger: aa2_data::Trigger::OnCast,
                targeting: aa2_data::TargetingSpec::EnemiesInDelivery,
                delivery: aa2_data::Delivery::Instant,
                payload: vec![aa2_data::Payload::Heal { base }],
                illusion_interaction: aa2_data::IllusionInteraction::Disabled,
                mana_cost: vec![],
            }]),
        }
    }

    fn make_buff_ability(name: &str, duration: f32) -> AbilityDef {
        AbilityDef {
            name: "TestAbility".to_string(),
            cooldown: vec![10.0],
            mana_cost: vec![100.0],
            cast_point: 0.3,
            targeting: TargetType::SingleEnemy,
            description: String::new(), is_ultimate: false,
            aoe_shape: None,
            cast_range: 600.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
            effect_specs: Some(vec![aa2_data::EffectSpec {
                trigger: aa2_data::Trigger::OnCast,
                targeting: aa2_data::TargetingSpec::EnemiesInDelivery,
                delivery: aa2_data::Delivery::Instant,
                payload: vec![aa2_data::Payload::ApplyBuff(Box::new(aa2_data::BuffDef {
                    name: name.to_string(),
                    duration: vec![duration],
                    status: aa2_data::StatusFlags::default(),
                    stat_modifier: None,
                    tick_effect: None,
                    stacking: aa2_data::StackBehavior::RefreshDuration,
                    dispel_type: aa2_data::DispelType::BasicDispel,
                    is_debuff: true,
                    pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
                    on_death: None,
                }))],
                illusion_interaction: aa2_data::IllusionInteraction::Disabled,
                mana_cost: vec![],
            }]),
        }
    }

    #[test]
    fn test_ability_damage_physical() {
        let def = make_test_hero();
        let mut units = vec![
            Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0)),
            Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0)),
        ];
        let ability = make_damage_ability(DamageType::Physical, vec![100.0, 150.0, 200.0]);

        let hp_before = units[1].hp;
        let armor = units[1].armor;
        let events = execute_ability(&ability, 1, 0, 0, Vec2::new(0.0, 0.0), Some(1), None, &mut units, 10, &mut Vec::new());

        let expected_dmg = crate::combat::apply_armor(100.0, armor);
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
        let ability = make_damage_ability(DamageType::Magical, vec![200.0]);

        let hp_before = units[1].hp;
        let mr = units[1].magic_resistance; // 0.25
        execute_ability(&ability, 1, 0, 0, Vec2::new(0.0, 0.0), Some(1), None, &mut units, 10, &mut Vec::new());

        let expected_dmg = crate::combat::apply_magic_resistance(200.0, mr);
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
        let ability = make_damage_ability(DamageType::Pure, vec![100.0]);

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
        let ability = make_heal_ability(vec![50.0, 75.0, 100.0]);

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
        let ability = make_buff_ability("slow", 3.0);

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
                    on_death: None,
        });

        let hp_before = units[1].hp;
        let ability = make_damage_ability(DamageType::Magical, vec![200.0]);
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
                    on_death: None,
        });

        let hp_before = units[1].hp;
        let ability = make_damage_ability(DamageType::Physical, vec![100.0]);
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
                description: String::new(), is_ultimate: false,
                aoe_shape: None,
                cast_range: 600.0,
                cast_behavior: aa2_data::CastBehavior::default(),
                max_charges: None,
                effect_specs: Some(vec![aa2_data::EffectSpec {
                    trigger: aa2_data::Trigger::OnCast,
                    targeting: aa2_data::TargetingSpec::EnemiesInDelivery,
                    delivery: aa2_data::Delivery::Instant,
                    payload: vec![aa2_data::Payload::Damage { kind: DamageType::Magical, base: vec![100.0] }],
                    illusion_interaction: aa2_data::IllusionInteraction::Disabled,
                    mana_cost: vec![],
                }]),
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
                    on_death: None,
        });

        let units = vec![u0.clone(), u1];
        let result = try_find_cast(&units[0], &units);
        // AI should not find a target (magic immune)
        assert!(result.is_none());
    }

    #[test]
    fn test_rage_dispels_on_cast() {
        use crate::buff::active_status;
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
                    on_death: None,
        });
        assert_eq!(units[0].buffs.len(), 1);

        let ability = AbilityDef {
            name: "Rage".to_string(),
            cooldown: vec![18.0],
            mana_cost: vec![80.0],
            cast_point: 0.0,
            targeting: TargetType::NoTarget,
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
                    on_death: None,
                    })),
                ],
                illusion_interaction: aa2_data::IllusionInteraction::Disabled, mana_cost: vec![],
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
                    on_death: None,
        });

        attacker.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Essence Shift".to_string(),
                cooldown: vec![0.0],
                mana_cost: vec![0.0],
                cast_point: 0.0,
                targeting: TargetType::Passive,
                description: String::new(), is_ultimate: false,
                aoe_shape: None,
                cast_range: 0.0,
                cast_behavior: aa2_data::CastBehavior::default(),
                max_charges: None,
                effect_specs: Some(vec![aa2_data::EffectSpec {
                    trigger: aa2_data::Trigger::OnAttack,
                    targeting: aa2_data::TargetingSpec::AttackTarget,
                    delivery: aa2_data::Delivery::Instant,
                    payload: vec![aa2_data::Payload::StatSteal {
                        str_steal: vec![1.0],
                        agi_steal: vec![1.0],
                        int_steal: vec![1.0],
                        agi_gain: vec![3.0],
                        duration: vec![30.0],
                    }],
                    illusion_interaction: aa2_data::IllusionInteraction::Disabled, mana_cost: vec![],
                }]),
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
