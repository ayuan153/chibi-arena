//! Attack modifier system: passive abilities that trigger on auto-attack hits.
//!
//! Handles Fury Swipes (stacking damage), Chaos Strike (PRD crit + lifesteal),
//! and Essence Shift (stat steal). Integrates into the damage pipeline between
//! base damage roll and armor reduction.

use aa2_data::{Payload, Trigger, value_at_level};
use crate::buff::{Buff, StackBehavior, DispelType, StatusFlags, StatModifier};
use crate::unit::Unit;
use crate::TICK_RATE;

/// PRD (Pseudo-Random Distribution) state for a proc-based ability.
#[derive(Debug, Clone)]
pub struct PrdState {
    /// The C constant for this proc chance.
    pub c_value: f32,
    /// Current accumulated chance (increases each non-proc).
    pub current_chance: f32,
}

impl PrdState {
    /// Create a new PRD state for the given nominal proc probability.
    pub fn new(nominal_chance: f32) -> Self {
        let c = prd_c_from_p(nominal_chance);
        Self { c_value: c, current_chance: c }
    }

    /// Roll the PRD using the given RNG. Returns true if proc'd.
    pub(crate) fn roll(&mut self, rng: &mut crate::Rng) -> bool {
        if rng.chance(self.current_chance) {
            self.current_chance = self.c_value;
            true
        } else {
            self.current_chance += self.c_value;
            false
        }
    }
}

/// Per-target attack modifier state (e.g. Fury Swipes stacks).
#[derive(Debug, Clone)]
pub struct TargetModifierState {
    /// Current Fury Swipes stacks on this target.
    pub fury_swipes_stacks: u32,
    /// Tick when stacks expire.
    pub fury_swipes_expiry_tick: u32,
}

/// Result of processing attack modifiers before armor.
#[derive(Debug, Clone)]
pub struct AttackResult {
    /// Final damage after crit and post-crit bonus (before armor).
    pub damage: f32,
    /// Crit multiplier applied (1.0 = no crit).
    pub crit_multiplier: f32,
    /// Lifesteal percentage from Chaos Strike (0.0 if no crit).
    pub lifesteal_pct: f32,
    /// Post-crit flat bonus (Fury Swipes).
    pub post_crit_bonus: f32,
    /// Bonus magical damage from Glaives of Wisdom (before magic resist).
    pub bonus_magical_damage: f32,
    /// Whether Glaives fired this attack (for bounce logic).
    pub glaives_active: bool,
}

