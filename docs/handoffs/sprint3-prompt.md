Project: AA2
  Repo root: this directory
  
  Read docs/handoffs/sprint3-handoff.md and follow the instructions. It contains full context, task breakdown, and build order for Sprint 3 (god pick grid overlay, summary overlay, phase visibility polish) and beyond.

  Reference screenshots are at ~/Downloads/full game screenshots/ (convert to jpg with sips before viewing).

  Before starting implementation:
  1. Read AGENTS.md — dev process, commit convention, test loop, Definition of Done, Test Failure Protocol
  2. Read docs/design/godot-dev-workflow.md — how to build/run/debug the Godot client
  3. Read docs/handoffs/sprint3-handoff.md — THE MAIN HANDOFF DOC — current state, what works, what to build
  4. Run `./dev test` to verify all 33 integration tests pass before making changes
  5. Look at reference screenshots in ~/Downloads/full game screenshots/ (convert to jpg first with sips)

  Key constraints:
  - All layout in .tscn files, all logic in Rust #[func] methods
  - GameManager at path /root/MainScene/GameManager — all UIs query it
  - Must close + reopen Godot to pick up new dylib (no hot-reload)
  - Every bug fix and new behavior MUST have a regression test in client/tests/
  - `cargo clippy -- -D warnings && cargo test && ./dev test` must all pass before commit
  - When a test fails, fix the CODE not the test (see Test Failure Protocol in AGENTS.md)
