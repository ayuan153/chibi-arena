# AA2 Godot Client — Local Dev Workflow

## Quick Reference

```bash
./dev setup    # One-time: install Godot via Homebrew
./dev editor   # First run: opens editor, triggers import (REQUIRED once)
./dev          # Build + launch game
./dev check    # cargo check + clippy + test (no Godot)
```

## How It Works

- `aa2-client` crate compiles to `libaa2_client.dylib` (cdylib)
- `./dev` copies it to `client/bin/` where Godot finds it via `res://bin/`
- Godot loads it via `client/aa2_client.gdextension`
- Our Rust classes (MainScene, GameManager, ShopUI, etc.) register as native Godot types

## First Time Setup

1. `./dev setup` — installs Godot
2. `./dev editor` — opens editor, triggers initial project import (creates `.godot/`)
3. Close editor
4. `./dev` — builds and launches the game

**The `.godot/` directory MUST exist for extensions to load.** If deleted, run `./dev editor` again.

## Iteration Loop

```
Edit Rust → ./dev → see changes (~3s rebuild)
```

Godot must be restarted to pick up a new dylib (no hot-reload).

## Debugging

- `./dev editor` — open editor, inspect scene tree, check Output panel
- Run from editor (▶ button) — see Remote tab for live scene tree
- Dev console (always visible in-game) — type commands to inspect/mutate state
- `godot --path client/ --verbose` — see extension loading details

## Common Issues

| Symptom | Cause | Fix |
|---------|-------|-----|
| Blank window, no UI | Extension not loading | Run `./dev editor` once to trigger import |
| "Cannot get class 'X'" | .godot/ missing or stale | Run `./dev editor` to reimport |
| Classes not in Cmd+A | Extension loaded but editor cache stale | Close + reopen editor |
| "Parent node busy" error | add_child during tree setup | Use call_deferred or add to self |

## Architecture

```
client/project.godot          — Godot project config
client/main.tscn              — Entry scene (instantiates MainScene class)
client/aa2_client.gdextension — Tells Godot where to find the dylib
client/bin/libaa2_client.dylib — Copied by ./dev script
crates/aa2-client/src/
├── lib.rs                    — GDExtension entry point
├── main_scene.rs             — Root controller (phase transitions)
├── game_manager.rs           — Holds GameState, exposes actions
├── shop_ui.rs                — Shop screen
├── board_ui.rs               — Hero positioning
├── bench_ui.rs               — Ability management
├── combat_viewer_ui.rs       — Combat replay playback
├── god_pick_ui.rs            — God selection
├── draft_ui.rs               — Hero draft
├── scoreboard_ui.rs          — Player status
└── dev_console.rs            — Debug console
```

## Key Patterns

- All UI built programmatically in `ready()` — no .tscn dependencies
- GameManager at path `/root/MainScene/GameManager`
- Signal connection: `btn.connect("pressed", &self.base().callable("method_name"))`
- State queries: `manager.bind().get_gold(0)`
- State mutations: `manager.bind_mut().apply_player_action(0, "Buy".into(), "0".into())`
- Phase visibility: MainScene shows/hides screens based on `get_phase()`
