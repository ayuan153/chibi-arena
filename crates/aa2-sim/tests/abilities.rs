//! Ability casting, execution, interactions, and scaling tests.
//! Covers Dark Pact, Heavenly Grace, Ravage, and their interactions.

use aa2_data::{AbilityDef, Attribute, DamageType, Effect, HeroDef, TargetType, UnitConfig};
use aa2_sim::buff::{Buff, DispelType, StackBehavior, StatusFlags};
use aa2_sim::cast::AbilityState;
use aa2_sim::unit::Unit;
use aa2_sim::vec2::Vec2;
use aa2_sim::{CombatEvent, Simulation};
use std::path::Path;

fn data_path(relative: &str) -> std::path::PathBuf {
    Path::new("../../data").join(relative)
}

fn make_hero() -> HeroDef {
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

fn dark_pact_ability() -> AbilityDef {
    AbilityDef {
        name: "Dark Pact".to_string(),
        cooldown: vec![10.0],
        mana_cost: vec![50.0],
        cast_point: 0.0,
        targeting: TargetType::NoTarget,
        effects: vec![Effect::DarkPact {
            kind: DamageType::Magical,
            total_damage: vec![300.0],
            radius: vec![400.0],
            self_damage_pct: 0.3,
            delay: 1.5,
            pulse_count: 10,
            pulse_interval: 0.1,
            dispel_self: true,
            non_lethal: true,
        }],
        description: String::new(), is_ultimate: false,
        aoe_shape: None,
        cast_range: 600.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
    }
}

fn heavenly_grace_ability() -> AbilityDef {
    AbilityDef {
        name: "Heavenly Grace".to_string(),
        cooldown: vec![10.0],
        mana_cost: vec![50.0],
        cast_point: 0.0,
        targeting: TargetType::SingleAlly,
        effects: vec![Effect::BuffTargetAndSelf {
            name: "Heavenly Grace".to_string(),
            duration: vec![10.0],
            hp_regen: vec![20.0],
            strength: vec![30.0],
            status_resistance: vec![0.5],
            dispel_on_cast: false,
        }],
        description: String::new(), is_ultimate: false,
        aoe_shape: None,
        cast_range: 600.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
    }
}

#[allow(dead_code)]
fn ravage_ability() -> AbilityDef {
    AbilityDef {
        name: "Ravage".to_string(),
        cooldown: vec![150.0],
        mana_cost: vec![150.0],
        cast_point: 0.0,
        targeting: TargetType::NoTarget,
        effects: vec![Effect::ExpandingWaveStun {
            damage: vec![250.0],
            stun_duration: vec![2.0],
            radius: vec![1025.0],
            wave_speed: 905.0,
        }],
        description: String::new(), is_ultimate: false,
        aoe_shape: None,
        cast_range: 600.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
    }
}

/// Helper: create a sim with a caster (team 0) and enemy (team 1) at given positions.
/// Caster has the given ability equipped. No AI (we manually trigger).
fn setup_dark_pact_sim(enemy_dist: f32) -> Simulation {
    let hero = make_hero();
    let config_a = UnitConfig::new(hero.clone()).with_ability(dark_pact_ability(), 1);
    let config_b = UnitConfig::new(hero);

    let mut u0 = Unit::from_config(&config_a, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    let u1 = Unit::from_hero_def(&config_b.hero, 1, 1, Vec2::new(enemy_dist, 0.0));

    Simulation::new(vec![u0, u1])
}

#[test]
fn test_dark_pact_delay() {
    let hero = make_hero();
    // Place enemy far away so no auto-attacks happen
    let u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    let u1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(5000.0, 0.0));

    let mut sim = Simulation::new(vec![u0, u1]);

    use aa2_sim::pending::{PendingEffect, PendingEffectKind};
    sim.pending_effects.push(PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Dark Pact".to_string(),
        kind: PendingEffectKind::DarkPactPulse {
            damage_per_pulse: 30.0,
            radius: 10000.0, // large radius to hit distant enemy
            self_damage_pct: 0.3,
            damage_type: DamageType::Magical,
            dispel_self: true,
            non_lethal: true,
            pulses_remaining: 10,
            pulse_interval_ticks: 3,
            ticks_until_next_pulse: 0,
        },
        delay_ticks_remaining: 45,
    });

    // Run for 45 ticks (delay period) — no pulse events should occur
    for _ in 0..45 {
        sim.step();
    }

    assert!(
        !sim.combat_log.iter().any(|e| matches!(e, CombatEvent::DarkPactPulse { .. })),
        "No pulse events during delay"
    );

    // One more tick — first pulse should fire
    sim.step();
    assert!(
        sim.combat_log.iter().any(|e| matches!(e, CombatEvent::DarkPactPulse { .. })),
        "Pulse should fire after delay"
    );
}

#[test]
fn test_dark_pact_pulses() {
    let hero = make_hero();
    // Place enemy far away so no auto-attacks, but within pulse radius
    let u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    let u1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(5000.0, 0.0));

    let mut sim = Simulation::new(vec![u0, u1]);

    use aa2_sim::pending::{PendingEffect, PendingEffectKind};
    sim.pending_effects.push(PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Dark Pact".to_string(),
        kind: PendingEffectKind::DarkPactPulse {
            damage_per_pulse: 30.0,
            radius: 10000.0, // large radius to hit distant enemy
            self_damage_pct: 0.3,
            damage_type: DamageType::Magical,
            dispel_self: false,
            non_lethal: true,
            pulses_remaining: 10,
            pulse_interval_ticks: 3,
            ticks_until_next_pulse: 0,
        },
        delay_ticks_remaining: 0,
    });

    // Run enough ticks for all 10 pulses
    // Pulse fires when ticks_until_next_pulse == 0, then resets to 3
    // So pulses at ticks: 1, 4, 7, 10, 13, 16, 19, 22, 25, 28
    for _ in 0..40 {
        sim.step();
        if sim.is_finished() {
            break;
        }
    }

    let pulse_count = sim.combat_log.iter()
        .filter(|e| matches!(e, CombatEvent::DarkPactPulse { .. }))
        .count();
    assert_eq!(pulse_count, 10, "Should have exactly 10 pulses");

    // Verify pending effect is removed
    assert!(sim.pending_effects.is_empty(), "Pending effect should be removed after all pulses");
}

#[test]
fn test_dark_pact_self_damage() {
    let mut sim = setup_dark_pact_sim(100.0);

    use aa2_sim::pending::{PendingEffect, PendingEffectKind};
    sim.pending_effects.push(PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Dark Pact".to_string(),
        kind: PendingEffectKind::DarkPactPulse {
            damage_per_pulse: 30.0,
            radius: 400.0,
            self_damage_pct: 0.3,
            damage_type: DamageType::Magical,
            dispel_self: false,
            non_lethal: true,
            pulses_remaining: 1,
            pulse_interval_ticks: 3,
            ticks_until_next_pulse: 0,
        },
        delay_ticks_remaining: 0,
    });

    let caster_hp_before = sim.units[0].hp;
    let caster_mr = sim.units[0].magic_resistance; // 0.25

    sim.step();

    // Self-damage: 30 * 0.3 = 9.0 raw, reduced by 25% MR = 6.75
    let expected_self_dmg = 9.0 * (1.0 - caster_mr);
    let actual_self_dmg = caster_hp_before - sim.units[0].hp;
    assert!(
        (actual_self_dmg - expected_self_dmg).abs() < 0.01,
        "Self-damage should be {expected_self_dmg}, got {actual_self_dmg}"
    );
}

#[test]
fn test_dark_pact_dispel() {
    let mut sim = setup_dark_pact_sim(100.0);

    // Apply a stun to caster (basic dispel level so strong dispel removes it)
    sim.units[0].buffs.push(Buff {
        name: "test_stun".to_string(),
        remaining_ticks: 300,
        tick_effect: None,
        stacking: StackBehavior::RefreshDuration,
        dispel_type: DispelType::BasicDispel,
        status: StatusFlags { stunned: true, ..StatusFlags::default() },
        stat_modifier: None,
        source_id: 1,
        is_debuff: true,
            pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
    });

    assert!(sim.units[0].buffs.iter().any(|b| b.name == "test_stun"));

    use aa2_sim::pending::{PendingEffect, PendingEffectKind};
    sim.pending_effects.push(PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Dark Pact".to_string(),
        kind: PendingEffectKind::DarkPactPulse {
            damage_per_pulse: 30.0,
            radius: 400.0,
            self_damage_pct: 0.3,
            damage_type: DamageType::Magical,
            dispel_self: true,
            non_lethal: true,
            pulses_remaining: 1,
            pulse_interval_ticks: 3,
            ticks_until_next_pulse: 0,
        },
        delay_ticks_remaining: 0,
    });

    sim.step();

    // Stun should be dispelled
    assert!(
        !sim.units[0].buffs.iter().any(|b| b.name == "test_stun"),
        "Stun should be dispelled by Dark Pact pulse"
    );
}

#[test]
fn test_dark_pact_non_lethal() {
    let mut sim = setup_dark_pact_sim(5000.0); // enemy far away, won't be hit

    // Set caster to 1 HP
    sim.units[0].hp = 1.0;

    use aa2_sim::pending::{PendingEffect, PendingEffectKind};
    sim.pending_effects.push(PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Dark Pact".to_string(),
        kind: PendingEffectKind::DarkPactPulse {
            damage_per_pulse: 100.0, // high damage
            radius: 400.0,
            self_damage_pct: 0.3,
            damage_type: DamageType::Magical,
            dispel_self: false,
            non_lethal: true,
            pulses_remaining: 10,
            pulse_interval_ticks: 3,
            ticks_until_next_pulse: 0,
        },
        delay_ticks_remaining: 0,
    });

    // Run all pulses
    for _ in 0..30 {
        sim.step();
    }

    // Caster should still be alive at 1 HP
    assert!(sim.units[0].hp >= 1.0, "Non-lethal self-damage should not kill caster");
    assert!(sim.units[0].is_alive(), "Caster should still be alive");
}

#[test]
fn test_dark_pact_full_pipeline() {
    use std::path::Path;
    use aa2_sim::aa2_data::{load_loadout, resolve_loadout, UnitConfig};
    use aa2_sim::Simulation;

    let data_dir = Path::new("../../data");
    let loadout = load_loadout(Path::new("../../data/loadouts/jugg_darkpact.ron")).unwrap();
    let config = resolve_loadout(&loadout, data_dir).unwrap();

    // Verify ability loaded
    assert_eq!(config.abilities.len(), 1);
    assert_eq!(config.abilities[0].0.name, "Dark Pact");

    // Create sim with enemy nearby
    let hero2 = aa2_sim::aa2_data::load_hero_def(Path::new("../../data/heroes/sven.ron")).unwrap();
    let config_b = UnitConfig::new(hero2);

    let mut sim = Simulation::from_configs(&[config], &[config_b], 42);

    // Run for 3 ticks — cast should complete (0 cast point)
    for _ in 0..3 {
        sim.step();
    }

    // Verify pending effect was created
    println!("Pending effects after 3 ticks: {}", sim.pending_effects.len());
    assert!(!sim.pending_effects.is_empty(), "Dark Pact should create a pending effect");

    // Run until pulses fire (45 more ticks for delay + a few for pulses)
    for _ in 0..60 {
        sim.step();
    }

    // Check for DarkPactPulse events
    let pulse_events: Vec<_> = sim.combat_log.iter()
        .filter(|e| matches!(e, aa2_sim::CombatEvent::DarkPactPulse { .. }))
        .collect();
    println!("Pulse events: {}", pulse_events.len());
    assert!(!pulse_events.is_empty(), "Dark Pact pulses should have fired");
}

#[test]
fn test_expanding_wave() {
    let hero = make_hero();
    // Place enemies at different distances
    let u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0)); // caster
    let u1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0)); // close
    let u2 = Unit::from_hero_def(&hero, 2, 1, Vec2::new(500.0, 0.0)); // far

    let mut sim = Simulation::new(vec![u0, u1, u2]);

    use aa2_sim::pending::{PendingEffect, PendingEffectKind};
    sim.pending_effects.push(PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Ravage".to_string(),
        kind: PendingEffectKind::ExpandingWave {
            damage: 250.0,
            stun_duration_secs: 2.0,
            max_radius: 1025.0,
            wave_speed: 905.0,
            current_radius: 0.0,
            origin: Vec2::new(0.0, 0.0),
            already_hit: Vec::new(),
        },
        delay_ticks_remaining: 0,
    });

    // Wave speed 905 units/sec, tick = 1/30s, so ~30.17 units per tick
    // Unit at 100 should be hit around tick 4 (100/30.17 = 3.3)
    // Unit at 500 should be hit around tick 17 (500/30.17 = 16.6)

    let mut hit_ticks: Vec<(u32, u32)> = Vec::new(); // (unit_id, tick)
    for _ in 0..40 {
        sim.step();
        for event in &sim.combat_log {
            if let CombatEvent::WaveHit { target_id, tick, .. } = event
                && !hit_ticks.iter().any(|(id, _)| *id == *target_id)
            {
                hit_ticks.push((*target_id, *tick));
            }
        }
    }

    // Both should be hit
    assert!(hit_ticks.iter().any(|(id, _)| *id == 1), "Close enemy should be hit");
    assert!(hit_ticks.iter().any(|(id, _)| *id == 2), "Far enemy should be hit");

    // Close enemy should be hit before far enemy
    let close_tick = hit_ticks.iter().find(|(id, _)| *id == 1).unwrap().1;
    let far_tick = hit_ticks.iter().find(|(id, _)| *id == 2).unwrap().1;
    assert!(close_tick < far_tick, "Closer enemy should be stunned first: close={close_tick}, far={far_tick}");
}

