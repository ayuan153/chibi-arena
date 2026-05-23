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
cargo check                        # Fast compile check
cargo test                         # All tests (231, should always pass)
cargo clippy -- -D warnings        # Lint (treat warnings as errors)
```

## Playing Locally (CLI Dev Mode)

```bash
cargo run -p aa2-game --bin aa2-dev
```

This launches an interactive single-player game against 7 AI opponents. You play as Player 0.

**Optional:** Pass a seed for reproducible runs:
```bash
cargo run -p aa2-game --bin aa2-dev -- 42
```

### Quick Start

1. Pick a god (1 or 2)
2. `draft <1-3>` — pick a hero from the tier-locked choices
3. `buy <1-4>` — buy abilities from the shop
4. `equip <ability> <hero>` — equip to a hero (use snake_case: `equip fury_swipes spectre`)
5. `position <hero> <x> <y>` — place hero on your half (0-2000 x, 0-1000 y)
6. `ready` — end shop phase, start combat

### All Commands

| Command | Description |
|---------|-------------|
| `ready` | End shop phase |
| `shop` | Show shop offerings |
| `buy <index>` | Buy ability (1-indexed) |
| `sell <name>` | Sell ability (snake_case) |
| `reroll` | Reroll shop (1g) |
| `lock` | Toggle shop lock |
| `upgrade` | Upgrade shop level |
| `bench` | Show bench |
| `heroes` | Show heroes + stats + abilities |
| `equip <a> <h>` | Equip ability to hero |
| `unequip <a> <h>` | Unequip ability (1g) |
| `draft <1\|2\|3>` | Pick hero from draft |
| `reroll-hero <h>` | Replace hero (2g) |
| `position <h> <x> <y>` | Set hero position |
| `god` | Show god info |
| `buff <hero>` | Set Paladin buff target |
| `status` | Show gold, HP, round |
| `players` | Show all players' HP |
| `log` | List combat logs |
| `log <N>` | View specific matchup log |
| `help` | Show all commands |

### Tips

- Names use snake_case in commands: `crystal_maiden`, `fury_swipes`, `spear_of_mars`
- The `heroes` command shows slugs in brackets: `Spectre [spectre]`
- After combat, type `log` to see available matchup logs, then `log 1` to view details
- Sorcery (Archmage) procs are shown with ✨ at shop start

## Agent Skills

| Skill | File | Purpose |
|-------|------|---------|
| Adding abilities | `skills/adding-abilities.md` | Research + add new abilities to data |
| Adding hero bodies | `skills/add-hero-body.md` | Research + add new heroes to data |
| Sprint execution | `skills/sprint-execution.md` | Autonomous sprint implementation workflow |
