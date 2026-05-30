# Composable Ability Effects — Design Note (for sign-off)

> Status: **PROPOSED — design for review, not yet implemented.** Defines a data-driven, composable
> replacement for today's bespoke per-ability `Effect` enum so new abilities can be authored in RON
> alone. Key decisions are resolved in §7.

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

## 8. Migration path

1. Add the composable `EffectSpec` types in aa2-data **alongside** the existing `Effect` enum (no break).
2. Implement the generic resolvers for the primitives the simplest abilities need; port **Rage + one
   plain damage ability** as the proof. Move those abilities' tests to the new model.
3. Port the remaining abilities **one at a time** — each port = bespoke variant → composition, then
   delete that variant + its sim arm(s) and migrate its tests. The fixed-seed sim tests gate each port.
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
  existing `IllusionInteraction`; confirm during that port.
- **PRD / attack-timing** for OnAttack triggers — preserve current Chaos Strike / Glaives behavior
  exactly (tests pin it).
- **Sub-effects / chaining** (caustic finale, fire trail) — modeled as a triggered child `EffectSpec`;
  validate the recursion stays bounded.
- **Over-generalization risk** — this is deliberately a fixed primitive set, not an interpreter; flag
  if a port pushes toward a VM.

---

*Sign-off:* once approved, implement per §8 — composable types in aa2-data, generic resolvers in
aa2-sim, port abilities incrementally (tests green at each step), then remove the bespoke enum.
Composable effects are the gating item for adding game content by data alone.