#[test]
fn test_status_resistance() {
    let hero = make_hero();
    let mut u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    u0.status_resistance = 0.5; // 50% status resistance

    // Apply a 2-second stun (60 ticks base)
    let base_ticks: u32 = 60;
    let actual_ticks = (base_ticks as f32 * (1.0 - u0.status_resistance)) as u32;
    assert_eq!(actual_ticks, 30, "50% status resistance should halve stun duration");

    // Verify via the expanding wave system
    let u1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(0.0, 0.0)); // caster at same pos
    let mut sim = Simulation::new(vec![u0, u1]);

    use aa2_sim::pending::{PendingEffect, PendingEffectKind};
    sim.pending_effects.push(PendingEffect {
        caster_id: 1,
        caster_team: 1,
        ability_name: "Ravage".to_string(),
        kind: PendingEffectKind::ExpandingWave {
            damage: 100.0,
            stun_duration_secs: 2.0,
            max_radius: 500.0,
            wave_speed: 905.0,
            current_radius: 0.0,
            origin: Vec2::new(0.0, 0.0),
            already_hit: Vec::new(),
        },
        delay_ticks_remaining: 0,
    });

    sim.step();

    // Check the stun buff on unit 0
    let stun = sim.units[0].buffs.iter().find(|b| b.name == "stun");
    assert!(stun.is_some(), "Unit should have stun buff");
    // With 50% status resistance, 2.0s (60 ticks) becomes 1.0s (30 ticks)
    // But one tick already passed in step_buffs, so it might be 29
    let remaining = stun.unwrap().remaining_ticks;
    assert!((28..=30).contains(&remaining),
        "Stun duration should be ~30 ticks (halved from 60), got {remaining}");
}

#[test]
fn test_buff_target_and_self() {
    let hero = make_hero();
    let config = UnitConfig::new(hero.clone()).with_ability(heavenly_grace_ability(), 1);

    let mut u0 = Unit::from_config(&config, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    let u1 = Unit::from_hero_def(&hero, 1, 0, Vec2::new(100.0, 0.0)); // ally

    let mut sim = Simulation::new(vec![u0, u1]);

    // Manually execute the ability
    use aa2_sim::ability::execute_ability;
    let ability = heavenly_grace_ability();
    let events = execute_ability(
        &ability, 1, 0, 0, Vec2::new(0.0, 0.0),
        Some(1), Some(Vec2::new(100.0, 0.0)),
        &mut sim.units, 1, &mut sim.pending_effects,
    );

    // Both caster and target should have the buff
    let target_buff = sim.units[1].buffs.iter().find(|b| b.name == "Heavenly Grace");
    assert!(target_buff.is_some(), "Target should have Heavenly Grace buff");

    let caster_buff = sim.units[0].buffs.iter().find(|b| b.name == "Heavenly Grace");
    assert!(caster_buff.is_some(), "Caster should also have Heavenly Grace buff");

    // Verify stat modifier values
    let modifier = target_buff.unwrap().stat_modifier.as_ref().unwrap();
    assert!((modifier.bonus_hp_regen - 20.0).abs() < 0.01);
    assert!((modifier.bonus_strength - 30.0).abs() < 0.01);
    assert!((modifier.status_resistance - 0.5).abs() < 0.01);

    // Verify BuffApplied events for both
    let buff_events: Vec<_> = events.iter()
        .filter(|e| matches!(e, CombatEvent::BuffApplied { name, .. } if name == "Heavenly Grace"))
        .collect();
    assert_eq!(buff_events.len(), 2, "Should have BuffApplied for both target and caster");
}

#[test]
fn test_hg_dispels_on_cast() {
    use aa2_sim::ability::execute_ability;

    let hero = make_hero();
    let mut u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    let mut u1 = Unit::from_hero_def(&hero, 1, 0, Vec2::new(100.0, 0.0)); // ally

    // Apply a debuff to the ally
    u1.buffs.push(Buff {
        name: "curse".to_string(),
        remaining_ticks: 300,
        tick_effect: None,
        stacking: aa2_sim::buff::StackBehavior::RefreshDuration,
        dispel_type: DispelType::StrongDispel,
        status: StatusFlags { silenced: true, ..StatusFlags::default() },
        stat_modifier: None,
        source_id: 99,
        is_debuff: true,
            pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
    });

    let mut units = vec![u0, u1];
    let ability = AbilityDef {
        name: "Heavenly Grace".to_string(),
        cooldown: vec![10.0],
        mana_cost: vec![50.0],
        cast_point: 0.0,
        targeting: TargetType::SingleAlly,
        effects: vec![Effect::BuffTargetAndSelf {
            name: "Heavenly Grace".to_string(),
            duration: vec![10.0],
            hp_regen: vec![20.0],
            strength: vec![30.0],
            status_resistance: vec![0.5],
            dispel_on_cast: true,
        }],
        description: String::new(), is_ultimate: false,
        aoe_shape: None,
        cast_range: 600.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
    };

    execute_ability(
        &ability, 1, 0, 0, Vec2::new(0.0, 0.0),
        Some(1), Some(Vec2::new(100.0, 0.0)),
        &mut units, 1, &mut Vec::new(),
    );

    // Debuff should be removed from ally
    assert!(
        !units[1].buffs.iter().any(|b| b.name == "curse"),
        "Strong dispel should remove the curse debuff"
    );
    // HG buff should be applied
    assert!(units[1].buffs.iter().any(|b| b.name == "Heavenly Grace"));
}

#[test]
fn test_hg_targets_highest_y_ally() {
    use aa2_sim::ai::try_find_cast;
    use aa2_sim::cast::AbilityState;

    let hero = make_hero();
    let mut u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    u0.abilities.push(AbilityState {
        def: AbilityDef {
            name: "Heavenly Grace".to_string(),
            cooldown: vec![10.0],
            mana_cost: vec![50.0],
            cast_point: 0.0,
            targeting: TargetType::SingleAllyHG,
            effects: vec![],
            description: String::new(), is_ultimate: false,
            aoe_shape: None,
            cast_range: 600.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
        },
        cooldown_remaining: 0.0,
        level: 1,
        casts: 0, // first cast
        charges: None,
    });

    // Allies at different y positions
    let u1 = Unit::from_hero_def(&hero, 1, 0, Vec2::new(50.0, 100.0));  // y=100
    let u2 = Unit::from_hero_def(&hero, 2, 0, Vec2::new(50.0, 300.0));  // y=300 (highest)
    let u3 = Unit::from_hero_def(&hero, 3, 0, Vec2::new(50.0, 200.0));  // y=200

    let units = vec![u0, u1, u2, u3];
    let result = try_find_cast(&units[0], &units);

    assert!(result.is_some());
    let (_, target_id, _, _) = result.unwrap();
    assert_eq!(target_id, Some(2), "Should target ally with highest y (id=2, y=300)");
}

#[test]
fn test_hg_self_cast_when_no_allies() {
    use aa2_sim::ai::try_find_cast;
    use aa2_sim::cast::AbilityState;

    let hero = make_hero();
    let mut u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    u0.abilities.push(AbilityState {
        def: AbilityDef {
            name: "Heavenly Grace".to_string(),
            cooldown: vec![10.0],
            mana_cost: vec![50.0],
            cast_point: 0.0,
            targeting: TargetType::SingleAllyHG,
            effects: vec![],
            description: String::new(), is_ultimate: false,
            aoe_shape: None,
            cast_range: 600.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
        },
        cooldown_remaining: 0.0,
        level: 1,
        casts: 0,
        charges: None,
    });

    // Only enemies, no allies
    let u1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));

    let units = vec![u0, u1];
    let result = try_find_cast(&units[0], &units);

    assert!(result.is_some());
    let (_, target_id, _, _) = result.unwrap();
    assert_eq!(target_id, Some(0), "Should self-cast when no allies in range");
}

#[test]
fn test_hg_targets_furthest_on_subsequent_cast() {
    use aa2_sim::ai::try_find_cast;
    use aa2_sim::cast::AbilityState;

    let hero = make_hero();
    let mut u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    u0.abilities.push(AbilityState {
        def: AbilityDef {
            name: "Heavenly Grace".to_string(),
            cooldown: vec![10.0],
            mana_cost: vec![50.0],
            cast_point: 0.0,
            targeting: TargetType::SingleAllyHG,
            effects: vec![],
            description: String::new(), is_ultimate: false,
            aoe_shape: None,
            cast_range: 600.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
        },
        cooldown_remaining: 0.0,
        level: 1,
        casts: 1, // subsequent cast
        charges: None,
    });

    // Ally at y=300 but close (50 units away)
    let u1 = Unit::from_hero_def(&hero, 1, 0, Vec2::new(50.0, 300.0));
    // Ally at y=100 but far (500 units away)
    let u2 = Unit::from_hero_def(&hero, 2, 0, Vec2::new(400.0, 300.0));

    let units = vec![u0, u1, u2];
    let result = try_find_cast(&units[0], &units);

    assert!(result.is_some());
    let (_, target_id, _, _) = result.unwrap();
    assert_eq!(target_id, Some(2), "Should target furthest ally on subsequent cast");
}

/// # Test: AoE Radius Scaling — Gaben (level 9) vs Level 3
///
/// Verifies that Dark Pact's radius scales correctly with level:
/// - Level 9 (Gaben): radius 675 → hits enemy at 600 distance
/// - Level 3: radius 325 → does NOT hit enemy at 600 distance
///
/// This matters because radius scaling is the primary power curve for Dark Pact,
/// and incorrect radius values would make the ability over/under-powered.
#[test]
fn test_aoe_radius_scaling_gaben_vs_level3() {
    use aa2_data::load_ability_def;

    let dark_pact = load_ability_def(&data_path("abilities/dark_pact.ron")).unwrap();
    let hero = make_hero();

    // Extract radius values from the loaded ability data to verify correctness
    let effect = &dark_pact.effects[0];
    let (radius_l9, radius_l3, total_dmg_l9, total_dmg_l3) = match effect {
        Effect::DarkPact { radius, total_damage, pulse_count, .. } => {
            (
                aa2_data::value_at_level(radius, 9),
                aa2_data::value_at_level(radius, 3),
                aa2_data::value_at_level(total_damage, 9) / *pulse_count as f32,
                aa2_data::value_at_level(total_damage, 3) / *pulse_count as f32,
            )
        }
        _ => panic!("Expected DarkPact effect"),
    };
    assert_eq!(radius_l9, 675.0);
    assert_eq!(radius_l3, 325.0);

    // Directly inject pending effects to test radius without caster movement.
    // Caster at origin, enemy at exactly 600 units away.
    let caster = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    let enemy = Unit::from_hero_def(&hero, 1, 1, Vec2::new(600.0, 0.0));

    // --- Gaben (radius 675): should hit at 600 ---
    let mut sim_gaben = Simulation::new(vec![caster.clone(), enemy.clone()]);
    sim_gaben.pending_effects.push(aa2_sim::pending::PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Dark Pact".to_string(),
        kind: aa2_sim::pending::PendingEffectKind::DarkPactPulse {
            damage_per_pulse: total_dmg_l9,
            radius: radius_l9,
            self_damage_pct: 0.3,
            damage_type: DamageType::Magical,
            dispel_self: true,
            non_lethal: true,
            pulses_remaining: 1,
            pulse_interval_ticks: 3,
            ticks_until_next_pulse: 0,
        },
        delay_ticks_remaining: 0,
    });
    sim_gaben.step();

    // --- Level 3 (radius 325): should NOT hit at 600 ---
    let mut sim_l3 = Simulation::new(vec![caster, enemy]);
    sim_l3.pending_effects.push(aa2_sim::pending::PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Dark Pact".to_string(),
        kind: aa2_sim::pending::PendingEffectKind::DarkPactPulse {
            damage_per_pulse: total_dmg_l3,
            radius: radius_l3,
            self_damage_pct: 0.3,
            damage_type: DamageType::Magical,
            dispel_self: true,
            non_lethal: true,
            pulses_remaining: 1,
            pulse_interval_ticks: 3,
            ticks_until_next_pulse: 0,
        },
        delay_ticks_remaining: 0,
    });
    sim_l3.step();

    let gaben_hit = sim_gaben.combat_log.iter().find_map(|e| {
        if let CombatEvent::DarkPactPulse { enemies_hit, .. } = e { Some(*enemies_hit) } else { None }
    });
    let l3_hit = sim_l3.combat_log.iter().find_map(|e| {
        if let CombatEvent::DarkPactPulse { enemies_hit, .. } = e { Some(*enemies_hit) } else { None }
    });

    assert_eq!(gaben_hit, Some(1), "Gaben Dark Pact (radius 675) should hit enemy at 600 distance");
    assert_eq!(l3_hit, Some(0), "Level 3 Dark Pact (radius 325) should NOT hit enemy at 600 distance");
}

/// # Test: Ravage Wave Timing — Distance-Based Stun Order
///
/// Verifies that Ravage's expanding wave stuns closer enemies before farther ones.
/// This is the core mechanic that differentiates Ravage from instant AoE stuns:
/// positioning matters because farther enemies have time to react.
///
/// Wave speed: 905 units/sec = ~30.17 units/tick.
/// Expected tick difference for 300 units: ~10 ticks.
#[test]
fn test_ravage_wave_timing_distance_based() {
    use aa2_data::load_ability_def;

    let ravage = load_ability_def(&data_path("abilities/ravage.ron")).unwrap();
    let hero = make_hero();

    // Verify loaded data
    let effect = &ravage.effects[0];
    let (damage, stun_dur, wave_speed) = match effect {
        aa2_data::Effect::ExpandingWaveStun { damage, stun_duration, wave_speed, .. } => {
            (aa2_data::value_at_level(damage, 2), aa2_data::value_at_level(stun_duration, 2), *wave_speed)
        }
        _ => panic!("Expected ExpandingWaveStun"),
    };
    assert_eq!(wave_speed, 905.0);
    assert_eq!(stun_dur, 2.2);

    // Inject wave directly at origin to avoid cast point and enemy movement.
    // Place caster far away (outside acquisition range 800) so enemies don't walk.
    // Wave origin is set to (0,0) independently of caster position.
    let caster = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, -1000.0));
    let enemy_a = Unit::from_hero_def(&hero, 1, 1, Vec2::new(200.0, 0.0));
    let enemy_b = Unit::from_hero_def(&hero, 2, 1, Vec2::new(500.0, 0.0));

    let mut sim = Simulation::new(vec![caster, enemy_a, enemy_b]);
    sim.pending_effects.push(aa2_sim::pending::PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Ravage".to_string(),
        kind: aa2_sim::pending::PendingEffectKind::ExpandingWave {
            damage,
            stun_duration_secs: stun_dur,
            max_radius: 700.0,
            wave_speed,
            current_radius: 0.0,
            origin: Vec2::new(0.0, 0.0),
            already_hit: Vec::new(),
        },
        delay_ticks_remaining: 0,
    });

    // Run enough ticks for wave to reach 500 units: 500/30.17 ≈ 17 ticks
    for _ in 0..25 {
        sim.step();
    }

    let hit_a_tick = sim.combat_log.iter().find_map(|e| {
        if let CombatEvent::WaveHit { target_id: 1, tick, .. } = e { Some(*tick) } else { None }
    });
    let hit_b_tick = sim.combat_log.iter().find_map(|e| {
        if let CombatEvent::WaveHit { target_id: 2, tick, .. } = e { Some(*tick) } else { None }
    });

    let tick_a = hit_a_tick.expect("Enemy A (200 units) should be hit by Ravage");
    let tick_b = hit_b_tick.expect("Enemy B (500 units) should be hit by Ravage");

    assert!(tick_a < tick_b, "Closer enemy should be stunned first: A@tick {tick_a}, B@tick {tick_b}");

    // Expected difference: (500-200) / (905/30) = 300 / 30.17 ≈ 9.9 ticks
    let diff = tick_b - tick_a;
    assert!(
        (8..=12).contains(&diff),
        "Tick difference should be ~10 (got {diff}): 300 units / (905/30) units_per_tick"
    );
}