/// Process attack modifiers when an attack lands. Called BEFORE armor reduction.
///
/// Pipeline: base_damage * crit_multiplier + post_crit_bonus (Fury Swipes).
pub(crate) fn process_attack_modifiers(
    attacker: &mut Unit,
    target_id: u32,
    base_damage: f32,
    tick: u32,
    rng: &mut crate::Rng,
    ally_chaos_strike: Option<(f32, f32, f32, f32)>, // (proc_chance, crit_min, crit_max, lifesteal)
    target_is_magic_immune: bool,
) -> AttackResult {
    let mut crit_multiplier = 1.0_f32;
    let mut lifesteal_pct = 0.0_f32;
    let mut post_crit_bonus = 0.0_f32;
    let mut bonus_magical_damage = 0.0_f32;
    let mut glaives_active = false;

    let ability_count = attacker.abilities.len();
    for ai in 0..ability_count {
        let level = attacker.abilities[ai].level;
        if level == 0 { continue; }
        // Composable path: abilities with effect_specs use the generic OnAttack resolver
        if attacker.abilities[ai].def.effect_specs.is_some() {
            let specs = attacker.abilities[ai].def.effect_specs.clone().unwrap();
            // Illusion check for composable specs
            if attacker.is_illusion {
                let any_full = specs.iter().any(|s| s.illusion_interaction == aa2_data::IllusionInteraction::Full);
                if !any_full { continue; }
            }
            for spec in &specs {
                if spec.trigger != Trigger::OnAttack { continue; }
                // Mana cost gate: check and spend once per spec before any payloads
                if !spec.mana_cost.is_empty() {
                    // Magic-immune target blocks the entire spec (Glaives behavior)
                    if target_is_magic_immune { continue; }
                    let cost = value_at_level(&spec.mana_cost, level);
                    if attacker.mana < cost {
                        continue; // Can't afford — skip entire spec
                    }
                    attacker.mana -= cost;
                }
                for payload in &spec.payload {
                    match payload {
                        Payload::StackingBonusDamage { damage_per_stack, stack_duration } => {
                            let duration_secs = value_at_level(stack_duration, level);
                            let stacks = get_fury_swipes_stacks(&attacker.attack_modifier_state, target_id, tick);
                            let dmg_per = value_at_level(damage_per_stack, level);
                            post_crit_bonus += stacks as f32 * dmg_per;
                            let expiry = tick + (duration_secs * TICK_RATE) as u32;
                            set_fury_swipes_stacks(&mut attacker.attack_modifier_state, target_id, stacks + 1, expiry);
                        }
                        Payload::Crit { proc_chance, crit_min, crit_max } => {
                            let chance = value_at_level(proc_chance, level);
                            let prd = get_or_create_prd(&mut attacker.prd_states, ai, chance);
                            if prd.roll(rng) {
                                let min_c = value_at_level(crit_min, level);
                                let max_c = value_at_level(crit_max, level);
                                let crit = rng.range_f32(min_c, max_c) / 100.0;
                                if crit > crit_multiplier {
                                    crit_multiplier = crit;
                                    // Apply lifesteal from co-located Lifesteal payload
                                    for ls_payload in &spec.payload {
                                        if let Payload::Lifesteal { pct } = ls_payload {
                                            lifesteal_pct = value_at_level(pct, level) / 100.0;
                                        }
                                    }
                                }
                            }
                        }
                        Payload::IntScaledDamage { factor }
                            if !target_is_magic_immune => {
                            // INT-scaled bonus magical damage (blocked by magic immunity)
                            let total_int = crate::unit::effective_stat(attacker.base_int, 0.0);
                            let f = value_at_level(factor, level);
                            bonus_magical_damage += total_int * f;
                            glaives_active = true;
                        }
                        Payload::AttackBounce { .. } => {
                            // Bounce is handled post-damage; just mark glaives_active
                            // (already set by IntScaledDamage above if mana was spent)
                        }
                        _ => {}
                    }
                }
            }
            continue;
        }
    }

    // Gaben aura: ally Chaos Strike
    if crit_multiplier <= 1.0
        && let Some((chance, cmin, cmax, ls)) = ally_chaos_strike
    {
        // Use a separate PRD roll inline (no persistent state for aura)
        if rng.chance(chance) {
            let crit = rng.range_f32(cmin, cmax) / 100.0;
            if crit > crit_multiplier {
                crit_multiplier = crit;
                lifesteal_pct = ls / 100.0;
            }
        }
    }

    let damage = base_damage * crit_multiplier + post_crit_bonus;
    AttackResult { damage, crit_multiplier, lifesteal_pct, post_crit_bonus, bonus_magical_damage, glaives_active }
}

