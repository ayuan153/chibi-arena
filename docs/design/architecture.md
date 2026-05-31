# AA2 Technical Architecture

## Overview

AA2 (Ability Arena 2) is a standalone cross-platform port of the Dota 2 mod Ability Arena — an 8-player free-for-all autobattler. Players pick gods, draft hero bodies (tiers 1–5), equip abilities (levels 1–9), and watch fully automated combat resolve with Dota 2-fidelity mechanics.

**Targets:** macOS (primary dev), iOS, Android, Windows, Linux  
**Art style:** 2D chibi/anime, top-down perspective  
**Engine:** Godot 4.6 (presentation) + Rust (simulation + game logic)

---

## System Architecture

The system is a hybrid: a deterministic Rust simulation drives all game logic, while Godot handles rendering, UI, audio, and platform deployment. The client crate (aa2-client) is loaded by Godot as a GDExtension — no FFI boundary or serialization layer between client and game logic.

```
┌─────────────────────────────────────────────────────────┐
│ Godot 4.6 (GDExtension)                                │
│   Scenes, UI, Rendering, Audio, Input                   │
│         ↕ (gdext bindings)                              │
│ ┌─────────────────────────────────────────────────────┐ │
│ │ aa2-client (Rust cdylib)                            │ │
│ │   GDExtension classes, scene management             │ │
│ │         ↓ (direct Rust calls, same process)         │ │
│ │ ┌─────────────────────────────────────────────────┐ │ │
│ │ │ aa2-game                                        │ │ │
│ │ │   Game state machine, economy, draft, shop      │ │ │
│ │ │         ↓                                       │ │ │
│ │ │ ┌───────────────────────────────────────────┐   │ │ │
│ │ │ │ aa2-sim                                   │   │ │ │
│ │ │ │   Deterministic combat simulation (30Hz)  │   │ │ │
│ │ │ └───────────────────────────────────────────┘   │ │ │
│ │ └─────────────────────────────────────────────────┘ │ │
│ └─────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│ aa2-server (implemented)                                │
│   WebSocket server (tokio + tokio-tungstenite),         │
│   authoritative GameState, two-window clock, AI bots    │
│         ↓                                               │
│   aa2-game → aa2-sim → aa2-data                         │
└─────────────────────────────────────────────────────────┘
```

### Layer Responsibilities

| Layer | Technology | Role |
|-------|-----------|------|
| Simulation | Rust (`aa2-sim`) | Deterministic ECS combat at 30Hz, f32 math, server-authoritative |
| Game Logic | Rust (`aa2-game`) | Game state machine, economy, draft, shop, matchups |
| Client | Rust (`aa2-client`) + Godot 4.6 | GDExtension bridge, UI, rendering, audio |
| Server | Rust (`aa2-server`) | WebSocket server (tokio + tokio-tungstenite), authoritative state, two-window clock |
| Wire Types | Rust (`aa2-net`) | Serde `ClientMsg`/`ServerMsg`/`StateSnapshot` DTOs shared by client + server |
| Data | Rust (`aa2-data`) | Shared types, RON/JSON deserialization, validation |

---

## Crate Architecture

### Dependency Graph

```
Godot (scenes/UI) ←→ aa2-client (cdylib, gdext) → aa2-game → aa2-sim → aa2-data
                          ↕ (networked mode)            ↑
                       aa2-net (wire types)             │
                          ↕                             │
aa2-server ─────────────────────────────────────────────┘ (same game logic)
```

No C boundary. No JSON serialization for client-game communication in local mode. aa2-client calls aa2-game directly as Rust library code in the same process. In networked mode, the client reads server snapshots via aa2-net types instead.

### `aa2-sim`

The core combat simulation. ECS-based (custom lightweight ECS, not bevy) running at 30 ticks/second.

**Responsibilities:**
- Entity management (heroes, projectiles, summons)
- Attribute system (STR/AGI/INT derived stats)
- Attack loop with BAT, attack speed, animation timing
- Ability casting (cast points, targeting)
- Composable effect resolver (`effect_spec.rs`): one generic resolver per axis (Trigger, Targeting, Delivery, Payload) — abilities are `EffectSpec` compositions authored in RON, not bespoke code
- Projectile system (homing, travel time)
- Buff/debuff framework (stacking rules, tick-based durations); runtime `Buff` constructed via `Buff::from_def` from aa2-data's `BuffDef`
- Direct movement with collision push-apart (grid-based A* pathfinding is planned, not yet implemented)
- Targeting AI (aggro, priority, range checks)
- Turn rate and movement

