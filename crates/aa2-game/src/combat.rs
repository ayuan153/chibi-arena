//! Combat integration: bridges game state to aa2-sim simulation.

use std::collections::HashMap;

use aa2_data::{AbilityDef, HeroDef, UnitConfig};
use aa2_sim::buff::Buff;
use aa2_sim::unit::Unit;
use aa2_sim::vec2::Vec2;
use aa2_sim::Simulation;

use crate::god;
use crate::matchup::Matchup;
use crate::player::PlayerState;

/// Maximum ticks before combat times out (50s * 30 ticks/s).
pub const COMBAT_MAX_TICKS: u32 = 1500;

/// Result of a single combat matchup.
#[derive(Debug, Clone)]
pub struct CombatResult {
    /// The matchup that was fought.
    pub matchup: Matchup,
    /// Winner player ID, or None for draw/timeout.
    pub winner: Option<u8>,
    /// Surviving units on team A.
    pub survivors_a: u32,
    /// Surviving units on team B.
    pub survivors_b: u32,
}

/// Build UnitConfigs for a player's team.
///
/// `hero_level` scales hero attributes via stat gain.
/// If `mirror` is true, positions are flipped for the "top" team.
/// Returns (UnitConfig, position, pre-combat buffs) for each hero.
pub fn build_team(
    player: &PlayerState,
    hero_defs: &HashMap<String, HeroDef>,
    ability_defs: &HashMap<String, AbilityDef>,
    hero_level: u8,
    round: u32,
) -> Vec<(UnitConfig, Vec2, Vec<Buff>)> {
    let mut team = Vec::new();

    // Determine Paladin buff params if applicable
    let paladin_params = god::paladin_buff_params(player);

    for hero_name in &player.heroes {
        let Some(hero_def) = hero_defs.get(hero_name) else { continue };

        let mut config = UnitConfig::new(hero_def.clone()).with_level(hero_level);

        // Add equipped abilities
        if let Some(equipped) = player.equipped.get(hero_name) {
            for ability_name in equipped {
                let Some(ability_def) = ability_defs.get(ability_name) else { continue };
                let level = player.abilities.get(ability_name).copied().unwrap_or(1) as u8;
                config.abilities.push((ability_def.clone(), level));
            }
        }

        // Get position
        let pos = player
            .hero_positions
            .get(hero_name)
            .copied()
            .unwrap_or((1000.0, 500.0));

        // Paladin: apply Radiant Shield buffs to the targeted hero
        let mut buffs = Vec::new();
        if let Some((hp_per_round, reflection_pct)) = paladin_params
            && player.god_buff_target.as_deref() == Some(hero_name.as_str())
        {
            let bonus_hp = hp_per_round * round as f32;
            let mut hp_buff = aa2_sim::buff::damage_reflection("Radiant Shield", reflection_pct);
            hp_buff.stat_modifier = Some(aa2_sim::buff::StatModifier {
                bonus_strength: bonus_hp / 22.0, // STR → HP conversion: 22 HP per STR
                ..Default::default()
            });
            buffs.push(hp_buff);
        }

        team.push((config, Vec2::new(pos.0, pos.1), buffs));
    }

    team
}

/// Run a single combat between two teams.
///
/// Returns (winner_team: Option<0|1>, survivors_a, survivors_b).
pub fn run_combat(
    team_a: &[(UnitConfig, Vec2, Vec<Buff>)],
    team_b: &[(UnitConfig, Vec2, Vec<Buff>)],
    seed: u32,
) -> (Option<u8>, u32, u32) {
    if team_a.is_empty() && team_b.is_empty() {
        return (None, 0, 0);
    }
    if team_a.is_empty() {
        return (Some(1), 0, team_b.len() as u32);
    }
    if team_b.is_empty() {
        return (Some(0), team_a.len() as u32, 0);
    }

    let mut units = Vec::new();
    let mut id = 0u32;

    for (config, pos, buffs) in team_a {
        let mut unit = Unit::from_config(config, id, 0, *pos);
        for buff in buffs {
            aa2_sim::buff::apply_buff(&mut unit.buffs, buff.clone());
        }
        units.push(unit);
        id += 1;
    }
    for (config, pos, buffs) in team_b {
        let mirrored = crate::player::mirror_position(pos.x, pos.y);
        let mut unit = Unit::from_config(config, id, 1, Vec2::new(mirrored.0, mirrored.1));
        for buff in buffs {
            aa2_sim::buff::apply_buff(&mut unit.buffs, buff.clone());
        }
        units.push(unit);
        id += 1;
    }

    let mut sim = Simulation::with_seed(units, seed);

    for _ in 0..COMBAT_MAX_TICKS {
        if sim.is_finished() {
            break;
        }
        sim.step();
    }

    let survivors_a = sim.units.iter().filter(|u| u.team == 0 && u.is_alive()).count() as u32;
    let survivors_b = sim.units.iter().filter(|u| u.team == 1 && u.is_alive()).count() as u32;

    let winner = if sim.is_finished() {
        sim.winner()
    } else {
        // Timeout: team with more survivors wins
        match survivors_a.cmp(&survivors_b) {
            std::cmp::Ordering::Greater => Some(0),
            std::cmp::Ordering::Less => Some(1),
            std::cmp::Ordering::Equal => None,
        }
    };

    (winner, survivors_a, survivors_b)
}

/// Run all matchups for a round and return results.
///
/// Does NOT apply damage — caller is responsible for that.
pub fn run_all_matchups(
    matchups: &[Matchup],
    players: &[PlayerState],
    hero_defs: &HashMap<String, HeroDef>,
    ability_defs: &HashMap<String, AbilityDef>,
    hero_level: u8,
    round: u32,
    base_seed: u32,
) -> Vec<CombatResult> {
    let mut results = Vec::new();

    for (i, matchup) in matchups.iter().enumerate() {
        let seed = base_seed.wrapping_add(i as u32);

        let player_a = players.get(matchup.player_a as usize);
        let team_a = player_a
            .map(|p| build_team(p, hero_defs, ability_defs, hero_level, round))
            .unwrap_or_default();

        let team_b_source = if matchup.ghost {
            matchup.ghost_source.and_then(|id| players.get(id as usize))
        } else {
            players.get(matchup.player_b as usize)
        };
        let team_b = team_b_source
            .map(|p| build_team(p, hero_defs, ability_defs, hero_level, round))
            .unwrap_or_default();

        let (winner_team, survivors_a, survivors_b) = run_combat(&team_a, &team_b, seed);

        let winner = match winner_team {
            Some(0) => Some(matchup.player_a),
            Some(1) => {
                if matchup.ghost {
                    matchup.ghost_source
                } else {
                    Some(matchup.player_b)
                }
            }
            _ => None,
        };

        results.push(CombatResult {
            matchup: matchup.clone(),
            winner,
            survivors_a,
            survivors_b,
        });
    }

    results
}
