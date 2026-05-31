# AA2 — Phased Development Plan

Solo-dev project (with AI agent assistance). Cross-platform autobattler with Dota2-fidelity combat simulation.

**Timeline:** ~36 weeks to platform release, ongoing content thereafter.

> Note: the week numbers below are **nominal/relative effort estimates, not calendar weeks**.
> Actual elapsed development has run well ahead of this schedule.

---

## Phase 0: Foundation + Dev Mode (Weeks 1–4) ✓ COMPLETE

| Week | Focus |
|------|-------|
| 1 | Monorepo setup (Rust workspace), sim crate skeleton |
| 2 | aa2-sim crate: ECS skeleton, attribute system |
| 3 | Basic attack loop (BAT, attack speed, armor reduction) |
| 4 | Dev CLI binary with 1v1 combat viewer |

**Deliverables:**
- Rust workspace with `aa2-sim` crate
- LOCAL DEV MODE: sim runs in-process, 1v1 combat viewer
- Placeholder art (colored polygons with labels)

**Milestone:** Two units fighting with correct Dota2 attack timing.

**Success Criteria:**
- Attack interval matches `BAT / (AS / 100)` formula
- Damage reduced by armor formula: `multiplier = 1 - (0.06 * armor) / (1 + 0.06 * |armor|)`

**Completed:** All success criteria met. Combat simulation operational with:
- Attribute system (STR/AGI/INT → HP, mana, armor, AS, damage)
- Attack loop with BAT formula, frontswing timing, damage variance (min/max roll)
- Armor reduction, innate melee damage block (50% × 16)
- Projectile system (homing, speed-based travel)
- Turn rate, targeting AI, movement
- Base magic resistance (25%)
- Seeded RNG (xoshiro128++) for deterministic replays
- Dev CLI binary with tick-by-tick combat log
- 7 heroes with real Dota2 stats (Sven, Drow, CK, Jugg, CM + 2 generic)

---

## Phase 1: Combat Fidelity (Weeks 5–12) ✓ COMPLETE

*Already completed from Phase 0 overflow: attribute system, projectile system, turn rate, targeting AI.*

| Week | Focus |
|------|-------|
| 5–6 | Buff/debuff framework (stacking, duration, tick effects, dispel) |
| 7 | Cast system (cast point, mana cost, cooldown, channeling) |
| 8 | Ability execution engine (read AbilityDef, execute effects) |
| 9 | AoE system (circle, cone, line), damage types (magical/pure now functional) |
| 10 | Advanced targeting (unit-targeted vs ground-targeted abilities) |
| 11 | Multi-unit combat (5v5), pathfinding with collision avoidance |
| 12 | Replay system, hot-reload, dev mode with bot draft |

**Deliverables:**
- Complete combat simulation matching Dota2 mechanics
- Replay recording + deterministic playback
- Dev mode with 5v5 bot battles and data hot-reload

**Milestone:** 5v5 combat that feels like Dota2.

**Success Criteria:**
- Side-by-side comparison with Dota2 mod shows matching timing/behavior
- Projectile travel time, turn rates, and cast points within 1 tick of Dota2 values
- Replays are deterministic (same seed → identical outcome)

**Completed:** All success criteria met. Full combat fidelity achieved with:
- Buff/debuff system (stacking rules, duration, tick damage, dispel types)
- Cast system (cast point, backswing, mana cost, cooldown). Channeling is specified in mechanics-reference but **not yet implemented**.
- Data-driven ability engine (AbilityDef → effect execution pipeline)
- AoE shapes (circle, cone, line), all damage types functional (physical/magical/pure)
- Unit-targeted and ground-targeted ability support
- Multi-unit combat (5v5) with spatial partitioning and collision avoidance
- Replay playback (same-platform deterministic; see architecture "Determinism" note)
- Data files loaded at startup (live hot-reload: **planned, not yet implemented**)
- Dev CLI with bot draft and 5v5 battles

---

## Phase 2: Game Systems (Weeks 13-20) ✓ COMPLETE

> **Summary:** All game systems implemented. Core game state, economy, shop, pool, draft, hero bodies, matchups, combat integration, god system, CLI dev mode, and AI opponents all complete. Full game playable from god pick to final placement.

