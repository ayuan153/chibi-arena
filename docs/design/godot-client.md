# Godot Client Design

## Overview

The Godot client is the visual presentation layer for AA2. The `aa2-client` crate is a GDExtension (cdylib) that Godot loads at startup. It calls aa2-game directly as Rust library code — no FFI boundary, no serialization. In local mode, the full game runs in-process. In networked mode (Phase 4), it becomes a thin client receiving state from the server.

## Technology

- **Godot 4.3+** with GDExtension
- **gdext 0.5** (Rust GDExtension bindings)
- **Code-first approach:** hand-written `project.godot`, no editor dependency
- **Crate type:** `cdylib` — produces .dylib/.so/.dll loaded by Godot

## Project Structure

```
client/                         # Godot project root
├── project.godot               # Hand-written project config
├── aa2.gdextension             # Points to ../target/ for built library
├── scenes/
│   ├── main.tscn               # Entry point
│   ├── god_pick.tscn           # God selection screen
│   ├── shop.tscn               # Shop phase (main gameplay)
│   ├── draft.tscn              # Hero draft overlay
│   ├── combat.tscn             # Combat replay viewer
│   └── scoreboard.tscn         # Player standings
└── assets/
    └── placeholder/            # Colored shapes with labels

crates/aa2-client/              # Rust GDExtension crate
├── Cargo.toml                  # [lib] crate-type = ["cdylib"]
├── src/
│   ├── lib.rs                  # gdext entry point, ExtensionLibrary impl
│   ├── game_manager.rs         # Owns GameState, drives game loop
│   ├── shop_ui.rs              # Shop screen logic
│   ├── combat_viewer.rs        # Replay playback with event-driven tweens
│   ├── draft_ui.rs             # Hero draft logic
│   └── god_pick_ui.rs          # God selection logic
```

## Architecture

```
Godot (scenes/signals/input) ←→ aa2-client (gdext classes) → aa2-game → aa2-sim → aa2-data
```

- aa2-client exposes GDExtension classes that Godot instantiates in scenes
- These classes own aa2-game state directly (no pointer indirection, no C API)
- Player actions translate to direct `aa2-game` function calls
- Combat results returned as Rust structs, not serialized JSON

## Game Screens

### 1. God Pick (pre-game)
Full-screen choice between available gods with descriptions.

### 2. Shop Phase (main gameplay)
- Board view with positioned heroes
- Shop offerings (buy/sell/reroll/lock/upgrade)
- Bench with unequipped abilities
- Hero panels with equipped ability slots
- Timer bar, gold display, HP display
- Draft overlay appears on draft rounds (pick 1 of 3)

### 3. Combat Phase
- Arena view with both teams
- Units move smoothly (tweened from MoveTo/StartMoving events)
- Attack animations, projectiles, ability VFX (placeholder: colored shapes)
- Floating damage numbers, health bars
- Speed controls (1x, 2x, 4x, skip)

### 4. Scoreboard
All 8 players: HP, heroes, god, placement (if eliminated).

## Combat Replay System

Combat is NOT rendered in real-time. Instead:
1. Rust runs the full combat instantly (~50ms for aa2-sim), producing a `Vec<CombatEvent>`
2. Client receives the full event stream (Attack, ProjectileSpawn, ProjectileHit, Death, CastStart, CastComplete, AbilityDamage, Heal, BuffApplied, BuffExpired, MoveTo, StartMoving, etc.)
3. Events carry tick numbers — client converts tick → time (tick / 30 = seconds) for scheduling
4. Godot animates events using tweens/AnimationPlayer for smooth 60fps playback
5. Player can speed up (2x, 4x) or skip

The only sim change needed is adding movement events (`MoveTo { tick, unit_id, x, y }` or `StartMoving { tick, unit_id, target_x, target_y, speed }`). All other events already exist in the CombatEvent enum (13 event types).

Benefits:
- No frame-rate coupling between sim and rendering
- Replays are deterministic and replayable
- Can show other players' fights too
- Data size: ~10KB per fight (vs ~100KB for snapshots)
- Network-friendly: only transmit when something happens
- Client animation is cosmetic-only, doesn't need to be deterministic

## Local vs Networked Mode

### Local Mode (Phase 3 — what we build first)

```
Godot scenes ←→ aa2-client (gdext) → aa2-game (in-process)
```

- aa2-client owns the game loop
- Calls `aa2-game` tick/action functions directly
- AI opponents run inside Rust (same as CLI dev mode)
- No network dependency

### Networked Mode (Phase 4 — added later)

```
Godot scenes ←→ aa2-client (gdext) → WebSocket → aa2-server → aa2-game
```

- Server owns the game loop and timer
- aa2-client sends actions via WebSocket
- Server broadcasts state snapshots (10Hz)
- Client interpolates between snapshots for shop/board state

The transition is clean: swap direct aa2-game calls for WebSocket messages. UI code doesn't change.

## Platform Targets

| Platform | Library Output | Notes |
|----------|---------------|-------|
| macOS | .dylib | Primary dev platform |
| iOS | .dylib | Godot iOS export |
| Android | .so | Godot Android export |
| Windows | .dll | Godot Windows export |
| Linux | .so | Godot Linux export |

## Build

```bash
# Build the GDExtension library (dev)
cargo build -p aa2-client

# Build release
cargo build -p aa2-client --release

# Godot loads the library via .gdextension file:
# client/aa2.gdextension → ../target/debug/libaa2_client.dylib (or release)
```

The `.gdextension` file points to `../target/` so no manual copy step is needed.

For cross-compilation:
```bash
cargo build -p aa2-client --release --target aarch64-apple-ios
cargo build -p aa2-client --release --target aarch64-linux-android
```

## Success Criteria

- Playable full game in Godot with placeholder art
- All game actions work via UI (no CLI needed)
- Combat viewer shows fights with smooth unit movement
- Runs at 60fps on macOS
- Code-first: project builds without opening Godot editor
