# AA2 — Ability Arena 2

<!-- Badges -->
![Status](https://img.shields.io/badge/status-Phase%203%20Client-blue)
![Rust](https://img.shields.io/badge/rust-stable-orange)
![License](https://img.shields.io/badge/license-MIT-green)

A standalone cross-platform autobattler inspired by the Dota 2 mod Ability Arena. Eight players compete in a free-for-all, picking gods, drafting hero bodies, and equipping abilities to outlast their opponents.

## Status: Phase 3 In Progress — Client (Godot + gdext)

Game systems complete with full game loop, economy, draft, shop, gods, and 234 tests. Godot client has all screens (shop, board, draft, combat viewer, scoreboard, dev console).

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Simulation | Rust (`aa2-sim`, `aa2-data` crates) |
| Game Logic | Rust (`aa2-game` crate) |
| Client | Godot 4.3 + gdext (`aa2-client` crate) |
| Server | Rust (`aa2-server`, Phase 4) |
| Networking | WebSocket, state-sync at 10 Hz |
| Data | RON files (dev) / PostgreSQL JSONB (production) |

## Project Structure

```
aa2/
├── crates/
│   ├── aa2-sim/        # Deterministic combat simulation
│   ├── aa2-data/       # Shared types, schemas, RON loaders
│   ├── aa2-game/       # Game state machine, economy, draft
│   ├── aa2-client/     # Godot GDExtension (gdext)
│   └── aa2-server/     # Authoritative game server (Phase 4)
├── client/             # Godot 4.3 project
├── data/               # RON data files (gods, abilities, bodies)
├── docs/               # Architecture & design documentation
├── tests/              # Integration tests
└── README.md
```

## Getting Started

### Prerequisites

- **Rust** (stable, latest) — [rustup.rs](https://rustup.rs)
- **Godot 4.3+** — for client work only

### Build & Test

```bash
cargo build
cargo test              # Run all tests (unit + integration)
cargo test --test integration_mechanics  # Run only mechanic interaction tests
cargo clippy            # Lint (must pass with no warnings)
```

### Test Philosophy

Every mechanic verification is encoded as an automated test. If you can observe it in the combat log, it should be an assertion in a test file. Tests use actual RON data files and deterministic seeds — same input always produces same output.

### Run CLI Dev Mode (no Godot needed)

```bash
cargo run --bin aa2-dev                                    # 1v1 default (Sven vs Drow)
cargo run --bin aa2-dev -- data/heroes/sven.ron data/heroes/drow.ron  # Custom 1v1
cargo run --bin aa2-dev -- --5v5                            # 5v5 brawl with all heroes
cargo run --bin aa2-dev -- --loadout data/loadouts/sven_ravage.ron data/loadouts/cm_ravage.ron  # With abilities
```

### Run Godot Client (visual game)

**Prerequisites:** [Godot 4.3+](https://godotengine.org/download) installed and in PATH.

```bash
# First time setup (installs Godot via Homebrew)
./dev setup

# Build + launch (one command)
./dev

# Other modes:
./dev client   # Rebuild aa2-client only + launch (fastest iteration)
./dev run      # Skip build, just launch Godot (already built)
./dev check    # cargo check + clippy + test (no Godot)
```

Cargo handles incremental compilation automatically — only changed crates rebuild. Typical iteration: edit Rust → `./dev` → see changes in ~3s.

**Dev Console:** Always visible on the right side. Type commands:
- `state` — show gold, HP, phase, round
- `gold 99` — set gold to 99
- `hp 50` — set HP to 50
- `buy 0` — buy shop slot 0
- `reroll` — reroll shop
- `combat` — force run combat
- `ready` — end current phase
- `help` — list all commands

## Heroes Available

Sven, Drow Ranger, Chaos Knight, Juggernaut, Crystal Maiden, Io (6 heroes, 11 abilities)

## Game Overview

1. **God Pick** — Each player selects a god that grants a passive bonus for the match.
2. **Draft Phase** — Players draft hero bodies and abilities from a shared pool.
3. **Equip** — Assign abilities to hero body slots, building synergies.
4. **Combat** — Automated round-robin battles between player boards.
5. **Elimination** — Players lose HP on defeat; last player standing wins.

Matches support 8 players with rounds of increasing intensity.

## Documentation

| Document | Description |
|----------|-------------|
| [docs/architecture.md](docs/architecture.md) | Technical architecture & system design |
| [docs/project-plan.md](docs/project-plan.md) | Phased development plan |
| [docs/mechanics-reference.md](docs/mechanics-reference.md) | Engine formulas & combat mechanics |

## Contributing

This project is in early development. Contribution guidelines will be published once the foundation stabilizes.

## License

[MIT](LICENSE)
