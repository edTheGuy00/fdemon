## Task: Minor fixes batch

**Objective**: Address all minor issues from the Phase 3 review in a single pass.

**Depends on**: Task 04 (memory_chart extraction), Task 05 (frame nav dedup) — to avoid merge conflicts

**Source**: Review Minor Issues #6-#11

### Scope

- `crates/fdemon-daemon/src/vm_service/performance.rs`: Delete duplicate test
- `crates/fdemon-app/src/handler/devtools/performance.rs`: Fix `.map().flatten()` anti-pattern
- `crates/fdemon-app/src/session/performance.rs`: Handle `allocation_sort` dead state + fix constant visibility
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/mod.rs`: Add `Y_AXIS_WIDTH` constant
- `crates/fdemon-app/src/actions.rs`: Add documentation comment for `Arc` design constraint

### Details

#### 1. Delete duplicate test `test_parse_memory_usage`

**File:** `crates/fdemon-daemon/src/vm_service/performance.rs:332-344`

`test_parse_memory_usage` is byte-for-byte identical to `test_parse_memory_usage_still_works` (lines 318-330). Delete the duplicate.

```rust
// DELETE this entire function (lines 332-344):
#[test]
fn test_parse_memory_usage() { ... }
```

Keep `test_parse_memory_usage_still_works` as the surviving test.

#### 2. Fix `.map().flatten()` anti-pattern

**File:** `crates/fdemon-app/src/handler/devtools/performance.rs:131-137`

```rust
// Before
fn current_selected_frame(state: &AppState) -> Option<usize> {
    state
        .session_manager
        .selected()
        .map(|h| h.session.performance.selected_frame)
        .flatten()
}

// After
fn current_selected_frame(state: &AppState) -> Option<usize> {
    state
        .session_manager
        .selected()
        .and_then(|h| h.session.performance.selected_frame)
}
```

This is flagged by Clippy as `option_map_flatten`. The two forms are semantically identical.

#### 3. Remove `allocation_sort` dead state (or add TODO)

**File:** `crates/fdemon-app/src/session/performance.rs:70`

The `allocation_sort: AllocationSortColumn` field is:
- Declared and initialised to `AllocationSortColumn::BySize` in `Default` and `with_memory_history_size`
- Tested for its default value in 3 tests
- **Never read or written** by any handler, key binding, or widget

Two options:

**Option A: Remove entirely** — Delete the field, the `AllocationSortColumn` enum, and the 3 tests. Clean removal of dead code.

**Option B: Keep with TODO** — Add a `// TODO: wire to allocation table sort interaction` comment and keep the field. This preserves the design intent for future allocation table sorting.

**Recommendation:** Option B — keep with TODO. The allocation table sorting is a natural next step, and the types are well-designed. Removing and re-adding later is wasteful. Also remove the `AllocationSortColumn` from the `pub use` re-export in `session/mod.rs` since no external consumer uses it.

#### 4. Add `Y_AXIS_WIDTH` named constant

**File:** `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/mod.rs` (post-extraction) or `memory_chart.rs` (pre-extraction)

```rust
// Add to the constants block (after MIN_TABLE_HEIGHT):
const Y_AXIS_WIDTH: u16 = 7;
```

Replace the `let y_axis_width: u16 = 7;` local binding in `render_chart_area()` with `Y_AXIS_WIDTH`.

#### 5. Fix `DEFAULT_MEMORY_SAMPLE_SIZE` visibility

**File:** `crates/fdemon-app/src/session/performance.rs:19`

```rust
// Before
pub const DEFAULT_MEMORY_SAMPLE_SIZE: usize = 120;

// After
pub(crate) const DEFAULT_MEMORY_SAMPLE_SIZE: usize = 120;
```

Also remove it from the `pub use` re-export in `crates/fdemon-app/src/session/mod.rs`:

```rust
// Before
pub use performance::{AllocationSortColumn, PerformanceState, DEFAULT_MEMORY_SAMPLE_SIZE};

// After
pub use performance::PerformanceState;
```

(If `AllocationSortColumn` is kept per item 3 Option B, it should be `pub(crate)` or removed from the re-export.)

#### 6. Document `Arc` design constraint on `perf_shutdown_tx`

**File:** `crates/fdemon-app/src/actions.rs:618`

The `Arc::new(perf_shutdown_tx)` is **necessary** because `Message` derives `Clone` and `watch::Sender` does not implement `Clone`. The `Arc` is the correct workaround given this constraint.

Add a brief comment:

```rust
// Arc is required because Message derives Clone and watch::Sender does not impl Clone.
let perf_shutdown_tx = std::sync::Arc::new(perf_shutdown_tx);
```

This is documentation, not a code change. The original review flagged it as "unnecessary" but research confirms it's required.

### Acceptance Criteria

1. No duplicate test functions in `vm_service/performance.rs`
2. No `.map().flatten()` — uses `.and_then()` instead
3. `allocation_sort` has a TODO comment explaining it's reserved for future use
4. Magic number `7` replaced with `Y_AXIS_WIDTH` constant
5. `DEFAULT_MEMORY_SAMPLE_SIZE` is `pub(crate)`, not `pub`
6. `Arc` usage on `perf_shutdown_tx` is documented with explanation
7. `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

### Testing

No new tests needed. Run full verification:

```bash
cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings
```

### Notes

- Items 1-5 are mechanical changes with no behavioral impact.
- Item 6 is documentation only — no code logic changes.
- This task is intentionally last to avoid merge conflicts with earlier tasks that touch the same files.
