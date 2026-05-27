# Chibi Arena

A cross-platform autobattler built in Rust with Godot 4.3 via GDExtension. Deterministic combat simulation, multi-crate architecture, WebSocket networking with 10 Hz state sync, and 263 tests across Rust and GDScript.

Eight players compete in free-for-all matches — picking gods, drafting hero bodies, and equipping abilities to outlast their opponents.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Simulation | Rust — deterministic combat engine with fixed-seed reproducibility |
| Game Logic | Rust — state machine, economy, draft system |
| Client | Godot 4.3 + GDExtension via gdext (Rust → GDScript FFI) |
| Server | Rust — authoritative game server (Phase 4) |
| Networking | WebSocket, state-sync at 10 Hz |
| Data | RON files (dev) / PostgreSQL JSONB (production) |

## Status

Sprint 1 complete — core game loop playable:
- God pick → Shop (buy/reroll/lock/upgrade) → Draft heroes → Equip abilities → Combat → repeat
- 2-player local dev mode, 6 heroes, 11 abilities
- 29 integration tests, 234 unit tests

## Quick Start

### Prerequisites

- **Rust** (stable) — [rustup.rs](https://rustup.rs)
- **Godot 4.3+** — for client work

### Build & Run

```bash
./dev          # Build + launch Godot client
./dev editor   # Build + open Godot editor
./dev check    # cargo check + clippy + test
./dev test     # Build + run integration tests (requires display)
```

### Testing

```bash
cargo test              # 234 Rust tests (game logic, sim, data)
./dev test              # 29 GDScript integration tests (full Godot + FFI)
cargo clippy -- -D warnings  # Lint
```

All tests use fixed seeds for determinism. Integration tests require a display server (macOS works natively, Linux needs Xvfb).

## Architecture

```
chibi-arena/
├── crates/
│   ├── aa2-sim/        # Deterministic combat simulation
│   ├── aa2-data/       # Shared types, schemas, RON loaders
│   ├── aa2-game/       # Game state machine, economy, draft
│   ├── aa2-client/     # Godot GDExtension (gdext)
│   └── aa2-server/     # Authoritative game server (Phase 4)
├── client/             # Godot 4.3 project
│   └── tests/          # GDScript integration tests
├── data/               # RON data files (gods, abilities, bodies)
├── docs/               # Architecture & design documentation
├── tests/              # Rust integration tests
└── README.md
```

Key design decisions:
- **Multi-crate workspace** — sim, data, game, client, and server are independent crates with clean dependency boundaries
- **Deterministic simulation** — fixed-seed combat enables reproducible tests, replays, and server-authoritative validation
- **Rust → Godot FFI via gdext** — game logic lives in Rust; Godot handles rendering and input through GDExtension bindings
- **Data-driven design** — game content (gods, abilities, heroes) defined in RON files, loaded at runtime

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

All changes must pass before merge:
```bash
cargo clippy -- -D warnings
cargo test
./dev test
```

See [AGENTS.md](AGENTS.md) for test philosophy and commit conventions.

## License

[MIT](LICENSE)
