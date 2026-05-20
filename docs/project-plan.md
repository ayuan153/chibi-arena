# AA2 — Phased Development Plan

Solo-dev project (with AI agent assistance). Cross-platform autobattler with Dota2-fidelity combat simulation.

**Timeline:** ~36 weeks to platform release, ongoing content thereafter.

---

## Phase 0: Foundation + Dev Mode (Weeks 1–4) ✓ COMPLETE

| Week | Focus |
|------|-------|
| 1 | Monorepo setup (Rust workspace + Unity project), FFI bridge prototype |
| 2 | aa2-sim crate: ECS skeleton, attribute system |
| 3 | Basic attack loop (BAT, attack speed, armor reduction) |
| 4 | Unity combat viewer (1v1, placeholder art, dev mode) |

**Deliverables:**
- Rust workspace with `aa2-sim` crate
- Unity native plugin loading Rust dylib via C FFI
- LOCAL DEV MODE: sim runs in-process, 1v1 combat viewer
- Placeholder art (colored polygons with labels)

**Milestone:** Two units fighting with correct Dota2 attack timing.

**Success Criteria:**
- Attack interval matches `BAT / (AS / 100)` formula
- Damage reduced by armor formula: `multiplier = 1 - (0.06 * armor) / (1 + 0.06 * |armor|)`
- FFI bridge works on macOS and iOS simulator

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
- Cast system (cast point, backswing, mana cost, cooldown, channeling with interrupts)
- Data-driven ability engine (AbilityDef → effect execution pipeline)
- AoE shapes (circle, cone, line), all damage types functional (physical/magical/pure)
- Unit-targeted and ground-targeted ability support
- Multi-unit combat (5v5) with spatial partitioning and collision avoidance
- Replay system with deterministic playback (same seed → identical outcome)
- Hot-reload for ability/hero data files
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

## Phase 3: Multiplayer (Weeks 21–28)

| Week | Focus |
|------|-------|
| 21–22 | aa2-server binary (headless sim, WebSocket server) |
| 23–24 | State-sync protocol (10Hz snapshots, delta compression) |
| 25 | Matchmaking + lobby system (region + MMR filtering) |
| 26 | Reconnect support (full state snapshot on rejoin) |
| 27 | Spectating (subscribe to other player boards) |
| 28 | Anti-cheat (server-authoritative validation), load testing |

**Deliverables:**
- Dedicated server binary running headless simulation
- WebSocket-based state sync with delta compression
- Matchmaking, reconnect, and spectating

**Milestone:** 8 humans playing online.

**Success Criteria:**
- Stable 8-player game with <100ms perceived latency
- Reconnect restores full game state within 2 seconds
- Server validates all client actions (no trust-the-client)

---

## Phase 4: Polish + Platform (Weeks 29–36)

| Week | Focus |
|------|-------|
| 29–30 | Full UI/UX (draft screen, shop, combat viewer, scoreboard) |
| 31–32 | Art assets (AI-generated chibi characters, ability VFX, audio) |
| 33 | iOS build + TestFlight submission |
| 34 | Android build + Play Store |
| 35 | Steam integration (achievements, friends) |
| 36 | F2P monetization (battle pass, cosmetics shop, IAP) |

**Deliverables:**
- Production UI across all game screens
- Art and audio assets (AI-generated where possible)
- Builds for iOS, Android, and Steam

**Milestone:** App Store approved, playable on all platforms.

**Success Criteria:**
- Passes Apple review on first or second submission
- Runs at 60fps on iPhone 12+
- IAP and battle pass functional on all platforms

---

## Phase 5: Content + Launch (Weeks 37+)

- Expand to full god roster, ability pool, hero bodies
- Balance tuning via automated simulation + manual adjustment
- Launch cadence: closed beta → open beta → soft launch → full launch
- Seasonal content: new gods, abilities, battle pass each season
- Ongoing: community feedback, balance patches, live ops

**Milestone:** Sustainable live game with active player base.

**Success Criteria:**
- Day-7 retention > 20%
- Stable matchmaking queue times < 60s at launch

---

## Risk Register

| Risk | Impact | Mitigation |
|------|--------|------------|
| iOS App Store rejection | Blocks mobile launch | Follow guidelines strictly, TestFlight early in Phase 4 |
| Combat feel doesn't match Dota2 | Core value prop fails | Phase 1 dedicated entirely to this, replay comparison tooling |
| Solo dev burnout | Project stalls | Realistic timeline, MVP subset, heavy agent assistance |
| Unity–Rust FFI issues on iOS | Blocks mobile | Prototype FFI bridge in Phase 0 week 1, test on device early |
| Networking complexity | Delays multiplayer | State-sync (simpler than lockstep), defer entirely to Phase 3 |

---

## Dependencies

| Dependency | When Needed | Notes |
|------------|-------------|-------|
| Rust stable + cross-compilation | Phase 0 | aarch64-apple-ios, aarch64-linux-android targets |
| Unity 6 LTS (6000.0) | Phase 0 | Long-term support, mobile build support |
| PostgreSQL | Phase 3+ | Player accounts, matchmaking, leaderboards |
| Cloud hosting | Phase 3+ | Game servers, matchmaking service |
| Apple Developer account | Phase 4 | $99/year, needed for TestFlight and App Store |
| Art assets (AI-generated) | Phase 4 | Characters, VFX, UI elements |
