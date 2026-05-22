# Game Scenario Test Framework

## Purpose

Deterministic, fixture-based integration tests for the aa2-game crate. Every game mechanic change and bugfix gets locked in with a scenario test that replays a specific sequence of actions and asserts specific outcomes.

## Design Goals

1. **Deterministic** — Fixed seed, fixed actions, same result every time
2. **Readable** — A scenario reads like a game transcript: "round 1, player buys X, equips Y"
3. **Fast** — No I/O, no CLI, no real-time. Pure game logic replay. Target: <10ms per scenario
4. **Composable** — Reuse setup helpers, mix scripted players with random AI
5. **Regression-friendly** — When a bug is found, capture the state as a scenario in minutes

## Core Types

```rust
/// A complete test scenario with deterministic replay.
pub struct GameScenario {
    /// RNG seed for all randomness (pool init, AI decisions, combat)
    pub seed: u64,
    /// Number of players (2-8)
    pub num_players: u8,
    /// Initial setup applied before round 1
    pub setup: Vec<SetupAction>,
    /// Scripted player actions, keyed by (round, player_id)
    pub actions: Vec<RoundActions>,
    /// Assertions checked after specific rounds
    pub assertions: Vec<RoundAssertion>,
}

/// Actions applied during game setup (before round 1)
pub enum SetupAction {
    /// Give a player a specific hero at a position
    AddHero { player: u8, hero: String, x: f32, y: f32 },
    /// Give a player an ability at a specific level
    AddAbility { player: u8, ability: String, level: u32 },
    /// Equip an ability on a hero
    Equip { player: u8, ability: String, hero: String },
    /// Set a player's god
    SetGod { player: u8, god: God },
    /// Set a player's gold
    SetGold { player: u8, gold: u32 },
    /// Set a player's shop level
    SetShopLevel { player: u8, level: u32 },
}

/// Actions for a specific round
pub struct RoundActions {
    pub round: u32,
    pub player: u8,
    pub actions: Vec<Action>,
}

/// A player action during the shop phase
pub enum Action {
    Buy(usize),              // Buy from shop slot (0-indexed)
    Sell(String),            // Sell ability by name
    Equip(String, String),   // Equip ability to hero
    Unequip(String, String), // Unequip ability from hero
    RerollShop,              // Reroll shop (1g)
    UpgradeShop,             // Upgrade shop level
    LockShop,                // Toggle shop lock
    DraftHero(usize),        // Pick draft choice (0-indexed)
    RerollHero(String),      // Replace existing hero (2g)
    SetPosition(String, f32, f32), // Move hero
    SetGodBuff(String),      // Paladin: set buff target
}

/// An assertion checked after a specific round's combat
pub struct RoundAssertion {
    pub after_round: u32,
    pub check: fn(&GameState) -> Result<(), String>,
}
```

## Execution Model

```
1. Create GameState with seed
2. Apply SetupActions (heroes, abilities, gold, gods)
3. For each round:
   a. If draft round: auto-pick for unscripted players (random)
   b. Roll shops for all players
   c. Execute scripted RoundActions for this round
   d. Unscripted players: random AI decisions (buy, equip)
   e. Run combat (deterministic with seed)
   f. Apply damage, eliminate dead
   g. Run RoundAssertions for this round
   h. If alive_count <= 1: game over
4. Run any final assertions
```

## Unscripted Players (Degrees of Freedom)

Players without scripted actions for a round use random AI:
- Buy random affordable abilities
- Equip to heroes with empty slots
- Pick random draft hero if available
- Never reroll, sell, or upgrade (keeps them predictable)

This means a 2-player combat test only needs to script 2 players — the other 6 are noise that doesn't affect the assertion (or can be set to `alive: false` in setup).

## Example Scenarios

