# AA2 Game Systems Reference

> Complete specification of game rules, economy, and flow for the AA2 autobattler.
> All numbers are base values — gods can modify any of these.

---

## Round Flow

Each game consists of ~30 rounds. 8 players compete in FFA elimination.

### Round Structure (80 seconds total)

```
┌─────────────────────────────────────────────────────────────┐
│ COMBAT PHASE (variable duration, up to ~50s)                 │
│ - Auto-combat between paired opponents                       │
│ - Ends when one side is eliminated or 30s remain (deadlock)  │
│ - Losing player takes damage                                 │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│ SHOP PHASE (remaining time after combat)                     │
│ - Buy/sell abilities, reroll shop, upgrade shop              │
│ - Equip abilities onto heroes, rearrange board               │
│ - Ends when timer expires or all players "ready up"          │
└─────────────────────────────────────────────────────────────┘
```

**Round 1 exception:** No combat. Full 80 seconds for initial shopping + god pick.

### Round Sequence

1. **Pre-round:** Hero level up (hero_level = 1 + round_number). Hero body draft on rounds 1/3/6/9/12.
2. **Combat:** Paired opponents fight. See "Combat Matchups" below.
3. **Damage:** Losing player takes damage based on round + surviving enemy heroes.
4. **Elimination:** Players at 0 HP are eliminated.
5. **Shop:** Remaining time for drafting abilities.

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
| Sell ability | 2 gold (refund) |
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

### Deadlock

- If combat hasn't resolved with 30 seconds remaining in the round: draw
- Both players take reduced damage (or no damage — TBD)

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

Gods are selected at game start (round 1). All gods available to all players. Duplicates allowed.

### God Ability Patterns

Gods modify game rules. Common patterns:

| Pattern | Example |
|---------|---------|
| Global combat passive (scales with shop level) | "All heroes gain +2 armor per shop level" |
| Single-unit buff (scales with abilities purchased) | "Your strongest hero gains +5 damage per ability owned" |
| Economy modifier | "Rerolls cost 0", "Start with +2 gold per round" |
| Slot modifier | "+2 ability slots on all heroes" |
| Draft modifier | "See 2 extra choices per roll", "Ultimates available at shop level 2" |

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
