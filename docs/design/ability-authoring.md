# Ability Authoring Reference (RON-only, no Rust)

A new ability is **just a RON file** in `data/abilities/`. The engine resolves it at runtime via the
composable effect system — no Rust code changes required. This document is the practical reference
for authoring abilities.

---

## The Model: EffectSpec

Every ability carries one or more `EffectSpec`s (in `AbilityDef.effect_specs`). Each spec is a
composition of four orthogonal axes:

> **Trigger** (when) × **TargetingSpec** (who) × **Delivery** (how it reaches them) × **Payload[]** (what happens)

The engine has one generic resolver per axis. You compose primitives — you don't write code.

---

## Catalog

### Triggers

| Variant | When it fires |
|---------|---------------|
| `OnCast` | When the ability is cast (after cast point). |
| `OnAttack` | When the owning unit lands an auto-attack (passive on-hit modifier). |
| `OnKill` | When a nearby enemy dies (proximity-attributed, index-order scan). |

### TargetingSpec

| Variant | Who is affected |
|---------|-----------------|
| `Caster` | The casting unit itself. |
| `EnemiesInDelivery` | All alive enemies within the delivery area. |
| `TargetAndCaster` | The cast target AND the caster (dedup if equal; caster-only if no target). |
| `AttackTarget` | The auto-attack target (used with `Trigger::OnAttack`). |

### Delivery

| Variant | Fields | Behavior |
|---------|--------|----------|
| `Instant` | — | Applied immediately, no travel time. |
| `ExpandingWave` | `max_radius: Vec<f32>`, `speed: f32` | Expands outward from caster at `speed` u/s; hits enemies as the wavefront reaches them. |
| `DelayedPulse` | `delay: f32`, `pulse_count: u32`, `pulse_interval: f32`, `radius: Vec<f32>` | Fires `pulse_count` pulses after `delay` seconds, each `pulse_interval` apart, hitting enemies within `radius`. Also applies `SelfDamage` payloads to caster. |
| `CasterTravel` | `width: f32`, `speed: f32`, `range: Vec<f32>` | Caster travels a line toward target; capsule hit detection (`width`). Caster is invulnerable during travel. |
| `Aoe` | `radius: Vec<f32>` | Instant AoE around the delivery origin. Hits enemies within radius. |
| `Projectile` | `homing: bool`, `speed: f32`, `width: f32`, `range: Vec<f32>`, `wall_bounces: Vec<u32>`, `fire_trail_dps: Vec<f32>`, `fire_trail_slow: Vec<f32>`, `fire_trail_duration: Vec<f32>`, `stun_duration: Vec<f32>`, `bounce_radius: Vec<f32>`, `bounce_count: Vec<u32>` | If `homing: true` → tracks target, bounces on hit (Spirit Lance). If `homing: false` → linear, impales first hero, wall-pin stun (Spear of Mars). |

All `Vec<f32>` / `Vec<u32>` fields are **per-level** (index 0 = level 1). If the ability level
exceeds the vec length, the last element is used.

### Payload

| Variant | What it does | Key fields |
|---------|-------------|------------|
| `Damage` | Deal damage (armor/MR applied). | `kind: DamageType`, `base: Vec<f32>` |
| `Heal` | Heal target (clamped to max_hp). | `base: Vec<f32>` |
| `ApplyBuff` | Apply a buff/debuff (see BuffDef below). | `Box<BuffDef>` |
| `Dispel` | Remove debuffs up to given strength. | `strength: DispelType` |
| `Chain` | Trigger a chained sub-effect (bounded by `MAX_EFFECT_CHAIN_DEPTH=2`). | `Box<EffectSpec>` |
| `SelfDamage` | Deal damage to the CASTER (fraction of pulse damage). | `pct: f32`, `non_lethal: bool` |
| `DamageWithSourceMaxHp` | Damage = `base[level] + max_hp_pct × source_max_hp`. For on-death explosions. | `kind: DamageType`, `base: Vec<f32>`, `max_hp_pct: f32` |
| `StackingBonusDamage` | Per-target stacking bonus damage (Fury Swipes). | `damage_per_stack: Vec<f32>`, `stack_duration: Vec<f32>` |
| `Crit` | PRD-based critical strike. | `proc_chance: Vec<f32>`, `crit_min: Vec<f32>`, `crit_max: Vec<f32>` |
| `Lifesteal` | Heal attacker for `pct/100` of damage dealt (only on crit). | `pct: Vec<f32>` |
| `StatSteal` | Steal STR/AGI/INT from target, grant AGI to attacker. | `str_steal`, `agi_steal`, `int_steal`, `agi_gain`, `duration: Vec<f32>` each |
| `IntScaledDamage` | Bonus magical damage = `factor[level] × caster_INT`. | `factor: Vec<f32>` |
| `AttackBounce` | Secondary 50%-physical attack on nearest enemy within radius. | `radius: Vec<f32>` |
| `PermanentIntSteal` | On nearby kill: permanently steal INT from victim. | `amount: Vec<f32>`, `radius: f32` |
| `Spawn` | Spawn an illusion of the caster at target's position. | `damage_dealt: Vec<f32>`, `damage_taken: f32`, `duration: Vec<f32>` |

