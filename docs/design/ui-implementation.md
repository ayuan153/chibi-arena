# UI Implementation Plan

## Current State

- Phase 3 client exists with functional game logic (buy, equip, draft, combat all work in code)
- UI is programmatically built with layering/input issues
- Need to rebuild as proper .tscn scene hierarchy

## Architecture: .tscn + Rust Scripts

- Layout defined in .tscn files (positions, sizes, containers)
- Logic in Rust gdext classes (state queries, action dispatch, dynamic content)
- Each major UI region is a separate scene (composable)

**Principle:** Scenes own layout. Rust owns behavior. Never position nodes from code if it can be done in the editor.

## Scene Hierarchy

### main.tscn (root)

```
MainScene (Control, script: MainScene) [full rect]
├── PersistentChrome [full rect, always visible]
│   ├── TopBar (HBoxContainer) [top strip, 32px]
│   │   ├── IconButtons (HBox) [left]
│   │   ├── GameInfo (Label) [center] — "Round N · Phase · Timer"
│   │   └── SummaryButton (Button) [right]
│   ├── PlayerList (VBoxContainer) [left sidebar, 10% width]
│   │   └── PlayerRow × 8 (HBox: god_icon + hp + name)
│   ├── GodPortrait (Control) [bottom-left corner]
│   │   ├── PortraitFrame (TextureRect) — placeholder colored circle
│   │   ├── HPLabel
│   │   ├── NameLabel
│   │   └── SellBin (Button) — trash icon
│   └── EnemyGod (Control) [top-right corner, hidden during shop]
│       ├── PortraitFrame
│       ├── NameLabel
│       └── HPLabel
├── ArenaRegion (Control) [center, 75% width, 62% height]
│   ├── ArenaGrid (Control) — flat 2D grid background
│   ├── UnitContainer (Control) — spawned unit nodes during combat
│   └── DraftOverlay (Control) [centered in arena, hidden unless draft active]
│       ├── Title (Label) — "SELECT A UNIT"
│       ├── CardContainer (HBoxContainer)
│       │   ├── DraftCard0 (Button) — hero portrait + name + tier badge
│       │   ├── DraftCard1 (Button)
│       │   └── DraftCard2 (Button)
│       ├── TimerBar (ProgressBar)
│       └── ConfirmButton (Button)
├── BottomPanel (Control) [bottom 38%, left 10% to right 75%]
│   ├── ShopRow (HBoxContainer) [top of bottom panel]
│   │   ├── GoldDisplay (Label) — "20"
│   │   ├── UpgradeButton (Button) — "Upgrade 10"
│   │   ├── AbilitySlots (HBoxContainer)
│   │   │   └── AbilityCard × 10 (Button) — icon placeholder + name
│   │   ├── RerollButton (Button) — "Reroll 1"
│   │   └── LockButton (Button) — "Lock"
│   └── LoadoutGrid (GridContainer or VBox of HBoxes) [below shop row]
│       ├── HeroLoadout × 5 (HBoxContainer)
│       │   ├── Portrait (Button) — hero icon, click to select
│       │   ├── AbilitySlot0-3 (Button) — equipped ability icons
│       │   └── RerollHeroBtn (Button) — small, below portrait
│       └── BenchSlots (HBoxContainer)
│           └── BenchSlot × 5 (Button) — ability icons on bench
├── UnitInfoPanel (Control) [bottom-right 25%, hidden unless selected]
│   ├── Portrait
│   ├── HPBar / ManaBar
│   ├── AbilityIcons (HBox)
│   ├── StatsGrid (labels for STR/AGI/INT, armor, AS, damage)
│   └── BuffList
├── DamageMeter (Control) [right sidebar, below enemy god]
│   ├── Header (Label) — opponent name
│   ├── UnitDamageRows (VBox)
│   │   └── Row × N (HBox: icon + name + bar + number)
│   └── ToggleButtons (HBox: Dealt/Taken/Healing)
├── Overlays [on top of everything, hidden by default]
│   ├── GodPickOverlay (Control) [full screen]
│   │   ├── Title (Label) — "Draft Your God"
│   │   ├── GodGrid (GridContainer) — god portraits
│   │   ├── SelectedGodPreview (right side)
│   │   ├── ConfirmButton / DiscardButton
│   │   ├── TimerBar
│   │   └── RandomButton
│   ├── SummaryOverlay (Control) [semi-transparent, covers arena]
│   │   └── PlayerRows × 8 (placement, name, god, W/L, hero icons, ability icons)
│   ├── SpellDeckOverlay (Control) [modal panel, center]
│   │   ├── SearchBar
│   │   ├── Tabs (Available/Banned/Favorites)
│   │   ├── PlayerFilter (VBox)
│   │   ├── CategoryFilter (VBox of checkboxes)
│   │   └── AbilityGrid (GridContainer of icons)
│   └── EndgameScreen (Control) [full screen]
│       ├── PlacementHeader
│       ├── RankDisplay
│       └── PlayerResultRows × 8
└── GameManager (Node, script: GameManager) [no visual]
```

