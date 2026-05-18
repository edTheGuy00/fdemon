## Task: Deduplicate `clamp_chart_scroll` and `ScrollDir` between performance and memory handlers

**Objective:** Resolve m9 — the `clamp_chart_scroll` function and `ScrollDir` enum were duplicated verbatim between `handler/devtools/performance.rs:31-40` and `handler/devtools/memory.rs:32-41` when T03 of Phase 1 extracted the memory handlers. Two identical copies mean either one can drift independently and unit tests cover them asymmetrically. Extract both into a single shared location under `handler/devtools/`.

**Depends on:** 03 (to ensure `handler/devtools/performance.rs` and `handler/devtools/memory.rs` are stable post-Wave-1), 04 (T04 also edits `handler/devtools/performance.rs`'s docstring — T05 must rebase atop those changes)

**Agent:** implementor

**Estimated Time:** 0.5–1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/devtools/mod.rs` OR a new `crates/fdemon-app/src/handler/devtools/scroll_helpers.rs` — single canonical definition of `ScrollDir` and `clamp_chart_scroll`. Choose based on the existing module layout (see "Implementation choice" below).
- `crates/fdemon-app/src/handler/devtools/performance.rs` — remove the local definition, add an import from the shared location.
- `crates/fdemon-app/src/handler/devtools/memory.rs` — remove the local definition, add an import from the shared location.

**Files Read (Dependencies):**
- T03 Completion Summary — confirm no other helpers were extracted that should also be hoisted.
- `crates/fdemon-app/src/handler/devtools/mod.rs` — existing module declarations and re-exports.

### Background

The Phase 1 T03 task plan accepted the duplication as out-of-scope follow-up work. The risks reviewer flagged it as drift risk: "if one copy changes (e.g., signed-overflow handling), the other can silently diverge." T03's completion summary mentioned creating a follow-up but no task file was opened — this task closes that gap.

Both copies are bit-identical at the time of writing. The function clamps a candidate scroll offset against a max-back constraint:

```rust
fn clamp_chart_scroll(current: usize, delta: i64, max_back: usize) -> usize {
    let new = current as i64 + delta;
    new.clamp(0, max_back as i64) as usize
}
```

And the helper enum:

```rust
enum ScrollDir { Up, Down }
```

Both are module-private; they aren't exported beyond `handler/devtools/{performance,memory}`. The dedup mechanically lifts them to a parent scope.

### Implementation choice

The codebase has two reasonable homes for shared private helpers under `handler/devtools/`:

**Option 1: Inline in `handler/devtools/mod.rs` (`pub(super)` items).** Smaller change, no new file. Tradeoff: `mod.rs` already houses `handle_switch_panel`, `handle_enter_devtools_mode`, `parse_default_panel`, and the panel-switching glue — adding helpers may dilute its identity slightly but is consistent with how `handle_*` helpers there work.

**Option 2: New `handler/devtools/scroll_helpers.rs` file with `pub(super)` items, declared from `mod.rs` via `mod scroll_helpers; use scroll_helpers::*;`.** Cleaner separation, easier to extend (Phase 2 may add more scroll utilities). Slight increase in module count.

Either is acceptable. The implementor picks; record the choice in the Completion Summary.

### Details

#### 1. Extract the shared definition

**If Option 1:** Add to `crates/fdemon-app/src/handler/devtools/mod.rs` (private at end of file or in a clearly demarcated section):

```rust
// ── Shared scroll helpers ──────────────────────────────────────────────────
// Used by handler/devtools/performance.rs and handler/devtools/memory.rs.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScrollDir {
    Up,
    Down,
}

/// Clamp a candidate chart scroll offset against the maximum back-scroll.
///
/// `current` and `delta` are combined as signed i64 to avoid underflow when
/// the user attempts to scroll back past the live edge. Returns the clamped
/// non-negative offset.
pub(super) fn clamp_chart_scroll(current: usize, delta: i64, max_back: usize) -> usize {
    let new = current as i64 + delta;
    new.clamp(0, max_back as i64) as usize
}
```

**If Option 2:** Create `crates/fdemon-app/src/handler/devtools/scroll_helpers.rs` containing the same items (drop the `pub(super)` if exporting via `mod scroll_helpers; pub(super) use scroll_helpers::{ScrollDir, clamp_chart_scroll};`) and add the `mod scroll_helpers;` declaration at the top of `mod.rs`.

Add doc comments more substantial than the inline-block version: explain *why* the helper exists (shared rendering between memory chart and frame chart) and the underflow guarantee.

#### 2. Remove local copies and add imports

**`crates/fdemon-app/src/handler/devtools/performance.rs`:**

- Delete the local `ScrollDir` enum and `clamp_chart_scroll` function (lines 31-40 area).
- Add `use super::{clamp_chart_scroll, ScrollDir};` (or `use super::scroll_helpers::{clamp_chart_scroll, ScrollDir};` if Option 2) near the existing `use` block.

**`crates/fdemon-app/src/handler/devtools/memory.rs`:**

- Delete the local `ScrollDir` enum and `clamp_chart_scroll` function (lines 32-41 area).
- Add the same import as above.

#### 3. Test coverage

The existing test suite already covers `clamp_chart_scroll` indirectly via the page/scroll/jump handlers. Run the full suite to confirm no regressions:

```bash
cargo test --workspace
```

If any test in `handler/devtools/performance.rs` or `handler/devtools/memory.rs` referenced the local types by path (`super::ScrollDir`), update to the new path.

Optionally add a focused unit test for `clamp_chart_scroll` next to the shared definition. The existing call-site tests are sufficient — a dedicated test is a small bonus, not a requirement.

#### 4. Quality gate

`cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`

All four green.

### Acceptance Criteria

- [ ] `cargo check`, `cargo test`, `cargo clippy` all green.
- [ ] Exactly one definition of `clamp_chart_scroll` exists in the codebase. Verify with `rg "fn clamp_chart_scroll" crates/`.
- [ ] Exactly one definition of `ScrollDir` exists in the codebase. Verify with `rg "enum ScrollDir" crates/`.
- [ ] Both `handler/devtools/performance.rs` and `handler/devtools/memory.rs` import the helpers from the new shared location.
- [ ] All pre-existing tests pass without modification beyond import-path updates.
- [ ] Completion Summary names the chosen implementation option (1 or 2) and the final location of the shared module.

### Module Structure

- **Option 1:** No new file. The two helpers live in `crates/fdemon-app/src/handler/devtools/mod.rs` as `pub(super)` items.
- **Option 2:** New file `crates/fdemon-app/src/handler/devtools/scroll_helpers.rs`, declared from `mod.rs` via `mod scroll_helpers;` and consumed by the two sibling handler modules.
