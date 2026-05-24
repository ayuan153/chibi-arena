# UI Layout Design

Base resolution: 1920×1080. Godot stretch mode: `canvas_items` / `expand`.
Scales to tablet (2048×1536) and laptop (1440×900) without rework.
Art style: flat top-down 2D, chibi-style characters.

## Screen Regions

```
┌──────────────────────────────────────────────────────────────────────────┐
│ TOP BAR (32px)                                                           │
│ [⚙][🏆][📋][SPELL DECK]          55s  ROUND 4  FIGHTING        [SUMMARY]│
├────────┬─────────────────────────────────────────────────┬───────────────┤
│        │                                                 │               │
│ PLAYER │                                                 │   DAMAGE      │
│ LIST   │                                                 │   METER       │
│ (left) │                    ARENA                        │   (right)     │
│        │               (center, ~60%)                    │               │
│ ♥200   │                                                 │  Tinker  754  │
│ ♥200   │         ┌─────────────────────┐                 │  ──────────── │
│ ♥193   │         │   Enemy half (top)  │                 │  Sven    391  │
│ ♥192   │         │                     │                 │  ──────────── │
│ ♥187   │         │─ ─ ─ ─ ─ ─ ─ ─ ─ ─│                 │  Snapfire 728 │
│ ♥186   │         │  Player half (bot)  │                 │  ──────────── │
│ ♥180   │         │  [drag heroes here] │                 │  Axe     302  │
│ ♥180   │         └─────────────────────┘                 │               │
│        │                                                 │               │
├────────┼──────────────────────────────────────────┬──────┴───────────────┤
│ MY GOD │          BOTTOM CENTER                   │     UNIT INFO        │
│(b-left)│                                          │     (b-right)        │
│        │  [Gold:0] [Upgrade 15] [A][A][A][A][A][A]│ [Reroll][Lock]       │
│ ┌────┐ │                                          │                      │
│ │GOD │ │  ┌Hero1─────┐ ┌Hero2─────┐ ┌Hero3─────┐│  ┌──────────────┐    │
│ │PORT│ │  │[P][1][2][3][4]│ │[P][1][2][3][4]│ │[P][1][2][3][4]││  │  TINKER      │    │
│ └────┘ │  │  [reroll] │ │  [reroll] │ │  [reroll] ││  │  ♥629/736    │    │
│ ♥180   │  └───────────┘ └───────────┘ └───────────┘│  │  [abilities] │    │
│username│  ┌Hero4─────┐ ┌Hero5─────┐ ┌Bench──────┐│  │  [stats]     │    │
│ 🗑sell │  │[P][1][2][3][4]│ │[P][1][2][3][4]│ │[B1][B2][B3][B4][B5]││  └──────────────┘    │
│        │  └───────────┘ └───────────┘ └───────────┘│                      │
├────────┴──────────────────────────────────────────┴──────────────────────┤
│ ENEMY GOD (top-right corner, opposite player god)                        │
│ Shows during combat: enemy portrait + username + HP                      │
└──────────────────────────────────────────────────────────────────────────┘
```

## Region Specifications

### Top Bar
- **Height:** 32px fixed
- **Anchors:** full width, top-pinned
- **Left:** Icon buttons (settings, cosmetics, spell deck)
- **Center:** Timer, round number, phase name
- **Right:** Summary button

### Player List (Left Sidebar)
- **Anchors:** top=0.03, bottom=0.65, left=0, right=0.10
- **Contents:** 8 player rows sorted by HP descending. Each row: god icon (small), HP with heart, username
- **Always visible** across all phases

### Arena (Center)
- **Anchors:** top=0.03, bottom=0.65, left=0.10, right=0.85
- **Contents:** Flat 2D grid. Top half = enemy units, bottom half = player units.
- **Shop phase:** Player's units in position, draggable on bottom half
- **Combat phase:** Animated fight playback
- **Panning:** Click-drag empty space to view other players' arenas. Space = reset to home.

