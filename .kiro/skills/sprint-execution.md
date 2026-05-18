# Skill: Sprint Execution

## Purpose
Autonomously execute sprint items from `docs/project-plan.md` with proper scoping, verification, and documentation — without requiring repeated user prompting.

## Trigger
User asks to work on a sprint, implement sprint items, or continue Phase N work.

## Workflow

### 1. Scope Assessment
Read `docs/project-plan.md` to identify the current sprint's deliverables. Break them into atomic implementation chunks that can each be:
- Implemented in one delegation
- Verified independently
- Tested in isolation

Present the scope breakdown to the user as a TODO list. Each item should be concrete enough that "done" is unambiguous.

### 2. Open Questions
Before implementing, identify ambiguities in the spec. Ask the user ONLY about:
- **Design decisions** not covered by docs/game-systems.md
- **Edge cases** where multiple valid behaviors exist
- **AA-specific choices** that differ from Dota2

Do NOT ask about:
- Things already specified in docs/game-systems.md
- Implementation details (you decide those)
- Obvious Dota2 mechanics (research those via yolo-librarian)

Keep questions to a single batch. Wait for answers before proceeding.

### 3. Implement
For each TODO item, delegate to the appropriate subagent with:
- The exact spec from game-systems.md
- Any user answers from step 2
- File paths of relevant existing code
- The verification command to run after

**Delegation order:**
1. Data model changes first (types, structs, enums)
2. Core logic second (algorithms, formulas)
3. Integration third (wiring systems together)
4. Tests last (but written alongside, not after)

**Parallel where possible:** Independent items (e.g., two unrelated modules) should be delegated simultaneously.

### 4. Verify
After each implementation chunk, run the full verification loop:
```bash
cargo check -p <crate>
cargo test -p <crate>
cargo clippy -p <crate> --no-deps -- -D warnings
```

If any step fails, fix before moving on. Never leave the codebase in a broken state.

### 5. Lock In with Tests
Every new behavior MUST have a test that exercises it. Tests should:
- Use actual game parameters (not arbitrary numbers)
- Test edge cases (empty pool, max level, 0 gold, etc.)
- Be deterministic (fixed seeds for anything with RNG)
- Have doc comments explaining what they verify

**Integration tests** (in `tests/`) for multi-system interactions.
**Unit tests** (in-module `#[cfg(test)]`) for single-function behavior.

### 6. Update Documentation
After completing a sprint item, update:
- `docs/project-plan.md` — mark items as complete if appropriate
- `docs/architecture.md` — if new systems/modules were added
- Inline doc comments on all new public items

### 7. Report
After all items are done, present:
- What was implemented (module/file list)
- What's tested (test count, what behaviors are locked in)
- What's NOT yet covered (gaps for next sprint)
- Any deviations from the spec (and why)

## Rules

### Code Quality
- Follow AGENTS.md conventions (conventional commits, clippy clean, doc comments)
- No `unwrap()` in library code
- All public items documented
- Match existing code style in the workspace

### Testing Standards
- Every formula has a unit test with known-good values
- Every state transition has a test
- Every error path has a test (rejected buy, full bench, etc.)
- Integration tests use actual RON data files where relevant

### Sprint Boundaries
- Do NOT implement items from future sprints
- If a current sprint item depends on something from a future sprint, stub it with a TODO comment and move on
- If you discover the spec is incomplete for a current item, ask the user rather than guessing

### Commit Hygiene
- Only commit when user explicitly asks
- Group related changes logically
- Use conventional commit format: `feat(game): implement shop upgrade decay`
- Include `Prompt:` trailer

## Example Sprint Execution

```
User: "Execute Sprint 2 (Weeks 15-16)"

Agent:
1. Reads project-plan.md → identifies 6 deliverables
2. Creates TODO list with atomic items
3. Asks: "Two questions before I start:
   - For hero reroll, does the old hero go back to the draft pool or is it gone?
   - Can a player sell an equipped ability directly, or must they unequip first?"
4. User answers
5. Implements each item, verifying after each
6. Reports: "Sprint 2 complete. 8 new modules, 34 tests. Gaps: [list]"
```

## Verification Checklist (run before declaring any item done)
- [ ] `cargo check -p aa2-game` passes
- [ ] `cargo test -p aa2-game` — all tests pass
- [ ] `cargo clippy -p aa2-game --no-deps -- -D warnings` — clean
- [ ] `cargo test --workspace` — no regressions
- [ ] New public items have `///` doc comments
- [ ] Edge cases tested (empty, full, boundary values)