/// Apply post-hit effects: Essence Shift stat steal, Chaos Strike lifesteal.
pub fn post_attack_effects(
    attacker: &mut Unit,
    target: &mut Unit,
    damage_dealt: f32,
    lifesteal_pct: f32,
    _tick: u32,
) {
    // Chaos Strike lifesteal
    if lifesteal_pct > 0.0 {
        let heal = damage_dealt * lifesteal_pct;
        attacker.hp = (attacker.hp + heal).min(attacker.max_hp);
    }

    // Composable OnAttack post-damage: apply buff payloads (armor reduction, etc.)
    let ability_count2 = attacker.abilities.len();
    for ai in 0..ability_count2 {
        let level = attacker.abilities[ai].level;
        if level == 0 { continue; }
        if attacker.abilities[ai].def.effect_specs.is_some() {
            let specs = attacker.abilities[ai].def.effect_specs.clone().unwrap();
            if attacker.is_illusion {
                let any_full = specs.iter().any(|s| s.illusion_interaction == aa2_data::IllusionInteraction::Full);
                if !any_full { continue; }
            }
            for spec in &specs {
                if spec.trigger != Trigger::OnAttack { continue; }
                // Mana cost gate (post-damage): check remaining mana matches pre-damage gate
                if !spec.mana_cost.is_empty() {
                    let cost = value_at_level(&spec.mana_cost, level);
                    if attacker.mana < cost { continue; }
                }
                for payload in &spec.payload {
                    if let Payload::ApplyBuff(def) = payload {
                        // Skip zero-effect buffs (e.g. armor reduction = 0 at lower levels)
                        if let Some(ref sm) = def.stat_modifier {
                            let resolved = sm.resolve(level);
                            if resolved.bonus_armor == 0.0
                                && resolved.bonus_attack_speed == 0.0
                                && resolved.bonus_move_speed == 0.0
                                && resolved.bonus_damage == 0.0
                                && resolved.bonus_magic_resistance == 0.0
                                && resolved.bonus_hp_regen == 0.0
                                && resolved.bonus_strength == 0.0
                                && resolved.bonus_agi == 0.0
                                && resolved.bonus_int == 0.0
                                && resolved.bonus_strength == 0.0
                            {
                                continue;
                            }
                        }
                        let buff = crate::effect_spec::buff_from_def(def, level, attacker.id);
                        target.buffs.push(buff);
                    }
                    if let Payload::StatSteal { str_steal, agi_steal, int_steal, agi_gain, duration } = payload {
                        let dur_secs = value_at_level(duration, level);
                        let dur_ticks = (dur_secs * TICK_RATE) as u32;
                        let s_steal = value_at_level(str_steal, level);
                        let a_steal = value_at_level(agi_steal, level);
                        let i_steal = value_at_level(int_steal, level);
                        let a_gain = value_at_level(agi_gain, level);

                        // Debuff on target: lose stats (pierces magic immunity)
                        let debuff = Buff {
                            name: "essence_shift_debuff".to_string(),
                            remaining_ticks: dur_ticks,
                            tick_effect: None,
                            stacking: StackBehavior::Independent,
                            dispel_type: DispelType::Undispellable,
                            status: StatusFlags::default(),
                            stat_modifier: Some(StatModifier {
                                bonus_strength: -s_steal,
                                bonus_agi: -a_steal,
                                bonus_int: -i_steal,
                                ..StatModifier::default()
                            }),
                            source_id: attacker.id,
                            is_debuff: true,
                            pierces_magic_immunity: true,
                            damage_reflection_pct: 0.0,
                            on_death: None,
                        };
                        target.buffs.push(debuff);

                        // Buff on attacker: gain AGI
                        let buff = Buff {
                            name: "essence_shift_buff".to_string(),
                            remaining_ticks: dur_ticks,
                            tick_effect: None,
                            stacking: StackBehavior::Independent,
                            dispel_type: DispelType::Undispellable,
                            status: StatusFlags::default(),
                            stat_modifier: Some(StatModifier {
                                bonus_agi: a_gain,
                                ..StatModifier::default()
                            }),
                            source_id: attacker.id,
                            is_debuff: false,
                            pierces_magic_immunity: false,
                            damage_reflection_pct: 0.0,
                            on_death: None,
                        };
                        attacker.buffs.push(buff);
                    }
                }
            }
        }
    }
}

/// Find Chaos Strike aura parameters from allies within 1200 range.
/// Returns the best (highest proc chance) ally Chaos Strike params if any.
pub fn find_ally_chaos_strike_aura(
    attacker: &Unit,
    units: &[Unit],
) -> Option<(f32, f32, f32, f32)> {
    let mut best: Option<(f32, f32, f32, f32)> = None;
    for ally in units {
        if ally.id == attacker.id || ally.team != attacker.team || !ally.is_alive() {
            continue;
        }
        if attacker.position.distance(ally.position) > 1200.0 {
            continue;
        }
        for ability in &ally.abilities {
            if ability.level == 0 { continue; }
            if let Some(ref specs) = ability.def.effect_specs {
                for spec in specs {
                    if spec.trigger != Trigger::OnAttack { continue; }
                    let mut crit_params: Option<(f32, f32, f32)> = None;
                    let mut ls_val = 0.0_f32;
                    for payload in &spec.payload {
                        match payload {
                            Payload::Crit { proc_chance, crit_min, crit_max } => {
                                let chance = value_at_level(proc_chance, ability.level) * 0.5;
                                let cmin = value_at_level(crit_min, ability.level);
                                let cmax = value_at_level(crit_max, ability.level);
                                crit_params = Some((chance, cmin, cmax));
                            }
                            Payload::Lifesteal { pct } => {
                                ls_val = value_at_level(pct, ability.level);
                            }
                            _ => {}
                        }
                    }
                    if let Some((chance, cmin, cmax)) = crit_params
                        && (best.is_none() || chance > best.unwrap().0)
                    {
                        best = Some((chance, cmin, cmax, ls_val));
                    }
                }
            }
        }
    }
    best
}

