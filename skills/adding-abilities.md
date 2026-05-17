# Skill: Adding Abilities to AA2

## CRITICAL: Research First, Implement Second

**Do NOT implement from memory or training data.** Dota2 abilities have exact values that change every patch. You MUST web-search the ability on liquipedia.net/dota2/{HeroName} and get current patch values before writing any code.

### What to research (EXACT numbers required):

| Field | Example |
|-------|---------|
| Cast point (seconds) | 0.25 |
| Cooldown per level | 14/13/12/11 |
| Mana cost per level | 90/100/110/120 |
| Damage per level | 100/175/250/325 |
| Duration/stun per level | 1.3/1.6/1.9/2.2 |
| Radius/range per level | 900/1000/1100/1200 |
| Projectile/travel speed | 1400 u/s |
| Width/hit radius | 125 |
| Damage type | Magical/Physical/Pure |
| Pierces magic immunity? | Yes/No/Partially |
| Dispellable? | Basic/Strong/Undispellable |
| Special mechanics | Wave timing, delays, knockback, etc. |

---

## AA 9-Level Scaling Rules

Abilities scale at levels 1, 2, 3, 6 (Super), and 9 (Gaben) ONLY. Levels 4-5 = level 3. Levels 7-8 = level 6.

**Mapping from Dota4 levels to AA 9 levels:**
- Regular abilities: AA L1 = Dota L1, AA L2 = Dota L2, AA L3 = Dota L4 (SKIP Dota L3)
- Ultimate abilities: AA L1 = Dota L1, AA L2 = Dota L2, AA L3 = Dota L3

**Array format:** `[L1, L2, L3, L4, L5, L6_Super, L7, L8, L9_Gaben]`

Example:
```
damage: [100.0, 175.0, 325.0, 325.0, 325.0, 425.0, 425.0, 425.0, 425.0]
//       L1     L2     L3=DotaL4  plateau   Super(+100)  plateau   Gaben
```

---

## Implementation Checklist

### 1. Determine required mechanics

Before coding, list what the ability needs:
- [ ] New Effect variant in `aa2-data`?
- [ ] New PendingEffectKind in `pending.rs`? (for delays, travel, waves)
- [ ] New StatusFlags? (invulnerable, magic_immune, etc.)
- [ ] Interaction with magic immunity? (most magical abilities are blocked)
- [ ] Interaction with existing systems? (damage block, armor, stat steal)
- [ ] Cast behavior? (Lazy/Seek/SeekPlus)
- [ ] Charges? (Gaben upgrades sometimes add charges)
- [ ] Arena wall interaction? (Spear of Mars, etc.)

### 2. Add Effect variant to `crates/aa2-data/src/lib.rs`

All per-level values use `Vec<f32>`. Fixed values use `f32`. Add serde derives.

### 3. Add execution logic

**Active abilities** → `crates/aa2-sim/src/ability.rs` (execute_ability function)
- Simple instant effects: handle directly
- Delayed/traveling effects: create a PendingEffect

**Passive/attack modifiers** → `crates/aa2-sim/src/attack_modifier.rs`
- Hook into `process_attack_modifiers` (pre-hit) or `post_attack_effects` (post-hit)

**Pending effects** → `crates/aa2-sim/src/lib.rs` (step_pending_effects)
- Projectiles, waves, delays, travel effects

### 4. Create RON data file at `data/abilities/{snake_case_name}.ron`

Template:
```ron
AbilityDef(
    name: "Ability Name",
    cooldown: [14.0, 13.0, 11.0, 11.0, 11.0, 11.0, 11.0, 11.0, 11.0],
    mana_cost: [90.0, 100.0, 120.0, 120.0, 120.0, 120.0, 120.0, 120.0, 120.0],
    cast_point: 0.25,
    targeting: SingleEnemy,  // or NoTarget, SingleAlly, SingleAllyHG, Passive
    effects: [ ... ],
    description: "...",
    aoe_shape: None,
    cast_range: 900.0,
    cast_behavior: Lazy,  // or Seek (default), SeekPlus(200.0)
    max_charges: None,     // or Some(2)
)
```

### 5. Add deserialization test in `crates/aa2-data/tests/load_heroes.rs`

```rust
#[test]
fn load_ability_name() {
    let ability = aa2_data::load_ability_def(Path::new("../../data/abilities/ability_name.ron")).unwrap();
    assert_eq!(ability.name, "Ability Name");
}
```