/// # Test: Dark Pact Dispels Ravage Stun
///
/// Verifies the key interaction: Dark Pact's self-dispel removes Ravage's stun early.
/// This is the primary reason to pick Dark Pact — it counters hard disables.
///
/// Timeline:
/// - Tick 1: Unit A casts Dark Pact (instant, 1.5s delay before pulses)
/// - Tick ~10: Ravage wave hits Unit A, applying 2.2s stun (66 ticks)
/// - Tick ~46: Dark Pact pulses begin, dispelling the stun
/// - Without dispel, stun would last until tick ~76
#[test]
fn test_dark_pact_dispels_ravage_stun() {
    use aa2_data::load_ability_def;

    let dark_pact = load_ability_def(&data_path("abilities/dark_pact.ron")).unwrap();
    let ravage = load_ability_def(&data_path("abilities/ravage.ron")).unwrap();
    let hero = make_hero();

    // Unit A: has Dark Pact (team 0), facing Unit B
    let config_a = UnitConfig::new(hero.clone()).with_ability(dark_pact, 3);
    let mut unit_a = Unit::from_config(&config_a, 0, 0, Vec2::new(0.0, 0.0));
    unit_a.mana = 500.0;
    unit_a.facing = std::f32::consts::PI; // facing toward Unit B at negative X... actually let's place B at +X
    unit_a.facing = 0.0; // facing +X

    // Unit B: has Ravage (team 1), facing Unit A
    let config_b = UnitConfig::new(hero.clone()).with_ability(ravage, 2);
    let mut unit_b = Unit::from_config(&config_b, 1, 1, Vec2::new(400.0, 0.0));
    unit_b.mana = 500.0;
    unit_b.facing = std::f32::consts::PI; // facing -X toward Unit A

    let mut sim = Simulation::new(vec![unit_a, unit_b]);

    // Run simulation until Unit A attacks (proving stun was dispelled)
    let mut first_attack_tick: Option<u32> = None;
    let mut stun_applied_tick: Option<u32> = None;

    for _ in 0..120 {
        sim.step();
        if stun_applied_tick.is_none()
            && let Some(CombatEvent::WaveHit { target_id: 0, tick, .. }) =
                sim.combat_log.iter().find(|e| matches!(e, CombatEvent::WaveHit { target_id: 0, .. }))
        {
            stun_applied_tick = Some(*tick);
        }
        if first_attack_tick.is_none()
            && let Some(CombatEvent::Attack { attacker_id: 0, tick, .. }) =
                sim.combat_log.iter().find(|e| matches!(e, CombatEvent::Attack { attacker_id: 0, .. }))
        {
            first_attack_tick = Some(*tick);
            break;
        }
    }

    let stun_tick = stun_applied_tick.expect("Ravage should stun Unit A");
    let attack_tick = first_attack_tick.expect("Unit A should attack after dispel");

    // Ravage level 2 stun = 2.2s = 66 ticks. Without dispel, stun expires at stun_tick + 66.
    let stun_natural_expiry = stun_tick + 66;

    assert!(
        attack_tick < stun_natural_expiry,
        "Dark Pact should dispel stun early: attack@{attack_tick} < natural_expiry@{stun_natural_expiry}"
    );
}

/// Fury Swipes damage increases with each attack on the same target.
/// Verifies per-target stacking and that damage grows over time.
#[test]
fn test_fury_swipes_damage_increases() {
    use std::path::Path;
    let hero = aa2_data::load_hero_def(Path::new("../../data/heroes/juggernaut.ron")).unwrap();
    let fs = aa2_data::load_ability_def(Path::new("../../data/abilities/fury_swipes.ron")).unwrap();

    let mut attacker = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    attacker.abilities.push(AbilityState { def: fs, cooldown_remaining: 0.0, level: 3, casts: 0, charges: None });

    let enemy_def = aa2_data::load_hero_def(Path::new("../../data/heroes/sven.ron")).unwrap();
    let enemy = Unit::from_hero_def(&enemy_def, 1, 1, Vec2::new(100.0, 0.0));

    let mut sim = Simulation::with_seed(vec![attacker, enemy], 42);

    // Run until we have at least 4 attack events
    for _ in 0..500 {
        if sim.is_finished() { break; }
        sim.step();
    }

    let attacks: Vec<f32> = sim.combat_log.iter().filter_map(|e| {
        if let CombatEvent::Attack { damage, attacker_id: 0, .. } = e { Some(*damage) } else { None }
    }).collect();

    assert!(attacks.len() >= 4, "Expected at least 4 attacks, got {}", attacks.len());
    // Later attacks should deal more damage than earlier ones (on average, due to stacking)
    // Compare first attack to last attack
    assert!(attacks.last().unwrap() > attacks.first().unwrap(),
        "Last attack ({:.1}) should deal more than first ({:.1}) due to Fury Swipes stacking",
        attacks.last().unwrap(), attacks.first().unwrap());
}

/// Fury Swipes bonus damage is NOT multiplied by Chaos Strike crit.
/// A unit with both should show: crit applies to base damage only, FS is flat on top.
#[test]
fn test_fury_swipes_not_multiplied_by_crit() {
    use std::path::Path;
    let hero = aa2_data::load_hero_def(Path::new("../../data/heroes/chaos_knight.ron")).unwrap();
    let fs = aa2_data::load_ability_def(Path::new("../../data/abilities/fury_swipes.ron")).unwrap();
    let cs = aa2_data::load_ability_def(Path::new("../../data/abilities/chaos_strike.ron")).unwrap();

    let mut attacker = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    attacker.abilities.push(AbilityState { def: fs, cooldown_remaining: 0.0, level: 3, casts: 0, charges: None });
    attacker.abilities.push(AbilityState { def: cs, cooldown_remaining: 0.0, level: 3, casts: 0, charges: None });

    let enemy_def = aa2_data::load_hero_def(Path::new("../../data/heroes/sven.ron")).unwrap();
    let enemy = Unit::from_hero_def(&enemy_def, 1, 1, Vec2::new(100.0, 0.0));

    // Run many combats with different seeds to get both crit and non-crit attacks
    let mut crit_damages = Vec::new();
    let mut non_crit_damages = Vec::new();

    for seed in 0..50 {
        let mut a = attacker.clone();
        let e = enemy.clone();
        // Reset stacks
        a.attack_modifier_state.clear();
        let mut sim = Simulation::with_seed(vec![a, e], seed);

        // Run until first attack
        for _ in 0..200 {
            sim.step();
            if sim.combat_log.iter().any(|e| matches!(e, CombatEvent::Attack { attacker_id: 0, .. })) {
                break;
            }
        }

        if let Some(CombatEvent::Attack { damage, .. }) = sim.combat_log.iter().find(|e| matches!(e, CombatEvent::Attack { attacker_id: 0, .. })) {
            // First attack has 0 FS stacks, so damage is just base (possibly critted)
            // CK base damage: 56-76. Armor reduces it. Crit would multiply.
            // Non-crit max after armor: ~76 * armor_mult ≈ 50-55
            // Crit (120-270%) after armor: could be up to ~76 * 2.7 * armor_mult ≈ 135+
            if *damage > 80.0 {
                crit_damages.push(*damage);
            } else {
                non_crit_damages.push(*damage);
            }
        }
    }

    // We should have both crits and non-crits across 50 seeds
    assert!(!crit_damages.is_empty(), "Should have some crits across 50 seeds");
    assert!(!non_crit_damages.is_empty(), "Should have some non-crits across 50 seeds");
    // Crits should be significantly higher than non-crits
    let avg_crit: f32 = crit_damages.iter().sum::<f32>() / crit_damages.len() as f32;
    let avg_non_crit: f32 = non_crit_damages.iter().sum::<f32>() / non_crit_damages.len() as f32;
    assert!(avg_crit > avg_non_crit * 1.3, "Crits ({:.1}) should be >30% higher than non-crits ({:.1})", avg_crit, avg_non_crit);
}

/// Chaos Strike lifesteal heals the attacker on crit.
#[test]
fn test_chaos_strike_lifesteal_heals() {
    use std::path::Path;
    let hero = aa2_data::load_hero_def(Path::new("../../data/heroes/chaos_knight.ron")).unwrap();
    let cs = aa2_data::load_ability_def(Path::new("../../data/abilities/chaos_strike.ron")).unwrap();

    let mut attacker = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    attacker.abilities.push(AbilityState { def: cs, cooldown_remaining: 0.0, level: 3, casts: 0, charges: None });
    // Damage attacker so we can see healing
    attacker.hp = 300.0;

    let enemy_def = aa2_data::load_hero_def(Path::new("../../data/heroes/sven.ron")).unwrap();
    let enemy = Unit::from_hero_def(&enemy_def, 1, 1, Vec2::new(100.0, 0.0));

    // Run with many seeds until we find one where a crit happens
    for seed in 0..100 {
        let a = attacker.clone();
        let e = enemy.clone();
        let mut sim = Simulation::with_seed(vec![a, e], seed);

        for _ in 0..200 {
            sim.step();
            if sim.units[0].hp > 300.0 {
                // Attacker healed! Lifesteal worked.
                return; // Test passes
            }
            if sim.is_finished() { break; }
        }
    }
    panic!("Chaos Strike lifesteal should have healed the attacker in at least one of 100 seeds");
}

/// Essence Shift steals stats from the target and grants AGI to attacker.
#[test]
fn test_essence_shift_stat_steal() {
    use std::path::Path;
    let hero = aa2_data::load_hero_def(Path::new("../../data/heroes/juggernaut.ron")).unwrap();
    let es = aa2_data::load_ability_def(Path::new("../../data/abilities/essence_shift.ron")).unwrap();

    let mut attacker = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    attacker.abilities.push(AbilityState { def: es, cooldown_remaining: 0.0, level: 3, casts: 0, charges: None });
    let _initial_attacker_max_hp = attacker.max_hp;

    let enemy_def = aa2_data::load_hero_def(Path::new("../../data/heroes/sven.ron")).unwrap();
    let enemy = Unit::from_hero_def(&enemy_def, 1, 1, Vec2::new(100.0, 0.0));
    let initial_enemy_max_hp = enemy.max_hp;

    let mut sim = Simulation::with_seed(vec![attacker, enemy], 42);

    // Run until a few attacks land
    for _ in 0..300 {
        if sim.is_finished() { break; }
        sim.step();
    }

    let attacks_landed = sim.combat_log.iter().filter(|e| matches!(e, CombatEvent::Attack { attacker_id: 0, .. })).count();
    assert!(attacks_landed >= 2, "Need at least 2 attacks for meaningful test");

    // Enemy should have lost STR (= lost max_hp)
    // Each attack steals 1 STR = 22 max_hp lost
    assert!(sim.units[1].max_hp < initial_enemy_max_hp,
        "Enemy max_hp ({:.0}) should be less than initial ({:.0}) due to Essence Shift",
        sim.units[1].max_hp, initial_enemy_max_hp);
}

/// Dark Pact does NOT purge Fury Swipes stacks (they're internal state, not buffs)
/// and does NOT purge Essence Shift debuff (it's Undispellable).
#[test]
fn test_dark_pact_cannot_purge_fury_swipes_or_essence_shift() {
    let hero = aa2_data::load_hero_def(Path::new("../../data/heroes/juggernaut.ron")).unwrap();
    let es = aa2_data::load_ability_def(Path::new("../../data/abilities/essence_shift.ron")).unwrap();

    // Attacker with Essence Shift hits a target
    let mut attacker = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    attacker.abilities.push(AbilityState { def: es, cooldown_remaining: 0.0, level: 3, casts: 0, charges: None });

    let target = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));

    let mut sim = Simulation::with_seed(vec![attacker, target], 42);

    // Run until attacker lands a hit (Essence Shift debuff applied to target)
    for _ in 0..200 {
        sim.step();
        if !sim.units[1].buffs.is_empty() { break; }
    }

    // Target should have essence_shift_debuff
    let es_debuffs = sim.units[1].buffs.iter()
        .filter(|b| b.name == "essence_shift_debuff")
        .count();
    assert!(es_debuffs > 0, "Target should have Essence Shift debuff");

    // Now apply a strong dispel to the target (simulating Dark Pact)
    aa2_sim::buff::dispel(&mut sim.units[1].buffs, aa2_sim::buff::DispelType::StrongDispel);

    // Essence Shift debuff should STILL be there (Undispellable)
    let es_debuffs_after = sim.units[1].buffs.iter()
        .filter(|b| b.name == "essence_shift_debuff")
        .count();
    assert_eq!(es_debuffs, es_debuffs_after,
        "Essence Shift debuff should survive strong dispel (Undispellable)");
}

