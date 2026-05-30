# Networked Smoke Test Runbook

How to verify the networking stack works — both the automated smoke test and a manual 2-client playtest.

---

## Automated Smoke Test (`./dev net-smoke`)

A headless integration test that starts a real `aa2-server`, connects a headless Godot client via
the `.dylib`, and drives through: WebSocket handshake → GodPick phase → pick a god + ready → Shop.

### Running

```bash
./dev net-smoke
```

This is **not** part of `./dev test` or CI — it's a manually-triggered check for the networking path.

### What it asserts

1. Server starts and binds `127.0.0.1:9001`.
2. Client connects, receives `Welcome` with a valid `player_id`.
3. Phase transitions to `GodPick`; client picks a god; server accepts.
4. Phase transitions to `Shop`; client sees shop offerings.

### Expected output

A successful run ends with:

```
SMOKE PASS
```

Any failure prints a descriptive error before exiting non-zero.

---

## Manual 2-Client Networked Playtest

A full end-to-end playtest with two human-controlled clients against AI bots.

### 1. Start the server

```bash
cargo run -p aa2-server
```

Listens on `127.0.0.1:9001` (unauthenticated local dev server). Leave this terminal open.

### 2. Launch clients

**Option A — environment variable (recommended):**

Open two separate terminals and run in each:

```bash
AA2_SERVER=ws://127.0.0.1:9001 ./dev
```

Each connection claims one seat. First connection = seat 0.

**Option B — dev console:**

Launch `./dev` normally (local mode), then in the in-game dev console type:

```
connect ws://127.0.0.1:9001
start
```

### 3. Start the game

Press Start in any client (or type `start` in the dev console). Remaining seats (N..8) are filled
with AI bots. All 8 players are alive.

### 4. What to verify

| Phase | Expected behavior |
|-------|-------------------|
| God Pick | Both clients see the god grid; each can pick independently |
| Shop | Each client sees their own shop offerings, gold, bench |
| Draft | Draft overlay appears at the correct rounds; picks apply |
| Equip | Drag-and-drop equip/sell/reposition works; changes reflected |
| Combat | Both clients animate their respective matchups from the server's event log |
| Endgame | Game reaches a winner; `GameOver` overlay shows placements |

### 5. Sockpuppet mode

To control multiple seats from one machine, open additional terminals with
`AA2_SERVER=ws://127.0.0.1:9001 ./dev`. Each connection is a separate seat.

---

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| `Address already in use` on port 9001 | Kill the existing server process: `lsof -ti:9001 \| xargs kill` |
| Client crashes on launch / missing `.godot/` | Run `./dev editor` once to generate the import cache, then retry |
| `dylib not found` or extension load failure | Run `./dev` (which builds first) rather than launching Godot directly |
| Client connects but no phase change | Ensure you press Start or type `start` in the dev console |
| Stale build after code changes | Close Godot fully and re-run `./dev` — no hot-reload for the gdext dylib |
