# Networking — Vertical Slice Design (for sign-off)

> Status: **APPROVED — design locked, implementation pending.** This note defines the smallest
> end-to-end networked build that proves the architecture, plus what it deliberately omits.
> Key decisions are resolved in §8.

## 1. Goal

Stand up the thinnest possible **server-authoritative, dumb-client** multiplayer build: two clients
connect to one server process, play a full game (god pick → shop/draft/equip → combat → elimination)
entirely over a WebSocket, with **no game simulation on the client**. The point is to validate the
architecture and surface hidden assumptions early — not to ship production networking.

### What this proves
1. The existing `apply_player_action` (commands) + `CombatEvent` log (results) boundary serializes
   cleanly over the wire with no hidden in-process assumptions.
2. A "dumb client" can drive the full existing UI from received snapshots + event logs alone.
3. Combat-as-watched-replay works over the network (server runs the sim, streams the event log,
   client animates) and feels acceptable.
4. Where the authoritative clock and wire types should live.

## 2. Model: dumb-client state-sync (decided)

- The **server owns the single `aa2-game::GameState`** and is the only thing that runs `aa2-sim`.
- Clients send **intents** (actions); they never mutate authoritative state and never simulate.
- The server validates each action, applies it, and broadcasts the resulting **state** (prep phases)
  or **combat event log** (combat phase).
- The client renders received state/events through the **same UI it already has**.

Why this fits: an autobattler has no player input *during* combat, so there is nothing to lockstep or
predict. State-sync gives trivial reconnect, simple spectating, and server-authoritative anti-cheat by
default. It also makes the f32 cross-platform determinism question moot — clients never re-simulate, so
platform-divergent f32 results cannot cause desync (see architecture "Determinism").

## 3. Key leverage: the boundary already exists

The current in-process API is already a clean command/event seam, so aa2-game barely changes:

| Today (local, in-process)                           | Networked (this slice)                                    |
|-----------------------------------------------------|-----------------------------------------------------------|
| `GameManager.apply_player_action(pid, type, param)` | Client sends `Action{type,param}` → server applies it     |
| Client polls `get_gold/get_bench/...` every frame   | Client reads the **last received snapshot**               |
| `run_combat()` → `last_combat_results[].combat_log` | Server runs combat, sends each player their matchup's log |
| `combat_viewer_ui` animates the `CombatEvent` stream| Identical — fed from the network instead of locally       |

So the client's getter API stays; in networked mode its **data source** swaps from a local
`GameState` to the latest server snapshot. Combat rendering is unchanged.

## 4. Crate layout

```
crates/
├── aa2-net/      # NEW (small): serde wire types shared by server + client
│                 #   - ClientMsg / ServerMsg enums
│                 #   - StateSnapshot DTO (player-visible state)
│                 #   deps: aa2-sim (reuse CombatEvent) + aa2-data (shared enums) + serde
├── aa2-server/   # NEW: tokio + tungstenite WebSocket server; owns GameState; drives the
│                 #   clock; builds StateSnapshot from GameState (the projection lives HERE)
├── aa2-game/     # UNCHANGED — stays pure, no networking dependency
└── aa2-client/   # add a NetClient data source behind the existing getter API
```

Wire types live in their own tiny `aa2-net` crate so `aa2-data`/`aa2-game` stay free of transport
concerns. The `GameState -> StateSnapshot` **projection lives in `aa2-server`**, not `aa2-game`, so
`aa2-game` keeps zero knowledge of networking (preserving "crates independent / sim compiles to WASM").

## 5. Message protocol (minimal)

Transport: one WebSocket per client, JSON for the slice (swap to binary later if needed).

**Client → Server**
```
Join   { name }
Action { action_type: String, param: String }   // reuses the existing string action protocol:
                                                 // Buy, Sell, Equip, Unequip, SwapAbilities,
                                                 // SetPosition, PickGod, DraftHero, RerollHero,
                                                 // RerollShop, UpgradeShop, LockShop, Ready
```

**Server → Client**
```
Welcome      { your_player_id, player_count }
Snapshot     { StateSnapshot }                              // full player-visible state (no delta yet)
ActionResult { ok: bool, reason: String }                   // mirrors apply_player_action's return
CombatStart  { matchup_index, event_log: Vec<CombatEvent> } // server-recorded; client animates
PhaseChange  { phase, round, timer_secs }
GameOver     { placements }
```

`StateSnapshot` = exactly what the UI reads today: gold, shop offerings/level/locked, bench, per-hero
equipped abilities + positions, hp/alive for all players, phase, round, draft choices, and other
players' *public* info only (hp, hero count — not their ability details).