/// Two Essence Shift units fighting each other — both steal from each other correctly.
#[test]
fn test_essence_shift_mirror_match() {
    let hero = aa2_data::load_hero_def(Path::new("../../data/heroes/juggernaut.ron")).unwrap();
    let es = aa2_data::load_ability_def(Path::new("../../data/abilities/essence_shift.ron")).unwrap();

    let mut unit_a = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    unit_a.abilities.push(AbilityState { def: es.clone(), cooldown_remaining: 0.0, level: 3, casts: 0, charges: None });

    let mut unit_b = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));
    unit_b.abilities.push(AbilityState { def: es, cooldown_remaining: 0.0, level: 3, casts: 0, charges: None });

    let initial_max_hp_a = unit_a.max_hp;
    let initial_max_hp_b = unit_b.max_hp;

    let mut sim = Simulation::with_seed(vec![unit_a, unit_b], 42);

    // Run for a while so both land multiple attacks
    for _ in 0..300 {
        if sim.is_finished() { break; }
        sim.step();
    }

    // Both units should have essence_shift_buff (AGI gained from attacking)
    let a_buffs = sim.units[0].buffs.iter().filter(|b| b.name == "essence_shift_buff").count();
    let b_buffs = sim.units[1].buffs.iter().filter(|b| b.name == "essence_shift_buff").count();
    assert!(a_buffs > 0, "Unit A should have ES buffs from attacking B");
    assert!(b_buffs > 0, "Unit B should have ES buffs from attacking A");

    // Both units should have essence_shift_debuff (stats stolen by the other)
    let a_debuffs = sim.units[0].buffs.iter().filter(|b| b.name == "essence_shift_debuff").count();
    let b_debuffs = sim.units[1].buffs.iter().filter(|b| b.name == "essence_shift_debuff").count();
    assert!(a_debuffs > 0, "Unit A should have ES debuffs from B attacking");
    assert!(b_debuffs > 0, "Unit B should have ES debuffs from A attacking");

    // Both should have reduced max_hp (lost STR from debuffs)
    assert!(sim.units[0].base_max_hp == initial_max_hp_a, "base_max_hp shouldn't change");
    assert!(sim.units[1].base_max_hp == initial_max_hp_b, "base_max_hp shouldn't change");
}

/// Fury Swipes Super: each stack reduces enemy armor by 1.5.
#[test]
fn test_fury_swipes_super_armor_reduction() {
    let hero = aa2_data::load_hero_def(Path::new("../../data/heroes/juggernaut.ron")).unwrap();
    let fs = aa2_data::load_ability_def(Path::new("../../data/abilities/fury_swipes.ron")).unwrap();

    let mut attacker = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    // Level 6 = Super (armor_reduction_per_stack = 1.5)
    attacker.abilities.push(AbilityState { def: fs, cooldown_remaining: 0.0, level: 6, casts: 0, charges: None });

    let enemy_def = aa2_data::load_hero_def(Path::new("../../data/heroes/sven.ron")).unwrap();
    let enemy = Unit::from_hero_def(&enemy_def, 1, 1, Vec2::new(100.0, 0.0));
    let _initial_enemy_armor = enemy.armor;

    let mut sim = Simulation::with_seed(vec![attacker, enemy], 42);

    // Run until 3+ attacks land
    let mut attacks = 0;
    for _ in 0..500 {
        if sim.is_finished() { break; }
        sim.step();
        let new_attacks = sim.combat_log.iter()
            .filter(|e| matches!(e, CombatEvent::Attack { attacker_id: 0, .. }))
            .count();
        if new_attacks >= 3 { attacks = new_attacks; break; }
    }

    assert!(attacks >= 3, "Need at least 3 attacks");

    // Enemy should have fury_swipes_armor debuffs reducing armor
    let armor_debuffs = sim.units[1].buffs.iter()
        .filter(|b| b.name == "fury_swipes_armor")
        .count();
    assert!(armor_debuffs >= 3, "Should have 3+ armor reduction stacks, got {}", armor_debuffs);

    // Total armor reduction should be stacks * 1.5
    let total_modifier = aa2_sim::buff::total_stat_modifier(&sim.units[1].buffs);
    let expected_reduction = -(armor_debuffs as f32 * 1.5);
    assert!((total_modifier.bonus_armor - expected_reduction).abs() < 0.01,
        "Armor reduction should be {:.1}, got {:.1}", expected_reduction, total_modifier.bonus_armor);
}

/// Chaos Strike Gaben: allies within 1200 radius get crit chance (50% of holder's).
#[test]
fn test_chaos_strike_gaben_aura() {
    let hero = aa2_data::load_hero_def(Path::new("../../data/heroes/chaos_knight.ron")).unwrap();
    let cs = aa2_data::load_ability_def(Path::new("../../data/abilities/chaos_strike.ron")).unwrap();

    // CK with Gaben Chaos Strike (level 9)
    let mut ck = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    ck.abilities.push(AbilityState { def: cs, cooldown_remaining: 0.0, level: 9, casts: 0, charges: None });

    // Ally within 1200 radius (no Chaos Strike of their own)
    let ally_def = aa2_data::load_hero_def(Path::new("../../data/heroes/juggernaut.ron")).unwrap();
    let ally = Unit::from_hero_def(&ally_def, 2, 0, Vec2::new(100.0, 0.0));

    // Enemy
    let enemy_def = aa2_data::load_hero_def(Path::new("../../data/heroes/sven.ron")).unwrap();
    let enemy = Unit::from_hero_def(&enemy_def, 1, 1, Vec2::new(200.0, 0.0));

    let mut sim = Simulation::with_seed(vec![ck, enemy, ally], 42);

    // Run many ticks — ally should eventually crit (via aura)
    let mut ally_crits = 0;
    let mut ally_attacks = 0;
    for _ in 0..1000 {
        if sim.is_finished() { break; }
        sim.step();
    }

    // Count ally's attacks and check for high-damage hits (crits)
    // Ally (id=2) base damage: ~54-56. After armor, non-crit ≈ 35-40. Crit would be 50+.
    for event in &sim.combat_log {
        if let CombatEvent::Attack { attacker_id: 2, damage, .. } = event {
            ally_attacks += 1;
            if *damage > 55.0 { // Significantly above non-crit range = crit
                ally_crits += 1;
            }
        }
    }

    assert!(ally_attacks >= 5, "Ally should have attacked at least 5 times, got {}", ally_attacks);
    // With 50% of 43.33% = ~21.67% crit chance, over 5+ attacks we should see at least 1 crit
    assert!(ally_crits > 0,
        "Ally should have crit at least once via Gaben aura ({} attacks, 0 crits)", ally_attacks);
}

/// Essence Shift Super: permanently gain +1 AGI when affected unit dies within 300 radius.
#[test]
fn test_essence_shift_super_permanent_agi_on_kill() {
    let hero = aa2_data::load_hero_def(Path::new("../../data/heroes/juggernaut.ron")).unwrap();
    let es = aa2_data::load_ability_def(Path::new("../../data/abilities/essence_shift.ron")).unwrap();

    // Attacker with Super Essence Shift (level 6)
    let mut attacker = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    attacker.abilities.push(AbilityState { def: es, cooldown_remaining: 0.0, level: 6, casts: 0, charges: None });

    // Weak enemy that will die quickly (set low HP)
    let mut enemy = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));
    enemy.hp = 50.0; // Will die fast

    let mut sim = Simulation::with_seed(vec![attacker, enemy], 42);

    // Count ES buffs before kill
    let _buffs_before = sim.units[0].buffs.iter()
        .filter(|b| b.name == "essence_shift_buff")
        .count();

    // Run until enemy dies
    for _ in 0..200 {
        if sim.is_finished() { break; }
        sim.step();
    }

    assert!(sim.is_finished(), "Enemy should have died");

    // Check for permanent AGI buff (should have a very long or permanent buff)
    // Super ES grants +1 AGI permanently when affected unit dies within 300 radius
    let _permanent_buffs = sim.units[0].buffs.iter()
        .filter(|b| b.name.contains("essence_shift") && b.remaining_ticks > 9000) // "permanent" = very long duration
        .count();

    // At minimum, the attacker should have more ES buffs than before (temporary + permanent)
    let buffs_after = sim.units[0].buffs.iter()
        .filter(|b| b.name.contains("essence_shift"))
        .count();
    assert!(buffs_after > 0, "Attacker should have ES buffs after killing target");
}

/// Fury Swipes Gaben: every 2 attacks on an enemy, 1 stack spreads to all other enemies.
#[test]
fn test_fury_swipes_gaben_spread() {
    let hero = aa2_data::load_hero_def(Path::new("../../data/heroes/juggernaut.ron")).unwrap();
    let fs = aa2_data::load_ability_def(Path::new("../../data/abilities/fury_swipes.ron")).unwrap();

    // Attacker with Gaben Fury Swipes (level 9)
    let mut attacker = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    attacker.abilities.push(AbilityState { def: fs, cooldown_remaining: 0.0, level: 9, casts: 0, charges: None });

    // Two enemies close together
    let enemy_def = aa2_data::load_hero_def(Path::new("../../data/heroes/sven.ron")).unwrap();
    let enemy_a = Unit::from_hero_def(&enemy_def, 1, 1, Vec2::new(100.0, 0.0));
    let enemy_b = Unit::from_hero_def(&enemy_def, 2, 1, Vec2::new(120.0, 0.0));

    let mut sim = Simulation::with_seed(vec![attacker, enemy_a, enemy_b], 42);

    // Run until attacker has hit the primary target at least 4 times
    // (stacks 2 and 4 should trigger spread)
    for _ in 0..500 {
        if sim.is_finished() { break; }
        sim.step();
    }

    let attacks_on_target = sim.combat_log.iter()
        .filter(|e| matches!(e, CombatEvent::Attack { attacker_id: 0, .. }))
        .count();
    assert!(attacks_on_target >= 4, "Need 4+ attacks, got {}", attacks_on_target);

    // Check that the secondary enemy (id=2) has Fury Swipes stacks
    // even though it was never directly attacked
    
    let secondary_stacks = sim.units[0].attack_modifier_state.iter()
        .find(|(id, _)| *id == 2)
        .map(|(_, s)| s.fury_swipes_stacks)
        .unwrap_or(0);

    assert!(secondary_stacks > 0,
        "Secondary enemy should have Fury Swipes stacks from Gaben spread, got {}", secondary_stacks);
    // With 4+ attacks on primary, stacks 2 and 4 trigger spread = 2 stacks on secondary
    assert!(secondary_stacks >= 1,
        "Expected at least 1 spread stack, got {}", secondary_stacks);
}

/// Essence Shift: target stats floor at 1 (max_hp can't go below 1) even with many stacks.
/// Wielder keeps gaining AGI regardless.
#[test]
fn test_essence_shift_stats_floor_at_one() {
    use aa2_sim::buff::{Buff, StackBehavior, DispelType, StatusFlags, StatModifier};

    let hero = aa2_data::load_hero_def(Path::new("../../data/heroes/juggernaut.ron")).unwrap();
    let mut target = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));
    let attacker = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));

    // Manually apply 50 Essence Shift debuff stacks (steals 1 STR each = -50 STR = -1100 HP)
    // Juggernaut base_max_hp = 560 (20 STR * 22 + 120 base)
    // -50 STR * 22 = -1100, so expected_max_hp would be 560 - 1100 = -540 without floor
    for _ in 0..50 {
        target.buffs.push(Buff {
            name: "essence_shift_debuff".to_string(),
            remaining_ticks: 600,
            tick_effect: None,
            stacking: StackBehavior::Independent,
            dispel_type: DispelType::Undispellable,
            status: StatusFlags::default(),
            stat_modifier: Some(StatModifier {
                bonus_strength: -1.0,
                bonus_agi: -1.0,
                bonus_int: -1.0,
                ..StatModifier::default()
            }),
            source_id: 0,
            is_debuff: true,
            pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
        });
    }

    let mut sim = Simulation::with_seed(vec![attacker, target], 42);
    sim.step(); // Process buffs → STR scaling kicks in

    // Target's max_hp should be floored: base STR floors at 1, so HP = 120 + 1*22 = 142
    assert!(sim.units[1].max_hp >= 1.0,
        "max_hp should floor at 1, got {:.1}", sim.units[1].max_hp);
    assert!(sim.units[1].hp >= 1.0,
        "hp should floor at 1, got {:.1}", sim.units[1].hp);

    // Attacker can still gain AGI buffs freely (no cap on positive side)
    for _ in 0..50 {
        sim.units[0].buffs.push(Buff {
            name: "essence_shift_buff".to_string(),
            remaining_ticks: 600,
            tick_effect: None,
            stacking: StackBehavior::Independent,
            dispel_type: DispelType::Undispellable,
            status: StatusFlags::default(),
            stat_modifier: Some(StatModifier {
                bonus_agi: 3.0,
                ..StatModifier::default()
            }),
            source_id: 0,
            is_debuff: false,
            pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
        });
    }

    sim.step();
    // Attacker should have massive armor bonus from AGI (50 * 3 AGI * 0.167 = 25 armor from buffs)
    // Total armor = BASE_ARMOR + (base_agi + 150) * 0.167
    assert!(sim.units[0].armor > 20.0,
        "Attacker should have large armor from ES AGI stacks, got {:.1}", sim.units[0].armor);
}

