//! Tests for illusion system, CDR, Universal attribute, and Spirit Lance.

use aa2_data::{AbilityDef, Attribute, DamageType, Effect, HeroDef, TargetType};
use aa2_sim::cast::AbilityState;
use aa2_sim::unit::{Unit, UnitState};
use aa2_sim::vec2::Vec2;
use aa2_sim::{CombatEvent, Simulation};

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

fn make_universal_hero() -> HeroDef {
    HeroDef {
        name: "UniversalHero".to_string(),
        primary_attribute: Attribute::Universal,
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

fn spirit_lance_ability() -> AbilityDef {
    AbilityDef {
        name: "Spirit Lance".to_string(),
        cooldown: vec![10.0, 9.0, 7.0, 7.0, 7.0, 3.0, 3.0, 3.0, 3.0],
        mana_cost: vec![120.0],
        cast_point: 0.3,
        targeting: TargetType::SingleEnemy,
        effects: vec![Effect::SpiritLance {
            damage: vec![100.0, 160.0, 280.0, 280.0, 280.0, 280.0, 280.0, 280.0, 280.0],
            slow_pct: vec![14.0, 21.0, 35.0, 35.0, 35.0, 35.0, 35.0, 35.0, 35.0],
            slow_duration: vec![3.0],
            projectile_speed: 1000.0,
            illusion_damage_dealt: vec![0.20, 0.20, 0.20, 0.20, 0.20, 0.60, 0.60, 0.60, 0.60],
            illusion_damage_taken: 4.0,
            illusion_duration: vec![3.5, 5.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0],
            bounce_radius: vec![0.0, 0.0, 0.0, 0.0, 0.0, 750.0, 750.0, 750.0, 750.0],
            bounce_count: vec![0, 0, 0, 0, 0, 1, 1, 1, 1],
        }],
        description: String::new(), is_ultimate: false,
        aoe_shape: None,
        cast_range: 750.0,
        cast_behavior: aa2_data::CastBehavior::Seek,
        max_charges: None,
    }
}

#[test]
fn test_illusion_spawns_on_spirit_lance() {
    let hero = make_hero();
    let mut caster = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    caster.mana = 500.0;
    caster.abilities.push(AbilityState {
        def: spirit_lance_ability(),
        cooldown_remaining: 0.0,
        level: 1,
        casts: 0,
        charges: None,
    });
    let target = Unit::from_hero_def(&hero, 1, 1, Vec2::new(300.0, 0.0));

    let mut sim = Simulation::new(vec![caster, target]);

    // Run until Spirit Lance hits (cast + travel)
    for _ in 0..100 {
        sim.step();
        if sim.units.len() > 2 { break; }
    }

    // Illusion should have spawned
    assert!(sim.units.len() >= 3, "Expected illusion to spawn, got {} units", sim.units.len());
    let illusion = &sim.units[2];
    assert!(illusion.is_illusion);
    assert_eq!(illusion.team, 0); // same team as caster
    assert!(illusion.abilities.is_empty()); // Spirit Lance is Disabled for illusions
}

#[test]
fn test_illusion_deals_reduced_damage() {
    let hero = make_hero();
    let source = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));

    // Spawn illusion manually
    let illusion = Unit::spawn_illusion(&source, 10, Vec2::new(0.0, 0.0), 0.20, 4.0, 300, 0);
    let target = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));

    let mut sim = Simulation::new(vec![illusion, target]);

    // Run until first attack
    for _ in 0..100 {
        sim.step();
        if sim.combat_log.iter().any(|e| matches!(e, CombatEvent::Attack { attacker_id: 10, .. })) {
            break;
        }
    }

    let attack_event = sim.combat_log.iter().find(|e| matches!(e, CombatEvent::Attack { attacker_id: 10, .. }));
    assert!(attack_event.is_some(), "Illusion should attack");

    if let Some(CombatEvent::Attack { damage, .. }) = attack_event {
        // Normal damage would be ~50 (30 base + 20 STR) * armor_mult
        // Illusion deals 20% of that
        // With armor ~3.34 (20 AGI * 0.167), multiplier ~0.83
        // Normal: 50 * 0.83 = ~41.5, Illusion: ~8.3
        assert!(*damage < 15.0, "Illusion damage {damage} should be much less than normal");
        assert!(*damage > 0.0, "Illusion should deal some damage");
    }
}

