//! God system — passive abilities that modify gameplay.

use rand::Rng;

pub use aa2_data::{God, GodPassive};

use crate::player::PlayerState;

/// Create the Archmage god (for tests/convenience).
pub fn archmage() -> God {
    God {
        name: "Archmage".to_string(),
        passive: GodPassive::Sorcery { trigger_chance: 0.4 },
        description: "Sorcery: 40% chance to upgrade a random ability at shop start. Guaranteed on shop upgrade.".to_string(),
    }
}

/// Create the Paladin god (for tests/convenience).
pub fn paladin() -> God {
    God {
        name: "Paladin".to_string(),
        passive: GodPassive::RadiantShield {
            hp_per_round: 70.0,
            reflection_pct: 0.35,
        },
        description: "Radiant Shield: Buff selected unit with 70×round HP and 35% damage reflection.".to_string(),
    }
}

/// Returns all available gods (hardcoded fallback for tests).
pub fn all_gods() -> Vec<God> {
    vec![archmage(), paladin()]
}

/// Archmage Sorcery: upgrade a random ability by 1 level (free, no pool deduction).
/// Selects from all owned abilities below level 9.
/// Returns the name of the upgraded ability, or None if nothing to upgrade.
pub fn trigger_sorcery(player: &mut PlayerState, rng: &mut impl Rng) -> Option<String> {
    let upgradeable: Vec<String> = player
        .abilities
        .iter()
        .filter(|(_, level)| **level < 9)
        .map(|(name, _)| name.clone())
        .collect();
    if upgradeable.is_empty() {
        return None;
    }
    let idx = rng.gen_range(0..upgradeable.len());
    let name = &upgradeable[idx];
    // SAFETY: we just confirmed the key exists via the iterator above.
    *player.abilities.get_mut(name).unwrap() += 1;
    Some(name.clone())
}

/// Roll the Archmage sorcery chance at shop phase start.
/// Returns the upgraded ability name if triggered.
pub fn maybe_trigger_sorcery(player: &mut PlayerState, rng: &mut impl Rng) -> Option<String> {
    let trigger_chance = match &player.god {
        Some(god) => match &god.passive {
            GodPassive::Sorcery { trigger_chance } => *trigger_chance,
            _ => return None,
        },
        None => return None,
    };
    if rng.gen_range(0.0_f32..1.0) < trigger_chance {
        trigger_sorcery(player, rng)
    } else {
        None
    }
}

/// Check if a player has the Archmage god (for guaranteed trigger on upgrade).
pub fn is_archmage(player: &PlayerState) -> bool {
    matches!(
        player.god.as_ref().map(|g| &g.passive),
        Some(GodPassive::Sorcery { .. })
    )
}

