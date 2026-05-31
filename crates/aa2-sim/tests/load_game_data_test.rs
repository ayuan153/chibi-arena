/// Tests for `load_game_data` hot-reload semantics:
/// (i) picks up an edited RON value, (ii) rejects invalid RON.
///
/// Uses a temp directory with minimal data files — no external deps.
#[test]
fn load_game_data_picks_up_edits_and_rejects_invalid() {
    use std::fs;

    let tmp = std::env::temp_dir().join(format!(
        "aa2_test_load_game_data_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(tmp.join("heroes")).unwrap();
    fs::create_dir_all(tmp.join("abilities")).unwrap();
    fs::create_dir_all(tmp.join("gods")).unwrap();

    // Minimal hero
    fs::write(
        tmp.join("heroes/test_hero.ron"),
        r#"HeroDef(
            name: "TestHero",
            primary_attribute: Strength,
            base_str: 20.0, base_agi: 15.0, base_int: 10.0,
            str_gain: 2.0, agi_gain: 1.0, int_gain: 1.0,
            base_attack_time: 1.7, attack_range: 150.0, attack_point: 0.4,
            move_speed: 300.0, turn_rate: 0.6, collision_radius: 24.0,
            tier: 1, is_melee: true,
            base_damage_min: 30.0, base_damage_max: 35.0,
        )"#,
    ).unwrap();

    // Minimal ability
    fs::write(
        tmp.join("abilities/test_ability.ron"),
        r#"AbilityDef(
            name: "TestAbility",
            cooldown: [10.0],
            mana_cost: [50.0],
            cast_point: 0.3,
            targeting: NoTarget,
            description: "test",
            effect_specs: Some([
                EffectSpec(
                    trigger: OnCast,
                    targeting: Caster,
                    delivery: Instant,
                    payload: [Heal(base: [20.0])],
                ),
            ]),
        )"#,
    ).unwrap();

    // Minimal god
    fs::write(
        tmp.join("gods/test_god.ron"),
        r#"God(name: "TestGod", description: "test", passive: Sorcery(trigger_chance: 0.5))"#,
    ).unwrap();

    // (i) Load succeeds and picks up values
    let data = aa2_data::load_game_data(&tmp).unwrap();
    assert_eq!(data.heroes.len(), 1);
    assert_eq!(data.heroes["TestHero"].base_str, 20.0);
    assert_eq!(data.abilities.len(), 1);
    assert_eq!(data.gods.len(), 1);

    // Edit the hero file
    fs::write(
        tmp.join("heroes/test_hero.ron"),
        r#"HeroDef(
            name: "TestHero",
            primary_attribute: Strength,
            base_str: 99.0, base_agi: 15.0, base_int: 10.0,
            str_gain: 2.0, agi_gain: 1.0, int_gain: 1.0,
            base_attack_time: 1.7, attack_range: 150.0, attack_point: 0.4,
            move_speed: 300.0, turn_rate: 0.6, collision_radius: 24.0,
            tier: 1, is_melee: true,
            base_damage_min: 30.0, base_damage_max: 35.0,
        )"#,
    ).unwrap();

    let data2 = aa2_data::load_game_data(&tmp).unwrap();
    assert_eq!(data2.heroes["TestHero"].base_str, 99.0);

    // (ii) Corrupt the ability file — load should fail
    fs::write(tmp.join("abilities/test_ability.ron"), "INVALID RON {{{{").unwrap();
    let result = aa2_data::load_game_data(&tmp);
    assert!(result.is_err());

    // Validation: write a valid but structurally bad ability (empty payload)
    fs::write(
        tmp.join("abilities/test_ability.ron"),
        r#"AbilityDef(
            name: "BadAbility",
            cooldown: [10.0, 12.0],
            mana_cost: [50.0],
            cast_point: 0.3,
            targeting: NoTarget,
            description: "bad",
            effect_specs: Some([
                EffectSpec(
                    trigger: OnCast,
                    targeting: Caster,
                    delivery: Instant,
                    payload: [],
                ),
            ]),
        )"#,
    ).unwrap();

    let data3 = aa2_data::load_game_data(&tmp).unwrap();
    let bad_def = &data3.abilities["BadAbility"];
    let validation = aa2_data::validate_ability_def(bad_def);
    assert!(validation.is_err());
    let problems = validation.unwrap_err();
    assert!(problems.iter().any(|p| p.contains("empty payload")));

    // Cleanup
    let _ = fs::remove_dir_all(&tmp);
}