### Damage Meter (Right Sidebar)
- **Anchors:** top=0.30, bottom=0.65, left=0.85, right=1.0
- **Contents:** Per-unit damage bars, color-coded (red=physical, blue=magical, yellow=pure). Header shows opponent name.
- **Toggleable:** Damage dealt / damage taken / healing done
- **Always visible** (shows 0 on round 1, last fight's data during shop)
- **Priority:** Later deliverable

### Enemy God (Top-Right Corner)
- **Anchors:** top=0.03, bottom=0.25, left=0.85, right=1.0
- **Contents:** Enemy god portrait (ornate frame), username, HP
- **Visibility:** Always visible during combat; shows last opponent during shop

### My God (Bottom-Left)
- **Anchors:** top=0.65, bottom=1.0, left=0, right=0.10
- **Contents:** Large god portrait (ornate circular frame), god HP, username, rank icon
- **Below portrait:** Sell bin (trash icon, drag target)
- **God power:** Click portrait/button → cursor changes → click hero to apply

### Bottom Center (Shop + Loadouts)
- **Anchors:** top=0.65, bottom=1.0, left=0.10, right=0.75
- **Layout (top to bottom):**

#### Shop Row
- **Left of abilities:** Gold counter + Upgrade button (shows cost)
- **Center:** 5-10 ability card slots (click to buy, 3g each)
- **Right of abilities:** Reroll button (1g) + Lock toggle

#### Hero Loadouts + Bench
- **Grid layout:** Max 3 loadouts per row
- **Each loadout:** Portrait (square) + 4 ability slots (horizontal). Reroll button below portrait.
- **Bench:** Treated as a 6th "loadout" slot (5 ability slots, no portrait)
- **Wrapping:**
  - 1-3 heroes: `[H1] [H2] [H3] [Bench]` (single row)
  - 4-5 heroes: `[H1] [H2] [H3]` / `[H4] [H5] [Bench]` (two rows)

### Unit Info (Bottom-Right)
- **Anchors:** top=0.65, bottom=1.0, left=0.75, right=1.0
- **Contents:** Selected unit's full stats — portrait, HP/mana bars, ability icons with levels, base stats, bonus stats, active buffs
- **Visibility:** Only when a unit is selected (click hero portrait or arena unit)
- **Style:** Dota-style stat panel

## Interaction Model

### Shop Phase
- **Buy:** Click shop ability slot → goes to bench (3g)
- **Equip:** Drag bench ability → hero ability slot (free)
- **Unequip:** Drag equipped ability → bench (1g)
- **Sell:** Drag ability → sell bin at bottom-left (refund 2g × level)
- **Reroll:** Click reroll button (1g)
- **Upgrade shop:** Click upgrade button (variable cost)
- **Lock:** Click lock toggle (free)
- **Position hero:** Drag hero portrait → arena grid (bottom half)
- **Reroll hero:** Click reroll button below hero portrait (2g)
- **Select unit:** Click hero portrait → shows in Unit Info panel

### God Power (e.g., Paladin)
1. Click god power button (on god portrait area)
2. Cursor changes (purple glow)
3. Click a hero (in loadout or on arena)
4. God buff applied to that hero

### Arena Panning
- Click-drag empty arena space → pan to other players' boards
- Space key → snap back to home arena
- During combat: shows animated fight (event-driven playback)

## Phase-Specific Visibility

| Element | GodPick | Shop | Combat | Finished |
|---------|---------|------|--------|----------|
| Top Bar | ✓ | ✓ | ✓ | ✓ |
| Player List | hidden | ✓ | ✓ | ✓ |
| Arena | hidden | ✓ (positioning) | ✓ (animated) | ✓ |
| Damage Meter | hidden | ✓ (last fight) | ✓ (live) | ✓ |
| Enemy God | hidden | hidden | ✓ | hidden |
| My God | hidden | ✓ | ✓ | ✓ |
| Bottom Center | hidden | ✓ | hidden | hidden |
| Unit Info | hidden | on select | on select | hidden |
| God Pick UI | ✓ (fullscreen) | hidden | hidden | hidden |
| Draft UI | hidden | ✓ (overlay on arena) | hidden | hidden |
| Scoreboard | hidden | toggle | toggle | ✓ (fullscreen) |

## Implementation Plan

### Phase 1: Basic Layout (make it usable)
- Create root `.tscn` with MarginContainer regions
- Position existing UI classes into correct zones
- Set Godot project to 1920×1080 with canvas_items stretch
- Dark background color (#1a1a2e)
- Verify: can buy, equip, position, ready, fight

### Phase 2: Polish Existing Screens
- Style ability cards (bordered squares, level badges)
- Hero portraits (placeholder colored circles with name)
- HP bars on arena units
- Gold/cost displays with proper formatting

### Phase 3: New UI Elements
- Player list sidebar
- Damage meter (later)
- Unit info panel
- Enemy god display
- God power targeting cursor

### Phase 4: Interactions
- Drag-and-drop (equip, sell, position)
- Arena panning
- God power click-to-target flow

## Color Palette

- Background: #1a1a2e (dark navy)
- Panel backgrounds: #16213e (slightly lighter)
- Ability card border: #0f3460
- Ultimate ability border: #e94560 (red/pink)
- Gold text: #ffd700
- HP bar: #4caf50 → #f44336
- Mana bar: #2196f3
- Physical damage: #ff4444
- Magical damage: #4488ff
- Pure damage: #ffcc00
