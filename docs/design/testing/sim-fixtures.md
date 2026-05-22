# Sim Fixture Tests

## Purpose

Deterministic tests for the aa2-sim combat simulation. Every ability, buff interaction, and combat mechanic is locked in with a fixture that sets up specific units and asserts specific combat outcomes.

## Pattern

```rust
/// [What this tests] — [Why it matters for game feel]
#[test]
fn test_specific_interaction() {
    // 1. Load actual RON data files (proves data + code work together)
    let ability = load_ability_def(Path::new("../../data/abilities/X.ron")).unwrap();

    // 2. Set up exact initial conditions (deterministic)
    let mut unit = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    unit.abilities.push(AbilityState { def: ability, level: 3, .. });

    // 3. Run simulation with fixed seed
    let mut sim = Simulation::with_seed(vec![unit, enemy], 42);
    for _ in 0..N { sim.step(); }

    // 4. Assert specific properties (not approximate unless necessary)
    assert!(sim.combat_log.iter().any(|e| matches!(e, CombatEvent::X { .. })));
}
```

## Rules

- Tests MUST be deterministic (fixed seed, exact positions, no timing dependencies)
- Tests MUST be fast (< 100ms each, ideally < 1ms)
- Tests MUST use actual RON data files where the mechanic depends on data values
- Tests MUST have a doc comment explaining what interaction they verify
- Never verify by "eyeballing" output — if it's worth checking, it's worth asserting

## Where Tests Live

| Test type | Location | Tests what |
|-----------|----------|-----------|
| Unit tests | `crates/aa2-sim/src/*.rs` (`#[cfg(test)]`) | Individual formulas, single-function behavior |
| Integration tests | `crates/aa2-sim/tests/*.rs` | Multi-system interactions, full sim runs, data file loading |

## When to Write a Sim Test

- New ability added → test it deals expected damage/applies expected buff
- Buff interaction discovered → test the interaction
- Attack modifier added → test it modifies damage correctly
- Timing-sensitive mechanic → test tick-level precision
- Combat log event → test it appears at the right time

## Relationship to Game Scenarios

Sim tests verify combat *mechanics* (formulas, timing, interactions).
Game scenarios verify the *game loop* (economy, draft, how combat results affect game state).

A sim test: "Fury Swipes at level 3 deals 36 bonus damage per stack"
A game scenario: "Player with Fury Swipes level 5 wins against naked heroes by round 3"
