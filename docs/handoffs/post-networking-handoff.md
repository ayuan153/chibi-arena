# Post-Networking Milestone — Fresh Agent Handoff

## Starter Prompt

> Read these docs in order: `AGENTS.md`, `docs/design/architecture.md`,
> `docs/design/composable-effects.md` (the design for your task), `docs/design/networking.md`,
> `docs/handoffs/post-networking-handoff.md`, `docs/specs/mechanics-reference.md`. Confirm a green
> baseline by running the gate steps **separately** (`cargo clippy --all-targets -- -D warnings`;
> `cargo test`; `./dev test`). Your task is **composable ability effects**: the design note is
> written — get my sign-off on it first, then implement per its **§8 migration** (start with the
> Rage + one-damage-ability proof, tests green at each step). Check in after the proof before
> porting the rest.

---

## TL;DR

Networking is **done**. Server-authoritative dumb-client state-sync over WebSocket is working
end-to-end: lobby, bots, full game loop to `GameOver`. Your job: get sign-off on the
**composable ability effects** design note (`docs/design/composable-effects.md`), then implement it.
Everything below is committed on `main`.

---

## Current State (all committed)

The game is playable both **locally** (unchanged dev mode) and **networked** (2+ humans + AI fill
to 8 players over WebSocket). Full flow: god pick → shop → draft → equip → combat (animated) →
endgame.

### Crates

| Crate | Role |
|-------|------|
| `aa2-sim` | Deterministic combat simulation (30Hz ECS) |
| `aa2-data` | Shared types, schemas, RON loaders |
| `aa2-game` | Game state machine, economy, draft, **shared action dispatch** |
| `aa2-net` | Serde wire types (`ClientMsg`/`ServerMsg`/`StateSnapshot` DTOs) |
| `aa2-client` | Godot GDExtension (gdext); `NetClient` for networked mode |
| `aa2-server` | Tokio + tokio-tungstenite actor-model WebSocket server |

### Key architecture facts

- **Shared dispatch:** `aa2-game` owns `scenario::parse_action` and `GameState::apply_action` — used
  by both client (local mode) and server. `aa2-game` has **no networking dependency** (sim still
  compiles to WASM).
- **Server:** single central task owns lobby + `GameState` + RNG seed. Per-connection reader/writer
  tasks over channels (no locks). Owns the two-window clock (variable combat window + fixed prep
  window). Binds `127.0.0.1:9001`.
- **Client:** `NetClient` (background tokio thread + channels) + `NetState` behind existing
  `GameManager` getters. In networked mode, getters read the latest server `Snapshot`. Enter
  networked mode via `AA2_SERVER` env var or dev-console `connect <url>` command.
- **Lobby:** each WebSocket connection = one seat. Start fills remaining seats with AI bots.

### Content

21 heroes, 11 abilities, 2 gods. No items or art (placeholder shapes).

### Tests

- 271 Rust tests + 47 GDScript integration tests — all green, deterministic (fixed seed).
- `./dev net-smoke` — automated networked smoke test (not in CI).

---

## How to Build / Run / Test

### Build & run (local mode)

```bash
./dev              # Build + launch Godot client (local mode)
./dev editor       # Build + open Godot editor
```

### Build & run (networked mode)

```bash
# Terminal 1: start server
cargo run -p aa2-server

# Terminal 2+: launch clients
AA2_SERVER=ws://127.0.0.1:9001 ./dev
```

See `docs/runbooks/networked-smoke.md` for the full manual playtest procedure.

### Testing (gate — run steps SEPARATELY)

```bash
cargo clippy --all-targets -- -D warnings   # Lint gate
cargo test                                   # 271 Rust tests
./dev test                                   # 47 GDScript integration tests
./dev net-smoke                              # Networked smoke test (manual trigger)
```

**⚠️ Run these separately.** Chaining thrashes the incremental cache and produces transient failures.

---

## Gotchas

1. **Gate steps must run separately** — see above. There is no build race; `./dev` is sequential.
2. **No hot-reload** for the gdext dylib — close Godot fully and re-run `./dev` after code changes.
   First-ever run needs `./dev editor` once to create `.godot/`.
3. **Determinism:** never iterate a `HashMap` in gameplay-affecting code without sorting first.
4. **Clippy gate is `--all-targets`** — the codebase is clean under it.
5. **Dev bins:** `aa2-sim`'s combat CLI is `aa2-sim-dev` (`cargo run -p aa2-sim --bin aa2-sim-dev`);
   `aa2-game`'s interactive CLI is `aa2-dev` (`cargo run -p aa2-game --bin aa2-dev`).

---

## Reference Docs

| Doc | What it covers |
|-----|----------------|
| `AGENTS.md` | Dev process, commit convention, Definition of Done, Test Failure Protocol |
| `docs/design/architecture.md` | System architecture (updated for networking) |
| `docs/design/networking.md` | Networking design; **§10 lists deferred items** |
| `docs/design/composable-effects.md` | **Composable ability effects design — your next task** |
| `docs/runbooks/networked-smoke.md` | Smoke test + manual playtest runbook |
| `docs/specs/mechanics-reference.md` | Engine formulas & combat mechanics |
| `docs/project-plan.md` | Phased development plan |

---

## Remaining Tracks

### 1. Composable Ability Effects (next — design note written, awaiting sign-off)

Today each ability is a bespoke `Effect` enum variant in `aa2-data` (15 variants; all 11 abilities
are bespoke, none data-only). This won't scale: every new ability needs a Rust change across 3–6
files. The goal is a composable, data-driven effect system (abilities authored in RON alone).

**The design note is written:** `docs/design/composable-effects.md` — four orthogonal axes
(Trigger × Targeting × Delivery × Payload[]), reusing the already-composable buff system as the
payload primitive, with an incremental migration that keeps the ~140 sim tests green.

**Next step:** get sign-off on the design note, then implement per its **§8 migration** — add the
composable types alongside the existing enum, port **Rage + one damage ability** as the proof, then
port the remaining abilities one at a time (deleting each bespoke variant as it is ported). Do NOT
big-bang it.

### 2. Networking Hardening (deferred — only if pushing toward production)

See `docs/design/networking.md` §10 for the full list:
- Reconnect robustness
- Lobby UI screen
- Matchmaking / accounts / auth
- `wss://` + TLS
- Delta compression
- Persistence / PostgreSQL
- Spectating other boards
- 8-human scale (current slice = humans + AI fill)
- Client-side prediction (excluded by the dumb-client decision)

### 3. Content + Playtesting

Expand god roster, ability pool, hero bodies. Balance tuning. Art assets.

---

## Known Slice Limitation

`ServerMsg::CombatStart` sends only the `event_log` (not winner/survivor counts). The client's
`get_combat_result` returns placeholders, but it is currently unused (dead code) so nothing breaks.
Add winner/survivors to the wire only if a future combat-summary overlay needs them.

---

## First Steps for You

1. Read the docs listed in the Starter Prompt section above.
2. Confirm a green baseline: run the gate steps separately.
3. Get sign-off on `docs/design/composable-effects.md`, then implement per its §8 — port
   Rage + one damage ability as the proof first, and check in before porting the rest.
