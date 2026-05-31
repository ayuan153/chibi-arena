# Chibi Arena

A cross-platform autobattler built in Rust with Godot 4.6 via GDExtension. Deterministic combat simulation, multi-crate architecture, and server-authoritative WebSocket networking (dumb-client state-sync), with 271 Rust and 47 GDScript tests.

Eight players compete in free-for-all matches — picking gods, drafting hero bodies, and equipping abilities to outlast their opponents.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Simulation | Rust — deterministic combat engine with fixed-seed reproducibility |
| Game Logic | Rust — state machine, economy, draft, shared action dispatch |
| Client | Godot 4.6 + GDExtension via gdext (Rust → GDScript FFI) |
| Server | Rust — authoritative game server (tokio + tokio-tungstenite) |
| Networking | WebSocket, server-authoritative state-sync |
| Data | RON files (dev) / PostgreSQL JSONB (production) |

## Status

Local game loop + networking complete — playable end-to-end locally and over WebSocket:
- God pick → Shop (buy/reroll/lock/upgrade) → Draft → Equip → Combat (animated) → Endgame
- Server-authoritative dumb-client state-sync: lobby, AI bot fill, full game to GameOver
- 2-player local dev mode, and 2+ humans (sockpuppet seats) + AI fill to 8 over WebSocket
- 21 heroes, 11 abilities, 2 gods (data-driven); no items or art yet (placeholder shapes)
- 271 Rust tests, 47 GDScript integration tests (all deterministic, fixed seed)

Composable ability effects complete — abilities are now fully data-driven (RON-only, no Rust needed). Next: content + playtesting (hot-reload for fast iteration).

## Quick Start

### Prerequisites

- **Rust** (stable) — [rustup.rs](https://rustup.rs)
- **Godot 4.6** — for client work

### Build & Run

```bash
./dev          # Build + launch Godot client (local mode)
./dev editor   # Build + open Godot editor
./dev check    # cargo check + clippy + test
./dev test     # Build + run integration tests (requires display)
```

Networked play:
```bash
cargo run -p aa2-server                # Terminal 1: server on 127.0.0.1:9001
AA2_SERVER=ws://127.0.0.1:9001 ./dev   # Terminal 2+: each client claims a seat
```

### Testing

```bash
cargo test                                   # 271 Rust tests (game logic, sim, data)
./dev test                                   # 47 GDScript integration tests (full Godot + FFI)
./dev net-smoke                              # Networked smoke test (server + headless client)
cargo clippy --all-targets -- -D warnings    # Lint gate
```

All tests use fixed seeds for determinism. Integration tests require a display server (macOS works natively, Linux needs Xvfb). Run the gate steps separately — chaining thrashes the incremental cache.

## Architecture

```
chibi-arena/
├── crates/
│   ├── aa2-sim/        # Deterministic combat simulation
│   ├── aa2-data/       # Shared types, schemas, RON loaders
│   ├── aa2-game/       # Game state machine, economy, draft, shared action dispatch
│   ├── aa2-net/        # Serde wire types (ClientMsg/ServerMsg/DTOs)
│   ├── aa2-client/     # Godot GDExtension (gdext); NetClient for networked mode
│   └── aa2-server/     # Authoritative WebSocket game server (tokio + tungstenite)
├── client/             # Godot 4.6 project
│   └── tests/          # GDScript integration tests
├── data/               # RON data files (gods, abilities, bodies)
├── docs/               # Architecture & design documentation
├── tests/              # Rust integration tests
└── README.md
```

Key design decisions:
- **Multi-crate workspace** — sim, data, game, net, client, and server are independent crates with clean dependency boundaries
- **Deterministic simulation** — fixed-seed combat enables reproducible tests, replays, and server-authoritative validation
- **Server-authoritative dumb client** — the server owns game state and runs the sim; clients send intents and render received snapshots (no client-side simulation)
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
| [docs/design/architecture.md](docs/design/architecture.md) | Technical architecture & system design |
| [docs/design/networking.md](docs/design/networking.md) | Networking design (dumb-client state-sync) |
| [docs/design/ability-authoring.md](docs/design/ability-authoring.md) | Ability authoring reference (RON-only, no Rust) |
| [docs/project-plan.md](docs/project-plan.md) | Phased development plan |
| [docs/specs/mechanics-reference.md](docs/specs/mechanics-reference.md) | Engine formulas & combat mechanics |
| [docs/runbooks/networked-smoke.md](docs/runbooks/networked-smoke.md) | Networked smoke test + manual playtest |

## Contributing

All changes must pass before merge:
```bash
cargo clippy --all-targets -- -D warnings
cargo test
./dev test
```

See [AGENTS.md](AGENTS.md) for test philosophy and commit conventions.

## License

[MIT](LICENSE)