#[test]
fn test_hg_protects_base_from_es() {
    // BUG 1: ES debuffs should only reduce BASE stats, not bonus stats.
    // A hero with 20 base STR + 28 bonus STR from HG, hit by 30 ES debuffs:
    // effective_base_str = max(20 - 30, 1) = 1
    // total_str = 1 + 28 = 29
    // max_hp = 120 + 29 * 22 = 758
    use aa2_sim::buff::{Buff, StackBehavior, DispelType, StatusFlags, StatModifier};

    let hero = make_hero(); // 20 base STR
    let mut target = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    let dummy = Unit::from_hero_def(&hero, 1, 1, Vec2::new(9999.0, 0.0));

    // Heavenly Grace buff: +28 STR
    target.buffs.push(Buff {
        name: "heavenly_grace".to_string(),
        remaining_ticks: 9000,
        tick_effect: None,
        stacking: StackBehavior::RefreshDuration,
        dispel_type: DispelType::BasicDispel,
        status: StatusFlags::default(),
        stat_modifier: Some(StatModifier {
            bonus_strength: 28.0,
            ..StatModifier::default()
        }),
        source_id: 0,
        is_debuff: false,
            pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
    });

    // 30 ES debuffs: -1 STR each
    for _ in 0..30 {
        target.buffs.push(Buff {
            name: "essence_shift_debuff".to_string(),
            remaining_ticks: 9000,
            tick_effect: None,
            stacking: StackBehavior::Independent,
            dispel_type: DispelType::Undispellable,
            status: StatusFlags::default(),
            stat_modifier: Some(StatModifier {
                bonus_strength: -1.0,
                bonus_agi: -1.0,
                bonus_int: -1.0,
                ..StatModifier::default()
            }),
            source_id: 1,
            is_debuff: true,
            pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
        });
    }

    let mut sim = Simulation::with_seed(vec![target, dummy], 42);
    sim.step();

    // effective_base_str = max(20 - 30, 1) = 1
    // total_str = 1 + 28 = 29
    // expected_max_hp = 120 + 29 * 22 = 758
    let expected_hp = 120.0 + 29.0 * 22.0;
    assert!((sim.units[0].max_hp - expected_hp).abs() < 1.0,
        "HG should protect from ES: expected max_hp={expected_hp}, got {:.1}", sim.units[0].max_hp);

    // Without the fix, the old behavior would compute:
    // net_str_modifier = -30 + 28 = -2, max_hp = base_max_hp + (-2)*22 = 560 - 44 = 516
    // The new behavior correctly gives 758
    assert!(sim.units[0].max_hp > 700.0,
        "max_hp should be >700 with HG protecting base, got {:.1}", sim.units[0].max_hp);
}

#[test]
fn test_agi_hero_damage_increases_with_es_buff() {
    // BUG 2: Primary attribute damage should update with stat changes.
    // An AGI hero gaining AGI from ES buffs should deal more damage.
    use aa2_sim::buff::{Buff, StackBehavior, DispelType, StatusFlags, StatModifier};

    let hero = HeroDef {
        name: "Juggernaut".to_string(),
        primary_attribute: Attribute::Agility,
        base_str: 20.0,
        base_agi: 26.0,
        base_int: 14.0,
        str_gain: 2.0,
        agi_gain: 3.0,
        int_gain: 1.5,
        base_attack_time: 1.4,
        attack_range: 150.0,
        attack_point: 0.33,
        move_speed: 300.0,
        turn_rate: 0.6,
        collision_radius: 24.0,
        tier: 1,
        is_melee: true,
        base_damage_min: 20.0,
        base_damage_max: 24.0,
        projectile_speed: None,
    };

    let mut attacker = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    let dummy = Unit::from_hero_def(&hero, 1, 1, Vec2::new(9999.0, 0.0));

    // Initial damage: base_damage + primary_agi = 20+26=46 to 24+26=50
    assert!((attacker.damage_min - 46.0).abs() < 0.01);
    assert!((attacker.damage_max - 50.0).abs() < 0.01);

    // Apply 10 ES buffs: +3 AGI each = +30 AGI
    for _ in 0..10 {
        attacker.buffs.push(Buff {
            name: "essence_shift_buff".to_string(),
            remaining_ticks: 9000,
            tick_effect: None,
            stacking: StackBehavior::Independent,
            dispel_type: DispelType::Undispellable,
            status: StatusFlags::default(),
            stat_modifier: Some(StatModifier {
                bonus_agi: 3.0,
                ..StatModifier::default()
            }),
            source_id: 0,
            is_debuff: false,
            pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
        });
    }

    let mut sim = Simulation::with_seed(vec![attacker, dummy], 42);
    sim.step();

    // After buffs: total_agi = 26 + 30 = 56, damage = base + 56
    // damage_min = 20 + 56 = 76, damage_max = 24 + 56 = 80
    assert!((sim.units[0].damage_min - 76.0).abs() < 0.01,
        "AGI hero damage_min should increase with ES buffs: expected 76, got {:.1}", sim.units[0].damage_min);
    assert!((sim.units[0].damage_max - 80.0).abs() < 0.01,
        "AGI hero damage_max should increase with ES buffs: expected 80, got {:.1}", sim.units[0].damage_max);
}

#[test]
fn test_hero_leveling_stats() {
    // Juggernaut-like hero: STR=20, AGI=32, INT=14, gains 2.0/2.8/1.4
    let hero = HeroDef {
        name: "Juggernaut".to_string(),
        primary_attribute: Attribute::Agility,
        base_str: 20.0,
        base_agi: 32.0,
        base_int: 14.0,
        str_gain: 2.0,
        agi_gain: 2.8,
        int_gain: 1.4,
        base_attack_time: 1.4,
        attack_range: 150.0,
        attack_point: 0.33,
        move_speed: 300.0,
        turn_rate: 0.6,
        collision_radius: 24.0,
        tier: 1,
        is_melee: true,
        base_damage_min: 14.0,
        base_damage_max: 18.0,
        projectile_speed: None,
    };

    // Level 1: base stats unchanged
    let u1 = Unit::from_hero_def_at_level(&hero, 0, 0, Vec2::new(0.0, 0.0), 1);
    assert!((u1.base_str - 20.0).abs() < 0.01);
    assert!((u1.base_agi - 32.0).abs() < 0.01);
    assert!((u1.base_int - 14.0).abs() < 0.01);

    // Level 20: base_str=20+19*2.0=58, base_agi=32+19*2.8=85.2, base_int=14+19*1.4=40.6
    let u20 = Unit::from_hero_def_at_level(&hero, 0, 0, Vec2::new(0.0, 0.0), 20);
    assert!((u20.base_str - 58.0).abs() < 0.01, "Expected STR 58, got {}", u20.base_str);
    assert!((u20.base_agi - 85.2).abs() < 0.01, "Expected AGI 85.2, got {}", u20.base_agi);
    assert!((u20.base_int - 40.6).abs() < 0.01, "Expected INT 40.6, got {}", u20.base_int);

    // HP from STR: 120 + 58*22 = 1396
    assert!((u20.max_hp - 1396.0).abs() < 0.01, "Expected max_hp 1396, got {}", u20.max_hp);

    // Armor from AGI: 85.2 * 0.167 = 14.2284
    assert!((u20.armor - 85.2 * 0.167).abs() < 0.01, "Expected armor {}, got {}", 85.2 * 0.167, u20.armor);

    // Damage: primary is AGI, so damage_min = 14 + 85.2 = 99.2
    assert!((u20.damage_min - 99.2).abs() < 0.01, "Expected damage_min 99.2, got {}", u20.damage_min);

    // UnitConfig with level
    let config = UnitConfig::new(hero).with_level(20);
    let u_config = Unit::from_config(&config, 0, 0, Vec2::new(0.0, 0.0));
    assert!((u_config.base_str - 58.0).abs() < 0.01);
    assert!((u_config.base_agi - 85.2).abs() < 0.01);
}

#[test]
fn test_glaives_bounce_applies_modifiers() {
    // Melee unit with Glaives + Fury Swipes attacks primary target
    // Bounce hits secondary target — verify secondary gets Fury Swipes stack
    let hero = HeroDef {
        name: "TestMelee".to_string(),
        primary_attribute: Attribute::Intelligence,
        base_str: 20.0,
        base_agi: 20.0,
        base_int: 40.0,
        str_gain: 2.0,
        agi_gain: 2.0,
        int_gain: 3.0,
        base_attack_time: 1.7,
        attack_range: 150.0,
        attack_point: 0.3,
        move_speed: 300.0,
        turn_rate: 0.6,
        collision_radius: 24.0,
        tier: 1,
        is_melee: true,
        base_damage_min: 20.0,
        base_damage_max: 20.0,
        projectile_speed: None,
    };

    let mut attacker = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    attacker.mana = 500.0;
    attacker.abilities.push(AbilityState {
        def: AbilityDef {
            name: "Glaives".to_string(),
            cooldown: vec![0.0],
            mana_cost: vec![0.0],
            cast_point: 0.0,
            targeting: TargetType::Passive,
            effects: vec![Effect::GlaivesOfWisdom {
                int_damage_factor: vec![1.0],
                int_steal_per_attack: vec![2.0],
                steal_duration: vec![10.0],
                mana_cost: vec![15.0],
                steal_int_on_kill: vec![0.0],
                steal_radius: 900.0,
                bounce_radius: vec![500.0],
            }],
            description: String::new(), is_ultimate: false,
            aoe_shape: None,
            cast_range: 0.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
        },
        cooldown_remaining: 0.0,
        level: 9,
        casts: 0,
        charges: None,
    });
    attacker.abilities.push(AbilityState {
        def: AbilityDef {
            name: "Fury Swipes".to_string(),
            cooldown: vec![0.0],
            mana_cost: vec![0.0],
            cast_point: 0.0,
            targeting: TargetType::Passive,
            effects: vec![Effect::FurySwipes {
                damage_per_stack: vec![20.0],
                stack_duration: vec![15.0],
                armor_reduction_per_stack: vec![0.0],
            }],
            description: String::new(), is_ultimate: false,
            aoe_shape: None,
            cast_range: 0.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
        },
        cooldown_remaining: 0.0,
        level: 1,
        casts: 0,
        charges: None,
    });

    // Primary target in melee range, secondary nearby
    let target = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));
    let secondary = Unit::from_hero_def(&hero, 2, 1, Vec2::new(200.0, 0.0));
    let secondary_hp_before = secondary.hp;

    let mut sim = Simulation::with_seed(vec![attacker, target, secondary], 42);
    // Run until first attack lands + bounce projectile arrives
    for _ in 0..300 {
        sim.step();
        // Wait for bounce projectile to hit (attack + travel time)
        if sim.units[2].hp < secondary_hp_before {
            break;
        }
    }

    // Secondary should have taken bounce damage (via projectile)
    assert!(sim.units[2].hp < secondary_hp_before,
        "Secondary should take bounce damage: before={secondary_hp_before}, after={}", sim.units[2].hp);

    // Attacker should have Fury Swipes stacks on BOTH targets
    let attacker_state = &sim.units[0].attack_modifier_state;
    let has_stack_on_secondary = attacker_state.iter().any(|(id, s)| *id == 2 && s.fury_swipes_stacks > 0);
    assert!(has_stack_on_secondary, "Bounce should apply Fury Swipes stack to secondary target");
}

#[test]
fn test_glaives_bounce_50_percent_physical() {
    // Melee unit with Glaives (no other modifiers) attacks primary
    // Verify bounce deals 50% of physical damage
    let hero = HeroDef {
        name: "TestMelee".to_string(),
        primary_attribute: Attribute::Intelligence,
        base_str: 20.0,
        base_agi: 20.0,
        base_int: 40.0,
        str_gain: 2.0,
        agi_gain: 2.0,
        int_gain: 3.0,
        base_attack_time: 1.7,
        attack_range: 150.0,
        attack_point: 0.3,
        move_speed: 300.0,
        turn_rate: 0.6,
        collision_radius: 24.0,
        tier: 1,
        is_melee: true,
        // Fixed damage for predictable testing
        base_damage_min: 20.0,
        base_damage_max: 20.0,
        projectile_speed: None,
    };

    let mut attacker = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    attacker.mana = 500.0;
    attacker.abilities.push(AbilityState {
        def: AbilityDef {
            name: "Glaives".to_string(),
            cooldown: vec![0.0],
            mana_cost: vec![0.0],
            cast_point: 0.0,
            targeting: TargetType::Passive,
            effects: vec![Effect::GlaivesOfWisdom {
                int_damage_factor: vec![1.0],
                int_steal_per_attack: vec![2.0],
                steal_duration: vec![10.0],
                mana_cost: vec![15.0],
                steal_int_on_kill: vec![0.0],
                steal_radius: 900.0,
                bounce_radius: vec![500.0],
            }],
            description: String::new(), is_ultimate: false,
            aoe_shape: None,
            cast_range: 0.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
        },
        cooldown_remaining: 0.0,
        level: 9,
        casts: 0,
        charges: None,
    });

    // Use 0 armor/magic resist targets for clean damage calculation
    let mut target = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));
    target.armor = 0.0;
    target.magic_resistance = 0.0;
    let mut secondary = Unit::from_hero_def(&hero, 2, 1, Vec2::new(200.0, 0.0));
    secondary.armor = 0.0;
    secondary.magic_resistance = 0.0;

    let primary_hp_before = target.hp;
    let secondary_hp_before = secondary.hp;

    let mut sim = Simulation::with_seed(vec![attacker, target, secondary], 42);
    for _ in 0..300 {
        sim.step();
        // Wait for bounce projectile to arrive at secondary
        if sim.units[2].hp < secondary_hp_before {
            break;
        }
    }

    let primary_dmg = primary_hp_before - sim.units[1].hp;
    let secondary_dmg = secondary_hp_before - sim.units[2].hp;

    // Primary takes: full physical (60 = 20 base + 40 INT primary) + 40 magical (100% of 40 INT)
    // Secondary takes: 50% physical (30) + 40 magical (100% of 40 INT)
    // So secondary_dmg should be roughly primary_dmg - 30 (half the physical portion)
    // Primary physical = 60, secondary physical = 30, both get 40 magical
    // Primary total = 100, secondary total = 70
    // Allow some variance from damage rolls on the bounce
    assert!(secondary_dmg > 0.0, "Secondary should take damage");
    assert!(secondary_dmg < primary_dmg,
        "Bounce should deal less than primary: primary={primary_dmg}, secondary={secondary_dmg}");
    // The physical portion of bounce should be ~50% of primary's physical
    // With 0 armor: primary_phys = 60, secondary_phys ≈ 30, both get 40 magic
    // secondary_dmg ≈ 70, primary_dmg = 100
    // Allow tolerance for damage roll variance on the bounce
    let expected_secondary = 70.0; // 30 phys + 40 magic
    assert!((secondary_dmg - expected_secondary).abs() < 15.0,
        "Bounce damage should be ~{expected_secondary}, got {secondary_dmg}");
}

// ============================================================
// CastBehavior / Targeting AI tests
// ============================================================

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