### Week 13-14: Core Game State (aa2-game crate) ✓ COMPLETE
- Create aa2-game crate with PlayerState, GameState
- Economy system: gold per round (6/8/10...20), costs (buy 3, sell 2g × level, reroll 1, unequip 1)
- Shop system: shop levels 1-5, size 4/6/6/8/10, upgrade cost with decay
- Ability pool: 100 abilities × 20 copies, shared without replacement
- Round state machine: GodPick (pre-game) → Combat → GracePeriod → Shop cycle (GamePhase enum: GodPick, Combat, GracePeriod, Shop, Finished)
- Milestone: can advance through rounds programmatically

### Week 15-16: Draft & Hero Bodies ✓ COMPLETE
- Hero body draft: rounds 1/3/6/9/12, tiers D/C/B/A/S, 3 choices (STR/AGI/INT)
- Draft is concurrent with shop (overlay, not blocking phase)
- Hero body reroll (2 gold)
- Ability equip system: 4 slots per hero, 1 ultimate max, 5-slot bench
- Buy/sell/equip/unequip with gold costs
- Hero leveling: level = 1 + round, stats scale with gain
- Shop level 3 unlocks ultimates
- Milestone: can draft a full team and equip abilities

### Week 17-18: Combat Integration & Matchups ✓ COMPLETE
- Round-robin matchup pairing (randomized order, resilient to eliminations)
- Ghost opponent for odd player counts (clone loadout, deals damage, can't take)
- Build UnitConfigs from PlayerState at combat start
- Run aa2-sim combat, determine winner
- Player damage formula: base_damage(round) + per_hero * surviving_enemies
- Player elimination at 0 HP
- Milestone: full game loop runs to completion (1 winner)

**Completed:** Round-robin matchups with ghost seat, combat integration via aa2-sim, full game loop runs to 1 winner.

### Week 19-20: God System & Dev Mode ✓ COMPLETE
- God selection (all available, duplicates allowed)
- God passive system: modifiers to economy, slots, combat buffs
- Implement 3-5 starter gods with different playstyles
- CLI dev mode: playable single-player game against AI opponents
- AI opponents: random draft decisions (buy random, equip random)
- Timer system (80s rounds, combat-first then shop)
- Milestone: playable full game in terminal

**Completed:** God system (Archmage + Paladin), CLI dev mode (aa2-dev), AI opponents, damage reflection buff.

### Phase 2 Success Criteria: ✓ ALL MET
- ✓ Can play a full game from god pick to final placement
- ✓ Economy math works (gold, interest, shop upgrade decay)
- ✓ 8-player round-robin with ghost opponents
- ✓ AI opponents make valid (if random) decisions
- ✓ All game rules enforced (slot limits, ultimate limits, pool depletion)

---

## Phase 3: Client (Weeks 21–28) ✓ Local client complete (polish ongoing)

| Week | Focus |
|------|-------|
| 21 | aa2-client crate (gdext), Godot project setup, extension loading |
| 22 | Shop screen (buy/sell/reroll/equip via UI) |
| 23 | Board positioning (drag & drop heroes), bench UI |
| 24 | Combat replay viewer (animate CombatEvent stream with tweens; add MoveTo/StartMoving events to sim) |
| 25 | Draft screen, god pick, scoreboard |
| 26 | Full playable game in Godot (local mode, placeholder art) |
| 27 | Dev console (always-visible panel, cheat commands, state inspection) |
| 28 | Structural polish: HP bars, damage numbers, death fade, cast indicators |

**Deliverables:**
- Godot 4.6 project with GDExtension (gdext 0.5)
- `aa2-client` crate (cdylib) loaded by Godot via .gdextension
- All game screens: shop, draft, combat viewer, scoreboard, god pick
- Combat replay system (event-based — client animates CombatEvent stream using tweens)
- LOCAL MODE: aa2-client calls aa2-game directly (same process, no serialization)
- Placeholder art (colored shapes with labels)
- Code-first approach: hand-written project.godot, no editor dependency

**Architecture:**
```
Godot (GDScript scenes) → aa2-client (gdext cdylib) → aa2-game → aa2-sim → aa2-data
```

The aa2-client crate calls aa2-game directly in the same process. No FFI boundary, no JSON serialization — just Rust function calls.

**Milestone:** Playable full game in Godot with placeholder art.

**Success Criteria:**
- All game actions work via UI (no CLI needed)
- Combat viewer shows fights with smooth unit movement
- Runs at 60fps on macOS
- Dev console provides full observability

---

## Phase 4: Networking (Weeks 29–36) ✓ Vertical slice complete

> **Decision:** dumb-client state-sync (server-authoritative; clients render received state/events
> and do **not** run the sim or predict). See `docs/design/networking.md` for the design.

### Delivered (vertical slice)

- `aa2-net` crate: serde wire types (`ClientMsg`/`ServerMsg`/`StateSnapshot` DTOs)
- `aa2-server`: tokio + tokio-tungstenite actor-model WebSocket server; single central task owns
  lobby + `GameState` + RNG seed; per-connection reader/writer tasks over channels (no locks);
  two-window clock (variable combat window + fixed prep window); AI bot fill; `GameOver` placements
- Shared action dispatch: `aa2-game` owns `parse_action` + `apply_action` used by both client and
  server; `aa2-game` stays networking-free (sim still compiles to WASM)
- `aa2-client`: `NetClient` (background tokio thread + channels) + `NetState` behind existing
  `GameManager` getters; enter networked mode via `AA2_SERVER` env var or dev-console `connect`
- Lobby model: each WebSocket connection = one seat; Start fills remaining seats with AI bots
- `./dev net-smoke`: automated networked smoke test (handshake → GodPick → pick god → Shop)
- 271 Rust tests + 47 GDScript integration tests, all green

### Deferred (see `docs/design/networking.md` §10)

Reconnect robustness, lobby UI screen, matchmaking/accounts/auth, `wss://` + TLS, delta compression,
persistence/PostgreSQL, spectating other boards, 8-human scale, client-side prediction.

**Milestone:** Two+ human clients play a full game to elimination over WebSocket with server
authoritative and AI filling remaining seats. ✓ MET

**Next track:** Composable ability effects — ✅ **COMPLETE** (2026-05-30, merged to main). All 11
abilities are now data-driven `EffectSpec` compositions in RON; the bespoke `Effect` enum is
deleted. New abilities require only a RON file. Phase 5 (Content + Launch) is now unblocked —
recommended first content-velocity step: hot-reload of RON data + an ability load/validation
harness.

---

## Phase 5: Content + Launch (Weeks 37+)

- Expand to full god roster, ability pool, hero bodies
- Balance tuning via automated simulation + manual adjustment
- Launch cadence: closed beta → open beta → soft launch → full launch
- Seasonal content: new gods, abilities, battle pass each season
- Ongoing: community feedback, balance patches, live ops
- Platform targets: macOS, iOS, Android, Windows, Linux

**Milestone:** Sustainable live game with active player base.

**Success Criteria:**
- Day-7 retention > 20%
- Stable matchmaking queue times < 60s at launch

---

## Risk Register

| Risk | Impact | Mitigation |
|------|--------|------------|
| iOS App Store rejection | Blocks mobile launch | Follow guidelines strictly, TestFlight early |
| Combat feel doesn't match Dota2 | Core value prop fails | Phase 1 dedicated entirely to this, replay comparison tooling |
| Solo dev burnout | Project stalls | Realistic timeline, MVP subset, heavy agent assistance |
| gdext breaking changes | Blocks client progress | Pin gdext version, test upgrades in branch |
| Networking complexity | Delays multiplayer | State-sync (simpler than lockstep), defer entirely to Phase 4 |

---

## Dependencies

| Dependency | When Needed | Notes |
|------------|-------------|-------|
| Rust stable + cross-compilation | Phase 0 | aarch64-apple-ios, aarch64-linux-android targets |
| Godot 4.6+ | Phase 3 | GDExtension support, cross-platform builds |
| gdext 0.5 | Phase 3 | Rust GDExtension bindings |
| PostgreSQL | Phase 4+ | Player accounts, matchmaking, leaderboards |
| Cloud hosting | Phase 4+ | Game servers, matchmaking service |
| Apple Developer account | Phase 5 | $99/year, needed for TestFlight and App Store |
| Art assets (AI-generated) | Phase 5 | Characters, VFX, UI elements |