#[test]
fn test_illusion_takes_increased_damage() {
    let hero = make_hero();
    let source = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));

    // Spawn illusion as target
    let illusion = Unit::spawn_illusion(&source, 10, Vec2::new(100.0, 0.0), 0.20, 4.0, 9000, 0);
    let attacker = Unit::from_hero_def(&hero, 1, 1, Vec2::new(0.0, 0.0));

    let mut sim = Simulation::new(vec![attacker, illusion]);

    // Run until first attack on illusion
    for _ in 0..100 {
        sim.step();
        if sim.combat_log.iter().any(|e| matches!(e, CombatEvent::Attack { target_id: 10, .. })) {
            break;
        }
    }

    let attack_event = sim.combat_log.iter().find(|e| matches!(e, CombatEvent::Attack { target_id: 10, .. }));
    assert!(attack_event.is_some(), "Should attack illusion");

    if let Some(CombatEvent::Attack { damage, .. }) = attack_event {
        // Normal damage ~50 * armor_mult ~0.83 = ~41.5
        // Illusion takes 4x: ~166
        assert!(*damage > 100.0, "Illusion should take amplified damage, got {damage}");
    }
}

#[test]
fn test_illusion_expires_after_duration() {
    let hero = make_hero();
    let source = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));

    // Illusion lasts 30 ticks (1 second)
    let illusion = Unit::spawn_illusion(&source, 10, Vec2::new(500.0, 0.0), 0.20, 4.0, 30, 0);
    // Put a dummy enemy far away so sim doesn't end
    let enemy = Unit::from_hero_def(&hero, 1, 1, Vec2::new(9999.0, 0.0));

    let mut sim = Simulation::new(vec![source, illusion, enemy]);

    // Run 29 ticks - illusion should be alive
    for _ in 0..29 {
        sim.step();
    }
    assert!(sim.units[1].is_alive(), "Illusion should still be alive at tick 29");

    // Tick 30 - illusion should expire
    sim.step();
    assert_eq!(sim.units[1].state, UnitState::Dead, "Illusion should be dead at tick 30");
    assert!(sim.units[1].hp <= 0.0);
}

#[test]
fn test_cdr_reduces_cooldown() {
    let hero = make_hero();
    let mut caster = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    caster.mana = 500.0;
    caster.cooldown_reduction = 0.25; // 25% CDR
    caster.abilities.push(AbilityState {
        def: AbilityDef {
            name: "TestSpell".to_string(),
            cooldown: vec![10.0],
            mana_cost: vec![50.0],
            cast_point: 0.1,
            targeting: TargetType::SingleEnemy,
            effects: vec![Effect::Damage { kind: DamageType::Magical, base: vec![100.0] }],
            description: String::new(), is_ultimate: false,
            aoe_shape: None,
            cast_range: 600.0,
            cast_behavior: aa2_data::CastBehavior::default(),
            max_charges: None,
        },
        cooldown_remaining: 0.0,
        level: 1,
        casts: 0,
        charges: None,
    });
    let target = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));

    let mut sim = Simulation::new(vec![caster, target]);

    // Run until cast completes
    for _ in 0..60 {
        sim.step();
        if sim.combat_log.iter().any(|e| matches!(e, CombatEvent::CastComplete { .. })) {
            break;
        }
    }

    assert!(sim.combat_log.iter().any(|e| matches!(e, CombatEvent::CastComplete { .. })));
    // Cooldown should be 10.0 * (1 - 0.25) = 7.5
    let cd = sim.units[0].abilities[0].cooldown_remaining;
    assert!((cd - 7.5).abs() < 0.1, "Expected ~7.5 cooldown with 25% CDR, got {cd}");
}

#[test]
fn test_universal_attribute_damage() {
    let hero = make_universal_hero();
    let unit = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));

    // Universal: damage = base + (STR+AGI+INT) * 0.7
    // STR=20, AGI=20, INT=20 -> (60) * 0.7 = 42
    // base_damage_min = 30, so damage_min = 30 + 42 = 72
    let expected_primary = (20.0 + 20.0 + 20.0) * 0.7;
    assert!((unit.damage_min - (30.0 + expected_primary)).abs() < 0.01,
        "Expected damage_min = {}, got {}", 30.0 + expected_primary, unit.damage_min);
    assert!((unit.damage_max - (30.0 + expected_primary)).abs() < 0.01,
        "Expected damage_max = {}, got {}", 30.0 + expected_primary, unit.damage_max);
}