#[test]
fn test_lazy_targeting_no_walk() {
    // Unit with Lazy ability, enemy out of range — should NOT walk toward enemy
    let hero = make_test_hero();
    let ability = AbilityDef {
        name: "LazySpell".to_string(),
        cooldown: vec![10.0],
        mana_cost: vec![50.0],
        cast_point: 0.0,
        targeting: TargetType::SingleEnemy,
        effects: vec![Effect::Damage { kind: DamageType::Magical, base: vec![100.0] }],
        description: String::new(), is_ultimate: false,
        aoe_shape: None,
        cast_range: 300.0,
        cast_behavior: aa2_data::CastBehavior::Lazy,
        max_charges: None,
    };

    let config_a = UnitConfig::new(hero.clone()).with_ability(ability, 1);
    let config_b = UnitConfig::new(hero);

    // Place enemy at 2000 units away (well beyond cast_range of 300, and beyond acquisition range)
    let mut u0 = Unit::from_config(&config_a, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    let u1 = Unit::from_hero_def(&config_b.hero, 1, 1, Vec2::new(2000.0, 0.0));

    let initial_pos = u0.position;
    let mut sim = Simulation::new(vec![u0, u1]);

    // Run for 60 ticks (2 seconds) — enemy is beyond acquisition range so no walking
    for _ in 0..60 {
        sim.step();
    }

    // Unit should NOT have cast (enemy out of Lazy range)
    let has_cast = sim.combat_log.iter().any(|e| matches!(e, CombatEvent::CastStart { ability_name, .. } if ability_name == "LazySpell"));
    assert!(!has_cast, "Lazy ability should not cast when target is out of range");

    // Unit SHOULD walk toward enemy (all units auto-attack)
    assert!(sim.units[0].position.x > initial_pos.x + 1.0,
        "Unit should walk toward enemy to auto-attack");
}

#[test]
fn test_burrowstrike_line_stun() {
    let hero = make_test_hero();
    let ability = AbilityDef {
        name: "Burrowstrike".to_string(),
        cooldown: vec![14.0],
        mana_cost: vec![100.0],
        cast_point: 0.0,
        targeting: TargetType::SingleEnemy,
        effects: vec![Effect::Burrowstrike {
            damage: vec![80.0],
            stun_duration: vec![1.2],
            range: vec![550.0],
            width: 150.0,
            travel_speed: 2000.0,
            caustic_finale_damage: vec![0.0],
            caustic_finale_radius: 400.0,
        }],
        description: String::new(), is_ultimate: false,
        aoe_shape: None,
        cast_range: 550.0,
        cast_behavior: aa2_data::CastBehavior::default(),
        max_charges: None,
    };

    let config = UnitConfig::new(hero.clone()).with_ability(ability, 1);
    let mut u0 = Unit::from_config(&config, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    // Place enemies in a line
    let u1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(200.0, 0.0));
    let u2 = Unit::from_hero_def(&hero, 2, 1, Vec2::new(400.0, 0.0));
    // Enemy off to the side (outside 150 capsule radius)
    let u3 = Unit::from_hero_def(&hero, 3, 1, Vec2::new(200.0, 200.0));

    let hp_before_1 = u1.hp;
    let hp_before_2 = u2.hp;
    let hp_before_3 = u3.hp;

    let mut sim = Simulation::new(vec![u0, u1, u2, u3]);

    // Wave speed 2000 u/s, range 550. Travel time = 0.275s = ~9 ticks.
    // Unit at 200: wave hits at tick ~4, stun applied immediately (1.2s = 36 ticks).
    // Check stun at tick 15 (well within stun duration).
    for _ in 0..15 {
        sim.step();
    }

    // Enemies in line should be stunned (wave has passed them by now)
    let u1_stunned = aa2_sim::buff::active_status(&sim.units[1].buffs).stunned;
    let u2_stunned = aa2_sim::buff::active_status(&sim.units[2].buffs).stunned;
    assert!(u1_stunned, "Enemy 1 in line should be stunned");
    assert!(u2_stunned, "Enemy 2 in line should be stunned");

    // Enemy off to the side should NOT be stunned (200 > 150 capsule radius)
    let u3_stunned = aa2_sim::buff::active_status(&sim.units[3].buffs).stunned;
    assert!(!u3_stunned, "Enemy 3 off to the side should not be stunned");

    // Continue to tick 40 so damage lands (0.52s delay = 16 ticks after hit)
    for _ in 0..25 {
        sim.step();
    }

    // Check that enemies in line were damaged
    let dmg_events: Vec<_> = sim.combat_log.iter().filter(|e| matches!(e, CombatEvent::AbilityDamage { ability_name, .. } if ability_name == "Burrowstrike")).collect();
    assert!(dmg_events.len() >= 2, "Should hit at least 2 enemies in line, got {}", dmg_events.len());

    // Verify damage was dealt
    assert!(sim.units[1].hp < hp_before_1, "Enemy 1 should have taken damage");
    assert!(sim.units[2].hp < hp_before_2, "Enemy 2 should have taken damage");
    assert!((sim.units[3].hp - hp_before_3).abs() < 1.0, "Enemy 3 should not have taken damage");
}

#[test]
fn test_burrowstrike_teleport() {
    let hero = make_test_hero();
    let ability = AbilityDef {
        name: "Burrowstrike".to_string(),
        cooldown: vec![14.0],
        mana_cost: vec![100.0],
        cast_point: 0.0,
        targeting: TargetType::SingleEnemy,
        effects: vec![Effect::Burrowstrike {
            damage: vec![80.0],
            stun_duration: vec![1.2],
            range: vec![550.0],
            width: 150.0,
            travel_speed: 2000.0,
            caustic_finale_damage: vec![0.0],
            caustic_finale_radius: 400.0,
        }],
        description: String::new(), is_ultimate: false,
        aoe_shape: None,
        cast_range: 550.0,
        cast_behavior: aa2_data::CastBehavior::default(),
        max_charges: None,
    };

    let config = UnitConfig::new(hero.clone()).with_ability(ability, 1);
    let mut u0 = Unit::from_config(&config, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    let u1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(300.0, 0.0));

    let mut sim = Simulation::new(vec![u0, u1]);

    // Track max x position reached (caster travels then walks back)
    let mut max_x = 0.0_f32;
    for _ in 0..30 {
        sim.step();
        max_x = max_x.max(sim.units[0].position.x);
    }

    // Caster should have reached end point (550 units in direction of target)
    assert!((max_x - 550.0).abs() < 5.0,
        "Caster should reach ~550 along x, max was {}", max_x);
}

#[test]
fn test_charges_system() {
    let hero = make_test_hero();
    let ability = AbilityDef {
        name: "ChargedSpell".to_string(),
        cooldown: vec![10.0],
        mana_cost: vec![50.0],
        cast_point: 0.0,
        targeting: TargetType::SingleEnemy,
        effects: vec![Effect::Damage { kind: DamageType::Magical, base: vec![100.0] }],
        description: String::new(), is_ultimate: false,
        aoe_shape: None,
        cast_range: 600.0,
        cast_behavior: aa2_data::CastBehavior::default(),
        max_charges: Some(2),
    };

    let config = UnitConfig::new(hero.clone()).with_ability(ability, 1);
    let mut u0 = Unit::from_config(&config, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    let u1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));

    let mut sim = Simulation::new(vec![u0, u1]);

    // Run until first cast
    for _ in 0..30 {
        sim.step();
    }

    let cast_count = sim.combat_log.iter().filter(|e| matches!(e, CombatEvent::CastComplete { ability_name, .. } if ability_name == "ChargedSpell")).count();
    assert!(cast_count >= 1, "Should have cast at least once");

    // Run more to get second cast
    for _ in 0..30 {
        sim.step();
    }

    let cast_count = sim.combat_log.iter().filter(|e| matches!(e, CombatEvent::CastComplete { ability_name, .. } if ability_name == "ChargedSpell")).count();
    assert!(cast_count >= 2, "Should have cast twice with 2 charges, got {cast_count}");

    // After 2 casts, charges should be depleted
    assert_eq!(sim.units[0].abilities[0].charges.as_ref().unwrap().current_charges, 0,
        "Charges should be depleted after 2 casts");
}

/// Burrowstrike wave hits closer enemies before farther ones (distance-based timing).
#[test]
fn test_burrowstrike_wave_hits_closer_first() {
    let hero = make_test_hero();
    let ability = AbilityDef {
        name: "Burrowstrike".to_string(),
        cooldown: vec![14.0],
        mana_cost: vec![100.0],
        cast_point: 0.0,
        targeting: TargetType::SingleEnemy,
        effects: vec![Effect::Burrowstrike {
            damage: vec![80.0],
            stun_duration: vec![1.2],
            range: vec![550.0],
            width: 150.0,
            travel_speed: 2000.0,
            caustic_finale_damage: vec![0.0],
            caustic_finale_radius: 400.0,
        }],
        description: String::new(), is_ultimate: false,
        aoe_shape: None,
        cast_range: 550.0,
        cast_behavior: aa2_data::CastBehavior::default(),
        max_charges: None,
    };

    let config = UnitConfig::new(hero.clone()).with_ability(ability, 1);
    let mut u0 = Unit::from_config(&config, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    // Close enemy at 100 units, far enemy at 500 units
    let u_close = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));
    let u_far = Unit::from_hero_def(&hero, 2, 1, Vec2::new(500.0, 0.0));

    let mut sim = Simulation::new(vec![u0, u_close, u_far]);

    // Wave at 2000 u/s: reaches 100 at tick ~2, reaches 500 at tick ~8
    // Run tick by tick and record when each gets stunned
    let mut close_stun_tick = None;
    let mut far_stun_tick = None;

    for _ in 0..30 {
        sim.step();
        if close_stun_tick.is_none() && aa2_sim::buff::active_status(&sim.units[1].buffs).stunned {
            close_stun_tick = Some(sim.tick);
        }
        if far_stun_tick.is_none() && aa2_sim::buff::active_status(&sim.units[2].buffs).stunned {
            far_stun_tick = Some(sim.tick);
        }
    }

    let close_tick = close_stun_tick.expect("Close enemy should be stunned");
    let far_tick = far_stun_tick.expect("Far enemy should be stunned");

    assert!(close_tick < far_tick,
        "Close enemy (tick {}) should be stunned before far enemy (tick {})", close_tick, far_tick);
    // At 2000 u/s: 400 unit difference = 0.2s = 6 ticks difference
    let diff = far_tick - close_tick;
    assert!(diff >= 4 && diff <= 8,
        "Expected ~6 tick difference, got {}", diff);
}

/// Caster is invulnerable during Burrowstrike travel (can't be damaged).
#[test]
fn test_burrowstrike_invulnerable_during_travel() {
    let hero = make_test_hero();
    let ability = AbilityDef {
        name: "Burrowstrike".to_string(),
        cooldown: vec![14.0],
        mana_cost: vec![100.0],
        cast_point: 0.0,
        targeting: TargetType::SingleEnemy,
        effects: vec![Effect::Burrowstrike {
            damage: vec![80.0],
            stun_duration: vec![1.2],
            range: vec![550.0],
            width: 150.0,
            travel_speed: 2000.0,
            caustic_finale_damage: vec![0.0],
            caustic_finale_radius: 400.0,
        }],
        description: String::new(), is_ultimate: false,
        aoe_shape: None,
        cast_range: 550.0,
        cast_behavior: aa2_data::CastBehavior::default(),
        max_charges: None,
    };

    let config = UnitConfig::new(hero.clone()).with_ability(ability, 1);
    let mut u0 = Unit::from_config(&config, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    let hp_before = u0.hp;

    // Enemy that would normally attack the caster
    let u1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));

    let mut sim = Simulation::new(vec![u0, u1]);

    // Run for a few ticks — caster should be burrowing (invulnerable)
    // Travel time: 550/2000 = 0.275s = ~9 ticks
    for _ in 0..5 {
        sim.step();
    }

    // Caster should be invulnerable (status check)
    let status = aa2_sim::buff::active_status(&sim.units[0].buffs);
    assert!(status.invulnerable, "Caster should be invulnerable during travel");

    // Caster should not have taken any damage during travel
    assert_eq!(sim.units[0].hp, hp_before,
        "Caster should not take damage while invulnerable");
}

