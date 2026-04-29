# MSRV `is_multiple_of` Cleanup — Task Index

## Overview

Restore MSRV (`1.77.2`) compliance across 5 production-code call sites and add a one-line rationale comment to `HangingGetVmBackend`'s `#[allow(dead_code)]` attribute. All 4 tasks have disjoint write-file sets and run in parallel worktrees.

**Total Tasks:** 4
**Estimated Hours:** 0.75–1.25 hours total (parallelizable)

## Task Dependency Graph

```
Wave 1 (parallel — disjoint write-file sets)
├── 01-fix-fdemon-app-msrv             (1 site)
├── 02-fix-fdemon-dap-msrv             (1 site)
├── 03-fix-fdemon-tui-msrv             (3 sites across 2 files)
└── 04-document-hanging-get-vm-backend (1 comment)
```

No cross-task dependencies — Wave 1 is the only wave.

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-fix-fdemon-app-msrv](tasks/01-fix-fdemon-app-msrv.md) | Done | - | 0.25h | `crates/fdemon-app/` |
| 2 | [02-fix-fdemon-dap-msrv](tasks/02-fix-fdemon-dap-msrv.md) | Done | - | 0.25h | `crates/fdemon-dap/` |
| 3 | [03-fix-fdemon-tui-msrv](tasks/03-fix-fdemon-tui-msrv.md) | Done | - | 0.25–0.5h | `crates/fdemon-tui/` |
| 4 | [04-document-hanging-get-vm-backend](tasks/04-document-hanging-get-vm-backend.md) | Done | - | 0.1h | `crates/fdemon-dap/` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|--------------------------|
| 01-fix-fdemon-app-msrv | `crates/fdemon-app/src/state.rs` | `crates/fdemon-tui/src/widgets/devtools/network/tests.rs` (precedent reference) |
| 02-fix-fdemon-dap-msrv | `crates/fdemon-dap/src/adapter/breakpoints.rs` | `crates/fdemon-tui/src/widgets/devtools/network/tests.rs` (precedent reference) |
| 03-fix-fdemon-tui-msrv | `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/chart.rs`, `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/bars.rs` | `crates/fdemon-tui/src/widgets/devtools/network/tests.rs` (precedent reference) |
| 04-document-hanging-get-vm-backend | `crates/fdemon-dap/src/adapter/tests/request_timeouts_events.rs` | - |

### Overlap Matrix

Tasks 02 and 04 both touch `fdemon-dap` but write to different files (`adapter/breakpoints.rs` vs `adapter/tests/request_timeouts_events.rs`) — no write-file overlap.

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|--------------------|-------------------|
| 01 + 02 | None | Parallel (worktree) |
| 01 + 03 | None | Parallel (worktree) |
| 01 + 04 | None | Parallel (worktree) |
| 02 + 03 | None | Parallel (worktree) |
| 02 + 04 | None — different files within fdemon-dap | Parallel (worktree) |
| 03 + 04 | None | Parallel (worktree) |

## Strategy (per task)

Each task in this wave follows the same recipe (matching the precedent in `crates/fdemon-tui/src/widgets/devtools/network/tests.rs:11-12`):

1. Replace `x.is_multiple_of(N)` with `x % N == 0`. Preserve any surrounding parentheses.
2. On the **enclosing function** (not the line — clippy attribute placement constraint), prepend:
   ```rust
   // MSRV guard: `is_multiple_of` requires Rust 1.87; MSRV is 1.77.2 — suppress the lint.
   #[allow(clippy::manual_is_multiple_of)]
   ```
3. Verify per-crate:
   - `cargo clippy -p <crate> --all-targets -- -D warnings` exits 0.
   - `cargo test -p <crate>` passes.
   - `cargo fmt --all` is clean.

Task 04 is a one-line comment addition only — no behavior change, no clippy/test impact beyond confirming the file still compiles.

## Cross-Cutting Constraints

These apply to all tasks:

- **MSRV is `1.77.2`** (`Cargo.toml`). The fix must preserve this declaration.
- **Behavior must not change.** All affected `N` values are provably non-zero, so `% N == 0` and `is_multiple_of(N)` are observably identical:
  - `state.rs:756` — literal `15`
  - `breakpoints.rs:692` — guarded by `Ok(n) if n > 0 =>`
  - `chart.rs:111`, `chart.rs:223`, `bars.rs:180` — literal `2`
- **Match the precedent exactly.** The `#[allow]` placement, comment wording, and indentation should mirror `crates/fdemon-tui/src/widgets/devtools/network/tests.rs:11-12`.
- **No new modules, no public API changes, no refactors** beyond what the lint/MSRV requires.
- **Parentheses preserved** on `(x - line_start_x).is_multiple_of(2)` rewrite — `(x - line_start_x) % 2 == 0`.

## Success Criteria

This bug is resolved when:

- [ ] `grep -rn 'is_multiple_of' crates/` returns only `#[allow(...)]` attribute lines and pre-existing MSRV-suppressed test code (no production call sites).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] `cargo test --workspace` passes — no regression in animation loop (`fdemon-app`), breakpoint hit conditions (`fdemon-dap`), or DevTools chart rendering (`fdemon-tui`).
- [ ] `HangingGetVmBackend`'s `#[allow(dead_code)]` is preceded by a `//` comment explaining the retention rationale.
- [ ] No public API or behavior changes; only MSRV-driven edits.
- [ ] (Optional, manual) `cargo +1.77.2 check --workspace` exits 0 if the MSRV toolchain is installed locally.

## Notes

- **Source of bug:** Identified during the post-implementation review of `clippy-rust-191-cleanup` (see `workflow/reviews/bugs/clippy-rust-191-cleanup/REVIEW.md` and `ACTION_ITEMS.md`).
- **Why these were missed in the prior cleanup:** `manual_is_multiple_of` clippy lint fires on `% N == 0`, not on the stabilized `.is_multiple_of()` form. Once code already uses the API, clippy is silent — the violation is invisible to the tool.
- **CI does not enforce MSRV** (uses `dtolnay/rust-toolchain@stable`). This fix restores consistency with the declared MSRV but does not establish enforcement. See "Further Considerations" in `BUG.md` for that separate decision.
- After all 4 tasks land, archive this plan directory to `workflow/reviews/bugs/msrv-is-multiple-of-cleanup/` per repo convention.
