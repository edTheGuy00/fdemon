## Task: Remove dead `DetailsTab::next` / `DetailsTab::prev` methods and their tests

**Objective**: Delete the `DetailsTab::next()` and `DetailsTab::prev()` methods (`state.rs:179, 188`) and their two unit tests (`state.rs:2356–2365`). These were retained during Phase 3 "for backwards compatibility with existing tests" but are now unreferenced in production — `handle_cycle_tab` cycles via `visible_tabs()` indexing, not `next`/`prev`. Confirmed dead by grep.

**Depends on**: None

**Estimated Time**: 1 hour (mostly verification)

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/state.rs`

**Files Read (Dependencies):**
- All `crates/**/*.rs` — grep verification before deleting (see step 1 below)

### Details

#### Background

Phase 3 cross-cutting constraint #4 stated:

> `DetailsTab::next()` / `DetailsTab::prev()` are NO LONGER called from `handle_cycle_tab` — leave them as-is for backwards compatibility with existing tests, but cycling now goes through `visible_tabs()`.

Reviewer findings confirmed (`risks_tradeoffs` and code_quality_inspector): these methods are now dead in production code. Their only remaining callers are their own unit tests:

```
state.rs:179:    pub fn next(self) -> Self {
state.rs:188:    pub fn prev(self) -> Self {
state.rs:853:    pub fn next_tab(&mut self) {     ← UNRELATED: this is on a different enum (likely DevToolsView), keep
state.rs:861:    pub fn prev_tab(&mut self) {     ← UNRELATED: same
state.rs:2356:        assert_eq!(DetailsTab::Properties.next(), DetailsTab::RenderObject);
state.rs:2357:        assert_eq!(DetailsTab::RenderObject.next(), DetailsTab::FlexExplorer);
state.rs:2358:        assert_eq!(DetailsTab::FlexExplorer.next(), DetailsTab::Properties);
state.rs:2363:        assert_eq!(DetailsTab::Properties.prev(), DetailsTab::FlexExplorer);
state.rs:2364:        assert_eq!(DetailsTab::RenderObject.prev(), DetailsTab::Properties);
state.rs:2365:        assert_eq!(DetailsTab::FlexExplorer.prev(), DetailsTab::RenderObject);
```

Lines 853 and 861 (`next_tab` / `prev_tab`) are unrelated — they belong to a different enum (probably `DevToolsView` or session navigation). Verify before editing.

#### 1. Re-verify dead-code status before deleting

Run grep across the entire codebase (not just `state.rs`):

```bash
grep -rn "DetailsTab::next\|DetailsTab::prev" crates/ tests/ 2>/dev/null
grep -rn "\.next()\|\.prev()" crates/fdemon-app/src/handler/ 2>/dev/null
```

The first grep should show ONLY the two test references at `state.rs:2356–2365`. The second should show no `.next()` / `.prev()` calls in any handler module (production code) that are on a `DetailsTab` receiver.

If grep reveals an unexpected production caller — **abort the task and report**. Do not delete in that case; instead, re-open the task with an updated scope that handles the unexpected caller.

#### 2. Delete the methods

Remove the `impl DetailsTab { ... }` block containing `next` and `prev` (currently lines 179–197 or thereabouts). If the `impl` block contains other methods that ARE used, remove only `next` and `prev` and keep the block.

#### 3. Delete the tests

Remove the two unit tests (functions containing the assertions at lines 2356–2365). Their names are likely `detailstab_next_cycles_forward` and `detailstab_prev_cycles_backward` (or similar — confirm exact names in the file).

#### 4. Verify build and existing tests still pass

After deletion, the full quality gate must pass:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Particular attention to clippy — if the deletion leaves an orphan `impl` block or an unused import, fix it.

### Acceptance Criteria

1. `DetailsTab::next` and `DetailsTab::prev` are no longer defined in `crates/fdemon-app/src/state.rs`.
2. The two unit tests that exercised them are removed.
3. `grep -rn "DetailsTab::next\|DetailsTab::prev" crates/ tests/` returns zero results.
4. All other tests in `state.rs` still pass (the test removal is targeted; no surrounding tests should be affected).
5. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

No new tests added — this is a deletion task. Verification is the existing test suite continuing to pass.

If the `impl DetailsTab` block becomes empty after the deletion, remove the now-empty block as well.

### Notes

- **Strict scope:** Per cross-cutting constraint #6 in `TASKS.md`, this task ONLY removes `DetailsTab::next`, `DetailsTab::prev`, and their unit tests. Do not touch other `DetailsTab` methods, do not touch `DevToolsView::next_tab` / `prev_tab` (different enum, lines 853/861), do not touch any other dead-code candidates.
- **Why now:** This is the cleanest "while we're already opening `state.rs`" cleanup. The methods were retained intentionally during Phase 3 for backwards compatibility — that compatibility is no longer needed because the related production callers have been migrated to `visible_tabs()`-based cycling.
- **Risk:** Low. Pure deletion of clearly-dead code with grep verification.
- **What if grep finds a hidden caller?** Most likely a doctest, an example, or a benchmark. If so, that caller probably also wants migration to `visible_tabs()`; reopen the task with expanded scope. Do not silently update the caller in this task — keep the scope tight.

---

## Completion Summary

**Status:** Pending
**Branch:** _to be filled_

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | _to be filled_ |

### Notable Decisions/Tradeoffs

1. _to be filled_

### Testing Performed

- _to be filled_

### Risks/Limitations

1. _to be filled_