/// Get Paladin buff parameters for a player, if applicable.
/// Returns (hp_per_round, reflection_pct) if the player is a Paladin with a target set.
pub fn paladin_buff_params(player: &PlayerState) -> Option<(f32, f32)> {
    match &player.god {
        Some(god) => match &god.passive {
            GodPassive::RadiantShield {
                hp_per_round,
                reflection_pct,
            } => player.god_buff_target.as_ref().map(|_| (*hp_per_round, *reflection_pct)),
            _ => None,
        },
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::pool::AbilityPool;

    fn test_player_with_abilities() -> PlayerState {
        let mut player = PlayerState::new(0);
        player.abilities.insert("fireball".to_string(), 3);
        player.abilities.insert("heal".to_string(), 5);
        player.abilities.insert("shield".to_string(), 1);
        player
    }

    #[test]
    fn test_sorcery_upgrades_random_ability() {
        let mut player = test_player_with_abilities();
        let total_before: u32 = player.abilities.values().sum();
        let mut rng = rand::thread_rng();

        let result = trigger_sorcery(&mut player, &mut rng);
        assert!(result.is_some());
        let total_after: u32 = player.abilities.values().sum();
        assert_eq!(total_after, total_before + 1);
    }

    #[test]
    fn test_sorcery_skips_max_level() {
        let mut player = PlayerState::new(0);
        player.abilities.insert("fireball".to_string(), 9);
        player.abilities.insert("heal".to_string(), 9);
        let mut rng = rand::thread_rng();

        let result = trigger_sorcery(&mut player, &mut rng);
        assert!(result.is_none());
    }

    #[test]
    fn test_sorcery_no_pool_deduction() {
        let mut player = test_player_with_abilities();
        let mut counts = HashMap::new();
        counts.insert("fireball".to_string(), 10);
        counts.insert("heal".to_string(), 10);
        counts.insert("shield".to_string(), 10);
        let pool = AbilityPool::from_counts(counts.clone());
        let mut rng = rand::thread_rng();

        trigger_sorcery(&mut player, &mut rng);

        // Pool should be unchanged (sorcery doesn't touch the pool)
        assert_eq!(pool.counts["fireball"], 10);
        assert_eq!(pool.counts["heal"], 10);
        assert_eq!(pool.counts["shield"], 10);
    }

    #[test]
    fn test_god_buff_target_selection() {
        let mut player = PlayerState::new(0);
        player.god = Some(paladin());
        player.heroes.push("axe".to_string());
        player.heroes.push("lina".to_string());

        // Set target
        player.god_buff_target = Some("axe".to_string());
        assert_eq!(player.god_buff_target.as_deref(), Some("axe"));

        // Change target
        player.god_buff_target = Some("lina".to_string());
        assert_eq!(player.god_buff_target.as_deref(), Some("lina"));
    }

    #[test]
    fn test_maybe_trigger_sorcery_no_god() {
        let mut player = test_player_with_abilities();
        let mut rng = rand::thread_rng();
        let result = maybe_trigger_sorcery(&mut player, &mut rng);
        assert!(result.is_none());
    }

    #[test]
    fn test_maybe_trigger_sorcery_paladin_noop() {
        let mut player = test_player_with_abilities();
        player.god = Some(paladin());
        let mut rng = rand::thread_rng();
        let result = maybe_trigger_sorcery(&mut player, &mut rng);
        assert!(result.is_none());
    }

    #[test]
    fn test_is_archmage() {
        let mut player = PlayerState::new(0);
        assert!(!is_archmage(&player));
        player.god = Some(archmage());
        assert!(is_archmage(&player));
        player.god = Some(paladin());
        assert!(!is_archmage(&player));
    }

    #[test]
    fn test_paladin_buff_applied_in_combat() {
        use std::collections::HashMap;
        use aa2_data::{Attribute, HeroDef};
        use crate::combat::build_team;

        let hero = HeroDef {
            name: "TestHero".to_string(),
            primary_attribute: Attribute::Strength,
            base_str: 20.0,
            base_agi: 15.0,
            base_int: 10.0,
            str_gain: 2.0,
            agi_gain: 1.0,
            int_gain: 1.0,
            base_attack_time: 1.7,
            attack_range: 150.0,
            attack_point: 0.4,
            move_speed: 300.0,
            turn_rate: 0.6,
            collision_radius: 24.0,
            tier: 1,
            is_melee: true,
            base_damage_min: 30.0,
            base_damage_max: 35.0,
            projectile_speed: None,
        };

        let mut hero_defs = HashMap::new();
        hero_defs.insert("TestHero".to_string(), hero);
        let ability_defs = HashMap::new();

        let mut player = PlayerState::new(0);
        player.god = Some(paladin());
        player.heroes.push("TestHero".to_string());
        player.god_buff_target = Some("TestHero".to_string());
        player.hero_positions.insert("TestHero".to_string(), (1000.0, 500.0));

        let team = build_team(&player, &hero_defs, &ability_defs, 2, 5);
        assert_eq!(team.len(), 1);
        let (_, _, buffs) = &team[0];
        assert_eq!(buffs.len(), 1);
        assert_eq!(buffs[0].name, "Radiant Shield");
        assert!((buffs[0].damage_reflection_pct - 0.35).abs() < 0.001);
        // Bonus STR = 70*5 / 22 ≈ 15.9
        let stat_mod = buffs[0].stat_modifier.as_ref().expect("should have stat modifier");
        assert!((stat_mod.bonus_strength - 350.0 / 22.0).abs() < 0.1);
    }
}
