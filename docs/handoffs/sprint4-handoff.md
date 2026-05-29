# Sprint 4+ Handoff — Fresh Agent Guide

## Current State (Sprint 4 Partial Complete)

The game is **fully playable** in 2-player local dev mode with polished UI:
- God pick (grid overlay) → Shop → Draft → Equip → Combat (animated) → repeat until elimination
- Endgame screen on elimination with placement and standings
- 38 integration tests + 234 unit tests, all passing
- Attribute-colored hero displays, ability tooltips, summary overlay toggle

### What's Done (This Session)

**Sprint 3 (complete):**
- God pick grid overlay: 10-column grid, preview panel, confirm/discard, timer bar
- Summary overlay: player standings toggle via TopBar button
- Phase visibility: ScoreboardUI user-controlled (force-hide on GodPick, force-show on Finished)

**Sprint 4 (partial):**
- Better placeholder art: STR=red, AGI=green, INT=blue backgrounds on hero buttons; purple border on ultimates
- Ability tooltips: native Godot tooltips on all ability buttons (loadout + shop) with full info
- Finished phase: endgame overlay on elimination with "You placed Xth!", standings, SPECTATE button
- Gods data migration: moved from hardcoded Rust to RON data files (data/gods/*.ron)

### What Works
- Full game loop with phase transitions including Finished
- God pick grid with lazy-populated buttons (loads from RON data)
- Shop: buy, reroll, lock, upgrade (with correct cost decay and size scaling)
- Draft: appears on rounds 1, 3, 6, 9, 12 with tier-appropriate heroes
- Hero reroll: costs 2g, shows 3-card draft, auto-picks on Ready
- Equip/unequip/swap abilities, bench cap (5), level-up on duplicate
- Combat: full sim playback with unit movement, attacks, abilities, HP bars, death
- Arena: hero positioning in bottom half
- Unit info panel: shows hero stats when selected on board
- Summary overlay: toggle with TopBar button, shows all player standings
- Endgame: shows placement + standings when player is eliminated
- Attribute coloring on all hero buttons (loadout, board, draft)
- Ability tooltips on hover (loadout + shop)
- Gods loaded from data/gods/*.ron (data-driven, sorted alphabetically)

### Architecture
```
Godot 4.6 ←→ aa2-client (gdext 0.5, cdylib) → aa2-game → aa2-sim → aa2-data
```
- All game logic in Rust, all layout in .tscn
- GameManager at `/root/MainScene/GameManager` — single point of contact for all UI
- UI classes query GameManager every frame in `process()` (polling pattern)
- Shared UI helpers in `crates/aa2-client/src/ui_helpers.rs`

## What's Next — Sprint 4 Remaining

### Drag-and-Drop (High Priority, High Complexity)
- Equip: drag ability from bench to hero slot
- Sell: drag ability to sell bin (sell bin exists at PersistentChrome/GodPortrait/SellBin)
- Position: drag hero on board to reposition

**Implementation notes:**
- Godot's drag-and-drop uses `_get_drag_data()`, `_can_drop_data()`, `_drop_data()` virtual methods
- In gdext, these are `get_drag_data`, `can_drop_data`, `drop_data` on IControl
- Current equip/unequip uses button clicks + apply_player_action
- Drag source: bench ability buttons, equipped ability buttons, hero buttons on board
- Drop targets: hero ability slots (equip), sell bin (sell), board positions (reposition)

### DamageMeter (Medium Priority, Medium Complexity)
- Right sidebar during/after combat showing damage dealt per unit, grouped by team
- Reference: `~/Downloads/full game screenshots/shop round 7.png` (right side)
- Data source: combat events already have damage values in CombatEvent log
- Need: `get_combat_damage_summary()` API on GameManager that aggregates damage from last combat
- Display: VBoxContainer with team headers and unit rows (name + damage number)

### Sell Ability (Low Priority, Low Complexity)
- `apply_player_action(0, "Sell", "ability_name")` — already exists in game logic
- UI: click sell bin after selecting ability, or drag to sell bin
- Returns gold (amount TBD — check game.rs for sell logic)

## Key Files

| File | Purpose |
|------|---------|
| `crates/aa2-client/src/main_scene.rs` | Phase transitions, AI auto-actions, top-level wiring |
| `crates/aa2-client/src/game_manager.rs` | All game state queries and mutations (30+ #[func] methods) |
| `crates/aa2-client/src/ui_helpers.rs` | Shared: attribute_color, attribute_stylebox, ultimate_stylebox, format_ability_tooltip |
| `crates/aa2-client/src/shop_ui.rs` | Shop row (gold, upgrade, slots, reroll, lock) |
| `crates/aa2-client/src/loadout_ui.rs` | Hero rows + ability slots + bench |
| `crates/aa2-client/src/board_ui.rs` | Arena display + hero positioning |
| `crates/aa2-client/src/combat_viewer_ui.rs` | Combat playback (all 15 event types) |
| `crates/aa2-client/src/god_pick_ui.rs` | God selection grid overlay |
| `crates/aa2-client/src/draft_ui.rs` | 3-card hero draft overlay |
| `crates/aa2-client/src/scoreboard_ui.rs` | Summary overlay (toggle) |
| `crates/aa2-client/src/endgame_ui.rs` | Endgame placement + standings |
| `crates/aa2-data/src/lib.rs` | Data types + RON loaders (HeroDef, AbilityDef, God, loaders) |
| `crates/aa2-game/src/god.rs` | God gameplay logic (re-exports types from aa2-data) |
| `client/main.tscn` | Scene hierarchy with anchors |
| `client/tests/test_*.gd` | Integration tests (38 tests) |
| `data/gods/*.ron` | God definitions (Archmage, Paladin) |

## Dev Workflow

```bash
./dev          # Build + launch Godot
./dev editor   # Build + open editor (inspect scenes)
./dev check    # cargo check + clippy + test
./dev test     # Build + run 38 integration tests (requires display)
```

**Must close Godot before rebuilding** — no hot-reload with gdext.

## Testing Requirements

From `AGENTS.md`:
- New game behavior → add integration test in `client/tests/`
- Bug fix → add regression test
- `cargo clippy -- -D warnings && cargo test && ./dev test` must all pass before commit
- Tests use fixed seed 42, deterministic, no visual assertions
- When a test fails, fix the CODE not the test

## Key Patterns

### Adding a new UI panel
1. Create `crates/aa2-client/src/new_ui.rs` with `#[class(init, base=Control)]`
2. Add `mod new_ui;` to `lib.rs`
3. Add node to `client/main.tscn` (type=NewUI, anchors_preset=15)
4. Wire visibility in `main_scene.rs` `switch_to_phase()`

### Adding a new GameManager API
1. Add `#[func]` method to `game_manager.rs`
2. Return GDScript-friendly types: GString, i32, f32, bool, Array, VarDictionary, PackedStringArray

### Adding a new god
1. Create `data/gods/god_name.ron` with `God(name: "...", description: "...", passive: Variant(...))`
2. If new passive type: add variant to `GodPassive` enum in `aa2-data/src/lib.rs`, add gameplay logic in `aa2-game/src/god.rs`

## Reference Screenshots

Located at `~/Downloads/full game screenshots/`:
- `start - god pick.png` — God selection grid
- `god selected.png` — God selected with preview
- `shop round 7.png` — Full shop + loadout + damage meter (right side)
- `round 1 start.png` — Draft overlay
- `ability hovered, details shown.png` — Tooltip with full ability info
- `summary toggle.png` / `summary toggle 2.png` — Scoreboard overlay
- `round 19, summary toggled.png` — Summary during gameplay
- `endgame summary.png` — Final standings with placements
- `round 13 combat, 5v5.png` — Combat in progress

## Uncommitted Changes

All Sprint 3+4 work is uncommitted. Files changed (16 modified, 4 new):
- New: `endgame_ui.rs`, `ui_helpers.rs`, `data/gods/archmage.ron`, `data/gods/paladin.ron`
- Deleted: `data/gods/forge_lord.ron`
- Modified: god_pick_ui, scoreboard_ui, main_scene, game_manager, loadout_ui, board_ui, draft_ui, shop_ui, lib.rs, game.rs, god.rs, aa2-dev.rs, aa2-data/lib.rs, main.tscn, test_game_flow.gd