#[test]
fn test_spirit_lance_bounce_super() {
    let hero = make_hero();
    let mut caster = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    caster.mana = 500.0;
    // Level 6 = Super, has bounce
    caster.abilities.push(AbilityState {
        def: spirit_lance_ability(),
        cooldown_remaining: 0.0,
        level: 6,
        casts: 0,
        charges: None,
    });
    let target1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(300.0, 0.0));
    let target2 = Unit::from_hero_def(&hero, 2, 1, Vec2::new(500.0, 0.0));

    let mut sim = Simulation::new(vec![caster, target1, target2]);

    // Run until both targets are hit (bounce)
    for _ in 0..200 {
        sim.step();
    }

    // Both targets should have taken ability damage
    let hits: Vec<_> = sim.combat_log.iter().filter(|e| {
        matches!(e, CombatEvent::AbilityDamage { ability_name, .. } if ability_name == "Spirit Lance")
    }).collect();

    assert!(hits.len() >= 2, "Expected Spirit Lance to hit 2 targets (bounce), got {} hits", hits.len());

    // Check that both target IDs were hit
    let hit_ids: Vec<u32> = hits.iter().filter_map(|e| {
        if let CombatEvent::AbilityDamage { target_id, .. } = e { Some(*target_id) } else { None }
    }).collect();
    assert!(hit_ids.contains(&1), "First target should be hit");
    assert!(hit_ids.contains(&2), "Second target should be hit (bounce)");
}

/// Chaos Strike (crit) WORKS on illusions — illusions can crit.
#[test]
fn test_illusion_can_crit_with_chaos_strike() {
    use std::path::Path;
    let hero = aa2_data::load_hero_def(Path::new("../../data/heroes/chaos_knight.ron")).unwrap();
    let cs = aa2_data::load_ability_def(Path::new("../../data/abilities/chaos_strike.ron")).unwrap();

    // Create a real unit with Chaos Strike, then spawn an illusion from it
    let mut source = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    source.abilities.push(aa2_sim::cast::AbilityState {
        def: cs, cooldown_remaining: 0.0, level: 3, casts: 0, charges: None,
    });

    let illusion = Unit::spawn_illusion(&source, 10, Vec2::new(0.0, 0.0), 1.0, 1.0, 9000, 0);

    // Illusion should retain Chaos Strike (Full interaction)
    assert!(!illusion.abilities.is_empty(), "Illusion should retain Chaos Strike (Full-tagged)");
    assert_eq!(illusion.abilities[0].def.name, "Chaos Strike");

    // Run combat with many seeds to verify at least one crit happens
    let mut any_crit = false;
    for seed in 0..50 {
        let ill = illusion.clone();
        let base_dmg = ill.damage_min; // base damage for comparison
        let enemy = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));
        let mut sim = Simulation::with_seed(vec![ill, enemy], seed);

        for _ in 0..200 {
            if sim.is_finished() { break; }
            sim.step();
        }

        // Check if any attack dealt more than base damage (= crit happened)
        for event in &sim.combat_log {
            if let CombatEvent::Attack { attacker_id: 10, damage, .. } = event {
                // A crit should deal noticeably more than base after armor
                if *damage > base_dmg * 0.9 {
                    any_crit = true;
                    break;
                }
            }
        }
        if any_crit { break; }
    }
    assert!(any_crit, "Illusion with Chaos Strike should crit at least once across 50 seeds");
}

/// Illusions ignore flat bonus_armor from buffs.
#[test]
fn test_illusion_ignores_flat_armor_bonus() {
    use aa2_sim::buff::{Buff, StackBehavior, DispelType, StatusFlags, StatModifier};

    let hero = make_hero();
    let source = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    let mut illusion = Unit::spawn_illusion(&source, 10, Vec2::new(0.0, 0.0), 0.20, 4.0, 9000, 0);
    let armor_before = illusion.armor;

    // Apply a buff with +10 bonus_armor
    illusion.buffs.push(Buff {
        name: "armor_buff".to_string(),
        remaining_ticks: 90,
        tick_effect: None,
        stacking: StackBehavior::RefreshDuration,
        dispel_type: DispelType::BasicDispel,
        status: StatusFlags::default(),
        stat_modifier: Some(StatModifier { bonus_armor: 10.0, ..StatModifier::default() }),
        source_id: 0,
        is_debuff: false,
        pierces_magic_immunity: false,
    });

    let enemy = Unit::from_hero_def(&hero, 1, 1, Vec2::new(9999.0, 0.0));
    let mut sim = Simulation::new(vec![illusion, enemy]);
    sim.step(); // triggers step_buffs

    // Illusion armor should NOT have increased from the flat bonus
    assert!((sim.units[0].armor - armor_before).abs() < 0.01,
        "Illusion armor should ignore flat bonus_armor. Before: {armor_before}, after: {}", sim.units[0].armor);
}