---

## BuffDef Shape

```ron
BuffDef(
    name: "buff_name",
    duration: [3.0, 4.0, 5.0],       // seconds, per ability level
    status: StatusFlags(stunned: true), // or () for no status
    stat_modifier: Some(StatModifierSpec(
        bonus_armor: [5.0, 10.0],
        // ... any of: bonus_attack_speed, bonus_move_speed, bonus_damage,
        //     bonus_magic_resistance, bonus_hp_regen, bonus_strength,
        //     bonus_agi, bonus_int, status_resistance
    )),
    tick_effect: Some(TickEffectDef(
        damage: 50.0,                 // positive = damage, negative = heal
        damage_type: Magical,
        interval_ticks: 30,           // every N ticks (30 ticks = 1 second)
    )),
    stacking: RefreshDuration,        // or StackIntensity(max) or Independent
    dispel_type: BasicDispel,         // or StrongDispel or Undispellable
    is_debuff: true,                  // false = buff (on self/ally)
    pierces_magic_immunity: false,    // true = applies even to magic-immune units
    damage_reflection_pct: 0.0,       // 0.0–1.0
    on_death: None,                   // or Some(EffectSpec(...)) for death trigger
)
```

**Notes:**
- `duration` is `Vec<f32>` indexed by level−1. Duration is converted to ticks via `(secs * 30.0) as u32` (truncation, not rounding).
- `stat_modifier` fields are all `Vec<f32>` (per-level); empty vec = 0.0 at any level.
- `StatusFlags` fields (all `bool`, default `false`): `stunned`, `silenced`, `disarmed`, `rooted`, `hexed`, `invulnerable`, `magic_immune`.
- `DispelType` variants: `BasicDispel`, `StrongDispel`, `Undispellable`.
- `StackBehavior` variants: `RefreshDuration`, `StackIntensity(u32)`, `Independent`.

---

## EffectSpec-Level Fields

| Field | Type | Default | Notes |
|-------|------|---------|-------|
| `illusion_interaction` | `IllusionInteraction` | `Disabled` | `Full` = works on illusions (only Chaos Strike uses this). `Disabled` = illusions skip this spec. `CarriesAura` = illusion carries as aura. |
| `mana_cost` | `Vec<f32>` | `[]` (free) | Gates the entire spec. Checked and spent before payloads fire. If insufficient mana, the spec is skipped. Used to gate `OnAttack` procs (e.g. Glaives costs mana per attack). |

---

## Chain Depth Bound

`Payload::Chain` and `BuffDef.on_death` both trigger sub-effects. Recursion is bounded by
`MAX_EFFECT_CHAIN_DEPTH = 2` (defined in `crates/aa2-sim/src/effect_spec.rs`). If depth exceeds
this, the chain is silently skipped.

---

## Worked Examples

### Example 1: Simple OnCast self-buff (Rage)

Dispels debuffs, then grants magic immunity to the caster.

```ron
effect_specs: Some([
    EffectSpec(
        trigger: OnCast,
        targeting: Caster,
        delivery: Instant,
        payload: [
            Dispel(strength: BasicDispel),
            ApplyBuff(BuffDef(
                name: "rage",
                duration: [1.0, 2.0, 4.0, 4.0, 4.0, 7.0, 7.0, 7.0, 7.0],
                status: StatusFlags(magic_immune: true),
                stacking: RefreshDuration,
                dispel_type: Undispellable,
                is_debuff: false,
            )),
        ],
    ),
]),
```