### Minimal: Positioning Matters
```rust
GameScenario {
    seed: 42,
    num_players: 2,
    setup: vec![
        // Player 0: Jugg front, Drow back
        AddHero { player: 0, hero: "Juggernaut", x: 500.0, y: 100.0 },
        AddHero { player: 0, hero: "Drow Ranger", x: 500.0, y: 300.0 },
        // Player 1: Drow front, Jugg far corner
        AddHero { player: 1, hero: "Drow Ranger", x: 500.0, y: 100.0 },
        AddHero { player: 1, hero: "Juggernaut", x: 1900.0, y: 900.0 },
    ],
    actions: vec![],  // No shop actions needed
    assertions: vec![
        RoundAssertion { after_round: 2, check: |g| {
            assert!(g.players[0].hp > g.players[1].hp);
            Ok(())
        }},
    ],
}
```

### Regression: Archmage Sorcery on Upgrade
```rust
GameScenario {
    seed: 100,
    num_players: 2,
    setup: vec![
        SetGod { player: 0, god: archmage() },
        AddHero { player: 0, hero: "Sven", x: 1000.0, y: 500.0 },
        AddAbility { player: 0, ability: "Rage", level: 1 },
        Equip { player: 0, ability: "Rage", hero: "Sven" },
        SetGold { player: 0, gold: 20 },
    ],
    actions: vec![
        RoundActions { round: 1, player: 0, actions: vec![Action::UpgradeShop] },
    ],
    assertions: vec![
        RoundAssertion { after_round: 1, check: |g| {
            // Sorcery should have triggered (guaranteed on upgrade)
            let rage_level = g.players[0].abilities["Rage"];
            assert_eq!(rage_level, 2, "Archmage sorcery should upgrade Rage on shop upgrade");
            Ok(())
        }},
    ],
}
```

### Full Game: Dominant Player Wins
```rust
GameScenario {
    seed: 7,
    num_players: 4,
    setup: vec![
        // Player 0: stacked team
        AddHero { player: 0, hero: "Juggernaut", x: 1000.0, y: 500.0 },
        AddAbility { player: 0, ability: "Fury Swipes", level: 5 },
        Equip { player: 0, ability: "Fury Swipes", hero: "Juggernaut" },
        // Players 1-3: naked heroes (auto-setup by framework)
    ],
    actions: vec![],
    assertions: vec![
        RoundAssertion { after_round: 15, check: |g| {
            assert_eq!(g.alive_count(), 1);
            assert!(g.players[0].alive);
            Ok(())
        }},
    ],
}
```

## File Organization

```
crates/aa2-game/
├── src/
│   └── scenario.rs          ← Framework implementation (pub mod, usable in tests)
└── tests/
    ├── fixtures.rs           ← Migrate existing fixtures to use GameScenario
    ├── mechanics.rs          ← Keep as-is (lower-level integration tests)
    └── game_loop.rs          ← Keep as-is (full loop smoke tests)
```

## When to Write a Scenario Test

| Trigger | Example |
|---------|---------|
| Bug found during manual testing | "Shop lock didn't preserve after upgrade" |
| New mechanic implemented | "Paladin buff applies 70×round HP" |
| Balance change | "Gold formula changed from X to Y" |
| Edge case discovered | "Ghost opponent with 0 heroes doesn't crash" |
| Combat interaction | "Fury Swipes level 5 kills Treant in N hits" |

## Relationship to Sim Tests

The sim crate (`aa2-sim`) has its own test patterns for combat mechanics (attack timing, buff interactions, ability effects). Game scenarios test the *game loop* layer — economy, draft, equip, round flow, god passives, and how combat results feed back into game state.

| Layer | Tests | Concerns |
|-------|-------|----------|
| `aa2-sim` | Unit + integration in sim crate | Combat formulas, buff interactions, ability effects |
| `aa2-game` unit | In-module `#[cfg(test)]` | Individual function correctness |
| `aa2-game` scenarios | `tests/fixtures.rs` via GameScenario | Multi-round game flow, player decisions, regressions |
