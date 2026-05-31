# Post-Composable-Effects Milestone — Fresh Agent Handoff

## Starter Prompt

> Read these docs in order: `AGENTS.md`, `docs/design/architecture.md`,
> `docs/design/composable-effects.md`, `docs/design/ability-authoring.md`,
> `docs/handoffs/post-composable-effects-handoff.md`, `docs/specs/mechanics-reference.md`.
> Confirm a green baseline by running the gate steps **separately** (`cargo clippy --all-targets
> -- -D warnings`; `cargo test`; `./dev test`). Everything is on `origin/main`. Your recommended
> next task: implement RON hot-reload + an ability load/validation harness (Tier 1) to unblock
> fast content iteration. Get sign-off on approach if it changes the dev loop.

---

## TL;DR

Composable ability effects are **done**. All 11 abilities are data-driven `EffectSpec` compositions
in RON — the bespoke `Effect` enum is deleted. A new ability requires only a RON file, no Rust.
Everything is merged to `main`. Next priority: hot-reload + validation harness for content velocity.

---

## Current State (shipped this session)

### What shipped

- **Composable effect system:** `EffectSpec` (Trigger × TargetingSpec × Delivery × Payload[])
  resolves all ability behavior generically.
- **All 11 abilities migrated:** Rage, Ravage, Heavenly Grace, Dark Pact, Burrowstrike, Spear of
  Mars, Fury Swipes, Chaos Strike, Essence Shift, Glaives of Wisdom, Spirit Lance.
- **Bespoke `Effect` enum deleted** — the old per-ability match arms are gone.
- **Buff data schema** (`BuffDef`, `StatModifierSpec`, `StatusFlags`, `StackBehavior`, `DispelType`,
  `TickEffectDef`) lives in aa2-data; runtime `Buff` constructed via `Buff::from_def`.
- **Behavior byte-identical** — fixed-seed tests pin it (271 Rust + 47 GDScript, all green).
- **wasm32 compiles** (`cargo build -p aa2-sim --target wasm32-unknown-unknown`).
- Merged to `origin/main` (commit `bbd3c75`).

### Content

21 heroes, 11 abilities, 2 gods. No items or art (placeholder shapes).

---

## How to Build / Run / Test

### Build & run (local mode)

```bash
./dev              # Build + launch Godot client (local mode)
./dev editor       # Build + open Godot editor
./dev check        # cargo check + clippy + test
./dev test         # Build + run GDScript integration tests (requires display)
```

### Build & run (networked mode)

```bash
cargo run -p aa2-server                # Terminal 1: server on 127.0.0.1:9001
AA2_SERVER=ws://127.0.0.1:9001 ./dev   # Terminal 2+: each client claims a seat
```

### Testing (gate — run steps SEPARATELY)

```bash
cargo clippy --all-targets -- -D warnings   # Lint gate
cargo test                                   # 271 Rust tests
./dev test                                   # 47 GDScript integration tests
./dev net-smoke                              # Networked smoke test (manual trigger)
```

### WASM build (sim portability check)

```bash
rustup target add wasm32-unknown-unknown
cargo build -p aa2-sim --target wasm32-unknown-unknown
```

**⚠️ Run gate steps separately.** Chaining thrashes the incremental cache and produces transient
failures.

---

## Gotchas

1. **Gate steps must run separately** — there is no build race; `./dev` is sequential, but
   interleaving cargo commands thrashes the cache.
2. **No hot-reload yet** — after changing RON data or Rust code, restart is required. This is the
   #1 content-velocity blocker (Tier 1 below).
3. **gdext dylib needs full Godot restart** — close Godot fully and re-run `./dev` after Rust code
   changes. First-ever run needs `./dev editor` once to create `.godot/`.
4. **Determinism:** never iterate a `HashMap` in seeded/gameplay-affecting code without sorting
   first. Fixed-seed tests will catch violations.
5. **Dev bins:** `aa2-sim`'s combat CLI is `aa2-sim-dev` (`cargo run -p aa2-sim --bin aa2-sim-dev`);
   `aa2-game`'s interactive CLI is `aa2-dev` (`cargo run -p aa2-game --bin aa2-dev`).

---

## Reference Docs

| Doc | What it covers |
|-----|----------------|
| `AGENTS.md` | Dev process, commit convention, Definition of Done, Test Failure Protocol |
| `docs/design/architecture.md` | System architecture (updated for composable model) |
| `docs/design/composable-effects.md` | Composable effects design + final-state §11 |
| `docs/design/ability-authoring.md` | **Practical ability authoring reference (RON-only)** |
| `docs/design/networking.md` | Networking design; §10 lists deferred items |
| `docs/runbooks/networked-smoke.md` | Smoke test + manual playtest runbook |
| `docs/specs/mechanics-reference.md` | Engine formulas & combat mechanics |
| `docs/project-plan.md` | Phased development plan |

---

## Remaining Tracks (prioritized)

### Tier 1 — Do first (content velocity)

1. **Hot-reload of RON data** — `notify`-crate file watcher that reloads `data/` on change without
   restarting the client. Architecture is designed for this (see `docs/design/architecture.md`).
2. **Ability load/validation harness** — a test or binary that loads every RON ability, validates
   the schema, and smoke-resolves each `EffectSpec` (e.g. fires `run_cast_effect_specs` with a
   dummy unit pair). Fails on malformed specs. Catches authoring errors before runtime.

### Tier 2 — Tracked composable follow-ups

- Split `Delivery::Projectile` into `Linear` / `Homing` variants (unused fields are confusing).
- Consider a `DamageFormula` enum if payload variants exceed ~18 (currently 15).
- Move `CasterTravel`'s hardcoded `"burrowstrike_invuln"` buff to a `travel_buff` field on the
  delivery.
- Add clarifying comments re: `Chain` no-op in on-death `Aoe` and `mana_cost` duality (spec-level
  vs `AbilityDef`-level).

### Tier 3 — Reactive (build when an ability/feature needs it)

- Pathfinding (A*) — currently units use simple collision avoidance.
- Channeling — specified in mechanics-reference but not yet implemented.
- Replay play/pause/seek/speed/board-switching.
- `CombatStart` winner/survivors wire gap (currently unused dead code).

### Deferred — Production track (not content-blocking)

Networking hardening per `docs/design/networking.md` §10:
- Reconnect robustness
- Lobby UI screen
- Matchmaking / accounts / auth
- `wss://` + TLS
- Delta compression
- Persistence / PostgreSQL
- Spectating other boards
- 8-human scale (current slice = humans + AI fill)

### Phase 5 — Content + Art

Expand god roster, ability pool, hero bodies. Balance tuning. Art assets. Launch cadence.

---

## First Steps for You

1. Read the docs listed in the Starter Prompt section above.
2. Confirm a green baseline: run the gate steps separately.
3. Implement Tier 1: RON hot-reload + ability load/validation harness. Get sign-off on approach if
   it changes the dev loop.