/// Get or create a PRD state for the given ability index.
fn get_or_create_prd(prd_states: &mut Vec<(usize, PrdState)>, ability_index: usize, nominal_chance: f32) -> &mut PrdState {
    if let Some(pos) = prd_states.iter().position(|(idx, _)| *idx == ability_index) {
        &mut prd_states[pos].1
    } else {
        prd_states.push((ability_index, PrdState::new(nominal_chance)));
        let last = prd_states.len() - 1;
        &mut prd_states[last].1
    }
}

/// Get current Fury Swipes stacks on a target (0 if expired or not found).
/// Fury Swipes Gaben: every 2 attacks on an enemy, apply 1 stack to all other enemies.
/// Call after post_attack_effects. Only triggers at level 9 (Gaben).
pub fn fury_swipes_gaben_spread(
    attacker: &mut Unit,
    target_id: u32,
    other_enemy_ids: &[u32],
    tick: u32,
) {
    // Check if attacker has Gaben Fury Swipes (level 9)
    let has_gaben_fs = attacker.abilities.iter().any(|a| {
        a.level >= 9 && a.def.effect_specs.as_ref().is_some_and(|specs| {
            specs.iter().any(|s| s.trigger == Trigger::OnAttack && s.payload.iter().any(|p| matches!(p, Payload::StackingBonusDamage { .. })))
        })
    });
    if !has_gaben_fs { return; }

    // Get current stacks on the target (already incremented this attack)
    let stacks = get_fury_swipes_stacks(&attacker.attack_modifier_state, target_id, tick);

    // Every 2 attacks (check if stack count is even)
    if stacks > 0 && stacks.is_multiple_of(2) {
        let dur_ticks = attacker.abilities.iter()
            .filter_map(|a| {
                if a.level < 9 { return None; }
                a.def.effect_specs.as_ref().and_then(|specs| {
                    specs.iter().find_map(|s| {
                        if s.trigger != Trigger::OnAttack { return None; }
                        s.payload.iter().find_map(|p| {
                            if let Payload::StackingBonusDamage { stack_duration, .. } = p {
                                Some((value_at_level(stack_duration, a.level) * TICK_RATE) as u32)
                            } else { None }
                        })
                    })
                })
            })
            .next()
            .unwrap_or(600);

        let expiry = tick + dur_ticks;
        for &enemy_id in other_enemy_ids {
            if enemy_id == target_id { continue; }
            let existing = get_fury_swipes_stacks(&attacker.attack_modifier_state, enemy_id, tick);
            set_fury_swipes_stacks(&mut attacker.attack_modifier_state, enemy_id, existing + 1, expiry);
        }
    }
}

fn get_fury_swipes_stacks(state: &[(u32, TargetModifierState)], target_id: u32, tick: u32) -> u32 {
    state.iter()
        .find(|(id, _)| *id == target_id)
        .map(|(_, s)| if tick < s.fury_swipes_expiry_tick { s.fury_swipes_stacks } else { 0 })
        .unwrap_or(0)
}

/// Set Fury Swipes stacks for a target.
fn set_fury_swipes_stacks(state: &mut Vec<(u32, TargetModifierState)>, target_id: u32, stacks: u32, expiry: u32) {
    if let Some(entry) = state.iter_mut().find(|(id, _)| *id == target_id) {
        entry.1.fury_swipes_stacks = stacks;
        entry.1.fury_swipes_expiry_tick = expiry;
    } else {
        state.push((target_id, TargetModifierState {
            fury_swipes_stacks: stacks,
            fury_swipes_expiry_tick: expiry,
        }));
    }
}

/// Compute PRD C value from nominal probability P using binary search.
pub fn prd_c_from_p(p: f32) -> f32 {
    if p <= 0.0 { return 0.0; }
    if p >= 1.0 { return 1.0; }
    let mut c_low = 0.0_f32;
    let mut c_high = p;
    for _ in 0..30 {
        let c_mid = (c_low + c_high) / 2.0;
        let p_from_c = prd_p_from_c(c_mid);
        if p_from_c < p {
            c_low = c_mid;
        } else {
            c_high = c_mid;
        }
    }
    (c_low + c_high) / 2.0
}

