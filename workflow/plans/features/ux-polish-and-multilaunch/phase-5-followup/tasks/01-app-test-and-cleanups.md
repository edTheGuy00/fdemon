## Task: fdemon-app review fixes — M1 test gap + app-layer cleanups

**Objective**: Close the MAJOR test-coverage gap (M1) where the test named for the
`toggle_checked_cursor` unsupported guard never calls that function, and apply the
app-layer minor code-quality cleanups from the Phase 5 review.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 1–2h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`
- `crates/fdemon-app/src/new_session_dialog/device_groups.rs`

**Files Read (Dependencies):**
- None.

### Details

**1. (M1 — MAJOR) Make `toggle_checked_cursor_skips_unsupported` exercise the function under test.**
`target_selector_state.rs:872-884`. Today the test only asserts on the private helper
`is_connected_device_supported`; the guard inside `toggle_checked_cursor` is never executed.
Rewrite the body to call `toggle_checked_cursor()` and assert the checked-set is unmodified.
The cursor cannot normally resolve to an unsupported id (it is filtered from the flat list),
so the realistic scenario is: a state containing only an unsupported device → the flat list
is empty → `selected_device_id()` returns `None` → `toggle_checked_cursor()` is a no-op.
Pre-seed `checked_device_ids` to prove nothing is added or removed:

```rust
#[test]
fn toggle_checked_cursor_skips_unsupported() {
    // Only an unsupported device: it is filtered from the flat list, so the cursor
    // cannot resolve to it and toggle_checked_cursor() must be a no-op.
    let mut state = state_with(vec![device("a", false)]);
    // Pre-seed to confirm the checked-set is left untouched by the no-op.
    state.checked_device_ids.insert("a".to_string());
    state.toggle_checked_cursor();
    assert!(
        state.checked_device_ids.contains("a"),
        "no-op must not remove a pre-seeded id"
    );
    assert_eq!(state.checked_device_ids.len(), 1, "no-op must not add ids");
    // Keep the direct-helper assertions too (they document the unknown-id case).
    assert!(!state.is_connected_device_supported("a"));
    assert!(!state.is_connected_device_supported("nonexistent"));
}
```

Use whatever the existing test helpers are named (`state_with`, `device`, etc.) — the above
is illustrative. Change the binding to `let mut state`.

**2. (m2 — MINOR) `is_connected_device_supported` → `.any()`.**
`target_selector_state.rs:465-471`. Replace the `.find().map().unwrap_or(false)` chain with
the equivalent, more idiomatic single-predicate form:

```rust
fn is_connected_device_supported(&self, id: &str) -> bool {
    self.connected_devices
        .iter()
        .any(|d| d.id == id && d.is_supported)
}
```

**3. (m3 — MINOR) `toggle_select_all` — collect directly into `BTreeSet`.**
`target_selector_state.rs:416-421`. Avoid the intermediate `Vec<String>` + second
`into_iter().collect()`. Collect the supported ids once into a `BTreeSet<String>`, compute
`all_checked` over it, then assign/clear:

```rust
let supported_ids: BTreeSet<String> = self
    .connected_devices
    .iter()
    .filter(|d| d.is_supported)
    .map(|d| d.id.clone())
    .collect();
let all_checked = !supported_ids.is_empty()
    && supported_ids.iter().all(|id| self.checked_device_ids.contains(id));
if all_checked {
    self.checked_device_ids.clear();
} else {
    self.checked_device_ids = supported_ids;
}
```
Preserve the existing empty-set behavior (no spurious "all checked" when there are no
supported devices) — the `!supported_ids.is_empty()` guard above does this.

**4. (n3 — NITPICK, optional) `cached_flat_list` `get_or_insert_with`.**
`target_selector_state.rs:~159-162`. If straightforward, replace the `is_none()` + later
`.as_ref().unwrap()` with `get_or_insert_with(|| self.compute_flat_list())` to remove the
bare `unwrap()`. Skip if it fights the borrow checker — it is pre-existing and non-blocking.

**5. (m6 — MINOR) Defensive indexing in `device_groups.rs`.**
Lines ~266/286/309 use `selectable[...]` after an `is_empty()` guard (safe today). Encode the
invariant structurally: `selectable.last().copied().unwrap_or(0)` for the last-index case and
`selectable.get(next_pos).copied().unwrap_or(0)` / `.get(prev_pos)...` for the others.
Behavior must be identical; this only hardens against future refactors removing the guard.

### Acceptance Criteria

1. `toggle_checked_cursor_skips_unsupported` calls `toggle_checked_cursor()` and asserts the checked-set is unchanged (M1 closed).
2. `is_connected_device_supported` uses `.any(|d| d.id == id && d.is_supported)`.
3. `toggle_select_all` no longer allocates an intermediate `Vec`; behavior (including the empty-supported-set case) is unchanged and still covered by `select_all_skips_unsupported`.
4. `device_groups.rs` index sites use `.last()` / `.get()` accessors; cursor navigation tests still pass.
5. No behavior change to filtering/selection semantics — all existing Phase 5 tests still pass.
6. `cargo test -p fdemon-app`, `cargo fmt`, `cargo clippy -p fdemon-app -- -D warnings` pass.

### Testing

- Run the existing `new_session_dialog` test suite; confirm `group_connected_excludes_unsupported`, `select_all_skips_unsupported`, `checked_devices_*`, and the rewritten `toggle_checked_cursor_skips_unsupported` all pass.
- No new public API; no new tests required beyond the M1 rewrite (cleanups are behavior-preserving).

### Notes

- All changes are behavior-preserving except the test rewrite (which adds coverage, not behavior).
- Do **not** touch the single filter chokepoint in `group_connected_devices` (the `is_supported` filter) — only the index-accessor style on lines ~266/286/309.

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` | M1: Rewrote `toggle_checked_cursor_skips_unsupported` to call `toggle_checked_cursor()` and verify checked-set unchanged. m2: Replaced `is_connected_device_supported` with `.any()` idiom. m3: `toggle_select_all` now collects directly into `BTreeSet<String>`, eliminating intermediate Vec. |
| `crates/fdemon-app/src/new_session_dialog/device_groups.rs` | m6: Replaced bare index access `selectable[...]` at three sites with `.last().copied().unwrap_or(0)` and `.get(n).copied().unwrap_or(0)`. |

### Notable Decisions/Tradeoffs

1. **n3 skipped**: The `get_or_insert_with` refactor for `flat_list()` was skipped as directed — `compute_flat_list(&self)` borrows `self` immutably while `get_or_insert_with` would already hold a mutable borrow of `self.cached_flat_list` (part of `self`), causing a borrow-checker conflict. The existing pattern is correct and pre-existing.

2. **M1 test shape**: The test now pre-seeds `checked_device_ids` with `"a"` before calling `toggle_checked_cursor()`, confirming neither addition nor removal happens. The original helper assertions (`is_connected_device_supported`) are preserved to document the unknown-id behavior.

### Testing Performed

- `cargo test -p fdemon-app` — Passed (2618 unit tests, 0 failures)
- `cargo fmt --all` — No formatting changes needed
- `cargo clippy -p fdemon-app -- -D warnings` — Passed (no warnings)