### 6. Write integration tests in `crates/aa2-sim/tests/abilities.rs`

**MANDATORY tests for every ability:**

| Test type | What to assert |
|-----------|---------------|
| Happy path | Ability fires, deals correct damage/effect |
| Timing | Delays, travel time, stun duration are tick-accurate |
| Magic immunity | Blocked if it should be (most magical abilities) |
| Edge case | What happens at boundaries (max range, arena wall, etc.) |
| Super/Gaben | Upgraded behavior works (if mechanically different) |

**Test pattern:**
```rust
/// [What this tests] — [Why it matters for game feel]
#[test]
fn test_ability_name_specific_behavior() {
    let hero = make_hero(); // or load from RON
    let ability = aa2_data::load_ability_def(Path::new("../../data/abilities/X.ron")).unwrap();
    
    let config = UnitConfig::new(hero.clone()).with_ability(ability, level);
    let mut caster = Unit::from_config(&config, 0, 0, position);
    caster.mana = 500.0; // ensure enough mana
    caster.facing = direction; // face toward target
    
    let target = Unit::from_hero_def(&hero, 1, 1, target_position);
    
    let mut sim = Simulation::with_seed(vec![caster, target], 42);
    
    // Track state changes over time (don't just check final state!)
    let mut was_stunned = false;
    for _ in 0..N {
        sim.step();
        if condition { was_stunned = true; }
    }
    
    assert!(was_stunned, "...");
}
```

---

## Common Gotchas (Learned from Experience)

### 1. Stun duration expires before test checks it
**Problem:** Test runs 300 ticks, stun lasts 60 ticks, assertion at end finds no stun.
**Fix:** Track `was_stunned` flag during simulation, or check within the stun window.

### 2. Magic immunity not checked in new code paths
**Problem:** New ability damages/stuns magic immune units.
**Fix:** Check `active_status(&target.buffs).magic_immune` before applying magical damage or debuffs.

### 3. NoTarget abilities return early from execute_ability
**Problem:** `execute_ability` has a `_ => { match target_id { None => return events } }` path that exits before processing NoTarget effects.
**Fix:** NoTarget has its own match arm that returns `vec![]` for target_indices, letting the second loop handle special effects.

### 4. Per-level values are 1-indexed
**Problem:** Level 1 ability uses `base[1]` instead of `base[0]`.
**Fix:** Always use `value_at_level(array, level)` which does `array[(level-1).min(len-1)]`.

### 5. Bounce/travel effects don't clear hit lists
**Problem:** Gaben bounce can't hit units that were already pass-through hit.
**Fix:** On bounce, clear `pass_through_hit` but retain the pinned unit's ID.

### 6. Arena bounds not applied to new movement
**Problem:** New ability moves units outside 2000x2000 arena.
**Fix:** Call `clamp_to_arena()` after any position change.

### 7. Projectile effects need attacker context
**Problem:** Ranged attack modifiers (Glaives bounce, etc.) need attacker info at impact time.
**Fix:** Store `attacker_id`, `bonus_magical_damage`, `lifesteal_pct` on Projectile struct.

### 8. Auto-attacks still happen during test
**Problem:** Test checks "no damage from ability" but unit takes physical auto-attack damage.
**Fix:** Assert on `CombatEvent::AbilityDamage` events specifically, not raw HP difference.

---

## Damage Pipeline Reference

```
1. Roll base damage (damage_min to damage_max)
2. Chaos Strike crit (PRD proc → multiply)
3. Fury Swipes flat bonus (POST-CRIT, not multiplied)
4. = total physical damage
5. Glaives bonus = INT × factor (separate magical damage)
6. Physical: apply armor → apply damage block (melee defender only)
7. Magical: apply magic resistance (blocked entirely by magic immunity)
8. Pure: no reduction
9. Post-hit: lifesteal, Essence Shift steal, Fury Swipes stack, Glaives INT steal
```

---

## File Locations Quick Reference

| What | Where |
|------|-------|
| Effect enum | `crates/aa2-data/src/lib.rs` |
| Ability execution | `crates/aa2-sim/src/ability.rs` |
| Pending effects | `crates/aa2-sim/src/pending.rs` + `lib.rs` (step_pending_effects) |
| Attack modifiers | `crates/aa2-sim/src/attack_modifier.rs` |
| Buff/debuff | `crates/aa2-sim/src/buff.rs` |
| AI targeting | `crates/aa2-sim/src/ai.rs` |
| Cast system | `crates/aa2-sim/src/cast.rs` |
| AoE shapes | `crates/aa2-sim/src/aoe.rs` |
| Unit struct | `crates/aa2-sim/src/unit.rs` |
| RON data | `data/abilities/*.ron` |
| Integration tests | `crates/aa2-sim/tests/abilities.rs` |
| Data load tests | `crates/aa2-data/tests/load_heroes.rs` |

