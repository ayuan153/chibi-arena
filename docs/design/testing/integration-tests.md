# Integration Tests — Headless Godot

## Overview

Automated integration tests that run the full Godot + GDExtension stack headlessly.
Tests exercise the `GameManager` Rust API through GDScript, verifying the complete
path: GDScript → gdext FFI → aa2-game logic → state queries.

## Running

```bash
godot --headless --path client/ --script tests/test_runner.gd
```

Exit code 0 = all pass, 1 = failures. Added to `./dev check`.

## Design Principles

- **Fixed seed (42)** — deterministic results, same shop/draft/combat every run
- **API-level assertions** — test game state, not pixel positions
- **Node path assertions** — verify named nodes exist at stable paths (the "div tagging" equivalent)
- **No visual assertions** — immune to layout/anchor/size changes
- **Fast** — <2s total (headless startup + ~20 API calls)
- **High precision, low recall** — only assert things that MUST be true; no fragile checks

## Trigger Policy

Run on any change to:
- `crates/aa2-client/`
- `crates/aa2-game/`
- `client/`

## File Structure

```
client/tests/
├── test_runner.gd              # Entry point: discovers test_*.gd, runs test_*() methods
├── test_game_flow.gd           # Full game loop transitions
├── test_shop_mechanics.gd      # Economy, buy, reroll, lock, upgrade
├── test_draft.gd               # Draft rounds, hero reroll
├── test_equip.gd               # Equip, unequip, swap, level up
└── test_combat.gd              # Combat resolution
```

## Test Runner (`test_runner.gd`)

- Loads `main.tscn` to get the full scene tree (GameManager, all UI nodes)
- Discovers all `res://tests/test_*.gd` scripts
- For each: instantiates, finds all methods starting with `test_`, calls them
- Each test gets a fresh `GameManager.init_game(42, 2, "../data")`
- Reports: test name, pass/fail, assertion message
- Exits with code 0 (all pass) or 1 (any failure)

## Test Cases

### `test_game_flow.gd` — Phase Transitions

| Method | Setup | Assert |
|--------|-------|--------|
| `test_god_pick_advances_to_shop` | Both players PickGod + Ready | `get_phase() == "Shop"` |
| `test_round_cycle` | Complete shop→combat→grace→shop | `get_round() == 2`, phase == "Shop" |
| `test_game_ends_on_elimination` | Set HP to 1, lose combat | `get_phase() == "Finished"` |

### `test_shop_mechanics.gd` — Economy

| Method | Setup | Assert |
|--------|-------|--------|
| `test_buy_deducts_gold` | Buy slot 0 | Gold decreased by ability cost |
| `test_buy_adds_to_bench` | Buy slot 0 | `get_bench(0).size() == 1` |
| `test_buy_fails_when_broke` | Set gold to 0, buy | Returns error, bench unchanged |
| `test_buy_fails_bench_full` | Fill bench to 5, buy new ability | Returns "Bench is full!" |
| `test_buy_levels_up_bypasses_bench_cap` | Fill bench with 4 + have duplicate in shop, buy | Levels up, no error |
| `test_reroll_changes_offerings` | Record offerings, reroll | Offerings differ, gold -1 |
| `test_lock_preserves_offerings` | Lock, advance round | Same offerings after round |
| `test_upgrade_cost_round1` | Check at round 1 | Cost == 10 |
| `test_upgrade_cost_round3` | Advance to round 3 | Cost == 8 (10 - 2 decay) |
| `test_upgrade_increases_size` | Upgrade from Lv1 | Offerings count == 6 |
| `test_upgrade_rerolls_shop` | Record offerings, upgrade | Offerings changed |

### `test_draft.gd` — Hero Drafting

| Method | Setup | Assert |
|--------|-------|--------|
| `test_draft_available_round1` | Enter shop round 1 | `get_draft_choices(0).size() == 3` |
| `test_draft_available_round3` | Advance to round 3 | `get_draft_choices(0).size() == 3` |
| `test_draft_hero_adds_to_roster` | DraftHero(0) | `get_heroes(0).size()` increased |
| `test_hero_reroll_costs_2g` | RerollHero | Gold decreased by 2 |
| `test_hero_reroll_generates_choices` | RerollHero | `get_draft_choices(0).size() == 3` |
| `test_hero_reroll_keeps_abilities` | Equip ability, reroll, draft | Equipped abilities preserved |

### `test_equip.gd` — Loadout Management

| Method | Setup | Assert |
|--------|-------|--------|
| `test_equip_from_bench` | Buy + equip | Bench shrinks, equipped grows |
| `test_unequip_to_bench` | Equip then unequip | Bench grows, equipped shrinks |
| `test_swap_abilities` | Equip 2, swap(0,1) | Order reversed |
| `test_level_up_on_duplicate` | Buy same ability twice | Level == 2, bench size == 1 (not 2) |

### `test_combat.gd` — Combat Resolution

| Method | Setup | Assert |
|--------|-------|--------|
| `test_combat_produces_events` | Position hero, run_combat | `get_combat_event_count(0) > 0` |
| `test_loser_takes_damage` | Run combat, end_combat | One player's HP < 200 |
| `test_combat_no_crash_without_heroes` | No heroes, run_combat | Doesn't crash, returns result |

### Node Path Tests (wiring stability)

| Method | Assert |
|--------|--------|
| `test_game_manager_exists` | Node at `/root/MainScene/GameManager` |
| `test_shop_row_exists` | Node at `/root/MainScene/BottomPanel/ShopRow` |
| `test_ready_button_exists` | Node at `/root/MainScene/ReadyButton` |

## Bench Cap Implementation

Add to `GameConfig`:
```rust
pub bench_capacity: u32,  // default: 5
```

In `Buy` action handler: if ability is NOT a duplicate (won't level up) and bench is full, reject with "Bench is full!".
