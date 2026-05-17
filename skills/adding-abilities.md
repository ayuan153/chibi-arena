# Skill: Adding Abilities to AA2

## Process (MANDATORY)

When implementing a new ability, follow this exact process:

### 1. Research the Base Dota2 Ability

**Before writing any code**, research the ability on liquipedia.net/dota2 or dota2.fandom.com. Get EXACT values for:

- Cast point (seconds)
- Cooldown at levels 1/2/3/4
- Mana cost at levels 1/2/3/4
- All effect values at levels 1/2/3/4 (damage, duration, radius, etc.)
- Damage type (Physical/Magical/Pure)
- Whether it pierces debuff/magic immunity
- Whether debuffs are dispellable (and by what strength)
- Projectile speed (if applicable)
- Any special mechanics (travel time, delay, stacking, etc.)
- Targeting type (unit-targeted, ground-targeted, no-target, passive)

### 2. Map to AA 9-Level Scaling

AA abilities scale at levels 1, 2, 3, 6 (Super), and 9 (Gaben). Levels 4-5 = level 3, levels 7-8 = level 6.

**Mapping rules:**
- Regular abilities: AA L1 = Dota L1, AA L2 = Dota L2, AA L3 = Dota L4 (skip Dota L3)
- Ultimate abilities: AA L1 = Dota L1, AA L2 = Dota L2, AA L3 = Dota L3
- Super (L6) and Gaben (L9) are AA-specific upgrades (provided by the user)

**Array format:** `[L1, L2, L3, L4, L5, L6, L7, L8, L9]` where L4-5 repeat L3, L7-8 repeat L6.

### 3. Determine What Mechanics Are Needed

Check if the ability requires mechanics not yet in the sim:
- New Effect variant?
- New PendingEffectKind?
- New StatusFlags?
- New targeting behavior?
- New damage pipeline interaction?

### 4. Implement

1. Add any new Effect variants to `crates/aa2-data/src/lib.rs`
2. Add execution logic to `crates/aa2-sim/src/ability.rs` or `pending.rs`
3. Create the RON data file at `data/abilities/{name}.ron`
4. Add a deserialization test in `crates/aa2-data/tests/load_heroes.rs`

### 5. Write Integration Tests

Every ability needs at least one integration test in `crates/aa2-sim/tests/abilities.rs` that:
- Loads the actual RON file (proves data + code work together)
- Sets up exact initial conditions
- Runs the sim with a fixed seed
- Asserts specific properties of the outcome

### 6. Verify

```bash
cargo check && cargo test && cargo clippy
```

## Key Design Principles

- **Fidelity over simplicity**: Match Dota2 mechanics exactly. Don't simplify wave timing, travel speeds, delays, or hit detection.
- **Damage pipeline order**: Roll base → Crit → Fury Swipes (post-crit) → Armor (physical) / Magic Resist (magical) → Damage Block (physical only, melee defender)
- **Magic immunity blocks**: All magical damage, most debuffs, spell targeting. Does NOT block physical, pure, or effects that explicitly pierce immunity.
- **Stat steal floors at 1**: Base stats can't go below 1. Bonus stats from buffs are separate and unaffected by steal.
- **Per-level arrays are 1-indexed**: Level 1 = array index 0. Use `value_at_level()`.

## Cast Behavior Reference

| Behavior | Description |
|----------|-------------|
| `Lazy` | Only casts if target in range. Won't walk. |
| `Seek` (default) | Walks toward target until in range. |
| `SeekPlus(X)` | Walks toward target within cast_range + X. |

## Common Patterns

### Projectile ability (e.g., Spear of Mars)
- Use PendingEffect with travel speed
- Hit detection each tick as projectile moves
- Apply effects on hit

### Self-buff (e.g., Rage)
- NoTarget, instant cast
- Apply buff with StatusFlags
- Dispel on cast if applicable

### AoE stun (e.g., Ravage)
- ExpandingWave pending effect
- Distance-based hit timing
- Stun buff applied on hit

### Attack modifier (e.g., Fury Swipes)
- Passive targeting, 0 cooldown, 0 mana
- Logic in `attack_modifier.rs`
- Hooks into damage pipeline
