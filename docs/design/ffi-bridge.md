# FFI Bridge Design

## Overview

The FFI bridge exposes aa2-game (and transitively aa2-sim) to Unity via a C-compatible API. Unity loads the Rust library as a native plugin and calls exported functions.

## Architecture

```
Unity (C#) → [P/Invoke] → libaa2_ffi.dylib/so/dll → aa2-game → aa2-sim
```

The bridge is a thin translation layer: C# calls C functions, which deserialize inputs, call Rust game logic, serialize outputs, and return.

## Crate: aa2-ffi

New crate: `crates/aa2-ffi/`

```toml
[lib]
crate-type = ["cdylib"]  # Produces .dylib/.so/.dll

[dependencies]
aa2-game = { path = "../aa2-game" }
aa2-sim = { path = "../aa2-sim" }
aa2-data = { path = "../aa2-data" }
serde_json = "1"
```

## Serialization: JSON

All complex data crosses the FFI boundary as JSON strings. Rationale:
- Unity has excellent JSON support (JsonUtility, Newtonsoft)
- Debuggable (human-readable)
- No schema compilation step (unlike FlatBuffers/Protobuf)
- Performance is fine for 10Hz state sync (game state is small)

Simple scalars (gold, HP, round number) can be returned directly as primitives.

## Memory Management

**Rust owns all game state.** Unity holds an opaque pointer (`*mut GameContext`).

```c
// Rust allocates, returns pointer
GameContext* aa2_create_game(const char* config_json);

// Unity passes pointer back for all operations
const char* aa2_get_state(GameContext* ctx);

// Rust frees when done
void aa2_destroy_game(GameContext* ctx);
```

**String returns:** Rust allocates a CString, returns `*const c_char`. Unity copies it immediately. Rust frees on next call (double-buffer pattern) or via explicit `aa2_free_string`.

## C API Surface

### Lifecycle

```c
/// Create a new game. Returns opaque context pointer.
/// config_json: { "seed": 42, "num_players": 8, "data_path": "/path/to/data" }
GameContext* aa2_create_game(const char* config_json);

/// Destroy game and free all memory.
void aa2_destroy_game(GameContext* ctx);
```

### State Queries (read-only, return JSON)

```c
/// Full game state snapshot (for initial load / reconnect).
const char* aa2_get_game_state(GameContext* ctx);

/// Player-specific view (what one player sees: their heroes, shop, opponents' HP).
const char* aa2_get_player_view(GameContext* ctx, uint8_t player_id);

/// Current shop offerings for a player.
const char* aa2_get_shop(GameContext* ctx, uint8_t player_id);

/// Draft choices (if draft is active).
const char* aa2_get_draft_choices(GameContext* ctx, uint8_t player_id);

/// Combat log from last round.
const char* aa2_get_combat_log(GameContext* ctx, uint8_t matchup_index);
```

### Player Actions (mutate state)

```c
/// Submit a player action. Returns success/error JSON.
/// action_json: { "type": "Buy", "slot": 0 }
///              { "type": "Equip", "ability": "Rage", "hero": "Sven" }
///              { "type": "DraftHero", "index": 1 }
///              etc.
const char* aa2_player_action(GameContext* ctx, uint8_t player_id, const char* action_json);
```

### Game Flow

```c
/// Advance game time by dt seconds. Returns events JSON.
const char* aa2_tick(GameContext* ctx, float dt);

/// Manually trigger phase transition (dev mode).
const char* aa2_ready(GameContext* ctx, uint8_t player_id);

/// Run combat for current round. Returns results JSON.
const char* aa2_run_combat(GameContext* ctx);
```

### Combat Replay (for visual playback)

```c
/// Get combat replay data: unit positions/states at each tick.
/// Unity uses this to animate the fight.
const char* aa2_get_combat_replay(GameContext* ctx, uint8_t matchup_index);
```

The replay contains per-tick snapshots:
```json
{
  "ticks": [
    { "tick": 0, "units": [{"id": 0, "x": 100, "y": 500, "hp": 700, "state": "Idle"}] },
    { "tick": 30, "units": [{"id": 0, "x": 200, "y": 500, "hp": 650, "state": "Attacking"}] }
  ],
  "events": [
    { "tick": 45, "type": "Attack", "attacker": 0, "target": 1, "damage": 55 }
  ]
}
```

Unity interpolates between snapshots for smooth animation.

## Unity C# Bindings

```csharp
public static class AA2Native {
    [DllImport("aa2_ffi")]
    private static extern IntPtr aa2_create_game(string configJson);
    
    [DllImport("aa2_ffi")]
    private static extern IntPtr aa2_get_player_view(IntPtr ctx, byte playerId);
    
    [DllImport("aa2_ffi")]
    private static extern IntPtr aa2_player_action(IntPtr ctx, byte playerId, string actionJson);
    
    [DllImport("aa2_ffi")]
    private static extern IntPtr aa2_tick(IntPtr ctx, float dt);
    
    [DllImport("aa2_ffi")]
    private static extern void aa2_destroy_game(IntPtr ctx);
    
    [DllImport("aa2_ffi")]
    private static extern void aa2_free_string(IntPtr str);
}
```

## Build Targets

| Platform | Target | Output |
|----------|--------|--------|
| macOS (dev) | `aarch64-apple-darwin` | `libaa2_ffi.dylib` |
| iOS | `aarch64-apple-ios` | `libaa2_ffi.a` (static) |
| Android | `aarch64-linux-android` | `libaa2_ffi.so` |
| Windows | `x86_64-pc-windows-msvc` | `aa2_ffi.dll` |

## Performance Budget

- `aa2_tick(dt)`: <1ms (just timer math + event generation)
- `aa2_player_action(...)`: <1ms (state mutation)
- `aa2_run_combat(...)`: <50ms (full combat sim, 1500 ticks max)
- `aa2_get_player_view(...)`: <5ms (JSON serialization)
- `aa2_get_combat_replay(...)`: <100ms (serialize full replay)

## Error Handling

All functions return JSON with an `"error"` field on failure:
```json
{"error": "not enough gold"}
```

Success returns the requested data or `{"ok": true}`.