/// Illusions DO benefit from AGI-derived armor (bonus_agi increases armor).
#[test]
fn test_illusion_keeps_agi_armor() {
    use aa2_sim::buff::{Buff, StackBehavior, DispelType, StatusFlags, StatModifier};

    let hero = make_hero();
    let source = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    let mut illusion = Unit::spawn_illusion(&source, 10, Vec2::new(0.0, 0.0), 0.20, 4.0, 9000, 0);
    let armor_before = illusion.armor;

    // Apply a buff with +20 bonus_agi (should add 20 * 0.167 = 3.34 armor)
    illusion.buffs.push(Buff {
        name: "agi_buff".to_string(),
        remaining_ticks: 90,
        tick_effect: None,
        stacking: StackBehavior::RefreshDuration,
        dispel_type: DispelType::BasicDispel,
        status: StatusFlags::default(),
        stat_modifier: Some(StatModifier { bonus_agi: 20.0, ..StatModifier::default() }),
        source_id: 0,
        is_debuff: false,
        pierces_magic_immunity: false,
    });

    let enemy = Unit::from_hero_def(&hero, 1, 1, Vec2::new(9999.0, 0.0));
    let mut sim = Simulation::new(vec![illusion, enemy]);
    sim.step(); // triggers step_buffs

    let expected_increase = 20.0 * 0.167;
    let actual_increase = sim.units[0].armor - armor_before;
    assert!((actual_increase - expected_increase).abs() < 0.01,
        "Illusion armor should increase from AGI. Expected +{expected_increase}, got +{actual_increase}");
}

/// Fury Swipes does NOT work on illusions — no stacking damage.
#[test]
fn test_illusion_cannot_use_fury_swipes() {
    use std::path::Path;
    let hero = aa2_data::load_hero_def(Path::new("../../data/heroes/juggernaut.ron")).unwrap();
    let fs = aa2_data::load_ability_def(Path::new("../../data/abilities/fury_swipes.ron")).unwrap();

    let mut source = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    source.abilities.push(aa2_sim::cast::AbilityState {
        def: fs, cooldown_remaining: 0.0, level: 3, casts: 0, charges: None,
    });

    // Spawn illusion — it should NOT have Fury Swipes (Disabled interaction)
    let illusion = Unit::spawn_illusion(&source, 10, Vec2::new(0.0, 0.0), 0.20, 4.0, 300, 0);
    assert!(illusion.abilities.is_empty(), "Illusion should not retain Disabled abilities like Fury Swipes");

    // Even if we manually give it FS, the illusion check should skip it
    let mut test_illusion = illusion.clone();
    let fs2 = aa2_data::load_ability_def(Path::new("../../data/abilities/fury_swipes.ron")).unwrap();
    test_illusion.abilities.push(aa2_sim::cast::AbilityState {
        def: fs2, cooldown_remaining: 0.0, level: 3, casts: 0, charges: None,
    });

    // Run a sim with this illusion attacking an enemy
    let enemy = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));
    let mut sim = Simulation::with_seed(vec![test_illusion, enemy], 42);

    for _ in 0..200 {
        if sim.is_finished() { break; }
        sim.step();
    }

    // Check that no Fury Swipes stacks were applied (attack_modifier_state should be empty)
    assert!(sim.units[0].attack_modifier_state.is_empty(),
        "Illusion should not build Fury Swipes stacks");
}

/// Essence Shift does NOT work on illusions — no stat steal.
#[test]
fn test_illusion_cannot_use_essence_shift() {
    use std::path::Path;
    let hero = aa2_data::load_hero_def(Path::new("../../data/heroes/juggernaut.ron")).unwrap();
    let es = aa2_data::load_ability_def(Path::new("../../data/abilities/essence_shift.ron")).unwrap();

    let mut test_illusion = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    test_illusion.is_illusion = true;
    test_illusion.illusion_damage_dealt_pct = 0.20;
    test_illusion.illusion_damage_taken_pct = 4.0;
    test_illusion.abilities.push(aa2_sim::cast::AbilityState {
        def: es, cooldown_remaining: 0.0, level: 3, casts: 0, charges: None,
    });

    let enemy = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));
    let enemy_buffs_before = enemy.buffs.len();

    let mut sim = Simulation::with_seed(vec![test_illusion, enemy], 42);

    for _ in 0..200 {
        if sim.is_finished() { break; }
        sim.step();
    }

    // Enemy should NOT have essence_shift_debuff (illusion can't steal stats)
    let es_debuffs = sim.units[1].buffs.iter()
        .filter(|b| b.name == "essence_shift_debuff")
        .count();
    assert_eq!(es_debuffs, 0,
        "Illusion should not apply Essence Shift debuffs, found {}", es_debuffs);
}
