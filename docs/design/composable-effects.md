# Composable Ability Effects — Design Note (for sign-off)

> Status: **IMPLEMENTED (2026-05-30).** The composable effect system is live — all 11 abilities
> ported, the bespoke `Effect` enum deleted, gate green (271 Rust + 47 GDScript tests). New
> abilities are authored in RON alone. See §11 for the final shipped primitives.

## 1. Problem

Every ability today is a bespoke `Effect` enum variant (`crates/aa2-data/src/lib.rs` — 15 variants)
plus hand-written simulation code. **All 11 shipped abilities have their own variant; none are
expressible with the generic `Damage`/`Heal`/`ApplyBuff` primitives alone.** Adding one new mechanic
touches 3–6 files:

- a new `Effect` variant + an `illusion_interaction()` arm (aa2-data),
- a match arm in `ability.rs::execute_ability` (one or both passes),
- often a `PendingEffectKind` variant + its per-tick logic (`pending.rs` + `lib.rs`) for
  delayed / projectile / over-time effects,
- sometimes an `attack_modifier.rs` arm (on-attack effects) or an on-death hook in `lib.rs`.

This won't scale. The next milestone is content, which today means a Rust change + recompile per
ability — designers cannot add abilities by data.

## 2. Goal

A new ability = a **composition of reusable primitives expressed in RON**, with **no new Rust** for
the common case. The engine implements each primitive once; abilities are data.

**Non-goal:** a fully general "effect VM." We want the smallest set of orthogonal primitives that
expresses the existing 11 abilities (and obvious near-future ones), not Turing-completeness.

## 3. Model: four orthogonal axes

An ability becomes a list of **`EffectSpec`s**, each = (Trigger, Targeting, Delivery, Payload[]):

| Axis | Answers | Composable primitives (data) |
|------|---------|------------------------------|
| **Trigger** | When does it fire? | `OnCast`, `OnAttack`(+PRD chance / mana), `OnHit`, `OnKill`, `Periodic` |
| **Targeting** | Who / where? | reuse `TargetType` (SingleEnemy/Ally/Point/None/Self) + `CastBehavior` |
| **Delivery** | How does it reach units? | `Instant`, `Aoe(AoeShape)`, `Projectile{homing\|linear, speed, bounce}`, `CasterTravel{width,speed}`, `ExpandingWave{speed}`, `Delayed`/`Pulse{delay,count,interval}` |
| **Payload[]** | What happens per affected unit? | `Damage{type,amount}`, `Heal{amount}`, `ApplyBuff(BuffSpec)`, `Dispel{type}`, `Spawn{illusion\|unit}`, `StatSteal`, `SelfDamage`, `Crit`/`Lifesteal` |

