# Unity Client Design

## Overview

The Unity client is the visual presentation layer for AA2. It calls the Rust game logic via FFI (native plugin) and renders the game state. In local mode, it runs the full game in-process. In networked mode, it becomes a thin client receiving state from the server.

## Project Structure

```
unity-aa2/
├── Assets/
│   ├── Plugins/
│   │   ├── macOS/libaa2_ffi.dylib
│   │   ├── iOS/libaa2_ffi.a
│   │   └── Android/libaa2_ffi.so
│   ├── Scripts/
│   │   ├── Core/
│   │   │   ├── AA2Bridge.cs        — FFI wrapper (P/Invoke calls)
│   │   │   ├── GameManager.cs      — Game lifecycle, state management
│   │   │   ├── GameState.cs        — C# mirror of Rust game state (deserialized from JSON)
│   │   │   └── ActionDispatcher.cs — Sends player actions to Rust
│   │   ├── UI/
│   │   │   ├── ShopPanel.cs        — Shop offerings, buy/sell/reroll/lock/upgrade
│   │   │   ├── DraftPanel.cs       — Hero draft choices
│   │   │   ├── HeroPanel.cs        — Hero stats, equipped abilities
│   │   │   ├── BenchPanel.cs       — Bench abilities
│   │   │   ├── ScoreboardPanel.cs  — All players HP, placement
│   │   │   ├── GodPanel.cs         — God info, buff target selection
│   │   │   └── TimerBar.cs         — Round timer display
│   │   ├── Combat/
│   │   │   ├── CombatViewer.cs     — Renders combat replay
│   │   │   ├── UnitRenderer.cs     — Individual unit sprite/animation
│   │   │   ├── ProjectileRenderer.cs
│   │   │   ├── HealthBar.cs
│   │   │   └── DamageNumber.cs     — Floating damage text
│   │   ├── Board/
│   │   │   ├── BoardManager.cs     — Hero positioning (drag & drop)
│   │   │   ├── HeroSlot.cs         — Draggable hero on board
│   │   │   └── GridOverlay.cs      — Visual grid for positioning
│   │   └── Dev/
│   │       ├── DevConsole.cs       — In-game console (logs, commands)
│   │       └── DebugOverlay.cs     — FPS, state info, combat stats
│   ├── Scenes/
│   │   ├── MainMenu.unity
│   │   ├── Game.unity              — Main game scene (all panels)
│   │   └── CombatReplay.unity      — Full-screen combat viewer
│   ├── Prefabs/
│   │   ├── Units/                  — Hero unit prefabs (placeholder art)
│   │   ├── Projectiles/
│   │   └── UI/
│   └── Art/
│       ├── Placeholder/            — Colored shapes with labels (Phase 3)
│       └── Production/             — Real art (Phase 5)
└── ProjectSettings/
```

## Game Screens

### 1. Shop Phase (main gameplay screen)

```
┌─────────────────────────────────────────────────┐
│ [Timer: 27s]  Round 5  Gold: 14  HP: 156        │
├─────────────────────────────────────────────────┤
│                                                 │
│   ┌─────── BOARD (your half) ───────┐           │
│   │  [Hero1]    [Hero2]             │           │
│   │       [Hero3]                   │           │
│   │                    [Hero4]      │           │
│   └─────────────────────────────────┘           │
│                                                 │
├─────────────────────────────────────────────────┤
│ SHOP: [Ability1] [Ability2] [SOLD] [Ability4]   │
│       [Reroll 1g] [Lock] [Upgrade 12g→Lv3]     │
├─────────────────────────────────────────────────┤
│ BENCH: [A1] [A2] [A3] [ ] [ ]                  │
├─────────────────────────────────────────────────┤
│ HEROES: [Hero1 stats] [Hero2 stats] ...         │
│ GOD: Archmage — Sorcery                         │
└─────────────────────────────────────────────────┘
```

**Interactions:**
- Click shop ability → buy (goes to bench)
- Drag bench ability → hero slot → equip
- Drag hero on board → reposition
- Click Reroll/Lock/Upgrade buttons
- Draft overlay appears on draft rounds (pick 1 of 3)

### 2. Combat Phase (auto-plays)