## Anchor & Layout Reference

| Node | Anchor Preset | Notes |
|------|--------------|-------|
| MainScene | Full Rect | Root control fills window |
| TopBar | Top Wide | anchor_bottom = 32px |
| PlayerList | Left Wide | anchor_right = 10%, margin_top = 32px |
| GodPortrait | Bottom-Left | 150×200px, margin from corner |
| EnemyGod | Top-Right | 150×150px, below TopBar |
| ArenaRegion | Custom | left=10%, right=75%, top=32px, bottom=62% |
| BottomPanel | Custom | left=10%, right=75%, top=62%, bottom=100% |
| UnitInfoPanel | Bottom-Right | right 25% width, bottom 38% height |
| DamageMeter | Custom | right sidebar, top=150px to bottom=62% |
| Overlays | Full Rect | z_index = 10, mouse_filter = STOP |

## Build Order (Priority)

### Sprint 1: Persistent Chrome + Shop (make it playable)

1. Set up project.godot (1920×1080, canvas_items stretch mode, dark bg `#1a1a2e`)
2. Build PersistentChrome: TopBar, PlayerList, GodPortrait
3. Build BottomPanel: ShopRow with clickable ability cards
4. Build LoadoutGrid: hero portraits + ability slots
5. Wire up: buy, reroll, upgrade, lock, equip (click-to-equip for now)
6. Add ReadyButton to advance phases
7. Verify: can play god pick → shop → buy → equip → ready

**Acceptance:** A player can open the client, see their gold/HP, buy abilities from the shop, equip them onto heroes, and press Ready to advance rounds.

### Sprint 2: Arena + Combat

1. Build ArenaRegion with grid background (ColorRect + draw lines)
2. Hero positioning: drag portraits from loadout to arena grid cells
3. Combat playback: spawn unit nodes, animate from CombatEvent stream
4. HP bars above units, floating damage numbers, death fade-out
5. Verify: can position heroes, run combat, watch fight play out

**Acceptance:** Heroes appear on the grid, combat animates attacks/movement/death, round result shown.

### Sprint 3: Draft + Overlays

1. Build DraftOverlay (cards appear in arena area during draft phase)
2. Build GodPickOverlay (grid of gods, confirm selection)
3. Build SummaryOverlay (toggle with SummaryButton)
4. Wire phase visibility (MainScene shows/hides based on GameState phase)

**Acceptance:** Full game loop from god pick through multiple rounds with draft phases.

### Sprint 4: Polish + Secondary Panels

1. UnitInfoPanel (click hero to see stats)
2. DamageMeter (post-combat breakdown)
3. EnemyGod display during combat
4. SpellDeck overlay (browse all abilities)
5. Ability hover tooltips (RichTextLabel popup)

## Key Technical Decisions

### Input Handling

- Use Godot's built-in `Button` nodes for all clickable elements
- Drag-and-drop via `Control._get_drag_data()` / `_can_drop_data()` / `_drop_data()`
- `mouse_filter = IGNORE` on container Controls that shouldn't block input
- Overlays use `mouse_filter = STOP` to capture all input when visible
- Never use `_input()` or `_unhandled_input()` for UI — let the scene tree handle propagation

### Dynamic Content

- Shop abilities: `ShopUI.refresh()` reads GameManager state and updates button text/icons
- Loadout grid: rebuilt when heroes change (draft pick, sell)
- Player list: updated from GameManager state on phase change / HP change
- Combat units: spawned/destroyed based on CombatEvent stream during playback

### Placeholder Art

All art is procedural placeholders until real assets exist:

- Ability cards: 64×64 `ColorRect` with 2-letter `Label` abbreviation
- Hero portraits: 80×80 colored circles (draw_circle in _draw) with hero name initial
- God portraits: 120×120 colored circles
- Attribute colors: STR=`#e74c3c`, AGI=`#2ecc71`, INT=`#3498db`, Ultimate=purple border `#9b59b6`
- Background: `#1a1a2e`, panels: `#16213e`, borders: `#0f3460`

