//! Hero draft logic — determines when drafts occur and what choices are offered.

use aa2_data::{Attribute, HeroDef};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

/// Rounds on which a hero draft occurs.
const DRAFT_ROUNDS: &[u32] = &[1, 3, 6, 9, 12];

/// Tier assigned to each draft round.
const DRAFT_TIERS: &[(u32, u8)] = &[(1, 0), (3, 1), (6, 2), (9, 3), (12, 4)];

/// Returns true if the given round is a hero draft round.
pub fn is_draft_round(round: u32) -> bool {
    DRAFT_ROUNDS.contains(&round)
}

/// Returns the tier for a draft round, or None if not a draft round.
pub fn tier_for_draft_round(round: u32) -> Option<u8> {
    DRAFT_TIERS.iter().find(|(r, _)| *r == round).map(|(_, t)| *t)
}

/// State of an active hero draft for one player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftState {
    /// Current choices: [STR, AGI, INT] hero names.
    pub choices: [Option<String>; 3],
    /// Tier for this draft round.
    pub round_tier: u8,
}

/// Generate draft choices: 1 STR, 1 AGI, 1 INT from the given tier only.
pub fn generate_draft_choices(
    available_heroes: &[&HeroDef],
    tier: u8,
    rng: &mut impl rand::Rng,
) -> [Option<String>; 3] {
    let filtered: Vec<&&HeroDef> = available_heroes
        .iter()
        .filter(|h| h.tier == tier)
        .collect();
    pick_one_per_attribute(&filtered, rng)
}

/// Generate reroll choices: 1 STR, 1 AGI, 1 INT from ALL available heroes.
pub fn generate_reroll_choices(
    available_heroes: &[&HeroDef],
    rng: &mut impl rand::Rng,
) -> [Option<String>; 3] {
    let refs: Vec<&&HeroDef> = available_heroes.iter().collect();
    pick_one_per_attribute(&refs, rng)
}

fn pick_one_per_attribute(heroes: &[&&HeroDef], rng: &mut impl rand::Rng) -> [Option<String>; 3] {
    let mut str_heroes: Vec<&str> = heroes
        .iter()
        .filter(|h| matches!(h.primary_attribute, Attribute::Strength))
        .map(|h| h.name.as_str())
        .collect();
    let mut agi_heroes: Vec<&str> = heroes
        .iter()
        .filter(|h| matches!(h.primary_attribute, Attribute::Agility))
        .map(|h| h.name.as_str())
        .collect();
    let mut int_heroes: Vec<&str> = heroes
        .iter()
        .filter(|h| matches!(h.primary_attribute, Attribute::Intelligence))
        .map(|h| h.name.as_str())
        .collect();

    str_heroes.shuffle(rng);
    agi_heroes.shuffle(rng);
    int_heroes.shuffle(rng);

    [
        str_heroes.first().map(|s| s.to_string()),
        agi_heroes.first().map(|s| s.to_string()),
        int_heroes.first().map(|s| s.to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hero(name: &str, attr: Attribute, tier: u8) -> HeroDef {
        HeroDef {
            name: name.to_string(),
            primary_attribute: attr,
            base_str: 20.0,
            base_agi: 20.0,
            base_int: 20.0,
            str_gain: 2.0,
            agi_gain: 2.0,
            int_gain: 2.0,
            base_attack_time: 1.7,
            attack_range: 150.0,
            attack_point: 0.3,
            move_speed: 300.0,
            turn_rate: 0.6,
            collision_radius: 24.0,
            tier,
            is_melee: true,
            base_damage_min: 50.0,
            base_damage_max: 60.0,
            projectile_speed: None,
        }
    }

    #[test]
    fn test_is_draft_round() {
        let expected: Vec<bool> = (1..=15)
            .map(|r| [1, 3, 6, 9, 12].contains(&r))
            .collect();
        let actual: Vec<bool> = (1..=15).map(is_draft_round).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_tier_for_draft_round() {
        assert_eq!(tier_for_draft_round(1), Some(0));
        assert_eq!(tier_for_draft_round(3), Some(1));
        assert_eq!(tier_for_draft_round(6), Some(2));
        assert_eq!(tier_for_draft_round(9), Some(3));
        assert_eq!(tier_for_draft_round(12), Some(4));
        assert_eq!(tier_for_draft_round(2), None);
        assert_eq!(tier_for_draft_round(5), None);
    }

    #[test]
    fn test_generate_draft_choices_correct_tier() {
        let heroes = [
            make_hero("str_t0", Attribute::Strength, 0),
            make_hero("agi_t0", Attribute::Agility, 0),
            make_hero("int_t0", Attribute::Intelligence, 0),
            make_hero("str_t1", Attribute::Strength, 1),
            make_hero("agi_t1", Attribute::Agility, 1),
            make_hero("int_t1", Attribute::Intelligence, 1),
        ];
        let refs: Vec<&HeroDef> = heroes.iter().collect();
        let mut rng = rand::thread_rng();

        let choices = generate_draft_choices(&refs, 0, &mut rng);
        assert_eq!(choices[0].as_deref(), Some("str_t0"));
        assert_eq!(choices[1].as_deref(), Some("agi_t0"));
        assert_eq!(choices[2].as_deref(), Some("int_t0"));
    }

    #[test]
    fn test_generate_reroll_choices_any_tier() {
        let heroes = [
            make_hero("str_t0", Attribute::Strength, 0),
            make_hero("agi_t1", Attribute::Agility, 1),
            make_hero("int_t2", Attribute::Intelligence, 2),
        ];
        let refs: Vec<&HeroDef> = heroes.iter().collect();
        let mut rng = rand::thread_rng();

        let choices = generate_reroll_choices(&refs, &mut rng);
        assert_eq!(choices[0].as_deref(), Some("str_t0"));
        assert_eq!(choices[1].as_deref(), Some("agi_t1"));
        assert_eq!(choices[2].as_deref(), Some("int_t2"));
    }

    #[test]
    fn test_choices_exclude_owned() {
        let heroes = [
            make_hero("str_a", Attribute::Strength, 0),
            make_hero("str_b", Attribute::Strength, 0),
            make_hero("agi_a", Attribute::Agility, 0),
            make_hero("int_a", Attribute::Intelligence, 0),
        ];
        // Simulate filtering out owned hero "str_a"
        let available: Vec<&HeroDef> = heroes.iter().filter(|h| h.name != "str_a").collect();
        let mut rng = rand::thread_rng();

        let choices = generate_draft_choices(&available, 0, &mut rng);
        // STR choice must be str_b since str_a is excluded
        assert_eq!(choices[0].as_deref(), Some("str_b"));
    }
}
