## Task: Wire Allocation Sort Interaction

**Objective**: Wire the existing `AllocationSortColumn` enum and `PerformanceState.allocation_sort` field to an actual key binding (`s` in Performance panel) that toggles the allocation table sort order between "by size" and "by instances". Remove the `#[allow(dead_code)]` annotations. Update the allocation table widget to read and use the sort column from state.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/session/performance.rs`: MODIFIED — Remove `#[allow(dead_code)]`, make `AllocationSortColumn` and `allocation_sort` `pub`
- `crates/fdemon-app/src/message.rs`: MODIFIED — Add `ToggleAllocationSort` message
- `crates/fdemon-app/src/handler/devtools/performance.rs`: MODIFIED — Add `handle_toggle_allocation_sort()`
- `crates/fdemon-app/src/handler/keys.rs`: MODIFIED — Add `s` key binding in Performance panel context
- `crates/fdemon-app/src/handler/update.rs`: MODIFIED — Wire `ToggleAllocationSort` to handler
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/table.rs`: MODIFIED — Accept and use sort column from state

### Details

#### 1. Remove dead code annotations (`session/performance.rs`)

Remove the two `#[allow(dead_code)]` annotations and the TODO comments on `AllocationSortColumn` and `allocation_sort`:

```rust
// Before:
#[allow(dead_code)] // TODO: wire to allocation table sort interaction
pub(crate) enum AllocationSortColumn { ... }

// After:
pub enum AllocationSortColumn { ... }
```

Change `pub(crate)` to `pub` on both the enum and the field so the TUI crate can access the sort column for rendering.

#### 2. Add `ToggleAllocationSort` message (`message.rs`)

Add a new message variant:

```rust
/// Toggle the allocation table sort column (Size ↔ Instances).
ToggleAllocationSort,
```

#### 3. Implement handler (`handler/devtools/performance.rs`)

Add a handler function:

```rust
/// Toggle the allocation table sort between BySize and ByInstances.
pub(crate) fn handle_toggle_allocation_sort(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.performance.allocation_sort = match handle.session.performance.allocation_sort {
            AllocationSortColumn::BySize => AllocationSortColumn::ByInstances,
            AllocationSortColumn::ByInstances => AllocationSortColumn::BySize,
        };
    }
    UpdateResult::none()
}
```

#### 4. Wire in `update.rs`

Add the match arm for `Message::ToggleAllocationSort`:

```rust
Message::ToggleAllocationSort => {
    devtools::performance::handle_toggle_allocation_sort(state)
}
```

#### 5. Add key binding (`handler/keys.rs`)

In the `handle_key_devtools()` function, add `s` when the Performance panel is active:

```rust
// In the Performance panel section:
InputKey::Char('s') if in_performance => Some(Message::ToggleAllocationSort),
```

Verify this does not conflict with other `s` bindings in DevTools mode. Currently `s` in DevTools mode maps to `NetworkSwitchDetailTab(ResponseBody)` only when `in_network` is true, so `s` when `in_performance` is free.

#### 6. Update allocation table widget (`memory_chart/table.rs`)

The `render_allocation_table()` function currently receives `Option<&AllocationProfile>` and sorts unconditionally by `new_size` descending. Update it to also accept the sort column:

```rust
pub(super) fn render_allocation_table(
    allocation_profile: Option<&AllocationProfile>,
    sort_column: AllocationSortColumn,
    area: Rect,
    buf: &mut Buffer,
) {
```

Change the sort logic:

```rust
// Currently:
stats.sort_by(|a, b| b.new_size.cmp(&a.new_size));

// Updated:
match sort_column {
    AllocationSortColumn::BySize => {
        stats.sort_by(|a, b| b.new_size.cmp(&a.new_size));
    }
    AllocationSortColumn::ByInstances => {
        stats.sort_by(|a, b| b.new_count.cmp(&a.new_count));
    }
}
```

Update the table header to indicate the active sort column (e.g., with an arrow indicator):

```rust
// Header shows which column is sorted:
// BySize:      "Class          Instances  Size ▼    Retained"
// ByInstances: "Class          Instances ▼ Size      Retained"
```

Update the caller in `memory_chart/mod.rs` to pass the sort column from `PerformanceState`.

#### 7. Add tests

In `handler/devtools/performance.rs`, add tests for the toggle handler:

```rust
#[test]
fn test_toggle_allocation_sort_size_to_instances() {
    let mut state = make_devtools_state();
    assert_eq!(
        state.session().performance.allocation_sort,
        AllocationSortColumn::BySize
    );
    handle_toggle_allocation_sort(&mut state);
    assert_eq!(
        state.session().performance.allocation_sort,
        AllocationSortColumn::ByInstances
    );
}

#[test]
fn test_toggle_allocation_sort_instances_to_size() {
    let mut state = make_devtools_state();
    state.session_mut().performance.allocation_sort = AllocationSortColumn::ByInstances;
    handle_toggle_allocation_sort(&mut state);
    assert_eq!(
        state.session().performance.allocation_sort,
        AllocationSortColumn::BySize
    );
}
```

In `handler/keys.rs` tests, add a test that `s` in Performance mode produces `ToggleAllocationSort`.

In `memory_chart/table.rs` tests, add tests that verify the table sorts correctly for each column.

### Acceptance Criteria

1. `AllocationSortColumn` enum has no `#[allow(dead_code)]` annotation
2. `PerformanceState.allocation_sort` field has no `#[allow(dead_code)]` annotation
3. Both are `pub` (not `pub(crate)`)
4. `ToggleAllocationSort` message exists and is handled
5. `s` key in Performance panel toggles allocation sort
6. Allocation table renders sorted by the active column
7. Table header shows sort indicator on the active column
8. `s` key does not conflict with other DevTools bindings
9. `cargo test -p fdemon-app -- devtools` passes
10. `cargo test -p fdemon-tui -- allocation` passes
11. `cargo clippy --workspace` clean (no dead_code warnings for these items)

### Testing

```bash
cargo test -p fdemon-app -- toggle_allocation
cargo test -p fdemon-app -- devtools
cargo test -p fdemon-tui -- allocation_table
cargo clippy --workspace
```

### Notes

- **Sort column values**: `BySize` sorts by `new_size` (total bytes allocated since last GC), `ByInstances` sorts by `new_count` (total instances allocated since last GC). Both use descending order — largest first.
- **Sort stability**: Since the allocation profile is replaced entirely on each fetch (not incrementally merged), re-sorting on each render is acceptable. No caching needed.
- **Retained size**: The PLAN.md mentions a "Retained Size" column, but the `ClassHeapStats` struct in `fdemon-core` does not have a retained size field. Do not add one — retained size requires heap snapshots which are expensive. The table shows `new_size`, `new_count`, and `accumulated_size` columns.