### Data Flow

```
GameManager (holds GameState, hero_defs, ability_defs, rng)
    ↓ #[func] queries (get_shop_slots, get_loadout, get_phase, etc.)
UI Nodes (read state, display it)
    ↓ button signals → #[func] handlers on UI scripts
GameManager.apply_player_action(action_string)
    ↓
GameState.apply_action()
    ↓
UI refreshes on next frame (or signal-driven)
```

### Rust Class → Scene Attachment

| Rust Class | Extends | Attached To |
|-----------|---------|-------------|
| `MainScene` | `Control` | main.tscn root |
| `GameManager` | `Node` | GameManager node in main.tscn |
| `ShopUi` | `HBoxContainer` | ShopRow in shop_row.tscn |
| `LoadoutUi` | `Control` | LoadoutGrid in loadout_grid.tscn |
| `ArenaUi` | `Control` | ArenaRegion in arena_grid.tscn |
| `CombatViewer` | `Control` | UnitContainer (spawns children) |
| `DraftUi` | `Control` | DraftOverlay in draft_overlay.tscn |
| `GodPickUi` | `Control` | GodPickOverlay in god_pick.tscn |
| `PlayerListUi` | `VBoxContainer` | PlayerList in player_list.tscn |
| `UnitInfoUi` | `Control` | UnitInfoPanel |
| `DamageMeterUi` | `Control` | DamageMeter |

### File Structure

```
client/
├── project.godot
├── main.tscn              — root scene
├── scenes/
│   ├── chrome/
│   │   ├── top_bar.tscn
│   │   ├── player_list.tscn
│   │   └── god_portrait.tscn
│   ├── shop/
│   │   ├── shop_row.tscn
│   │   └── loadout_grid.tscn
│   ├── arena/
│   │   ├── arena_grid.tscn
│   │   └── draft_overlay.tscn
│   └── overlays/
│       ├── god_pick.tscn
│       ├── summary.tscn
│       └── spell_deck.tscn
├── themes/
│   └── default_theme.tres — shared font, button styles, colors
├── aa2_client.gdextension
└── bin/                   — dylib (gitignored)

crates/aa2-client/src/
├── lib.rs
├── main_scene.rs          — phase management, overlay toggling
├── game_manager.rs        — state holder, action dispatch
├── shop_ui.rs             — shop row logic
├── loadout_ui.rs          — hero loadout grid logic (NEW)
├── arena_ui.rs            — arena grid + unit positioning (NEW)
├── combat_viewer.rs       — combat playback (RENAME from board_ui)
├── draft_ui.rs            — draft card selection
├── god_pick_ui.rs         — god selection grid
├── player_list_ui.rs      — sidebar player list (NEW)
├── unit_info_ui.rs        — selected unit stats (NEW)
└── damage_meter_ui.rs     — combat damage display (NEW)
```

## What Exists and Works (keep)

- `GameManager`: `init_game`, `apply_player_action`, all query methods
- Game logic: buy, sell, equip, unequip, reroll, draft, ready, combat
- Combat events: `UnitSpawn`, `MoveTo`, `Attack`, `Death`, `CastAbility`, `ApplyBuff`, etc.
- Data loading: `hero_defs`, `ability_defs` from RON files
- All `aa2-game`, `aa2-sim`, `aa2-data` crates — untouched

## What Needs Rewriting

- All UI classes (`shop_ui`, `board_ui`, `bench_ui`, etc.) — replace programmatic layout with .tscn-backed
- `main.tscn` — complete rebuild with proper hierarchy above
- `MainScene.rs` — simplify to just phase management + overlay toggling
- Remove: `dev_console.rs` (use Godot Output panel or in-game overlay later)
- Remove: `scoreboard_ui.rs` (replaced by `player_list_ui` + summary overlay)

## Migration Checklist

For each UI class being rewritten:

1. [ ] Create .tscn scene with correct node types and anchors
2. [ ] Create/update Rust class with `#[derive(GodotClass)]`, set `#[class(base=X)]`
3. [ ] Use `#[func]` for all methods called from signals or other nodes
4. [ ] Get child node references in `ready()` via `get_node_as::<T>(path)`
5. [ ] Connect button signals in `ready()` or in the .tscn editor
6. [ ] Test in isolation (scene can be run standalone for layout check)
7. [ ] Integrate into main.tscn as instanced scene
