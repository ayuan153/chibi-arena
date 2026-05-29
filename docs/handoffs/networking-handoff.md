# Networking Milestone — Fresh Agent Handoff

## TL;DR

The local game is complete and all tests are green. Your job: implement multiplayer networking.
The design is **signed off** in `docs/design/networking.md` — read it, then start with the `aa2-net`
crate. Everything below is committed on `main`.

## Current State (all committed)

The game is **fully playable in 2-player local dev mode**, end-to-end:
god pick → shop → draft → equip → combat (animated) → endgame, with drag-and-drop equip/sell/
reposition, a sell bin, a damage meter, tooltips, and a summary overlay.

- 236 Rust tests + 47 GDScript integration tests — **all green and deterministic** (fixed seed 42).
- Content: 21 heroes, 11 abilities, 2 gods, 0 items, no art (placeholder colored shapes).
- Crates: `aa2-sim` (combat), `aa2-data` (types/loaders), `aa2-game` (state machine/economy/draft),
  `aa2-client` (gdext/Godot). `aa2-server` does **not exist yet** — you create it.

### Recent commits (most recent last)
```
63d9d53 feat(client): wire sell bin to sell selected ability
2ef939e feat(client): add damage meter sidebar grouped by team
1708e09 feat(client): add drag-and-drop for equip, sell, unequip, and reposition
e4ef473 fix(game): make hero draft/reroll/shop offerings deterministic under fixed seed
800e59d docs: fix doc/reality drift and add approved networking design
```

## What's Next — implement networking

Follow the build order in `docs/design/networking.md` (dumb-client state-sync, server-authoritative):

1. **`aa2-net` crate** — `ClientMsg` / `ServerMsg` / `StateSnapshot` serde types. **Prereq:** add
   `Deserialize` to `CombatEvent` (+ `DamageType` and embedded types) in `aa2-sim` — it already
   derives `Serialize`; the client needs `Deserialize` to read the streamed combat log.
2. **`aa2-server`** — tokio + tungstenite WebSocket server, 2-player happy path: owns one
   `GameState`, drives the **two-window clock** (variable combat window + fixed shop window — see
   networking.md §6), validates actions, builds per-viewer `StateSnapshot`s, streams combat logs.
3. **`aa2-client`** — a `NetClient` data source behind the existing `GameManager` getter API
   (local mode stays untouched for dev/tests).

A separate follow-up track (not designed yet — needs its own design note first): **composable
ability effects** (today each ability is a bespoke `Effect` enum variant, which won't scale and
means abilities can't be added by data alone). Don't start it without a design note.

## Likely early refactor (flag, decide as you build §1/§2)

The string-action dispatch (`apply_player_action(player, "Buy"/"Sell"/"Equip"/…, param)` → `Action`
enum → game mutation) currently lives in **`aa2-client/src/game_manager.rs`**, not in `aa2-game`.
The server needs the *same* dispatch. Consider extracting the action-string → `Action` → apply logic
into `aa2-game` so client and server share one implementation rather than duplicating it. The `Action`
enum already lives in `aa2-game` (`scenario.rs`).

## Key APIs / facts for networking

- **Action enum:** `aa2-game/src/scenario.rs` — `Buy(usize)`, `Sell(String)`, `Equip(a,h)`,
  `Unequip(a,h)`, `SwapAbilities`, `SetPosition(h,x,y)`, `RerollShop`, `UpgradeShop`, `LockShop`,
  `PickGod`, `DraftHero(usize)`, `RerollHero(String)`, `Ready`.
- **Combat:** `GameState::run_combat_round(hero_defs, ability_defs, seed, rng) -> Vec<CombatResult>`;
  `CombatResult { combat_log: Vec<CombatEvent>, winner, survivors_a, survivors_b }`.
- **CombatEvent:** `aa2-sim/src/lib.rs` — already `derive(Serialize)`, needs `Deserialize`.
- **Snapshot:** `GameState`/`PlayerState`/`Shop`/`Pool`/`Draft`/`Matchup` already derive
  `Serialize + Deserialize`. **Do NOT send raw `GameState`** — it holds every player's private
  loadout. Build a **viewer-filtered `StateSnapshot`** (own state full, opponents' public-only).
  Put the projection in `aa2-server`, keep `aa2-game` networking-free (sim must compile to WASM).
- **Clock:** the phase timer/`RoundFlow` already exists in `aa2-game`; the server becomes the caller
  of `tick()` and the **owner of the RNG seed** (so clients can't predict/manipulate rolls).

## Constraints (read AGENTS.md fully)

- **Definition of Done gate (must pass before any commit):**
  `cargo clippy -- -D warnings` && `cargo test` && `./dev test` all green.
- Every bug fix and new behavior gets a test. **Fix the code, not the test** (Test Failure Protocol).
- Conventional Commits + a `Prompt:` trailer on every commit.
- Client: all layout in `.tscn`, logic in Rust `#[func]`. Keep `aa2-sim`/`aa2-game` pure.

## Gotchas (learned the hard way — save yourself the time)

1. **Run the gate steps SEPARATELY**, not chained in one shell line. `cargo clippy`, `cargo test`,
   and `./dev test` (which runs `cargo build`) thrash each other's incremental cache when chained,
   producing transient false failures. **There is no build race** — `./dev` is fully sequential.
2. **Clippy gate is `cargo clippy -- -D warnings` (NO `--all-targets`).** `--all-targets` surfaces a
   pre-existing lint in `aa2-game/src/shop.rs:303` (test code) — out of scope; don't fix it here.
3. **Determinism:** never iterate a `HashMap` (`hero_defs`/`ability_defs`/`pool.counts`) in
   gameplay-affecting code without sorting first — that was the root of the flaky tests (fixed in
   `draft.rs`/`pool.rs`). Keep all seeded selection input-order-independent.
4. **No hot-reload** for the gdext dylib — you must close + reopen Godot to pick up a rebuild.
   `./dev` / `./dev test` rebuild for you. First-ever run needs `./dev editor` once to create `.godot/`.
5. If `./dev test` ever shows a one-off failure, **re-run it cleanly** — transient failures came from
   running during a build; the authoritative result is the clean run.

## Dev workflow

```bash
./dev          # build + launch Godot client
./dev test     # build + run 47 GDScript integration tests (requires display; macOS native OK)
./dev check    # cargo check + clippy + test
cargo test     # 236 Rust tests
```

## Reference docs (in priority order)

1. `docs/design/networking.md` — **the signed-off design** (decisions in §8).
2. `AGENTS.md` — dev process, commit convention, Definition of Done, Test Failure Protocol.
3. `docs/design/godot-dev-workflow.md` — build/run/debug the client.
4. `docs/design/architecture.md`, `docs/project-plan.md` — system + plan context.

## First steps for you

1. Read the docs above.
2. Confirm a green baseline: `cargo clippy -- -D warnings`; `cargo test`; `./dev test` (separately).
3. Start §1: `aa2-net` crate + the `CombatEvent` `Deserialize` prereq, with tests. Verify the full
   gate, commit, and check in before starting the server (§2).