/// Compute actual average proc probability from a given C value.
/// P = 1 / E[N] where E[N] = sum over n of n * P(proc on exactly attack n).
fn prd_p_from_c(c: f32) -> f32 {
    if c <= 0.0 { return 0.0; }
    if c >= 1.0 { return 1.0; }
    let max_n = (1.0 / c).ceil() as u32 + 1;
    let mut expected_n = 0.0_f32;
    let mut p_not_yet = 1.0_f32;
    for n in 1..=max_n {
        let p_this = (n as f32 * c).min(1.0);
        let p_proc_here = p_not_yet * p_this;
        expected_n += n as f32 * p_proc_here;
        p_not_yet *= 1.0 - p_this;
        if p_not_yet < 0.00001 { break; }
    }
    // Account for any remaining probability mass
    expected_n += (max_n + 1) as f32 * p_not_yet;
    1.0 / expected_n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rng;

    #[test]
    fn test_prd_c_value() {
        let c = prd_c_from_p(0.3333);
        // C should be significantly less than P (PRD property)
        assert!(c < 0.3333, "C should be less than P, got {c}");
        assert!(c > 0.10 && c < 0.20, "C for P=0.3333 should be ~0.14, got {c}");
    }

    #[test]
    fn test_prd_c_value_43() {
        let c = prd_c_from_p(0.4333);
        assert!(c < 0.4333, "C should be less than P, got {c}");
        assert!(c > 0.15 && c < 0.30, "C for P=0.4333 should be ~0.20-0.23, got {c}");
    }

    #[test]
    fn test_prd_converges() {
        let mut rng = Rng::new(12345);
        let mut prd = PrdState::new(0.3333);
        let mut procs = 0u32;
        let trials = 10000;
        for _ in 0..trials {
            if prd.roll(&mut rng) {
                procs += 1;
            }
        }
        let actual_rate = procs as f32 / trials as f32;
        assert!(
            (actual_rate - 0.3333).abs() < 0.02,
            "PRD actual rate {actual_rate} should be within 2% of 0.3333"
        );
    }

    #[test]
    fn test_fury_swipes_stacking() {
        use aa2_data::{AbilityDef, TargetType, EffectSpec, Trigger, TargetingSpec, Delivery, Payload};
        use crate::cast::AbilityState;

        let mut rng = Rng::new(42);
        let ability = AbilityState {
            def: AbilityDef {
                name: "Fury Swipes".to_string(),
                cooldown: vec![0.0],
                mana_cost: vec![0.0],
                cast_point: 0.0,
                targeting: TargetType::Passive,
                effects: vec![],
                description: String::new(), is_ultimate: false,
                aoe_shape: None,
                cast_range: 0.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
                effect_specs: Some(vec![EffectSpec {
                    trigger: Trigger::OnAttack,
                    targeting: TargetingSpec::AttackTarget,
                    delivery: Delivery::Instant,
                    payload: vec![
                        Payload::StackingBonusDamage {
                            damage_per_stack: vec![20.0],
                            stack_duration: vec![15.0],
                        },
                    ],
                    illusion_interaction: aa2_data::IllusionInteraction::Disabled, mana_cost: vec![],
                }]),
            },
            cooldown_remaining: 0.0,
            level: 1,
            casts: 0,
            charges: None,        };

        let mut attacker = make_test_unit(0, 0);
        attacker.abilities.push(ability);
        let target_id = 1;
        let base_damage = 50.0;

        let mut damages = Vec::new();
        for tick in 0..5 {
            let result = process_attack_modifiers(&mut attacker, target_id, base_damage, tick, &mut rng, None, false);
            damages.push(result.damage);
        }

        // Hit 1: 0 stacks -> 50 + 0 = 50
        // Hit 2: 1 stack -> 50 + 20 = 70
        // Hit 3: 2 stacks -> 50 + 40 = 90
        // Hit 4: 3 stacks -> 50 + 60 = 110
        // Hit 5: 4 stacks -> 50 + 80 = 130
        assert!((damages[0] - 50.0).abs() < 0.01);
        assert!((damages[1] - 70.0).abs() < 0.01);
        assert!((damages[2] - 90.0).abs() < 0.01);
        assert!((damages[3] - 110.0).abs() < 0.01);
        assert!((damages[4] - 130.0).abs() < 0.01);
    }

    #[test]
    fn test_fury_swipes_no_crit() {
        use aa2_data::{AbilityDef, TargetType, EffectSpec, Trigger, TargetingSpec, Delivery, Payload};
        use crate::cast::AbilityState;

        let mut rng = Rng::new(42);
        let mut attacker = make_test_unit(0, 0);
        // Add both Fury Swipes and a guaranteed crit
        attacker.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Fury Swipes".to_string(),
                cooldown: vec![0.0],
                mana_cost: vec![0.0],
                cast_point: 0.0,
                targeting: TargetType::Passive,
                effects: vec![],
                description: String::new(), is_ultimate: false,
                aoe_shape: None,
                cast_range: 0.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
                effect_specs: Some(vec![EffectSpec {
                    trigger: Trigger::OnAttack,
                    targeting: TargetingSpec::AttackTarget,
                    delivery: Delivery::Instant,
                    payload: vec![
                        Payload::StackingBonusDamage {
                            damage_per_stack: vec![20.0],
                            stack_duration: vec![15.0],
                        },
                    ],
                    illusion_interaction: aa2_data::IllusionInteraction::Disabled, mana_cost: vec![],
                }]),
            },
            cooldown_remaining: 0.0,
            level: 1,
            casts: 0,
            charges: None,        });
        attacker.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Chaos Strike".to_string(),
                cooldown: vec![0.0],
                mana_cost: vec![0.0],
                cast_point: 0.0,
                targeting: TargetType::Passive,
                effects: vec![],
                description: String::new(), is_ultimate: false,
                aoe_shape: None,
                cast_range: 0.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
                effect_specs: Some(vec![aa2_data::EffectSpec {
                    trigger: Trigger::OnAttack,
                    targeting: TargetingSpec::AttackTarget,
                    delivery: Delivery::Instant,
                    payload: vec![
                        Payload::Crit {
                            proc_chance: vec![1.0], // guaranteed crit
                            crit_min: vec![200.0],
                            crit_max: vec![200.0],
                        },
                        Payload::Lifesteal { pct: vec![0.0] },
                    ],
                    illusion_interaction: aa2_data::IllusionInteraction::Full, mana_cost: vec![],
                }]),
            },
            cooldown_remaining: 0.0,
            level: 1,
            casts: 0,
            charges: None,        });

        let target_id = 1;
        let base_damage = 50.0;

        // First attack: 0 FS stacks. Crit = 2.0x. Damage = 50*2 + 0 = 100
        let r1 = process_attack_modifiers(&mut attacker, target_id, base_damage, 0, &mut rng, None, false);
        assert!((r1.damage - 100.0).abs() < 0.01, "Expected 100, got {}", r1.damage);

        // Second attack: 1 FS stack. Crit = 2.0x. Damage = 50*2 + 20 = 120 (NOT 50+20)*2=140)
        let r2 = process_attack_modifiers(&mut attacker, target_id, base_damage, 1, &mut rng, None, false);
        assert!((r2.damage - 120.0).abs() < 0.01, "Expected 120, got {}", r2.damage);
    }

    #[test]
    fn test_chaos_strike_lifesteal() {
        use aa2_data::{AbilityDef, TargetType, EffectSpec, Trigger, TargetingSpec, Delivery, Payload};
        use crate::cast::AbilityState;

        let mut rng = Rng::new(42);
        let mut attacker = make_test_unit(0, 0);
        attacker.hp = 400.0;
        attacker.max_hp = 600.0;

        attacker.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Chaos Strike".to_string(),
                cooldown: vec![0.0],
                mana_cost: vec![0.0],
                cast_point: 0.0,
                targeting: TargetType::Passive,
                effects: vec![],
                description: String::new(), is_ultimate: false,
                aoe_shape: None,
                cast_range: 0.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
                effect_specs: Some(vec![EffectSpec {
                    trigger: Trigger::OnAttack,
                    targeting: TargetingSpec::AttackTarget,
                    delivery: Delivery::Instant,
                    payload: vec![
                        Payload::Crit {
                            proc_chance: vec![1.0], // guaranteed
                            crit_min: vec![200.0],
                            crit_max: vec![200.0],
                        },
                        Payload::Lifesteal { pct: vec![50.0] }, // 50%
                    ],
                    illusion_interaction: aa2_data::IllusionInteraction::Full, mana_cost: vec![],
                }]),
            },
            cooldown_remaining: 0.0,
            level: 1,
            casts: 0,
            charges: None,        });

        let mut target = make_test_unit(1, 1);
        let base_damage = 50.0;
        let result = process_attack_modifiers(&mut attacker, target.id, base_damage, 0, &mut rng, None, false);

        // Damage dealt after armor would be less, but lifesteal is on pre-armor damage
        let damage_dealt = 60.0; // simulated post-armor
        post_attack_effects(&mut attacker, &mut target, damage_dealt, result.lifesteal_pct, 0);

        // 50% lifesteal on 60 damage = 30 heal
        assert!((attacker.hp - 430.0).abs() < 0.01, "Expected 430 HP, got {}", attacker.hp);
    }

    #[test]
    fn test_essence_shift_stat_steal() {
        use aa2_data::{AbilityDef, TargetType};
        use crate::cast::AbilityState;

        let mut attacker = make_test_unit(0, 0);
        let mut target = make_test_unit(1, 1);

        attacker.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Essence Shift".to_string(),
                cooldown: vec![0.0],
                mana_cost: vec![0.0],
                cast_point: 0.0,
                targeting: TargetType::Passive,
                effects: vec![],
                description: String::new(), is_ultimate: false,
                aoe_shape: None,
                cast_range: 0.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
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
            charges: None,        });

        post_attack_effects(&mut attacker, &mut target, 50.0, 0.0, 0);

        // Target should have a debuff reducing STR
        assert_eq!(target.buffs.len(), 1);
        assert_eq!(target.buffs[0].name, "essence_shift_debuff");
        let target_mod = target.buffs[0].stat_modifier.as_ref().unwrap();
        assert!((target_mod.bonus_strength - (-1.0)).abs() < 0.01);
        assert!((target_mod.bonus_agi - (-1.0)).abs() < 0.01);
        assert!((target_mod.bonus_int - (-1.0)).abs() < 0.01);

        // Attacker should have a buff adding AGI
        assert_eq!(attacker.buffs.len(), 1);
        assert_eq!(attacker.buffs[0].name, "essence_shift_buff");
        let attacker_mod = attacker.buffs[0].stat_modifier.as_ref().unwrap();
        assert!((attacker_mod.bonus_agi - 3.0).abs() < 0.01);
    }

    fn make_test_unit(id: u32, team: u8) -> Unit {
        use aa2_data::{Attribute, HeroDef};
        use crate::vec2::Vec2;

        let def = HeroDef {
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
            base_damage_max: 40.0,
            projectile_speed: None,
        };
        Unit::from_hero_def(&def, id, team, Vec2::new(0.0, 0.0))
    }

    #[test]
    fn test_glaives_bonus_damage() {
        use aa2_data::{AbilityDef, TargetType, EffectSpec, Trigger, TargetingSpec, Delivery, Payload};
        use crate::cast::AbilityState;

        let mut rng = Rng::new(42);
        let mut attacker = make_test_unit(0, 0);
        attacker.mana = 100.0;
        attacker.base_int = 40.0;

        attacker.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Glaives".to_string(),
                cooldown: vec![0.0],
                mana_cost: vec![0.0],
                cast_point: 0.0,
                targeting: TargetType::Passive,
                effects: vec![],
                description: String::new(), is_ultimate: false,
                aoe_shape: None,
                cast_range: 0.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
                effect_specs: Some(vec![EffectSpec {
                    trigger: Trigger::OnAttack,
                    targeting: TargetingSpec::AttackTarget,
                    delivery: Delivery::Instant,
                    payload: vec![
                        Payload::IntScaledDamage { factor: vec![0.8] },
                        Payload::AttackBounce { radius: vec![0.0] },
                    ],
                    illusion_interaction: aa2_data::IllusionInteraction::Disabled,
                    mana_cost: vec![15.0],
                }]),
            },
            cooldown_remaining: 0.0,
            level: 1,
            casts: 0,
            charges: None,
        });

        let result = process_attack_modifiers(&mut attacker, 1, 50.0, 0, &mut rng, None, false);
        // 80% of 40 INT = 32 bonus magical damage
        assert!((result.bonus_magical_damage - 32.0).abs() < 0.01, "Expected 32, got {}", result.bonus_magical_damage);
        assert!(result.glaives_active);
        assert!((attacker.mana - 85.0).abs() < 0.01); // 100 - 15
    }

    #[test]
    fn test_glaives_mana_cost() {
        use aa2_data::{AbilityDef, TargetType, EffectSpec, Trigger, TargetingSpec, Delivery, Payload};
        use crate::cast::AbilityState;

        let mut rng = Rng::new(42);
        let mut attacker = make_test_unit(0, 0);
        attacker.mana = 10.0; // Not enough for 15 mana cost
        attacker.base_int = 40.0;

        attacker.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Glaives".to_string(),
                cooldown: vec![0.0],
                mana_cost: vec![0.0],
                cast_point: 0.0,
                targeting: TargetType::Passive,
                effects: vec![],
                description: String::new(), is_ultimate: false,
                aoe_shape: None,
                cast_range: 0.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
                effect_specs: Some(vec![EffectSpec {
                    trigger: Trigger::OnAttack,
                    targeting: TargetingSpec::AttackTarget,
                    delivery: Delivery::Instant,
                    payload: vec![
                        Payload::IntScaledDamage { factor: vec![0.8] },
                        Payload::AttackBounce { radius: vec![0.0] },
                    ],
                    illusion_interaction: aa2_data::IllusionInteraction::Disabled,
                    mana_cost: vec![15.0],
                }]),
            },
            cooldown_remaining: 0.0,
            level: 1,
            casts: 0,
            charges: None,
        });

        let result = process_attack_modifiers(&mut attacker, 1, 50.0, 0, &mut rng, None, false);
        // No mana = no bonus damage
        assert!((result.bonus_magical_damage - 0.0).abs() < 0.01);
        assert!(!result.glaives_active);
        assert!((attacker.mana - 10.0).abs() < 0.01); // mana unchanged
    }

    #[test]
    fn test_glaives_bounce() {
        use aa2_data::{AbilityDef, TargetType, Attribute, HeroDef, EffectSpec, Trigger, TargetingSpec, Delivery, Payload};
        use crate::cast::AbilityState;
        use crate::vec2::Vec2;
        use crate::{Simulation, CombatEvent};

        let hero = HeroDef {
            name: "Silencer".to_string(),
            primary_attribute: Attribute::Intelligence,
            base_str: 20.0,
            base_agi: 20.0,
            base_int: 40.0,
            str_gain: 2.0,
            agi_gain: 2.0,
            int_gain: 3.0,
            base_attack_time: 1.7,
            attack_range: 600.0,
            attack_point: 0.3,
            move_speed: 300.0,
            turn_rate: 0.6,
            collision_radius: 24.0,
            tier: 1,
            is_melee: false,
            base_damage_min: 20.0,
            base_damage_max: 30.0,
            projectile_speed: Some(1200.0),
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
                effects: vec![],
                description: String::new(), is_ultimate: false,
                aoe_shape: None,
                cast_range: 0.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
                effect_specs: Some(vec![EffectSpec {
                    trigger: Trigger::OnAttack,
                    targeting: TargetingSpec::AttackTarget,
                    delivery: Delivery::Instant,
                    payload: vec![
                        Payload::IntScaledDamage { factor: vec![1.0] },
                        Payload::StatSteal {
                            str_steal: vec![0.0], agi_steal: vec![0.0],
                            int_steal: vec![2.0], agi_gain: vec![0.0],
                            duration: vec![10.0],
                        },
                        Payload::AttackBounce { radius: vec![500.0] },
                    ],
                    illusion_interaction: aa2_data::IllusionInteraction::Disabled,
                    mana_cost: vec![15.0],
                }]),
            },
            cooldown_remaining: 0.0,
            level: 9,
            casts: 0,
            charges: None,
        });

        // Target at 100 units, secondary enemy at 200 units (within 500 of target)
        let target = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));
        let secondary = Unit::from_hero_def(&hero, 2, 1, Vec2::new(200.0, 0.0));
        let secondary_hp_before = secondary.hp;

        let mut sim = Simulation::with_seed(vec![attacker, target, secondary], 42);
        // Run until bounce projectile hits secondary (unit 2)
        for _ in 0..600 {
            if sim.is_finished() { break; }
            sim.step();
            let hit_on_secondary = sim.combat_log.iter().any(|e| matches!(e, CombatEvent::ProjectileHit { target_id: 2, .. }));
            if hit_on_secondary { break; }
        }
        // Secondary target should have taken Glaives bounce damage
        let secondary_hp_after = sim.units[2].hp;
        assert!(secondary_hp_after < secondary_hp_before,
            "Secondary should take bounce damage: before={secondary_hp_before}, after={secondary_hp_after}");
    }

    #[test]
    fn test_stat_steal_floor() {
        use crate::unit::effective_stat;

        // base_int = 5, bonus = -10 → effective should be 1 (floored)
        assert!((effective_stat(5.0, -10.0) - 1.0).abs() < 0.01);
        // base_int = 20, bonus = -5 → effective should be 15
        assert!((effective_stat(20.0, -5.0) - 15.0).abs() < 0.01);
        // base_int = 1, bonus = -1 → effective should be 1 (can't go below)
        assert!((effective_stat(1.0, -1.0) - 1.0).abs() < 0.01);
    }
}