### Example 2: AoE expanding wave (Ravage)

Damages and stuns all enemies hit by an outward-expanding wave.

```ron
effect_specs: Some([
    EffectSpec(
        trigger: OnCast,
        targeting: EnemiesInDelivery,
        delivery: ExpandingWave(
            max_radius: [700.0, 700.0, 700.0, 700.0, 700.0, 700.0, 700.0, 700.0, 1300.0],
            speed: 905.0,
        ),
        payload: [
            Damage(kind: Magical, base: [275.0, 375.0, 475.0, 475.0, 475.0, 475.0, 475.0, 475.0, 475.0]),
            ApplyBuff(BuffDef(
                name: "stun",
                duration: [2.0, 2.2, 2.4, 2.4, 2.4, 3.4, 3.4, 3.4, 3.4],
                status: StatusFlags(stunned: true),
                stacking: RefreshDuration,
                dispel_type: StrongDispel,
                is_debuff: true,
            )),
        ],
    ),
]),
```

### Example 3: OnAttack passive with mana gate (Glaives of Wisdom)

Mana-gated INT-scaled damage + stat steal + attack bounce. Plus a separate OnKill spec for
permanent INT steal.

```ron
effect_specs: Some([
    EffectSpec(
        trigger: OnAttack,
        targeting: AttackTarget,
        delivery: Instant,
        payload: [
            IntScaledDamage(factor: [0.35, 0.50, 0.80, 0.80, 0.80, 0.80, 0.80, 0.80, 0.80]),
            StatSteal(
                str_steal: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                agi_steal: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                int_steal: [2.0, 3.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0],
                agi_gain: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                duration: [10.0, 20.0, 40.0, 40.0, 40.0, 40.0, 40.0, 40.0, 40.0],
            ),
            AttackBounce(radius: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 500.0]),
        ],
        mana_cost: [12.0, 14.0, 18.0, 18.0, 18.0, 18.0, 18.0, 18.0, 18.0],
    ),
    EffectSpec(
        trigger: OnKill,
        targeting: AttackTarget,
        delivery: Instant,
        payload: [
            PermanentIntSteal(amount: [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0], radius: 900.0),
        ],
    ),
]),
```

---

## How It Resolves at Runtime

The generic resolver lives in `crates/aa2-sim/src/effect_spec.rs`:

1. **`run_cast_effect_specs`** — entry point for `OnCast` triggers. Iterates specs, resolves
   targeting → delivery → payloads in deterministic order (specs in Vec order, targets in index
   order).
2. **Delivery dispatch** — `Instant` applies payloads immediately. `ExpandingWave`, `DelayedPulse`,
   `CasterTravel`, and `Projectile` create `PendingEffect` entries that tick each frame until
   complete. `Aoe` applies instantly to enemies within radius.
3. **`apply_payload_to_unit`** — single source of truth for payload math (damage reduction, buff
   application, magic-immunity gating, status-resistance duration reduction).
4. **`resolve_on_death_spec`** — resolves `BuffDef.on_death` specs when a buffed unit dies. Bounded
   by `MAX_EFFECT_CHAIN_DEPTH`.
5. **OnAttack/OnKill** — hooked into the attack pipeline (`attack_modifier.rs`) and `check_deaths`
   respectively; not dispatched through `run_cast_effect_specs`.

---

## Known Follow-ups (affect authoring)

- `Delivery::Projectile` currently uses a `homing: bool` to switch between linear (Spear of Mars)
  and homing (Spirit Lance) behavior. A future split into `Linear`/`Homing` variants would make
  the unused fields less confusing.
- `fire_trail_*` params live on `Projectile` even for homing projectiles (where they're unused).
- `CasterTravel` hardcodes a `"burrowstrike_invuln"` buff during travel — a future `travel_buff`
  field on the delivery would make this data-driven.
- `Chain` payload is scaffolded but currently no-ops in `apply_payloads` (Burrowstrike's chaining
  uses `BuffDef.on_death` instead). Will be wired when an ability needs mid-resolution chaining.

---

*See also:* [composable-effects.md](composable-effects.md) for the design rationale and migration
history.
