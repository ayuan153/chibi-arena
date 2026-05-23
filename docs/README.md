# AA2 Documentation

Start here. This is a standalone cross-platform autobattler (iOS/Android/PC) inspired by the Dota2 mod Ability Arena.

## Quick Orientation

| I want to... | Read |
|-------------|------|
| Understand the project timeline and current phase | [project-plan.md](project-plan.md) |
| Understand game rules (economy, draft, combat, gods) | [specs/game-systems.md](specs/game-systems.md) |
| Understand Dota2 combat formulas (damage, armor, attack speed) | [specs/mechanics-reference.md](specs/mechanics-reference.md) |
| Understand crate structure and system design | [design/architecture.md](design/architecture.md) |
| Understand the FFI bridge (Rust → Unity) | [design/ffi-bridge.md](design/ffi-bridge.md) |
| Understand the Unity client design | [design/unity-client.md](design/unity-client.md) |
| Understand combat AI behavior | [design/combat-ai.md](design/combat-ai.md) |
| Understand the test framework for game logic | [design/testing/game-scenarios.md](design/testing/game-scenarios.md) |
| Understand the test framework for combat sim | [design/testing/sim-fixtures.md](design/testing/sim-fixtures.md) |

## Project Structure

```
aa2/
├── crates/
│   ├── aa2-data/     — Data types + RON loaders (no logic)
│   ├── aa2-sim/      — Combat simulation (Dota2-fidelity)
│   └── aa2-game/     — Game loop: economy, draft, shop, gods, matchups
├── data/
│   ├── heroes/       — Hero body definitions (.ron)
│   └── abilities/    — Ability definitions (.ron)
├── docs/             — You are here
│   ├── design/       — How things are built (architecture, testing)
│   └── specs/        — What the game does (rules, formulas)
└── skills/           — Agent skills for common tasks
```

## Current Status

- **Phase 0** ✓ Foundation (Rust workspace, combat prototype)
- **Phase 1** ✓ Combat fidelity (buffs, abilities, 5v5, replays)
- **Phase 2** ✓ Game systems (economy, draft, shop, gods, CLI dev mode)
- **Phase 3** ← CURRENT: Client + Platform (FFI bridge, Unity, visual game)
- **Phase 4** Multiplayer (server, networking, matchmaking)

## Dev Workflow

```bash
cargo check            # Fast compile check
cargo test             # All tests (should always pass)
cargo clippy -- -D warnings  # Lint (treat warnings as errors)
cargo run -p aa2-game --bin aa2-dev -- <seed>  # Play the game (CLI)
```

## Agent Skills

| Skill | File | Purpose |
|-------|------|---------|
| Adding abilities | `skills/adding-abilities.md` | Research + add new abilities to data |
| Adding hero bodies | `skills/add-hero-body.md` | Research + add new heroes to data |
| Sprint execution | `skills/sprint-execution.md` | Autonomous sprint implementation workflow |
