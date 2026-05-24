# UI Layout Design

Base resolution: 1920×1080. Godot stretch mode: `canvas_items` / `expand`.
Scales to tablet (2048×1536) and laptop (1440×900) without rework.

## Screen Regions

```
┌─────────────────────────────────────────────────────────────────┐
│ TOP BAR (40px)                                                  │
│ [Spell Deck ▼] [Battle Pass ▼] [Settings ▼]     Round 3  0:42  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│                                                                 │
│                         ARENA (center)                          │
│                     ~60% of screen height                       │
│                                                                 │
│         ┌───────────────────────────────────┐                   │
│         │  Opponent's half (top)            │                   │
│  GOD    │                                   │                   │
│  INFO   │─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─│   UNIT            │
│  (left) │                                   │   INFO            │
│         │  Player's half (bottom)           │   (right)         │
│         │  [drag heroes here]               │                   │
│         └───────────────────────────────────┘                   │
│                                                                 │
├────────┬────────────────────────────────────────────┬───────────┤
│ SHOP   │              BOTTOM PANEL (~35%)           │ SHOP      │
│ CTRL   │                                            │ CTRL      │
│ (left) │  ┌─SHOP (5-10 ability slots)────────────┐ │ (right)   │
│        │  │ [Slot1] [Slot2] [Slot3] [Slot4] ...  │ │ [Reroll]  │
│ Gold:50│  └──────────────────────────────────────┘ │ [Lock]    │
│[Upgrade]│                                           │           │
│ [Sell] │  ┌─HEROES + BENCH─────────────────────┐  │           │
│  bin   │  │ Hero1: [P][A1][A2][A3][A4]         │  │           │
│        │  │ Hero2: [P][A1][A2][A3][A4]         │  │           │
│        │  │ Hero3: [P][A1][A2][A3][A4]         │  │           │
│        │  │ Hero4: [P][A1][A2][A3][A4]         │  │           │
│        │  │ Hero5: [P][A1][A2][A3][A4]         │  │           │
│        │  │ Bench: [B1][B2][B3][B4][B5]        │  │           │
│        │  └────────────────────────────────────┘  │           │
└────────┴────────────────────────────────────────────┴───────────┘
```

## Region Specifications

### Top Bar
- **Height:** 40px fixed
- **Anchors:** top=0, bottom=0, left=0, right=1 (full width, fixed height)
- **Contents:** Expandable dropdown buttons (Spell Deck, Battle Pass, Settings), round counter, phase timer
- **Visibility:** Always visible

### Arena (Center)
- **Anchors:** top=0.04, bottom=0.60, left=0.12, right=0.82
- **Contents:** 2000×2000 game-unit grid mapped to this region. Top half = opponent, bottom half = player.
- **Interaction:** Drag heroes on player's half. Click to select. Space to reset view after panning.
- **Panning:** Click-drag on empty space to pan to other players' arenas.

### God Info (Left Sidebar)
- **Anchors:** top=0.04, bottom=0.60, left=0, right=0.12
- **Width:** ~230px at 1920
- **Contents:** God portrait, god power button(s), passive description on hover
- **Interaction:** Click god power → cursor changes → click hero to apply buff (Paladin-style)

### Unit Info (Right Sidebar)
- **Anchors:** top=0.04, bottom=0.60, left=0.82, right=1.0
- **Width:** ~345px at 1920
- **Contents:** Selected unit's full stats (HP, armor, AS, damage, abilities, buffs)
- **Visibility:** Only when a unit is selected. Hidden otherwise.

### Bottom Panel
- **Anchors:** top=0.60, bottom=1.0, left=0, right=1.0
- **Height:** ~40% of screen
- **Sub-regions:**

#### Shop Controls Left (bottom-left)
- **Anchors:** top=0.60, bottom=1.0, left=0, right=0.10
- **Contents:** Gold display, Upgrade button (with cost), Sell bin (drag target)

#### Shop + Heroes + Bench (bottom-center)
- **Anchors:** top=0.60, bottom=1.0, left=0.10, right=0.85
- **Layout:** Vertical stack:
  1. **Shop row:** 5-10 ability card slots (horizontal). Click to buy.
  2. **Hero loadouts:** 5 rows, each = portrait + 4 ability slots. Click ability to select/unequip.
  3. **Bench:** 1 row of 5 ability slots. Click to select. Drag to hero to equip.

#### Shop Controls Right (bottom-right)
- **Anchors:** top=0.60, bottom=1.0, left=0.85, right=1.0
- **Contents:** Reroll button (with cost), Lock toggle button

## Interaction Model

### Shop Phase
- **Buy:** Click shop slot → ability goes to bench (costs 3g)
- **Equip:** Drag bench ability to hero slot (free)
- **Unequip:** Drag equipped ability to bench (costs 1g)
- **Sell:** Drag ability to sell bin (refunds 2g × level)
- **Reroll:** Click reroll button (costs 1g)
- **Upgrade:** Click upgrade button (variable cost)
- **Lock:** Click lock toggle (free)
- **Position:** Drag hero portrait onto arena grid

### God Power (Paladin example)
1. Click god power button (bottom-left panel)
2. Cursor changes to purple glow
3. Click a hero on the board or in loadout
4. God buff applied to that hero

### Arena Panning
- Click-drag empty arena space → pan to other players
- Space key → snap back to home arena
- During combat: shows live fight animation

## Phase-Specific Visibility

| Element | GodPick | Shop | Combat | Finished |
|---------|---------|------|--------|----------|
| Top Bar | ✓ | ✓ | ✓ | ✓ |
| Arena | hidden | ✓ | ✓ (animated) | ✓ |
| God Info | hidden | ✓ | ✓ | ✓ |
| Unit Info | hidden | on select | on select | hidden |
| Bottom Panel | hidden | ✓ | hidden | hidden |
| God Pick UI | ✓ (fullscreen overlay) | hidden | hidden | hidden |
| Draft UI | hidden | ✓ (overlay) | hidden | hidden |
| Scoreboard | hidden | toggle | toggle | ✓ (fullscreen) |

## Implementation Notes

- Use Godot Container nodes (VBox, HBox, MarginContainer) for responsive layout
- Ability cards and hero portraits are fixed-size (64×64 or 80×80) within flexible containers
- Arena uses a SubViewport or direct draw for the game grid
- All layout in `.tscn` files; all logic in Rust `#[godot_api]` scripts
- Drag-and-drop uses Godot's built-in DnD system (Control._get_drag_data, _can_drop_data, _drop_data)

## Color Palette (placeholder, refine later)

- Background: #1a1a2e (dark navy)
- Panel backgrounds: #16213e (slightly lighter)
- Ability cards: #0f3460 border, #e94560 for ultimates
- Gold text: #ffd700
- HP bar: #4caf50 (green) → #f44336 (red)
- Mana bar: #2196f3 (blue)
