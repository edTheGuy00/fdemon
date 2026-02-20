## Task: Verify No Regressions After Phase 1 Decomposition

**Objective**: Run the full quality gate across the workspace to confirm that all three decomposition tasks produced zero behavioral changes, no test failures, and no new warnings.

**Depends on**: 01-split-inspector-widget, 02-split-performance-widget, 03-split-handler-devtools

### Scope

- Workspace-wide: all 4 crates (`fdemon-core`, `fdemon-daemon`, `fdemon-app`, `fdemon-tui`) plus binary
- No file modifications — this task only verifies

### Verification Steps

#### 1. Format Check

```bash
cargo fmt --all --check
```

Expected: no formatting issues (the split should preserve original formatting within each function).

#### 2. Compilation Check

```bash
cargo check --workspace
```

Expected: clean compilation, no errors or warnings.

#### 3. Full Test Suite

```bash
cargo test --workspace
```

Expected: all tests pass. Specifically verify these counts:

| Crate | Test Area | Expected Count |
|-------|-----------|----------------|
| `fdemon-tui` | Inspector widget tests | 27 |
| `fdemon-tui` | Performance widget tests | 20 |
| `fdemon-tui` | Layout explorer tests | 19 (unchanged, not touched in Phase 1) |
| `fdemon-app` | Handler devtools tests | 42 |
| All | Total workspace tests | 1,532+ |

#### 4. Clippy Lint Check

```bash
cargo clippy --workspace -- -D warnings
```

Expected: zero warnings. Watch for:
- Unused imports (most common issue after file splits)
- Dead code warnings from visibility changes
- Needless `pub(super)` on items only used within their own file

#### 5. File Size Verification

Verify no file exceeds 600 lines (400 target, 600 hard limit):

```bash
wc -l crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs
wc -l crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs
wc -l crates/fdemon-tui/src/widgets/devtools/inspector/details_panel.rs
wc -l crates/fdemon-tui/src/widgets/devtools/performance/mod.rs
wc -l crates/fdemon-tui/src/widgets/devtools/performance/frame_section.rs
wc -l crates/fdemon-tui/src/widgets/devtools/performance/memory_section.rs
wc -l crates/fdemon-tui/src/widgets/devtools/performance/stats_section.rs
wc -l crates/fdemon-tui/src/widgets/devtools/performance/styles.rs
wc -l crates/fdemon-app/src/handler/devtools/mod.rs
wc -l crates/fdemon-app/src/handler/devtools/inspector.rs
wc -l crates/fdemon-app/src/handler/devtools/layout.rs
```

#### 6. Verify Old Files Deleted

```bash
# These should NOT exist:
test ! -f crates/fdemon-tui/src/widgets/devtools/inspector.rs
test ! -f crates/fdemon-tui/src/widgets/devtools/performance.rs
test ! -f crates/fdemon-app/src/handler/devtools.rs
```

#### 7. Verify Parent Module Declarations Unchanged

Confirm that `devtools/mod.rs` in the TUI crate and `handler/mod.rs` in the app crate have not been modified (their `pub mod` declarations resolve identically for file vs directory modules):

```bash
git diff crates/fdemon-tui/src/widgets/devtools/mod.rs
git diff crates/fdemon-app/src/handler/mod.rs
```

Expected: no diff (or minimal diff if re-exports were adjusted — but ideally zero changes).

### Acceptance Criteria

1. `cargo fmt --all --check` — clean
2. `cargo check --workspace` — no errors
3. `cargo test --workspace` — all tests pass, no decrease in test count
4. `cargo clippy --workspace -- -D warnings` — zero warnings
5. All new files are under 600 lines
6. Old monolithic files (`inspector.rs`, `performance.rs`, `handler/devtools.rs`) are deleted
7. No behavioral changes — same visual output, same test assertions

### Testing

The full quality gate:

```bash
cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings
```

### Notes

- This task is a verification-only pass. If any issues are found, they should be fixed in the relevant task (01, 02, or 03) — not patched here.
- If all three decomposition tasks independently pass their per-crate tests and clippy, this task should be a formality. Its value is catching cross-crate issues (e.g., a re-export that compiles within fdemon-app but breaks fdemon-tui's build).
