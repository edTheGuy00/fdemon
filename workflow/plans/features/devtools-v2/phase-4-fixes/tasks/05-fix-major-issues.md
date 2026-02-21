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