The engine has **one generic resolver per primitive**: a Targeting resolver, a Delivery layer that
places/animates and yields hit units over time (generalizing today's `PendingEffectKind`), and a
Payload applier that runs each component on each hit unit. **Composition replaces enumeration.**

## 4. Leverage: the buff system is already composable

`aa2-sim::buff::Buff` already composes `StatusFlags` (stun/silence/disarm/root/hex/invuln/
magic_immune), `StatModifier` (10 additive stats incl. `move_speed` → slows), `TickEffect` (DoT/HoT),
`StackBehavior`, `DispelType`, and `damage_reflection_pct` — with **no per-buff Rust**.

**`ApplyBuff(BuffSpec)` is the template for the whole design.** Most "status" mechanics today
(Rage's magic immunity, slows, stuns, Heavenly Grace's regen, Fury Swipes' armor reduction) are
already just buff configurations. The remaining work is generalizing the **Delivery** axis (today's
bespoke `PendingEffectKind`) and the **Trigger** axis (today split across `ability.rs`,
`attack_modifier.rs`, and on-death hooks) the same way the buff system already does for payloads.

## 5. Mapping (proof the model covers today's abilities)

| Ability | Trigger | Delivery | Payload |
|---------|---------|----------|---------|
| Rage | OnCast | Instant(self) | Dispel(Basic), ApplyBuff{magic_immune} |
| Heavenly Grace | OnCast | Instant(self+ally) | ApplyBuff{regen,str,status_resist}, Dispel |
| Dark Pact | OnCast | Delayed+Pulse{delay,count,interval; Circle} | Damage(AoE), SelfDamage, Dispel(self) |
| Ravage | OnCast | ExpandingWave{speed; Circle} | Damage, ApplyBuff{stun} |
| Burrowstrike | OnCast | CasterTravel{width,speed} | Damage, ApplyBuff{stun}; **+OnKill→**Aoe Damage (caustic finale) |
| Spirit Lance | OnCast | Projectile{homing, bounce} | Damage, ApplyBuff{slow}, Spawn{illusion} |
| Spear of Mars | OnCast | Projectile{linear, wall-bounce} | Damage, ApplyBuff{stun}, Periodic Aoe (fire trail) |
| Fury Swipes | OnAttack | Instant(target) | Damage(stacking), ApplyBuff{armor_reduction} |
| Chaos Strike | OnAttack(PRD) | Instant(target) | Crit, Lifesteal |
| Essence Shift | OnAttack | Instant(target) | StatSteal, ApplyBuff(self) |
| Glaives of Wisdom | OnAttack(mana) | Instant(target) | Damage(INT-scaled), StatSteal |

All 11 decompose into the four axes. The long tail (illusion spawn, fire trail, wall bounce, caustic
finale) becomes a **Delivery/Payload primitive reused across abilities**, not a one-off variant.

## 6. Crate impact

- **aa2-data:** replace the 15-variant `Effect` with composable `EffectSpec { trigger, targeting,
  delivery, payload: Vec<Payload> }` types (+ a `BuffSpec` mirroring the runtime `Buff`). Keep
  `AoeShape` / `TargetType` / `CastBehavior` / `DamageType` / `IllusionInteraction`.
- **aa2-sim:** one generic resolver per axis (Targeting; a Delivery layer over a generalized
  `PendingEffect`; a Payload applier; Trigger hooks unifying cast/attack/kill/hit/periodic). The
  bespoke `execute_ability` match, per-ability `PendingEffectKind` arms, and `attack_modifier` arms
  collapse into these resolvers.
- **aa2-game / aa2-net / aa2-client:** unaffected — abilities are still `AbilityDef` loaded from RON,
  and the wire `CombatEvent` log is unchanged.

## 7. Resolved decisions

1. **Reuse `Buff` as the payload primitive** — it is already composable; do not reinvent status effects.
2. **Generalize, don't enumerate, Delivery** — a fixed small set of delivery primitives
   (instant / aoe / projectile / caster-travel / expanding-wave / delayed-pulse), each implemented
   once and parameterized by data. Not a general geometry engine.
3. **Unify triggers** — fold the attack-modifier pipeline and on-death hooks into a `Trigger` axis so
   on-attack / on-kill effects are just `EffectSpec`s.
4. **Incremental migration, no big-bang** (§8) — keep the ~140 aa2-sim tests green throughout.
5. **RON stays the authoring format** — no new DSL; composition is nested RON. Revisit only if RON
   proves too verbose in practice.
6. **Sub-effects / chaining** (was open in §10) — modeled as a `Payload::Chain(Box<EffectSpec>)`
   variant with a hard recursion bound `MAX_EFFECT_CHAIN_DEPTH = 2`.
7. **Migration dispatch** — add an additive `#[serde(default)] effect_specs: Option<Vec<EffectSpec>>`
   field on `AbilityDef`. If present, the engine runs the new resolver path and SKIPS the old
   `Effect` match entirely for that ability (all-or-nothing per ability; never split one ability
   across both paths). Unported RON files stay untouched thanks to serde default. The old `Effect`
   enum + old paths are deleted only when all 11 abilities are ported.
8. **Buff data ownership** (was M1) — chose the refactor (option b) over a duplicate spec. The buff
   *data schema* (`StatusFlags`, `StatModifier`, `StackBehavior`, `DispelType`, and a new
   `TickEffectDef` + `BuffDef`) lives in aa2-data; the runtime `Buff` in aa2-sim reuses those same
   types and adds runtime state (`remaining_ticks`, tick countdown), constructed via
   `Buff::from_def(level, source_id)`. Single source of truth for the schema; no parallel duplicate
   type.
9. **Proof abilities** — Rage + **Ravage** (NOT a generic plain-damage ability — no shipped ability
   uses the generic `Effect::Damage`; it is test-fixture only). Ravage's `ExpandingWave` delivery
   deliberately exercises the Delivery axis early.

## 8. Migration path

1. Add the composable `EffectSpec` types in aa2-data **alongside** the existing `Effect` enum (no break).
2. Implement the generic resolvers for the primitives the simplest abilities need; port **Rage +
   Ravage** as the proof. Move those abilities' tests to the new model.
3. Port the remaining abilities **one at a time** — each port = bespoke variant → composition, then
   delete that variant + its sim arm(s) and migrate its tests. The fixed-seed sim tests gate each port.
   - **Test-migration discipline:** keep each ability's OLD tests compiling against the old variant
     until that variant is deleted; write PARALLEL new tests for the composed version; delete the old
     variant + its old tests in the SAME commit; never rewrite a test in place.
4. When all 11 are ported, delete the old `Effect` enum, the per-ability `PendingEffectKind` arms, and
   the `attack_modifier` match. New abilities thereafter are **RON-only**.

## 9. Success criteria

- A new representative ability (e.g. a nuke that also applies a slow, plus a pure DoT) is added by
  **RON alone** — no Rust, no recompile of sim logic.
- All 11 existing abilities are expressed as compositions; the full sim suite and `./dev test` stay green.
- Adding a brand-new *delivery* or *payload* primitive (rare) is a single localized resolver addition,
  reusable by any ability.

## 10. Explicitly deferred / open questions

- **How general does Delivery need to be?** Start with the six primitives above; resist a generic
  physics/geometry VM until a real ability demands it.
- **Spawn / illusion semantics** (Spirit Lance) — likely its own `Payload` primitive carrying the
  existing `IllusionInteraction`; confirm during that port. **Spirit Lance is ported LAST** — its
  `Spawn{illusion}` clones a full `Unit` into the `units` Vec, making it the highest-risk port.
- **PRD / attack-timing** for OnAttack triggers — preserve current Chaos Strike / Glaives behavior
  exactly (tests pin it).
- ~~**Sub-effects / chaining** (caustic finale, fire trail)~~ — **RESOLVED**, see §7 item 6.
- ~~**Proof-ability selection**~~ — **RESOLVED**, see §7 item 9.
- **Over-generalization guard** — this is deliberately a fixed primitive set, not an interpreter. If
  a new ability needs a delivery primitive with >3 parameters or requires a 3rd new primitive, treat
  it as a smell and consider a bespoke path.
- **Deterministic ordering** — triggers resolve in ability-slot order per unit; units processed in
  `self.units` index order.

---

## 11. Implemented (final state)

Implementation complete 2026-05-30 on branch `feat/composable-effects`. All 11 abilities ported;
the bespoke `Effect` enum and its per-ability match arms are deleted. Behavior is byte-identical
(271 Rust + 47 GDScript tests green, wasm32 compiles). A new ability now requires only a RON file.

### Final primitive sets shipped

| Axis | Primitives |
|------|-----------|
| **Trigger** | `OnCast`, `OnAttack`, `OnKill` |
| **TargetingSpec** | `Caster`, `EnemiesInDelivery`, `TargetAndCaster`, `AttackTarget` |
| **Delivery** | `Instant`, `ExpandingWave`, `DelayedPulse`, `CasterTravel`, `Aoe`, `Projectile{homing\|linear}` |
| **Payload** (15) | `Damage`, `Heal`, `ApplyBuff(BuffDef)`, `Dispel`, `Chain` (bounded by `MAX_EFFECT_CHAIN_DEPTH=2`), `SelfDamage`, `DamageWithSourceMaxHp`, `StackingBonusDamage`, `Crit`, `Lifesteal`, `StatSteal`, `IntScaledDamage`, `AttackBounce`, `PermanentIntSteal`, `Spawn(illusion)` |

### Where things live

- **aa2-data:** `EffectSpec`, `Payload`, `Delivery`, `TargetingSpec`, `Trigger`, plus the buff data
  schema (`BuffDef`, `StatModifierSpec`, `StatusFlags`, `StatModifier`, `StackBehavior`,
  `DispelType`, `TickEffectDef`). Abilities authored via `effect_specs: [EffectSpec]` in RON.
- **aa2-sim:** generic resolver in `crates/aa2-sim/src/effect_spec.rs`
  (`run_cast_effect_specs`, `apply_payload_to_unit`, `resolve_on_death_spec`). Deliveries driven by
  `PendingEffectKind::Composable*` variants in `lib.rs`. `OnAttack`/`OnKill` triggers hooked into
  the attack pipeline (`attack_modifier.rs`) and `check_deaths`.
- **Runtime buff:** `Buff::from_def` in aa2-sim constructs runtime `Buff` from aa2-data's `BuffDef`.

---

*Sign-off:* implemented per §8. Composable effects are live — new abilities are RON-only.