**Compile targets:** Native (macOS/Windows/Linux), `wasm32-unknown-unknown` (must always compile to WASM for future web client or server-side validation).

### `aa2-data`

Shared data definitions and loading.

**Responsibilities:**
- Type definitions: `Hero`, `Ability`, `God`, `Buff`, `Projectile`
- Composable ability-effect schema: `EffectSpec`, `Payload`, `Delivery`, `TargetingSpec`, `Trigger`
- Buff data schema: `BuffDef`, `StatModifierSpec`, `StatusFlags`, `StatModifier`, `StackBehavior`, `DispelType`, `TickEffectDef`
- `serde::Serialize` + `serde::Deserialize` on all types
- RON file loader (dev) — loaded at startup; live hot-reload is planned, not yet implemented
- Validation (stat ranges, ability references, tier constraints)
- Same structs deserialize from RON (dev) and PostgreSQL JSONB (prod)

### `aa2-game`

Owns the full game loop. Depends on aa2-sim and aa2-data.

**Responsibilities:**
- `PlayerState`: gold, HP, heroes, ability inventory, god, shop state
- `GameState`: 8 players, round counter, phase, ability pool, matchups
- `Economy`: gold calculation, shop upgrade costs with decay
- `Draft`: ability pool management, shop rolls, buy/sell/equip
- `RoundFlow`: timer-based state machine — GodPick → Combat → GracePeriod → Shop cycle
- `Matchmaker`: round-robin pairing with ghost opponents for odd counts
- `DamageCalc`: player damage formula
- `GodSystem`: god passive application, rule modifications

**Key design:** aa2-game is SHARED between client and server. This enables:
- Offline/dev mode (full game locally)
- Server-side validation (authoritative)
- Single action dispatch: `scenario::parse_action` + `GameState::apply_action` are used by both
  client (local mode) and server — no duplication of validation logic.

In multiplayer the client is a **dumb client**: it does not run aa2-sim or predict state. The server is authoritative and the client only renders received state/events. (Decision recorded — see `docs/design/networking.md`.)

aa2-game has **no networking dependency** — the sim still compiles to WASM.

### `aa2-client`

GDExtension crate loaded by Godot. Bridges Rust game logic to Godot scenes.

**Crate type:** `cdylib` (produces .dylib/.so/.dll loaded via .gdextension file)

**Responsibilities:**
- Exposes GDExtension classes (via gdext 0.5) for Godot to instantiate
- Owns an `aa2-game::GameState` instance in local mode
- In networked mode: `NetClient` (background tokio thread + channels) + `NetState` behind the same
  getter API; getters read the latest server `Snapshot` instead of local state
- Translates Godot input/signals into aa2-game actions (local) or `ClientMsg::Action` (networked)
- Provides combat replay data (event-based — schedules CombatEvent stream as Godot tweens)
- Manages screen transitions (god pick, draft, shop, combat viewer)
- Enter networked mode via `AA2_SERVER` env var or dev-console `connect <url>` command

**Architecture:** Direct Rust function calls to aa2-game in local mode. No serialization boundary, no C FFI, no JSON marshaling. In networked mode, the same getter API reads from the latest server snapshot.

### `aa2-server` (implemented)

Multiplayer game server. Actor-model architecture: a single central task owns the lobby + one
`GameState` + RNG seed; per-connection reader/writer tasks communicate over channels (no locks).

**Responsibilities:**
- WebSocket server (tokio + tokio-tungstenite 0.29), binds `127.0.0.1:9001`
- Lobby management (each connection = one seat; Start fills remaining with AI bots)
- Authoritative game loop: validates actions via `GameState::apply_action`, drives the two-window
  clock (variable combat window = longest matchup animation; fixed prep window)
- Runs combat via `GameState::run_combat_round`, streams each viewer their matchup's `CombatEvent` log
- Computes `GameOver` placements
- Builds per-viewer `StateSnapshot` from `GameState` (projection lives here, not in aa2-game)

### `aa2-net`

Serde wire types shared by client and server.

