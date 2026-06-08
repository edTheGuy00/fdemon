# Phase 1 — Reorder PATH after Flutter SDK — Task Index

## Overview

Swap the wizard's `PathConfig` and `FlutterSdk` steps so the order becomes
`Prerequisites → AndroidTools → FlutterSdk → PathConfig → Doctor` (PATH Configuration becomes the last
step before Flutter Doctor). This is a **pure display reorder** — no new types, no behavior change.
`AndroidTools` keeps its name in Phase 1 (it becomes the Platforms submenu in Phase 2).

The completion chain (`FlutterSdk` success → auto-config PATH) and all `match WizardStepKind` arms are
already order-independent and need **no** changes — only the `vec![]` literal in `build_steps()`, the
tests that hardcode step indices, one soft-tip wording, and the website docs.

**Total Tasks:** 2
**Estimated Hours:** 1.5–2.5 hours

## Task Dependency Graph

```
┌─────────────────────────────────┐     ┌─────────────────────────────────┐
│ 01-reorder-steps-and-tests      │     │ 02-update-website-docs          │
│ (fdemon-app + fdemon-tui)       │     │ (website/.../toolchain.rs)      │
└─────────────────────────────────┘     └─────────────────────────────────┘
        (no dependency between them — disjoint files, parallelizable)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-reorder-steps-and-tests](tasks/01-reorder-steps-and-tests.md) | Not Started | - | 1–2h | `install_wizard/state.rs`, `handler/install_wizard/actions.rs`, `widgets/install_wizard/step_detail.rs` |
| 2 | [02-update-website-docs](tasks/02-update-website-docs.md) | Not Started | - | 0.5h | `website/src/pages/docs/toolchain.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01-reorder-steps-and-tests | `crates/fdemon-app/src/install_wizard/state.rs`, `crates/fdemon-app/src/handler/install_wizard/actions.rs`, `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | — |
| 02-update-website-docs | `website/src/pages/docs/toolchain.rs` | `crates/fdemon-app/src/install_wizard/state.rs` (read for confirmed order) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | None | **Parallel (worktree)** |

## Success Criteria

Phase 1 is complete when:

- [ ] `build_steps()` returns the order `Prerequisites[0] → AndroidTools[1] → FlutterSdk[2] → PathConfig[3] → Doctor[4]`.
- [ ] Navigating top-to-bottom no longer hits the `"Install Flutter first"` dead-end (PATH is now after Flutter SDK).
- [ ] The `"Install Flutter first"` gate is **retained** (still fires on manual nav to PathConfig with no SDK).
- [ ] All install-wizard unit tests pass with the renumbered indices; `cargo test --workspace --lib` is green.
- [ ] `cargo fmt --all` and `cargo clippy --workspace -- -D warnings` are clean.
- [ ] The website toolchain docs (numbered table + ASCII art) reflect the new order.

## Notes

- **Do NOT rename `AndroidTools`** — that happens in Phase 2.
- The `installed_sdk_path` field doc-comment ("subsequent `PathConfig` step") stays accurate — PathConfig is
  still the step after FlutterSdk. No change there.
- `step_list.rs` and `navigation.rs` need **0 changes** — `step_list.rs`'s `make_steps()` is a local 4-step
  fixture (already FlutterSdk-at-2, no PathConfig), and `navigation.rs` tests only drive up/down clamping,
  never asserting a kind at a position.
- `docs/ARCHITECTURE.md` needs **0 changes** — its "Install Wizard Step Execution Flow" describes the
  execution sequence (FlutterSdk → PathConfig), which is unchanged by a display reorder.
- Line numbers in the task files are from the research snapshot and may drift — **locate by symbol/pattern**
  (test name, `WizardStepKind::…`, comment text), not by absolute line.
