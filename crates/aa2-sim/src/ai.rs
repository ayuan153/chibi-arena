//! Unit AI: ability casting decision logic.

use aa2_data::{CastBehavior, TargetType, value_at_level};
use crate::buff::active_status;
use crate::unit::Unit;
use crate::vec2::Vec2;

/// Try to find an ability to cast. Returns `(ability_index, target_id, target_pos, cast_behavior)` if found.
///
/// Iterates abilities in order; first ready ability with a valid target wins.
/// An ability is ready if off cooldown, unit has enough mana, and unit is not silenced.
pub fn try_find_cast(
    unit: &Unit,
    units: &[Unit],
) -> Option<(usize, Option<u32>, Option<Vec2>, CastBehavior)> {
    let status = active_status(&unit.buffs);
    if status.silenced || status.stunned || status.hexed {
        return None;
    }

    for (i, ability) in unit.abilities.iter().enumerate() {
        if matches!(ability.def.targeting, TargetType::Passive) {
            continue;
        }
        if !ability.is_ready() {
            continue;
        }
        if unit.mana < value_at_level(&ability.def.mana_cost, ability.level) {
            continue;
        }

        let cast_range = ability.def.cast_range;
        let search_range = match &ability.def.cast_behavior {
            CastBehavior::Lazy => cast_range,
            CastBehavior::Seek => 9999.0,
            CastBehavior::SeekPlus(extra) => cast_range + extra,
        };
        let behavior = ability.def.cast_behavior.clone();
        match &ability.def.targeting {
            TargetType::SingleEnemy | TargetType::PointAoE => {
                if let Some((id, pos)) = closest_living_enemy(unit, units, search_range) {
                    return Some((i, Some(id), Some(pos), behavior));
                }
            }
            TargetType::SingleAlly => {
                if let Some((id, pos)) = closest_living_ally(unit, units, search_range) {
                    return Some((i, Some(id), Some(pos), behavior));
                }
            }
            TargetType::SingleAllyHG => {
                let allies_in_range: Vec<_> = units.iter()
                    .filter(|u| u.id != unit.id && u.team == unit.team && u.is_alive())
                    .filter(|u| unit.position.distance(u.position) <= search_range)
                    .collect();

                if allies_in_range.is_empty() {
                    return Some((i, Some(unit.id), Some(unit.position), behavior));
                }

                // First cast (fresh cooldown): highest y-axis ally
                // Subsequent casts: furthest ally from caster
                let target = if ability.cooldown_remaining == 0.0 && ability.casts == 0 {
                    allies_in_range.iter()
                        .max_by(|a, b| a.position.y.partial_cmp(&b.position.y).unwrap())
                } else {
                    allies_in_range.iter()
                        .max_by(|a, b| {
                            let da = unit.position.distance(a.position);
                            let db = unit.position.distance(b.position);
                            da.partial_cmp(&db).unwrap()
                        })
                };

                if let Some(ally) = target {
                    return Some((i, Some(ally.id), Some(ally.position), behavior));
                }
                return Some((i, Some(unit.id), Some(unit.position), behavior));
            }
            TargetType::NoTarget => {
                return Some((i, None, None, behavior));
            }
            TargetType::Passive => unreachable!(),
        }
    }
    None
}

/// Find the closest living enemy within range.
fn closest_living_enemy(unit: &Unit, units: &[Unit], range: f32) -> Option<(u32, Vec2)> {
    units
        .iter()
        .filter(|u| u.team != unit.team && u.is_alive())
        .filter(|u| !active_status(&u.buffs).invulnerable)
        .filter(|u| !active_status(&u.buffs).magic_immune)
        .filter_map(|u| {
            let d = unit.position.distance(u.position);
            (d <= range).then_some((d, u.id, u.position))
        })
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        .map(|(_, id, pos)| (id, pos))
}

/// Find the closest living ally (not self) within range.
fn closest_living_ally(unit: &Unit, units: &[Unit], range: f32) -> Option<(u32, Vec2)> {
    units
        .iter()
        .filter(|u| u.team == unit.team && u.is_alive() && u.id != unit.id)
        .filter_map(|u| {
            let d = unit.position.distance(u.position);
            (d <= range).then_some((d, u.id, u.position))
        })
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        .map(|(_, id, pos)| (id, pos))
}
