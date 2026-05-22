# AA2 Game Systems Reference

> Complete specification of game rules, economy, and flow for the AA2 autobattler.
> All numbers are base values — gods can modify any of these.

---

## Round Flow

Each game consists of ~30 rounds. 8 players compete in FFA elimination.

### Core Loop (80 seconds per round)

```
┌─────────────────────────────────────────────────────────────┐
│ COMBAT (up to 50s max)                                       │
│ - Auto-combat between paired opponents                       │
│ - Ends when one side eliminated or 50s timeout (draw)        │
│ - Draw = both players take mutual damage                     │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│ GRACE PERIOD (3s)                                            │
│ - Previous round's gold still usable                         │
│ - Shop auto-rerolls (unless locked)                          │
│ - Damage animation plays                                     │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│ SHOP PHASE (remaining time)                                  │
│ - Gold resets to new round's formula                         │
│ - Buy/sell abilities, reroll shop, upgrade shop              │
│ - Equip abilities onto heroes, rearrange board               │
│ - Ends when timer expires or all players "ready up"          │
└─────────────────────────────────────────────────────────────┘
```

**GodPick:** Pre-game phase (not a round). All players select their god before Round 1.

**Round 1 special:** 40s shop+draft phase, no combat.

**Draft rounds (1/3/6/9/12):** Hero body draft is concurrent with shop (overlay, not blocking).

**Combat timeout:** 50s max. If unresolved, it's a draw — both players take mutual damage.

### Round Sequence

1. **Pre-round:** Hero level up (hero_level = 1 + round_number). Hero body draft on rounds 1/3/6/9/12 (concurrent with shop).
2. **Combat:** Paired opponents fight. See "Combat Matchups" below.
3. **Grace Period:** 3s window. Damage applied, shop auto-rerolls.
4. **Elimination:** Players at 0 HP are eliminated.
5. **Shop:** Gold resets, remaining time for drafting abilities.

---

## Economy

### Gold Per Round

Gold resets every round (no persistence between rounds).

| Round | Gold Available |
|-------|---------------|
| 1 | 6 |
| 2 | 8 |
| 3 | 10 |
| 4 | 12 |
| 5 | 14 |
| 6 | 16 |
| 7 | 18 |
| 8+ | 20 |

Formula: `gold = min(6 + 2 * (round - 1), 20)`

### Costs

| Action | Cost |
|--------|------|
| Buy ability | 3 gold |
| Sell ability | 2 gold × ability level (refund) |
| Reroll shop | 1 gold |
| Unequip/move ability | 1 gold |
| Reroll hero body | 2 gold |
| Shop upgrade | See below |

### Shop Upgrade Cost

Base costs: Level 1→2: **10**, Level 2→3: **14**, Level 3→4: **17**, Level 4→5: **20**

**Decay:** Cost decreases by 1 each round you don't upgrade (per level only).

Example:
- Round 1: Shop L1, upgrade to L2 costs 10
- Round 2: If not upgraded, L2 cost decays to 9
- Round 3: If still not upgraded, L2 cost decays to 8
- Player upgrades to L2. L3 cost starts at 14.
- Round 4: L3 cost decays to 13 (if not purchased)

### Shop Size (abilities shown per roll)

| Shop Level | Choices Shown | Unlocks |
|------------|---------------|---------|
| 1 | 4 | Regular abilities only |
| 2 | 6 | Regular abilities only |
| 3 | 6 | **Unlocks ultimate abilities** |
| 4 | 8 | Regular + ultimates |
| 5 | 10 | Regular + ultimates |

---

## Grace Period

A 3-second window between combat ending and the shop phase beginning.

- Previous round's gold is still usable (spend leftover gold)
- Shop auto-rerolls (unless locked — see Shop Lock)
- After grace period ends: gold resets to new round's formula
- This is when the damage animation plays

---

## Shop Lock

- Free toggle (no gold cost)
- Prevents the shop from auto-rerolling at combat end (during grace period)
- Auto-clears after one preservation (single-use per lock)
- Useful for holding a shop you want to buy from next round

---

## Ability Draft System

### Pool Setup (per game)

1. Select 100 abilities randomly from the full ability roster (or all if < 100 exist)
2. Each selected ability gets **20 copies** in the shared pool
3. All 8 players draw from this shared pool ("without replacement")
4. Ultimate abilities are in a separate sub-pool, only accessible at shop level 3+

### Buying Abilities

- Each purchase costs **3 gold**
- Buying a duplicate of an owned ability **levels it up** (level = copies purchased)
- Max level: **9** (after which the ability is removed from your possible shop choices)
- Abilities appear in shop slots; buying removes from display (reroll to see new options)

### Ability Levels (AA scaling)

| Level | Tier | Notes |
|-------|------|-------|
| 1-3 | Normal | Scaling at each level |
| 4-5 | Plateau | Same as level 3 (no scaling) |
| 6 | **Super** | Big power jump |
| 7-8 | Plateau | Same as level 6 |
| 9 | **Gaben** | Maximum power, game-changing |

### Equipping

- Each hero body has **4 ability slots** (modifiable by gods)
- Each hero can have at most **1 ultimate** equipped
- Abilities are marked as ultimates via `is_ultimate: bool` field in AbilityDef
- **5-slot bench** for unequipped abilities
- Moving an ability between heroes or to bench costs **1 gold**

---

## Hero Bodies

### Draft Schedule

