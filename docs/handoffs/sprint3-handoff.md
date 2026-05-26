# Sprint 3+ Handoff — Fresh Agent Guide

## Current State (Sprint 2 Complete)

The game is **fully playable** in a 2-player local dev mode:
- God pick → Shop (buy/reroll/lock/upgrade) → Draft heroes → Equip abilities → Combat (animated) → repeat
- 29 integration tests + 234 unit tests, all passing
- Combat viewer plays back all 15 event types with HP bars, damage popups, death fades, buff indicators, projectiles

### What Works
- Full game loop with phase transitions
- Shop: buy, reroll, lock, upgrade (with correct cost decay and size scaling)
- Draft: appears on rounds 1, 3, 6, 9, 12 with tier-appropriate heroes
- Hero reroll: costs 2g, shows 3-card draft (STR/AGI/INT from all tiers), auto-picks on Ready
- Equip/unequip/swap abilities, bench cap (5), level-up on duplicate
- Combat: full sim playback with unit movement, attacks, abilities, HP bars, death
- Arena: hero positioning in bottom half (1000x2000 of 2000x2000 arena)
- Unit info panel: shows hero stats when selected on board

### Architecture
```
Godot 4.6 ←→ aa2-client (gdext 0.5, cdylib) → aa2-game → aa2-sim → aa2-data
```
- All game logic in Rust, all layout in .tscn
- GameManager at `/root/MainScene/GameManager` — single point of contact for all UI
- UI classes query GameManager every frame in `process()`

## Sprint 3: Draft + Overlays

### GodPickOverlay (Priority 1)
Currently: 2 buttons listing gods. Should be: grid of god portraits (like reference screenshots).

**What to build:**
- Grid of clickable god portraits (use `GridContainer` with `Button` children)
- Selected god preview on the right (name, description, HP)
- Confirm/Discard buttons
- Timer bar (visual only for now — no timer logic yet)
- Reference: `~/Downloads/full game screenshots/start - god pick.png`

**API available:** `get_available_gods()` → Array of dicts with `name` + `description`

**File:** `crates/aa2-client/src/god_pick_ui.rs`

### SummaryOverlay (Priority 2)
Toggle with the Summary button in TopBar. Shows player standings.

**What to build:**
- Semi-transparent overlay covering arena area
- Player rows: placement, name, god, W/L record, hero icons, ability icons
- Reference: `~/Downloads/full game screenshots/summary toggle.png`

**API available:** `get_player_count()`, `get_player_hp()`, `get_player_god()`, `get_heroes()`, `get_equipped_abilities()`

**File:** `crates/aa2-client/src/scoreboard_ui.rs` (exists as stub)

### Phase Visibility Polish (Priority 3)
The `switch_to_phase()` in `main_scene.rs` handles basic show/hide. Needs:
- DraftUI should only show during Shop phase when choices exist (currently works)
- Hide BottomPanel during Combat (currently works)
- Summary button toggles ScoreboardUI visibility

## Sprint 4: Polish + Secondary Panels

### Drag-and-Drop
- Equip: drag ability from bench to hero slot
- Sell: drag ability to sell bin
- Position: drag hero on board

### Ability Tooltips
- Hover over ability card → show tooltip with name, description, damage values, cooldown
- Reference: `~/Downloads/full game screenshots/ability hovered, details shown.png`
- API needed: add `get_ability_info(name)` to GameManager (similar to `get_hero_info`)

### DamageMeter
- Right sidebar during/after combat
- Shows damage dealt per unit, grouped by team
- Reference: `~/Downloads/full game screenshots/shop round 7.png` (right side)

### Better Placeholder Art
- Colored rectangles by attribute: STR=#e74c3c, AGI=#2ecc71, INT=#3498db
- Ultimate abilities: purple border (#9b59b6)
- Hero portraits: 80×80 colored circles with name initial

## Game Logic Gaps (Any Sprint)

### Finished Phase
- When only 1 player remains alive → set `phase = GamePhase::Finished`
- Show EndgameScreen overlay with final standings
- Test commented out in `client/tests/test_game_flow.gd` (uncomment when implemented)

### Sell Ability
- `apply_player_action(0, "Sell", "ability_name")` — exists in game logic
- UI: drag to sell bin or click sell bin after selecting ability
- Returns gold (1g per ability? TBD)

## Key Files

| File | Purpose |
|------|---------|
| `crates/aa2-client/src/main_scene.rs` | Phase transitions, AI auto-actions, top-level wiring |
| `crates/aa2-client/src/game_manager.rs` | All game state queries and mutations (28+ #[func] methods) |
| `crates/aa2-client/src/shop_ui.rs` | Shop row (gold, upgrade, slots, reroll, lock) |
| `crates/aa2-client/src/loadout_ui.rs` | Hero rows + ability slots + bench |
| `crates/aa2-client/src/board_ui.rs` | Arena display + hero positioning |
| `crates/aa2-client/src/combat_viewer_ui.rs` | Combat playback (all 15 event types) |
| `crates/aa2-client/src/god_pick_ui.rs` | God selection (needs rebuild) |
| `crates/aa2-client/src/draft_ui.rs` | 3-card hero draft overlay |
| `crates/aa2-client/src/scoreboard_ui.rs` | Summary overlay (needs rebuild) |
| `client/main.tscn` | Scene hierarchy with anchors |
| `client/tests/test_*.gd` | Integration tests (29 tests) |

## Dev Workflow

```bash
./dev          # Build + launch Godot
./dev editor   # Build + open editor (inspect scenes)
./dev check    # cargo check + clippy + test
./dev test     # Build + run 29 integration tests (requires display)
```

**Must close Godot before rebuilding** — no hot-reload with gdext.

## Testing Requirements

From `AGENTS.md`:
- New game behavior → add integration test in `client/tests/`
- Bug fix → add regression test
- `./dev test` must pass before commit
- Tests use fixed seed 42, deterministic, no visual assertions

## Reference Screenshots

Located at `~/Downloads/full game screenshots/`:
- `start - god pick.png` — God selection grid
- `shop round 7.png` — Full shop + loadout + damage meter
- `round 1 start.png` — Draft overlay
- `ability hovered, details shown.png` — Tooltip
- `summary toggle.png` — Scoreboard overlay
- `endgame summary.png` — Final standings