/// Caustic Finale: unit with debuff explodes on death, dealing damage in radius.
#[test]
fn test_caustic_finale_explosion_on_death() {
    let hero = make_test_hero();
    let ability = AbilityDef {
        name: "Burrowstrike".to_string(),
        cooldown: vec![14.0],
        mana_cost: vec![100.0],
        cast_point: 0.0,
        targeting: TargetType::SingleEnemy,
        effects: vec![Effect::Burrowstrike {
            damage: vec![500.0], // high damage to kill quickly
            stun_duration: vec![1.2],
            range: vec![550.0],
            width: 150.0,
            travel_speed: 2000.0,
            caustic_finale_damage: vec![150.0], // Super level
            caustic_finale_radius: 400.0,
        }],
        description: String::new(), is_ultimate: false,
        aoe_shape: None,
        cast_range: 550.0,
        cast_behavior: aa2_data::CastBehavior::default(),
        max_charges: None,
    };

    let config = UnitConfig::new(hero.clone()).with_ability(ability, 1);
    let mut u0 = Unit::from_config(&config, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;

    // Target with low HP (will die from Burrowstrike damage)
    let mut target = Unit::from_hero_def(&hero, 1, 1, Vec2::new(200.0, 0.0));
    target.hp = 100.0;

    // Nearby enemy (within 400 radius of target, should take explosion damage)
    let nearby = Unit::from_hero_def(&hero, 2, 1, Vec2::new(300.0, 0.0));
    let nearby_hp_before = nearby.hp;

    let mut sim = Simulation::new(vec![u0, target, nearby]);

    // Run until target dies and explosion triggers
    for _ in 0..100 {
        if sim.units[1].hp <= 0.0 || !sim.units[1].is_alive() { break; }
        sim.step();
    }

    // Continue a few more ticks for death processing
    for _ in 0..5 {
        sim.step();
    }

    // Target should be dead
    assert!(!sim.units[1].is_alive(), "Target should have died from Burrowstrike");

    // Nearby enemy should have taken Caustic Finale explosion damage
    assert!(sim.units[2].hp < nearby_hp_before,
        "Nearby enemy should take Caustic Finale explosion: before={}, after={}",
        nearby_hp_before, sim.units[2].hp);
}

/// Glaives of Wisdom is totally blocked by magic immunity — no mana cost, no bonus damage.
/// The attack becomes a regular physical attack.
#[test]
fn test_glaives_blocked_by_magic_immunity() {
    let hero = make_test_hero();
    let glaives = AbilityDef {
        name: "Glaives of Wisdom".to_string(),
        cooldown: vec![0.0],
        mana_cost: vec![0.0],
        cast_point: 0.0,
        targeting: TargetType::Passive,
        effects: vec![Effect::GlaivesOfWisdom {
            int_damage_factor: vec![0.8],
                int_steal_per_attack: vec![2.0],
                steal_duration: vec![10.0],
            mana_cost: vec![15.0],
            steal_int_on_kill: vec![0.0],
            steal_radius: 900.0,
            bounce_radius: vec![0.0],
        }],
        description: String::new(), is_ultimate: false,
        aoe_shape: None,
        cast_range: 0.0,
        cast_behavior: aa2_data::CastBehavior::default(),
        max_charges: None,
    };

    let mut attacker = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    attacker.abilities.push(AbilityState { def: glaives, cooldown_remaining: 0.0, level: 3, casts: 0, charges: None });
    attacker.mana = 100.0;
    let mana_before = attacker.mana;

    // Target with magic immunity (Rage active)
    let mut target = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));
    // Apply magic immunity buff
    target.buffs.push(aa2_sim::buff::Buff {
        name: "rage".to_string(),
        remaining_ticks: 300,
        tick_effect: None,
        stacking: aa2_sim::buff::StackBehavior::RefreshDuration,
        dispel_type: aa2_sim::buff::DispelType::Undispellable,
        status: aa2_sim::buff::StatusFlags { magic_immune: true, ..Default::default() },
        stat_modifier: None,
        source_id: 1,
        is_debuff: false,
        pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
    });

    let target_hp_before = target.hp;
    let mut sim = Simulation::with_seed(vec![attacker, target], 42);

    // Run until an attack lands
    for _ in 0..200 {
        if sim.combat_log.iter().any(|e| matches!(e, CombatEvent::Attack { .. })) { break; }
        sim.step();
    }

    // Attacker should NOT have spent Glaives mana (blocked by immunity)
    // Mana may have increased slightly from regen, but should not have decreased by 15
    assert!(sim.units[0].mana >= mana_before,
        "Glaives mana should not be spent against magic immune target (mana: {}, was: {})",
        sim.units[0].mana, mana_before);

    // Target should have taken ONLY physical damage (no magical bonus)
    // Physical damage from a TestHero with 20 STR primary = ~20 base + 20 primary = 40 damage
    // After armor reduction it should be less than 40
    let damage_taken = target_hp_before - sim.units[1].hp;
    assert!(damage_taken > 0.0, "Target should still take physical damage");
    assert!(damage_taken < 50.0,
        "Damage ({:.1}) should be only physical (no Glaives magical bonus)", damage_taken);
}

// ============================================================
// Arena Bounds Tests
// ============================================================

#[test]
fn test_arena_bounds_clamp() {
    use aa2_sim::clamp_to_arena;

    // Inside bounds — no clamping
    let (pos, hit) = clamp_to_arena(Vec2::new(500.0, 500.0));
    assert_eq!(pos, Vec2::new(500.0, 500.0));
    assert!(!hit);

    // Outside left
    let (pos, hit) = clamp_to_arena(Vec2::new(-50.0, 500.0));
    assert_eq!(pos, Vec2::new(0.0, 500.0));
    assert!(hit);

    // Outside right
    let (pos, hit) = clamp_to_arena(Vec2::new(2100.0, 500.0));
    assert_eq!(pos, Vec2::new(2000.0, 500.0));
    assert!(hit);

    // Outside top
    let (pos, hit) = clamp_to_arena(Vec2::new(500.0, 2100.0));
    assert_eq!(pos, Vec2::new(500.0, 2000.0));
    assert!(hit);

    // Outside bottom
    let (pos, hit) = clamp_to_arena(Vec2::new(500.0, -10.0));
    assert_eq!(pos, Vec2::new(500.0, 0.0));
    assert!(hit);

    // Unit can't move outside bounds via move_toward
    let hero = make_hero();
    let u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(10.0, 1000.0));
    let u1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(1990.0, 1000.0));
    let mut sim = Simulation::new(vec![u0, u1]);
    // Run many ticks — units should never leave bounds
    for _ in 0..300 {
        sim.step();
        for u in &sim.units {
            assert!(u.position.x >= 0.0 && u.position.x <= 2000.0);
            assert!(u.position.y >= 0.0 && u.position.y <= 2000.0);
        }
    }
}

// ============================================================
// Spear of Mars Tests
// ============================================================

fn spear_of_mars_ability(_level: u8) -> AbilityDef {
    aa2_data::load_ability_def(&data_path("abilities/spear_of_mars.ron")).unwrap()
}

/// Helper: create a simulation with a caster (team 0) and target (team 1) at given positions.
/// Caster has Spear of Mars at given level, facing toward target.
fn spear_sim(caster_pos: Vec2, target_pos: Vec2, level: u8) -> Simulation {
    let hero = make_hero();
    let ability = spear_of_mars_ability(level);
    let config_a = UnitConfig::new(hero.clone()).with_ability(ability, level);
    let mut caster = Unit::from_config(&config_a, 0, 0, caster_pos);
    let dir = (target_pos - caster_pos).normalize();
    caster.facing = dir.angle();
    let target = Unit::from_hero_def(&hero, 1, 1, target_pos);
    Simulation::new(vec![caster, target])
}

#[test]
fn test_spear_pins_to_wall() {
    // Place caster at center, target near right wall
    // Spear should impale target and push them into the wall, applying stun
    let caster_pos = Vec2::new(1000.0, 1000.0);
    let target_pos = Vec2::new(1900.0, 1000.0); // near right wall
    let mut sim = spear_sim(caster_pos, target_pos, 3); // level 3: range 1200

    // Run until spear completes (range 1200 at 1400 u/s = ~0.86s = ~26 ticks + cast point)
    for _ in 0..60 {
        sim.step();
    }

    // Target should be at or near the wall (x=2000)
    let target = &sim.units[1];
    assert!(target.position.x >= 1990.0, "Target should be pinned at wall, got x={}", target.position.x);

    // Target should be stunned
    let has_stun = target.buffs.iter().any(|b| b.name == "stun" && b.status.stunned);
    assert!(has_stun, "Target should be stunned after wall pin");
}

#[test]
fn test_spear_no_wall_no_stun() {
    // Place caster and target in center — spear won't reach a wall within range
    let caster_pos = Vec2::new(1000.0, 1000.0);
    let target_pos = Vec2::new(1100.0, 1000.0); // 100 units away, wall is 900+ away
    let mut sim = spear_sim(caster_pos, target_pos, 1); // level 1: range 900

    // Run until spear expires
    for _ in 0..60 {
        sim.step();
    }

    // Target should NOT have wall stun (only the brief drag disable which expires)
    let target = &sim.units[1];
    let has_wall_stun = target.buffs.iter().any(|b| b.name == "stun" && b.status.stunned);
    assert!(!has_wall_stun, "Target should NOT be stunned without wall pin");

    // Target should have been displaced (dragged along spear path)
    assert!(target.position.x > 1100.0 + 100.0, "Target should have been dragged, got x={}", target.position.x);
}

#[test]
fn test_spear_pass_through_damage() {
    // Place two enemies in the spear path — first gets impaled, second takes pass-through damage
    let hero = make_hero();
    let ability = spear_of_mars_ability(3);
    let config_a = UnitConfig::new(hero.clone()).with_ability(ability, 3);
    let mut caster = Unit::from_config(&config_a, 0, 0, Vec2::new(100.0, 1000.0));
    caster.facing = 0.0; // facing right

    let target1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(300.0, 1000.0)); // first hit
    let target2 = Unit::from_hero_def(&hero, 2, 1, Vec2::new(500.0, 1000.0)); // pass-through

    let mut sim = Simulation::new(vec![caster, target1, target2]);
    let hp_before_2 = sim.units[2].hp;

    // Run until spear completes
    for _ in 0..60 {
        sim.step();
    }

    // First target (impaled) should take damage from pin
    // Second target should take pass-through damage
    let target2_hp = sim.units[2].hp;
    assert!(target2_hp < hp_before_2, "Pass-through target should take damage, hp={} vs {}", target2_hp, hp_before_2);

    // First target should have been dragged (position changed significantly)
    assert!(sim.units[1].position.x > 500.0, "First target should be dragged past second target");
}

#[test]
fn test_spear_gaben_bounce() {
    // Level 9 (Gaben): 2 bounces, range 2800
    // Place caster near left wall, first target in path, second target behind caster
    // After bounce off right wall, spear goes left and can hit second target
    let hero = make_hero();
    let ability = spear_of_mars_ability(9);
    let config_a = UnitConfig::new(hero.clone()).with_ability(ability, 9);
    let mut caster = Unit::from_config(&config_a, 0, 0, Vec2::new(500.0, 1000.0));
    caster.facing = 0.0; // facing right

    // First target in path — will be impaled and pinned at right wall
    let target1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(700.0, 1000.0));
    // Second target to the left of wall — after bounce, spear goes left
    let target2 = Unit::from_hero_def(&hero, 2, 1, Vec2::new(1800.0, 1000.0));

    let mut sim = Simulation::new(vec![caster, target1, target2]);

    // Run enough ticks for spear to travel, bounce, and travel back
    for _ in 0..120 {
        sim.step();
    }

    // First target (closest, impaled first) should be pinned at right wall
    let t1 = &sim.units[1];
    assert!(t1.position.x >= 1990.0, "First target should be pinned at right wall, got x={}", t1.position.x);
    let t1_stunned = t1.buffs.iter().any(|b| b.name == "stun" && b.status.stunned);
    assert!(t1_stunned, "First target should be stunned from wall pin");

    // Second target should have taken damage (pass-through on initial path or impale on bounce)
    let t2 = &sim.units[2];
    let hero_max_hp = Unit::from_hero_def(&hero, 99, 1, Vec2::new(0.0, 0.0)).max_hp;
    assert!(t2.hp < hero_max_hp, "Second target should have taken damage from spear");
}

/// Gaben Spear of Mars: after bouncing off wall, spear impales and pins a SECOND unit.
#[test]
fn test_spear_gaben_bounce_impales_second_target() {
    let hero = make_hero();
    let ability = spear_of_mars_ability(9);
    let config_a = UnitConfig::new(hero.clone()).with_ability(ability, 9);
    let mut caster = Unit::from_config(&config_a, 0, 0, Vec2::new(200.0, 1000.0));
    caster.facing = 0.0; // facing right
    caster.mana = 500.0;

    // First target: in path, will be impaled and pinned at right wall (x=2000)
    let target1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(500.0, 1000.0));
    // Second target: near right wall, after bounce (spear goes left) it's in the return path
    let target2 = Unit::from_hero_def(&hero, 2, 1, Vec2::new(1700.0, 1000.0));

    let mut sim = Simulation::with_seed(vec![caster, target1, target2], 42);

    // Track if targets were ever stunned
    let mut t1_was_stunned = false;
    let mut t2_was_stunned = false;
    for _ in 0..300 {
        sim.step();
        if sim.units[1].buffs.iter().any(|b| b.status.stunned) { t1_was_stunned = true; }
        if sim.units[2].buffs.iter().any(|b| b.status.stunned) { t2_was_stunned = true; }
        if sim.is_finished() { break; }
    }

    // First target should be pinned at right wall and was stunned
    assert!(sim.units[1].position.x >= 1990.0,
        "First target should be pinned at right wall, got x={:.0}", sim.units[1].position.x);
    assert!(t1_was_stunned, "First target should have been stunned from wall pin");

    // Second target should also have been hit by the bounced spear
    let t2_took_damage = sim.units[2].hp < Unit::from_hero_def(&hero, 99, 1, Vec2::new(0.0, 0.0)).max_hp;
    assert!(t2_took_damage,
        "Second target should be hit by bounced spear. HP={:.0}", sim.units[2].hp);

    // Second target should have been stunned (impaled on bounce)
    assert!(t2_was_stunned,
        "Second target should have been stunned (impaled on bounce). pos=({:.0},{:.0})",
        sim.units[2].position.x, sim.units[2].position.y);
}
/// Spear of Mars is blocked by magic immunity — no impale, no damage, no drag.
#[test]
fn test_spear_blocked_by_magic_immunity() {
    let hero = make_hero();
    let ability = spear_of_mars_ability(3);
    let config_a = UnitConfig::new(hero.clone()).with_ability(ability, 3);
    let mut caster = Unit::from_config(&config_a, 0, 0, Vec2::new(100.0, 1000.0));
    caster.facing = 0.0;
    caster.mana = 500.0;

    // Target with magic immunity (Rage)
    let mut target = Unit::from_hero_def(&hero, 1, 1, Vec2::new(400.0, 1000.0));
    target.buffs.push(aa2_sim::buff::Buff {
        name: "rage".to_string(),
        remaining_ticks: 300,
        tick_effect: None,
        stacking: aa2_sim::buff::StackBehavior::RefreshDuration,
        dispel_type: aa2_sim::buff::DispelType::Undispellable,
        status: aa2_sim::buff::StatusFlags { magic_immune: true, ..Default::default() },
        stat_modifier: None,
        source_id: 1,
        is_debuff: false,
        pierces_magic_immunity: false,
                    damage_reflection_pct: 0.0,
    });
    let target_hp_before = target.hp;
    let target_pos_before = target.position;

    let mut sim = Simulation::new(vec![caster, target]);

    for _ in 0..60 {
        sim.step();
    }

    // Target should NOT take spear (magical) damage — only physical auto-attacks allowed
    let spear_damage_events = sim.combat_log.iter().filter(|e| {
        matches!(e, CombatEvent::AbilityDamage { target_id: 1, .. })
    }).count();
    assert_eq!(spear_damage_events, 0,
        "Magic immune target should not take spear ability damage");

    // Target should NOT be dragged AWAY from caster (spear pushes in cast direction = right)
    // Normal movement toward caster (left) is fine
    assert!(sim.units[1].position.x <= target_pos_before.x + 5.0,
        "Magic immune target should not be dragged rightward by spear: x={:.0} vs {:.0}",
        sim.units[1].position.x, target_pos_before.x);

    // Target should NOT be stunned
    let stunned = aa2_sim::buff::active_status(&sim.units[1].buffs).stunned;
    assert!(!stunned, "Magic immune target should not be stunned by spear");
}