**Responsibilities:**
- `ClientMsg` enum: `Join`, `Action`, `Start`
- `ServerMsg` enum: `Welcome`, `Lobby`, `Snapshot`, `ActionResult`, `CombatStart`, `PhaseChange`, `GameOver`
- `StateSnapshot` and supporting DTOs (`OwnView`, `PlayerView`, `ShopView`, `HeroView`, `Phase`)
- Dependencies: `aa2-sim` (reuses `CombatEvent`) + `serde` only

---

## Networking Architecture (implemented)

AA2 uses a **state-sync** model. The server is the single source of truth.

### Data Flow

```
Server (30Hz sim) → Snapshot (10Hz) → Delta Compress → WebSocket → Client → Interpolate → Render (60fps)
```

### Key Design Decisions

| Aspect | Approach |
|--------|----------|
| Sync model | State-sync (not lockstep) |
| Server tick | 30Hz (33.33ms) |
| Snapshot broadcast | 10Hz (every 3rd tick) |
| Client rendering | 60fps with interpolation between snapshots |
| Compression | Delta encoding — only changed fields per snapshot |
| Bandwidth | ~3–5 KB/s per client (own board only) |
| Spectating | Client can subscribe to any board on demand |
| Draft phase | Request/response over same WebSocket |
| Reconnect | Server sends full state snapshot; instant catch-up |

### Why State-Sync

Autobattlers have no player input during combat — there's nothing to "lock step" on. State-sync gives us:
- Trivial reconnection (send latest snapshot)
- Simple spectating (subscribe to a board)
- Server-authoritative anti-cheat by default
- No desync debugging

---

## Combat Simulation Details

### Tick Rate

30Hz (33.33ms per tick). Chosen to match Dota 2's server tick rate for mechanical fidelity while remaining cheap enough for mobile.

### Determinism (scope)

The sim is deterministic **on a given platform/build**: a seeded xoshiro128++ RNG plus fixed
tick and iteration order produce identical results for the same seed. It uses `f32` math, so it is
**not** guaranteed bit-identical across architectures (x86 vs ARM). This is acceptable because the
multiplayer model is **server-authoritative with a dumb client**: the server is the single source
of truth and clients render the server's recorded event stream rather than re-simulating. So
"deterministic replay" means *re-running the same build on the same platform with the same seed*;
replays sent to clients are server-recorded event logs, not client-side resimulations. Cross-platform
bit-determinism (which would require fixed-point math) is **out of scope** unless we later add
client-side prediction.

### Attribute System

| Attribute | Per Point | Derived Stats |
|-----------|-----------|---------------|
| STR | +22 HP, +0.1 HP regen/s | Health pool, sustain |
| AGI | +1 attack speed, +0.167 armor | DPS, survivability |
| INT | +12 mana, +0.05 mana regen/s | Ability usage |

### Core Formulas

**Attack speed interval:**
```
interval = BAT / (total_attack_speed / 100)
total_attack_speed = clamped(base + bonus, 20, 700)
```

**Armor damage multiplier:**
```
mult = 1 - (0.06 * armor) / (1 + 0.06 * |armor|)
```

### Combat Systems

- **Attack loop:** Acquire target → turn → wind-up (attack point) → launch projectile/apply damage → backswing
- **Projectiles:** Homing with configurable speed. Travel time = distance / speed. On-hit effects applied on arrival.
- **Abilities:** Cast point → effect → cooldown. Targeting modes: unit, point, no-target, passive. Effects are composable `EffectSpec` compositions (Trigger × Targeting × Delivery × Payload[]) resolved by a generic engine — no per-ability Rust code.
- **Buffs/Debuffs:** Stack rules (refresh, independent, max stacks). Tick-based duration. Modifier priority system. DamageReflection buff.
- **Movement:** Direct movement toward target with collision push-apart resolution. (Grid-based A* pathfinding is planned, not yet implemented.)
- **Turn rate:** Units must face target before attacking/casting. Configurable degrees/second.

---

## Data Architecture

### Dual-Source Design

All game content is data-driven. The same Rust structs deserialize from two sources:

| Environment | Source | Format | Features |
|-------------|--------|--------|----------|
| Development | Local files | RON | Hot-reload, comments, human-readable |
| Production | PostgreSQL | JSONB | Queryable, versioned, admin-editable |

### Content Types

- **Gods** — passive/active abilities that define playstyle
- **Hero Bodies** — tiers D/C/B/A/S, base stats, BAT, attack range, movement speed
- **Abilities** — levels 1–9, scaling values, targeting, cooldowns