---

## Illusion Interaction Rules

When implementing a new ability, you MUST classify its `IllusionInteraction`:

```rust
impl Effect {
    pub fn illusion_interaction(&self) -> IllusionInteraction {
        match self {
            Effect::ChaosStrike { .. } => IllusionInteraction::Full,
            Effect::FurySwipes { .. } => IllusionInteraction::Disabled,
            Effect::EssenceShift { .. } => IllusionInteraction::Disabled,
            Effect::GlaivesOfWisdom { .. } => IllusionInteraction::Disabled,
            _ => IllusionInteraction::Disabled, // default: doesn't work
        }
    }
}
```

### Classification guide:

| Category | IllusionInteraction | Examples |
|----------|-------------------|----------|
| Critical strikes | `Full` | Chaos Strike, Coup de Grace, Daedalus |
| Lifesteal | `Full` | All lifesteal sources |
| Mana burn (innate) | `Full` | Mana Break, Curse of Avernus |
| Stat steal / on-hit debuffs | `Disabled` | Essence Shift, Glaives, Fury Swipes |
| Bash / proc damage | `Disabled` | Skull Basher, MKB, Time Lock |
| Cleave | `Disabled` | Battle Fury, Empower |
| Auras | `CarriesAura` | Radiance, Inner Beast, Assault Cuirass |

### Illusion stat rules (enforced in step_buffs):
- ✅ Base damage (from attributes)
- ✅ Attack speed (all sources)
- ✅ Armor from AGI (stat-based)
- ✅ All stat bonuses (STR/AGI/INT)
- ✅ Move speed, HP, Mana, Evasion
- ❌ Flat bonus armor (negated for illusions)
- ❌ Bonus +damage (green damage, negated)
- ❌ Flat magic resistance bonus (negated)

### When adding a new attack modifier:
1. Add the `IllusionInteraction` classification to `Effect::illusion_interaction()`
2. The attack modifier loops already check `is_illusion` + `illusion_interaction()` — no extra code needed
3. Write a test verifying the interaction (illusion can/cannot use it)

---

## Arena System

The combat arena is 2000×2000 units with impassable walls.
- Bounds: x ∈ [0, 2000], y ∈ [0, 2000]
- `clamp_to_arena(pos) -> (Vec2, bool)` — clamps position, returns whether wall was hit
- ALL movement must be clamped (walking, burrowstrike, spear push, knockback)
- Spear of Mars pins units to arena walls

---

## Cooldown Reduction (CDR)

- `unit.cooldown_reduction: f32` (0.0 = none, 0.25 = 25%)
- Applied when cooldown is SET (after cast): `effective_cd = base_cd * (1.0 - cdr)`
- NOT applied during tick-down
- Gaben upgrades may grant CDR (e.g., Spirit Lance Gaben = 25%)

---

## Universal Attribute

- `Attribute::Universal` — 4th attribute type
- Damage = `(STR + AGI + INT) * 0.7` (instead of `primary * 1.0`)
- Gaben stat bonuses: +15 to each stat (vs +45 to primary for other types)

---

## Illusion Spawning

When an ability spawns illusions (Spirit Lance, etc.):
1. Use `Unit::spawn_illusion(source, id, pos, damage_dealt_pct, damage_taken_pct, duration_ticks, tick)`
2. Illusion copies source stats, keeps `Full`-tagged passives, clears everything else
3. Push to `sim.units_to_spawn` (appended at end of tick)
4. Set `illusion_expiry_tick` — illusion auto-dies when tick is reached
5. Illusion damage modifiers applied in melee + projectile hit paths

---

## Charges System

Some abilities use charges instead of cooldown:
- `max_charges: Option<u32>` on AbilityDef (None = normal cooldown)
- `charges: Option<ChargeState>` on AbilityState
- `ability.is_ready()` checks charges > 0 OR cooldown <= 0
- `ability.consume()` decrements charge or sets cooldown
- Charges restore one at a time on a timer