Snapshots are sent **whole** on any change for the slice. Delta compression is deferred.

## 6. Server game loop (authoritative)

1. Accept 2 client connections (slice scope). Optionally fill remaining seats with the existing AI.
2. Own one `GameState`. The **server drives the phase clock** (today the client's `tick()` does this).
3. Prep phases: on `Action`, validate + `apply_player_action`; reply `ActionResult`; broadcast new
   `Snapshot`(s) to affected clients.
4. Combat phase (two-window clock): server calls `run_combat()` (instant), computes the
   **combat window** = the longest matchup's animation duration (`max_event_tick / 30`, bounded by the
   sim's combat timeout), and sends each client a `CombatStart` with their matchup's `event_log` + the
   window length. Clients animate within that window; a client whose fight ends early shows the result
   and waits. After the window closes, the server opens the **shop window** — a FIXED timer, decoupled
   from fight length so shop pacing isn't hostage to combat duration.
5. On elimination/finish: broadcast `GameOver`.

## 7. Changes required (bounded)

- **aa2-sim (prereq):** add `Deserialize` to `CombatEvent` (+ `DamageType` and embedded types). It
  already derives `Serialize`; the client needs `Deserialize` to read the streamed log. This is the
  only change to existing sim/game code.
- **aa2-server:** owns the authoritative phase clock (calls the existing `RoundFlow`/`tick` logic
  instead of the client) and the RNG seed; triggers `run_combat()`; runs AI for bot seats; and builds
  the per-viewer `StateSnapshot` from `GameState`'s public fields.
- **aa2-client:** introduce a `NetClient` that (a) sends `Action`s over WS and (b) stores the latest
  `Snapshot` + incoming `CombatStart`. Make `GameManager`'s getters read from `NetClient` in networked
  mode. Local mode stays as-is for dev/tests.
- **aa2-net:** the message + snapshot types.
- **aa2-game:** unchanged — no networking dependency; the server reads its public fields.

## 8. Resolved decisions

1. **Serialization:** send `Vec<CombatEvent>` directly — it already derives `Serialize`. Only prereq:
   add `Deserialize` to `CombatEvent`/`DamageType` (client side). `GameState`/`PlayerState`/etc. are
   already serde-ready, but we send a viewer-filtered `StateSnapshot`, never raw `GameState` (avoids
   leaking opponents' private loadouts).
2. **Delta compression:** NOT used. Traffic is ~4–8 KB snapshots on discrete changes (<5 Hz) + one
   combat log per round → <50 Kbps peak, already below what real-time games target *after* delta.
   Delta solves a high-frequency problem we don't have. If bandwidth ever bites at 8-player mobile
   scale, the first lever is per-client visibility filtering (inherent in the snapshot), not delta.
3. **Combat pacing:** two-window clock per round — a variable **combat window** (= longest matchup's
   animation duration, bounded by the sim timeout) followed by a **fixed shop window** decoupled from
   fight length. The server holds the combat window open for animation, then opens shop. (See §6.)
4. **Wire-types home:** dedicated `aa2-net` crate; the `GameState -> StateSnapshot` projection lives in
   `aa2-server` so `aa2-game` stays networking-free.
5. **Client → server:** phase clock + round-state timing, combat triggering, draft-choice generation,
   the RNG seed, and AI decisions all become server-authoritative. Rendering/animation/input/UI stay
   client-side. The timer logic already exists in `aa2-game`; only the caller and seed-owner move.

## 9. Milestone & success criteria

**Milestone:** two human clients play a full game to elimination over WebSocket against one server,
with the server authoritative and neither client running `aa2-sim`.

- [ ] Two clients connect; each is assigned a player id.
- [ ] God pick / draft / shop / equip / positioning all work via `Action` messages; server validates.
- [ ] Both clients render correct state purely from `Snapshot`s.
- [ ] Combat runs server-side; each client animates its matchup from the streamed event log.
- [ ] Game runs to a winner; `GameOver` shows correct placements.
- [ ] No `aa2-sim` / `aa2-game` mutation on the client.

## 10. Explicitly deferred (NOT in this slice)

Matchmaking, MMR, lobbies beyond one game, reconnect robustness, accounts/auth, anti-cheat hardening,
delta compression, persistence/PostgreSQL, spectating other boards, 8-player scale (slice = 2 humans,
AI fills the rest), TLS/`wss://`, and **client-side prediction (excluded by the dumb-client decision)**.

---

*Sign-off:* once approved, implementation starts with `aa2-net` types → a 2-player server happy path →
swapping the client to a networked data source. Composable ability effects are tracked separately but
are part of the same prototype milestone.
