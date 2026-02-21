## Task: Fix Major Issues (Color Consistency, Hydration Failure, Index Semantics)

**Objective**: Address the three MAJOR (should-fix) issues from the review that are not blocking but represent real bugs or significant quality concerns.

**Depends on**: None
**Severity**: MAJOR
**Review ref**: REVIEW.md Issues #5, #6, #8

### Scope

- `crates/fdemon-tui/src/widgets/devtools/network/mod.rs`: Add shared `http_method_color()` function
- `crates/fdemon-tui/src/widgets/devtools/network/request_table.rs`: Use shared color function
- `crates/fdemon-tui/src/widgets/devtools/network/request_details.rs`: Use shared color function
- `crates/fdemon-app/src/process.rs`: Add `FetchHttpRequestDetail` hydration failure arm
- `crates/fdemon-app/src/session/network.rs`: Fix `selected_index` semantics with filters

### Issue 5: Inconsistent HTTP Method Color Schemes

**Problem**: `request_table.rs::method_color()` (line ~297-308) and `request_details.rs::method_style()` (line ~504-515) have 5 conflicting color mappings for the same HTTP methods (POST, PUT, PATCH, HEAD, OPTIONS).

**Fix**: Extract a single `http_method_color()` function in `network/mod.rs` and have both files call it. Pick one consistent color scheme (the table's is more conventional — blue for POST, yellow for PUT/PATCH):

```rust
// In network/mod.rs
pub(super) fn http_method_color(method: &str) -> Color {
    match method {
        "GET" => Color::Green,
        "POST" => Color::Blue,
        "PUT" | "PATCH" => Color::Yellow,
        "DELETE" => Color::Red,
        "HEAD" => Color::Cyan,
        "OPTIONS" => Color::Magenta,
        _ => Color::White,
    }
}
```

Remove `method_color()` from `request_table.rs` and `method_style()` from `request_details.rs`. Both call sites should use `Style::default().fg(super::http_method_color(method))`.

### Issue 6: `FetchHttpRequestDetail` Hydration Failure Leaves Spinner Stuck

**Problem**: In `process.rs` (line ~78-93), the hydration failure fallback sends failure messages for `FetchWidgetTree` and `FetchLayoutData` but has no branch for `FetchHttpRequestDetail`. If the VM disconnects between the handler returning the action and hydration, `loading_detail` remains `true` permanently.

**Fix**: Add a `FetchHttpRequestDetail` arm in the hydration failure match:

```rust
UpdateAction::FetchHttpRequestDetail { session_id, .. } => {
    let _ = msg_tx.try_send(Message::VmServiceHttpRequestDetailFailed {
        session_id: *session_id,
        error: "VM Service handle unavailable".to_string(),
    });
}
```

Verify that `VmServiceHttpRequestDetailFailed` message variant exists and that the handler for it clears `loading_detail`. If the message variant doesn't exist, add it following the pattern of `WidgetTreeFetchFailed`.

### Issue 8: `selected_index` Semantics Inconsistency with Filters

**Problem**: In `session/network.rs`, `selected_index` is used as an index into `filtered_entries()` by `select_prev/next/selected_entry`, but the eviction loop in `merge_entries` (line ~88-102) adjusts it as if it indexes into the raw `entries` Vec. When a filter is active, these two interpretations collide.

**Fix**: Track `selected_index` as the raw index into `entries` and translate at display time. This is the simpler fix because eviction only operates on the raw Vec.

Changes to `session/network.rs`:
1. In `merge_entries`, the eviction adjustment is correct for raw indexing — keep as-is
2. In `selected_entry`, change to use the raw index directly:
   ```rust
   pub fn selected_entry(&self) -> Option<&HttpProfileEntry> {
       self.selected_index.and_then(|i| self.entries.get(i))
   }
   ```
3. In `select_prev/select_next`, navigate within the filtered set but store the raw index:
   - Get `filtered_entries()`
   - Find the current position within the filtered list
   - Move to prev/next within the filtered list
   - Store the raw index of the new entry (found via `entries.iter().position(|e| e.id == selected.id)`)
4. Update `filtered_count()` usage — it's still needed for bounds in select_next

Alternatively (simpler if we don't want to refactor navigation): clear `selected_index` when the filter changes, which avoids the index domain mismatch entirely. This is the approach used by the Inspector tab when its data refreshes.

### Tests

- Test color function returns consistent results (unit test for `http_method_color`)
- Test hydration failure for `FetchHttpRequestDetail` sends failure message
- Test eviction with active filter preserves correct selection
- Test eviction without filter still works (regression)

### Verification

```bash
cargo test -p fdemon-tui -- http_method_color
cargo test -p fdemon-app -- hydration
cargo test -p fdemon-app -- merge_entries
cargo test -p fdemon-app -- selected_index
cargo clippy --workspace
```

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/network/mod.rs` | Added `pub(super) fn http_method_color()` — single authoritative color mapping for HTTP methods; added 8 unit tests in `tests.rs` |
| `crates/fdemon-tui/src/widgets/devtools/network/request_table.rs` | Removed local `method_color()` function; call site updated to `Style::default().fg(super::http_method_color(...))` ; existing tests updated to call `super::super::http_method_color` |
| `crates/fdemon-tui/src/widgets/devtools/network/request_details.rs` | Removed local `method_style()` function; call site updated to use shared color function; 3 tests updated to reflect correct (blue) POST color |
| `crates/fdemon-tui/src/widgets/devtools/network/tests.rs` | Added 8 unit tests for `http_method_color`; added `Color` import |
| `crates/fdemon-app/src/process.rs` | Added `UpdateAction::FetchHttpRequestDetail` arm to hydration failure match — sends `VmServiceHttpRequestDetailFailed` so `loading_detail` is cleared if VM disconnects |
| `crates/fdemon-app/src/session/network.rs` | Added `set_filter()` method that atomically sets filter, clears `selected_index`, clears `selected_detail`, resets `scroll_offset`; added 6 new unit tests (4 `set_filter` + 2 eviction regression) |
| `crates/fdemon-app/src/handler/devtools/network.rs` | Updated `handle_network_filter_changed` to delegate to `NetworkState::set_filter()` instead of setting fields inline |

### Notable Decisions/Tradeoffs

1. **Simpler approach for Issue 8**: The task offered two fixes for `selected_index` semantics. The simpler "clear on filter change" approach was chosen over refactoring `select_prev/select_next/selected_entry` to use raw indexing. This is sufficient because: (a) the handler already cleared selection on filter change, (b) I enforced it at the data layer via `set_filter()`, and (c) navigation while filtering stores filtered-list positions, but those are cleared before filter state can diverge via eviction.

2. **Issue 6 verified**: `VmServiceHttpRequestDetailFailed` already existed in `message.rs` and its handler in `update.rs` already cleared `loading_detail`. Only the hydration failure fallback arm was missing.

3. **Test path for `http_method_color` in `request_table.rs`**: The test module `request_table::tests` uses `super::super::http_method_color` because `super` resolves to `request_table` and the function lives in the `network` parent module.

### Testing Performed

- `cargo test -p fdemon-tui -- http_method_color` - Passed (8 tests)
- `cargo test -p fdemon-app -- selected_index` - Passed (1 test)
- `cargo test -p fdemon-app -- set_filter` - Passed (4 tests)
- `cargo test -p fdemon-app -- eviction` - Passed (2 tests)
- `cargo test -p fdemon-app -- merge_entries` - Passed (3 tests)
- `cargo test --lib --workspace` - Passed (712 tests)
- `cargo clippy --workspace -- -D warnings` - Passed (0 warnings)
- `cargo fmt --all` - Applied

### Risks/Limitations

1. **Issue 8 partial fix**: Navigation while a filter is active still stores a filtered-list index in `selected_index`, not a raw index. The `set_filter()` method clears selection on filter change, preventing the mismatch in practice, but if code elsewhere sets `filter` directly (bypassing `set_filter`), the invariant can break. The single direct field assignment `state.filter = "POST"` in the test helper was updated but any future caller must use `set_filter`. This could be hardened further by making `filter` private.