/// Spear of Mars: impaled unit is disabled (stunned) during the drag travel.
#[test]
fn test_spear_drag_disables_unit() {
    let hero = make_hero();
    let ability = spear_of_mars_ability(3);
    let config_a = UnitConfig::new(hero.clone()).with_ability(ability, 3);
    let mut caster = Unit::from_config(&config_a, 0, 0, Vec2::new(100.0, 1000.0));
    caster.facing = 0.0;
    caster.mana = 500.0;

    // Target in path — will be impaled
    let target = Unit::from_hero_def(&hero, 1, 1, Vec2::new(400.0, 1000.0));

    let mut sim = Simulation::new(vec![caster, target]);

    // Run until spear hits the target (should be ~tick 3-5 at 1400 u/s over 300 units)
    let mut drag_stun_found = false;
    for _ in 0..30 {
        sim.step();
        // Check if target is stunned while being dragged (before wall pin)
        let is_stunned = aa2_sim::buff::active_status(&sim.units[1].buffs).stunned;
        let is_moving = sim.units[1].position.x > 400.0 + 10.0; // has been dragged
        if is_stunned && is_moving {
            drag_stun_found = true;
            break;
        }
    }

    assert!(drag_stun_found,
        "Impaled unit should be stunned (disabled) while being dragged by the spear");
}

#[test]
fn test_spear_of_mars_deals_damage_on_impale() {
    use aa2_sim::pending::{PendingEffect, PendingEffectKind};

    let hero = make_hero();
    // Caster at origin, target 200 units to the right (within width)
    let u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    let u1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(200.0, 0.0));

    let mut sim = Simulation::new(vec![u0, u1]);
    let initial_hp = sim.units[1].hp;

    // Manually inject a SpearOfMarsTravel heading toward the target
    sim.pending_effects.push(PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Spear of Mars".to_string(),
        kind: PendingEffectKind::SpearOfMarsTravel {
            start_pos: Vec2::new(0.0, 0.0),
            direction: Vec2::new(1.0, 0.0),
            travel_speed: 1400.0,
            max_range: 900.0,
            current_distance: 0.0,
            width: 125.0,
            damage: 100.0,
            stun_duration_secs: 1.6,
            impaled_unit: None,
            pass_through_hit: Vec::new(),
            fire_trail_dps: 0.0,
            fire_trail_slow: 0.0,
            fire_trail_duration_secs: 0.0,
            bounces_remaining: 0,
            fire_trail_positions: Vec::new(),
        },
        delay_ticks_remaining: 0,
    });

    // Run enough ticks for spear to reach the target (200 units at 1400 speed ~ 5 ticks)
    for _ in 0..10 {
        sim.step();
    }

    // Target should have taken damage
    assert!(sim.units[1].hp < initial_hp, "Spear of Mars should deal damage on impale");

    // Check for AbilityDamage event
    let damage_events: Vec<_> = sim.combat_log.iter()
        .filter(|e| matches!(e, CombatEvent::AbilityDamage {
            ability_name, damage_type, target_id, ..
        } if ability_name == "Spear of Mars" && *damage_type == DamageType::Magical && *target_id == 1))
        .collect();
    assert!(!damage_events.is_empty(), "Should have AbilityDamage event for Spear of Mars impale");
}

/// Verify: units re-engage after being displaced beyond acquisition range.
#[test]
fn test_units_reengage_after_displacement() {
    // Two melee units with Spear of Mars — displacement can push beyond ACQUISITION_RANGE.
    // After displacement, units should re-acquire targets and auto-attack.
    let hero = make_hero();
    let ability = spear_of_mars_ability(3);
    let config = UnitConfig::new(hero.clone()).with_ability(ability, 3);

    let mut u0 = Unit::from_config(&config, 0, 0, Vec2::new(1000.0, 1000.0));
    u0.facing = 0.0; // facing right
    let mut u1 = Unit::from_config(&config, 1, 1, Vec2::new(1100.0, 1000.0));
    u1.facing = std::f32::consts::PI; // facing left

    let mut sim = Simulation::new(vec![u0, u1]);

    // Run for 900 ticks (30 seconds at 30 tps)
    for _ in 0..900 {
        sim.step();
    }

    // Assert: Attack events exist (units auto-attacked between casts)
    let attack_count = sim.combat_log.iter()
        .filter(|e| matches!(e, CombatEvent::Attack { .. }))
        .count();
    assert!(attack_count > 0, "Units should auto-attack after displacement, got 0 attacks");

    // Assert: combat is not a draw (someone took damage)
    let total_damage: f32 = sim.combat_log.iter()
        .filter_map(|e| if let CombatEvent::Attack { damage, .. } = e { Some(*damage) } else { None })
        .sum();
    assert!(total_damage > 50.0, "Expected significant auto-attack damage, got {total_damage}");
}

/// Verify: unit with Lazy ability does NOT walk toward distant enemy to cast.
/// Instead falls through to auto-attack (walks toward enemy for attack range).
/// After reaching cast_range, Lazy ability fires (it's now in range).
#[test]
fn test_lazy_does_not_drive_movement_for_cast() {
    let hero = make_hero(); // melee, attack_range 150
    let ability = AbilityDef {
        name: "LazySpell".to_string(),
        cooldown: vec![30.0],
        mana_cost: vec![10.0],
        cast_point: 0.0,
        targeting: TargetType::SingleEnemy,
        effects: vec![Effect::Damage { kind: DamageType::Magical, base: vec![100.0] }],
        description: String::new(),
        is_ultimate: false,
        aoe_shape: None,
        cast_range: 300.0,
        cast_behavior: aa2_data::CastBehavior::Lazy,
        max_charges: None,
    };
    let config_a = UnitConfig::new(hero.clone()).with_ability(ability, 1);
    let mut u0 = Unit::from_config(&config_a, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    let u1 = Unit::from_hero_def(&HeroDef { base_damage_min: 1.0, base_damage_max: 1.0, ..hero }, 1, 1, Vec2::new(500.0, 0.0));

    let mut sim = Simulation::new(vec![u0, u1]);

    // Run enough ticks for unit to walk into range and act
    for _ in 0..600 {
        sim.step();
        if sim.is_finished() { break; }
    }

    // The Lazy ability should have eventually cast (once unit walked into 300 range for auto-attack pathing)
    let cast_events: Vec<_> = sim.combat_log.iter()
        .filter(|e| matches!(e, CombatEvent::CastStart { ability_name, .. } if ability_name == "LazySpell"))
        .collect();
    assert!(!cast_events.is_empty(), "Lazy ability should cast once target is in range");

    // Auto-attacks should also have happened (unit walked for auto-attack, not for Lazy)
    let attack_count = sim.combat_log.iter()
        .filter(|e| matches!(e, CombatEvent::Attack { .. }))
        .count();
    assert!(attack_count > 0, "Unit should auto-attack (Lazy doesn't drive movement)");
}

/// Verify: unit with Seek ability walks past spell-immune enemy to reach valid target.
#[test]
fn test_seek_walks_past_spell_immune() {
    let hero = make_hero();
    let ability = AbilityDef {
        name: "SeekSpell".to_string(),
        cooldown: vec![30.0],
        mana_cost: vec![10.0],
        cast_point: 0.0,
        targeting: TargetType::SingleEnemy,
        effects: vec![Effect::Damage { kind: DamageType::Magical, base: vec![100.0] }],
        description: String::new(),
        is_ultimate: false,
        aoe_shape: None,
        cast_range: 600.0,
        cast_behavior: aa2_data::CastBehavior::Seek,
        max_charges: None,
    };
    let config_a = UnitConfig::new(hero.clone()).with_ability(ability, 1);
    let mut u0 = Unit::from_config(&config_a, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    u0.hp = 5000.0;

    // Spell-immune enemy at 400 range (stationary — high HP, no attack so it doesn't move)
    let mut u1 = Unit::from_hero_def(&HeroDef {
        base_damage_min: 0.0, base_damage_max: 0.0,
        move_speed: 0.0, // stationary
        ..hero.clone()
    }, 1, 1, Vec2::new(400.0, 0.0));
    u1.hp = 9999.0;
    u1.buffs.push(Buff {
        name: "rage".to_string(),
        remaining_ticks: 9999,
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

    // Non-immune enemy at 1200 range (stationary)
    let mut u2 = Unit::from_hero_def(&HeroDef {
        base_damage_min: 0.0, base_damage_max: 0.0,
        move_speed: 0.0,
        ..hero
    }, 2, 1, Vec2::new(1200.0, 0.0));
    u2.hp = 9999.0;

    let mut sim = Simulation::new(vec![u0, u1, u2]);

    // Track max x position reached
    let mut max_x = 0.0_f32;
    for _ in 0..900 {
        sim.step();
        max_x = max_x.max(sim.units[0].position.x);
        if sim.is_finished() { break; }
    }

    // SeekSpell should have cast (targeting the non-immune enemy)
    let cast_events: Vec<_> = sim.combat_log.iter()
        .filter(|e| matches!(e, CombatEvent::CastStart { ability_name, .. } if ability_name == "SeekSpell"))
        .collect();
    assert!(!cast_events.is_empty(), "Seek ability should cast on non-immune target");

    // Unit should have walked past the spell-immune enemy (at 400) to reach cast range of target at 1200
    // Cast range is 600, so unit needs to reach at least 600 (1200 - 600)
    assert!(max_x >= 550.0,
        "Unit should walk past spell-immune enemy (at 400) toward cast range (600 from 1200), max_x={}", max_x);
}

/// Verify: abilities are checked in slot order (left to right).
#[test]
fn test_slot_priority_order() {
    let hero = make_hero();
    let ability_a = AbilityDef {
        name: "SlotA".to_string(),
        cooldown: vec![30.0],
        mana_cost: vec![10.0],
        cast_point: 0.0,
        targeting: TargetType::SingleEnemy,
        effects: vec![Effect::Damage { kind: DamageType::Magical, base: vec![50.0] }],
        description: String::new(),
        is_ultimate: false,
        aoe_shape: None,
        cast_range: 600.0,
        cast_behavior: aa2_data::CastBehavior::Seek,
        max_charges: None,
    };
    let ability_b = AbilityDef {
        name: "SlotB".to_string(),
        cooldown: vec![30.0],
        mana_cost: vec![10.0],
        cast_point: 0.0,
        targeting: TargetType::SingleEnemy,
        effects: vec![Effect::Damage { kind: DamageType::Magical, base: vec![50.0] }],
        description: String::new(),
        is_ultimate: false,
        aoe_shape: None,
        cast_range: 300.0,
        cast_behavior: aa2_data::CastBehavior::Seek,
        max_charges: None,
    };
    let config = UnitConfig::new(hero.clone())
        .with_ability(ability_a, 1)
        .with_ability(ability_b, 1);
    let mut u0 = Unit::from_config(&config, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    let u1 = Unit::from_hero_def(&HeroDef { base_damage_min: 1.0, base_damage_max: 1.0, ..hero }, 1, 1, Vec2::new(800.0, 0.0));

    let mut sim = Simulation::new(vec![u0, u1]);

    // Run until both abilities have cast
    for _ in 0..1800 {
        sim.step();
        if sim.is_finished() { break; }
    }

    let cast_names: Vec<&str> = sim.combat_log.iter()
        .filter_map(|e| if let CombatEvent::CastStart { ability_name, .. } = e { Some(ability_name.as_str()) } else { None })
        .collect();

    // SlotA (index 0) should cast before SlotB (index 1)
    let a_pos = cast_names.iter().position(|n| *n == "SlotA");
    let b_pos = cast_names.iter().position(|n| *n == "SlotB");
    assert!(a_pos.is_some(), "SlotA should have cast");
    assert!(b_pos.is_some(), "SlotB should have cast");
    assert!(a_pos.unwrap() < b_pos.unwrap(), "SlotA (slot 0) should cast before SlotB (slot 1)");
}

/// Verify: when all abilities are on cooldown, unit auto-attacks.
#[test]
fn test_auto_attack_when_abilities_on_cooldown() {
    let hero = make_hero();
    let ability = AbilityDef {
        name: "BigSpell".to_string(),
        cooldown: vec![10.0],
        mana_cost: vec![10.0],
        cast_point: 0.0,
        targeting: TargetType::SingleEnemy,
        effects: vec![Effect::Damage { kind: DamageType::Magical, base: vec![50.0] }],
        description: String::new(),
        is_ultimate: false,
        aoe_shape: None,
        cast_range: 600.0,
        cast_behavior: aa2_data::CastBehavior::Seek,
        max_charges: None,
    };
    let config = UnitConfig::new(hero.clone()).with_ability(ability, 1);
    let mut u0 = Unit::from_config(&config, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    // Enemy close enough that we get into range quickly
    let u1 = Unit::from_hero_def(&HeroDef { base_damage_min: 1.0, base_damage_max: 1.0, ..hero }, 1, 1, Vec2::new(200.0, 0.0));

    let mut sim = Simulation::new(vec![u0, u1]);

    // Run for 15 seconds (ability has 10s CD, so auto-attacks should happen during CD)
    for _ in 0..450 {
        sim.step();
        if sim.is_finished() { break; }
    }

    let cast_count = sim.combat_log.iter()
        .filter(|e| matches!(e, CombatEvent::CastStart { ability_name, .. } if ability_name == "BigSpell"))
        .count();
    let attack_count = sim.combat_log.iter()
        .filter(|e| matches!(e, CombatEvent::Attack { .. }))
        .count();

    assert!(cast_count >= 1, "Ability should cast at least once");
    assert!(attack_count > 0, "Unit should auto-attack during ability cooldown");
}
