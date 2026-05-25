# Phase 3 Handoff: UI Rebuild

Project: AA2 — Ability Arena 2 standalone port
Repo: https://github.com/ayuan153/aa2.git
Working directory: the repo root (aa2/)

## CONTEXT

This is a standalone cross-platform autobattler (iOS/Android/PC) inspired by the Dota2 mod Ability Arena.
The combat simulation engine, full game loop, and Godot client skeleton are built. The game logic works
(buy, equip, draft, combat, phase transitions) but the UI has layering/input issues from being built
programmatically. We're rebuilding the UI properly using .tscn scene files.

## START HERE

1. Read `AGENTS.md` — dev process, commit convention, test loop
2. Read `docs/design/godot-dev-workflow.md` — how to build/run/debug the Godot client
3. Read `docs/design/ui-layout.md` — screen regions, anchor values, interaction model
4. Read `docs/design/ui-implementation.md` — **THE MAIN SPEC** — full scene hierarchy, build order, technical decisions
5. Read `docs/design/architecture.md` — crate structure
6. Look at reference screenshots in `~/Downloads/full game screenshots/` (convert to jpg first with sips)

## WHAT EXISTS AND WORKS

- `crates/aa2-game/` — Full game logic: economy, shop, draft, combat, gods, matchups (234 tests)
- `crates/aa2-sim/` — Combat simulation with CombatEvent stream (UnitSpawn, MoveTo, Attack, Death, etc.)
- `crates/aa2-client/src/game_manager.rs` — **KEEP THIS** — holds GameState, exposes all queries and actions via #[func]
- `client/` — Godot 4.6 project, GDExtension loads successfully
- `./dev` script — build + launch workflow

## TASK: Rebuild the UI

Follow the build order in `docs/design/ui-implementation.md`:

### Sprint 1: Persistent Chrome + Shop (make it playable)
- Rebuild `main.tscn` with proper container hierarchy
- TopBar, PlayerList sidebar, GodPortrait (bottom-left)
- ShopRow with clickable ability cards (64×64 colored placeholders)
- LoadoutGrid: hero portraits + 4 ability slots each + bench
- Wire: buy, reroll, upgrade, lock, equip (click-to-equip), sell
- ReadyButton to advance phases
- **Acceptance:** Can play god pick → shop → buy → equip → ready → combat

### Sprint 2: Arena + Combat
- ArenaGrid with flat 2D background
- Hero positioning (drag from loadout to arena)
- Combat playback from CombatEvent stream
- HP bars, damage numbers, death fade
- **Acceptance:** Can position heroes, ready up, watch combat animate

### Sprint 3: Draft + Overlays
- DraftOverlay (3 hero cards in arena area)
- GodPickOverlay (grid of gods, select + confirm)
- SummaryOverlay (player rows with heroes + abilities)
- **Acceptance:** Full game loop playable from god pick to elimination

### Sprint 4: Polish + Secondary Panels
- UnitInfoPanel, DamageMeter, EnemyGod, SpellDeck
- Ability hover tooltips
- Drag-and-drop (equip, sell, position)

## KEY CONSTRAINTS

- All layout in .tscn files, all logic in Rust #[func] methods
- GameManager at path `/root/MainScene/GameManager` — all UIs query it
- mouse_filter = IGNORE on containers, STOP on overlays
- Placeholder art: colored rectangles (STR=red, AGI=green, INT=blue, Ult=purple border)
- Base resolution 1920×1080, canvas_items stretch mode
- Must close + reopen Godot to pick up new dylib (no hot-reload)
- Run `./dev editor` then ▶ to test. First run requires `./dev editor` once to create .godot/ cache.

## VERIFY

Before every commit:
```bash
cargo check && cargo clippy -- -D warnings && cargo test
cp target/debug/libaa2_client.dylib client/bin/
```

Then close Godot, `./dev editor`, hit ▶ to visually verify.