```
┌─────────────────────────────────────────────────┐
│ [Timer: 45s]  Round 5  YOU vs Player 3          │
├─────────────────────────────────────────────────┤
│                                                 │
│   ┌─────── ARENA (2000x2000) ──────┐           │
│   │  [Enemy1]        [Enemy2]       │  TOP      │
│   │                                 │           │
│   │         ← combat →              │           │
│   │                                 │           │
│   │  [Your1]    [Your2]            │  BOTTOM   │
│   └─────────────────────────────────┘           │
│                                                 │
│ [HP bars] [Ability cooldowns] [Buff icons]      │
│ [Combat log scrolling at bottom]                │
└─────────────────────────────────────────────────┘
```

**Rendering:**
- Units move smoothly (interpolate between tick positions)
- Attack animations (frontswing → projectile/hit → backswing)
- Ability VFX (placeholder: colored circles/lines)
- Floating damage numbers
- Health bars above units
- Death animation (fade out)

### 3. Scoreboard (always accessible)

Shows all 8 players: HP, heroes, god, placement (if eliminated).

### 4. God Pick (pre-game)

Full-screen choice between available gods with descriptions.

## Local vs Networked Mode

### Local Mode (Phase 3 — what we build first)

```
Unity → AA2Bridge.cs → libaa2_ffi → aa2-game (in-process)
```

- Unity owns the game loop
- Calls `aa2_tick(dt)` every frame
- Calls `aa2_player_action(...)` on user input
- Calls `aa2_run_combat(...)` when combat starts
- Gets replay data and plays it back visually
- AI opponents run inside Rust (same as CLI)

### Networked Mode (Phase 4 — added later)

```
Unity → WebSocket → aa2-server → aa2-game (server-authoritative)
```

- Server owns the game loop and timer
- Unity sends actions via WebSocket
- Server broadcasts state snapshots (10Hz)
- Unity interpolates between snapshots
- Combat replays sent as complete data (not streamed)
- Client-side prediction for responsiveness (optional)

**The transition is clean:** Replace `AA2Bridge.cs` calls with WebSocket messages. The UI code doesn't change.

## Combat Replay System

Combat is NOT rendered in real-time. Instead:
1. Rust runs the full combat instantly (~50ms)
2. Returns a replay: per-tick unit positions + events
3. Unity plays back the replay at 1x speed (or configurable)
4. Player can speed up (2x, 4x) or skip

This means:
- No frame-rate coupling between sim and rendering
- Replays are deterministic and replayable
- Can show other players' fights too

## Dev Console

In-game overlay (toggle with backtick `` ` ``):
- Shows game state JSON
- Shows combat log events in real-time during replay
- Can type commands (same as CLI: buy, equip, etc.)
- Shows FPS, tick rate, memory usage

## Placeholder Art (Phase 3)

- Heroes: colored circles with name labels (STR=red, AGI=green, INT=blue)
- Abilities: colored squares with name text
- Projectiles: small colored dots
- VFX: expanding circles (AoE), lines (Spear), flashes (damage)
- UI: Unity's built-in UI toolkit (no custom art needed)

## Build Pipeline

```bash
# Build Rust FFI library for current platform
cargo build -p aa2-ffi --release

# Copy to Unity Plugins folder
cp target/release/libaa2_ffi.dylib unity-aa2/Assets/Plugins/macOS/

# Open Unity project
open unity-aa2/
```

For mobile:
```bash
# iOS (static lib)
cargo build -p aa2-ffi --release --target aarch64-apple-ios
# Android (shared lib)
cargo build -p aa2-ffi --release --target aarch64-linux-android
```

## Phase 3 Milestones

| Week | Deliverable |
|------|-------------|
| 21 | FFI crate compiles, Unity loads plugin, can create/destroy game |
| 22 | Shop screen functional (buy/sell/reroll/equip via UI) |
| 23 | Board positioning (drag & drop heroes) |
| 24 | Combat replay viewer (units move, attack, die) |
| 25 | Draft screen, god pick, scoreboard |
| 26 | Full playable game in Unity (local mode, placeholder art) |
| 27 | Polish: animations, damage numbers, ability VFX |
| 28 | Dev console, iOS simulator build, performance profiling |

## Success Criteria

- Playable full game in Unity with placeholder art
- All game actions work via UI (no CLI needed)
- Combat viewer shows fights with smooth unit movement
- Runs at 60fps on macOS (dev) and iOS simulator
- Dev console provides same observability as CLI