### Hot-Reload (Dev) — Planned (not yet implemented)

> Today, RON files are loaded at startup; changing data requires restarting the client
> (the gdext dylib also requires a Godot restart). The workflow below is the intended future state.

The `notify` crate would watch RON files. On change:
1. File re-parsed and validated
2. Affected entities in the sim updated in-place
3. No restart required

---

## Dev Mode Architecture

A single developer can run the full game loop locally without a server.

- aa2-client loads aa2-game directly (same process, no network)
- Developer controls all 8 player slots (draft, positioning)
- Edit RON data files and restart to iterate on balance (live hot-reload planned)
- AI bots fill empty slots for testing combat
- Combat event log available for debugging combat sequences
- No network dependency — pure local execution

---

## Platform Deployment

| Platform | Godot Export | Rust Target | Output |
|----------|-------------|-------------|--------|
| macOS | .app bundle | `aarch64-apple-darwin` | .dylib |
| iOS | IPA (Xcode) | `aarch64-apple-ios` | .dylib |
| Android | APK/AAB | `aarch64-linux-android` | .so |
| Windows | Standalone | `x86_64-pc-windows-msvc` | .dll |
| Linux | Standalone | `x86_64-unknown-linux-gnu` | .so |
| Server | N/A | `aarch64-unknown-linux-gnu` | Binary |

Build: `cargo build` produces the native library, Godot loads it via `.gdextension` file pointing to `../target/`.

Server deployment: containerized Rust binary on Linux, horizontally scalable per game instance.

---

## Crate Structure

```
aa2/
├── crates/
│   ├── aa2-data/       # Shared types, RON loading ✓
│   ├── aa2-sim/        # Combat simulation engine ✓
│   ├── aa2-game/       # Game state machine, economy, draft ✓
│   ├── aa2-net/        # Serde wire types (ClientMsg/ServerMsg/DTOs) ✓
│   ├── aa2-client/     # GDExtension crate (gdext, cdylib) ✓
│   └── aa2-server/     # WebSocket server (tokio, actor-model) ✓
├── client/             # Godot 4.6 project
├── data/               # RON data files
└── docs/               # Architecture & design documentation
```

---

## Replay System

### Recording

During combat, the sim runs to completion instantly (~50ms) and produces a `Vec<CombatEvent>`. The event stream includes: Attack, ProjectileSpawn, ProjectileHit, Death, CastStart, CastComplete, AbilityDamage, Heal, BuffApplied, BuffExpired, MoveTo, StartMoving, etc. Each event carries a tick number for temporal ordering.

### Playback

The client receives the full event stream and animates it forward using Godot tweens. Tick → time conversion: `tick / 30 = seconds`. Animation is cosmetic-only and doesn't need to be deterministic. Planned (not yet implemented):
- Play/pause/seek
- Speed control (0.5x–4x)
- Board switching (view any player)

Data size: ~10KB per fight. Network-friendly — only transmit when something happens.

### Use Cases

- Post-game review
- Bug reproduction and debugging
- Content creation / streaming
- Spectating (live replays)

---

## Security Model

- **Server-authoritative:** Clients send intents (draft picks, board positions). The server validates and applies.
- **No client simulation during multiplayer:** Clients only interpolate received state. Cannot fabricate game state.
- **Replay integrity:** Replays are server-recorded, not client-generated.

---

## Client/Server Protocol (implemented)

### State Sync (Server → Client)
- During combat: state snapshots at 10Hz (unit positions, HP, buffs, events)
- During shop: PlayerState updates on change (gold, inventory, shop contents)
- Public info: other players' HP, hero count (not ability details)

### Actions (Client → Server)
All player actions are request/response over WebSocket:
```
BuyAbility(slot) → Ok/Err(reason)
SellAbility(id) → Ok/Err
RerollShop → Ok(new_choices)/Err
UpgradeShop → Ok/Err
EquipAbility(ability_id, hero_idx, slot_idx) → Ok/Err
UnequipAbility(hero_idx, slot_idx) → Ok/Err
PickGod(god_id) → Ok/Err
PickHeroBody(idx) → Ok/Err
RerollHeroBody → Ok(new_choices)/Err
PlaceHero(hero_idx, x, y) → Ok/Err
Ready → Ok
```

Server validates all actions against game rules. Invalid actions rejected with reason.

### Reconnect
Server sends full GameState snapshot. Client rebuilds from scratch.