| Round | Tier | Options |
|-------|------|---------|
| 1 | D (Tier 1) | 3 choices: 1 STR, 1 AGI, 1 INT |
| 3 | C (Tier 2) | 3 choices: 1 STR, 1 AGI, 1 INT |
| 6 | B (Tier 3) | 3 choices: 1 STR, 1 AGI, 1 INT |
| 9 | A (Tier 4) | 3 choices: 1 STR, 1 AGI, 1 INT |
| 12 | S (Tier 5) | 3 choices: 1 STR, 1 AGI, 1 INT |

- Universal heroes are categorized by their highest stat gain (tiebreak: INT > AGI > STR)
- Hero body reroll costs **2 gold** (get 3 new options across all tiers)
- Heroes are permanent (kept all game unless rerolled)
- By round 12, every player has **5 heroes**

### Hero Leveling

- Hero level = `1 + combat_round_number`
- Levels up automatically at start of each combat
- Stats scale: `base_stat + stat_gain * (level - 1)`

### Board

- All heroes must be fielded (up to 5)
- Placement: anywhere in the bottom half of arena (1000×2000 area, y ∈ [0, 1000])
- Drag-and-drop positioning

---

## Combat

### Matchups (Round Robin)

- Opponents are paired round-robin style
- Order is randomized each cycle (unpredictable)
- When odd number of players: insert a **ghost opponent**
  - Ghost uses a real player's loadout (clone)
  - Ghost can deal damage but cannot take damage
  - Ensures everyone fights each round

### Arena

- 2000×2000 units, impassable walls
- Each player's units start in their half (bottom y∈[0,1000] vs top y∈[1000,2000])
- Combat is fully automated (no player input during fights)

### Deadlock (Draw)

- If combat hasn't resolved within 50s: draw
- Draw = both players take mutual damage. Each takes damage based on the other's surviving heroes using the standard formula.

---

## Player Damage

Players start with **200 HP**.

### Damage Formula

```
damage = base_damage(round) + per_hero * surviving_enemy_heroes
```

Approximate scaling:
- Round 1, 1 hero survives: ~6 damage
- Round 15, 3 heroes survive: ~25 damage
- Round 30, 5 heroes survive: ~45 damage

Exact formula TBD (needs tuning), but the shape is:
```
base_damage = round * 0.5
per_hero = 1 + round * 0.1
damage = base_damage + per_hero * surviving_heroes
```

(These numbers will be tuned during playtesting.)

---

## Gods

Gods are selected at game start (pre-game phase, not a round). All gods available to all players. Duplicates allowed.

### Implemented Gods

#### Archmage — Sorcery (Passive)
- At shop start: **40% chance** to +1 level a random equipped ability
- On shop upgrade: **guaranteed** +1 level to a random equipped ability
- Free level — no pool deduction (does not consume a copy from the shared pool)
- Scales with shop upgrades (more upgrades = more guaranteed procs)

#### Paladin — Radiant Shield (Select-Buff)
- Buff one selected unit with **70 × round_number bonus HP** + **35% damage reflection**
- One unit at a time (selecting a new unit removes buff from previous)
- Buff applied during shop phase, takes effect at combat start
- **Undispellable** (cannot be purged)

### God Ability Patterns

Gods modify game rules. Common patterns:

| Pattern | Example |
|---------|---------|
| Global combat passive (scales with shop level) | "All heroes gain +2 armor per shop level" |
| Single-unit buff (scales with round) | Paladin Radiant Shield |
| Economy modifier | "Rerolls cost 0", "Start with +2 gold per round" |
| Slot modifier | "+2 ability slots on all heroes" |
| Draft modifier | "See 2 extra choices per roll", "Ultimates available at shop level 2" |
| Passive proc | Archmage Sorcery |

### Select-Buff Gods

- Player selects a target unit during shop phase
- Buff is applied at combat start
- Only one unit can hold the buff at a time
- Buff persists across rounds until a new unit is selected

### God Implementation

Gods are data-driven modifiers applied to `PlayerState`:
```rust
pub struct GodDef {
    pub name: String,
    pub passive: GodPassive,  // enum of modifier types
}
```

The `GodPassive` enum covers all modifier patterns. Gods can modify:
- `ability_slot_count` per hero
- `shop_size_bonus`
- `gold_per_round_bonus`
- `reroll_cost_override`
- Combat buffs (applied as permanent undispellable buffs at combat start)
- Any other game parameter

---

## State Ownership (Client/Server)

### Server Owns (Authoritative)

- Game state (all 8 PlayerStates)
- Ability pool (shared, tracks remaining copies)
- Combat simulation (runs all boards)
- Matchup pairing
- Damage calculation
- Timer management
- Action validation

### Client Receives

- Own PlayerState (full visibility)
- Combat state snapshots (10Hz during combat)
- Other players' public info (HP, hero count, placement — NOT ability details)
- Shop contents (own shop only)
- Round timer

### Client Sends (Actions)

- `BuyAbility(slot_index)`
- `SellAbility(ability_id)`
- `RerollShop`
- `UpgradeShop`
- `EquipAbility(ability_id, hero_index, slot_index)`
- `UnequipAbility(hero_index, slot_index)`
- `RerollHeroBody`
- `PickHeroBody(index)`
- `PickGod(god_id)`
- `PlaceHero(hero_index, position)`
- `Ready`

All actions validated server-side. Invalid actions rejected.
