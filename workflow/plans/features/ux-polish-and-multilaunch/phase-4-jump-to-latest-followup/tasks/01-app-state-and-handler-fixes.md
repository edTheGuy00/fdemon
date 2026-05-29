## Task: fdemon-app state + handler fixes (M2, M3, m1)

**Objective**: Close three review findings in `fdemon-app`: reset `unseen_log_count` on `clear_logs` (M2), add the false→true `auto_scroll` transition guard to `handle_page_down` (M3), and gate the `add_log` increment on the active log filter (m1).

**Depends on**: None. Runs in parallel with task 02 (different crate, disjoint files).

**Estimated Time**: 1–1.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/session/session.rs`: reset counter in `clear_logs`; gate the `add_log` increment on `filter_state.matches(&entry)`; update the field doc comment; add inline tests.
- `crates/fdemon-app/src/handler/scroll.rs`: add the transition guard to `handle_page_down`; add scroll-handler tests. (Optional: extract a shared `clear_pill_if_reengaged` helper.)

**Files Read (Dependencies):**
- `crates/fdemon-app/src/log_view_state.rs`: confirms `page_down()` calls `scroll_down(...)` and that `scroll_down` flips `auto_scroll` false→true at `max_offset`.
- The `FilterState::matches` definition (grep `fn matches` — it is the predicate already used at `session.rs:711`), to confirm the call signature (`matches(&self, entry: &LogEntry) -> bool`).

### Details

#### 1. M2 — reset `unseen_log_count` in `clear_logs`

`clear_logs` (`session.rs:403-410`) currently resets `error_count`, `offset`, and search state but not the counter. Add the reset alongside `error_count`:

```rust
pub fn clear_logs(&mut self) {
    self.logs.clear();
    self.log_view_state.offset = 0;
    self.error_count = 0;
    self.unseen_log_count = 0; // NEW (M2): no unseen entries remain after a wipe
    self.search_state.matches.clear();
    self.search_state.current_match = None;
}
```

Do **not** also flip `auto_scroll` here — that is out of scope for this fix and changing it risks a separate behavior regression. Resetting the counter is sufficient: with the buffer empty the pill has nothing to advertise.

#### 2. M3 — transition guard in `handle_page_down`

`handle_page_down` (`scroll.rs:62-67`) calls `log_view_state.page_down()` → `scroll_down(...)`, which sets `auto_scroll = true` when the page lands at `max_offset`. Mirror the guard already present in `handle_scroll_down` (`scroll.rs:19-30`):

```rust
pub fn handle_page_down(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        let was_following = handle.session.log_view_state.auto_scroll;
        handle.session.log_view_state.page_down();
        if !was_following && handle.session.log_view_state.auto_scroll {
            handle.session.mark_tail_followed();
        }
    }
    rescan_links_if_active(state);
    UpdateResult::none()
}
```

This also fixes mouse wheel-down, which dispatches `Message::PageDown` / `Message::ScrollDown` (`handler/mouse/link_highlight.rs`) into these same handlers — no extra change required.

**Optional refactor:** if you prefer, extract the repeated guard into a private helper and call it from both `handle_scroll_down` and `handle_page_down`:

```rust
/// Clear the unseen-log counter if a scroll call just re-engaged tail-follow.
fn clear_pill_if_reengaged(handle: &mut SessionHandle, was_following: bool) {
    if !was_following && handle.session.log_view_state.auto_scroll {
        handle.session.mark_tail_followed();
    }
}
```

Keep behavior identical; if you extract it, update both call sites and the existing `handle_scroll_down` test must still pass unchanged.

#### 3. m1 — gate the increment on the active filter

The increment in `add_log` (`session.rs:376-381`, placed after the eviction loop) currently fires for every entry while `!auto_scroll`. Gate it on the filter so the pill counts only entries that would be visible on jump:

```rust
// Track unseen logs for the jump-to-latest indicator (issue #31).
// Only count entries that are (a) arriving while scrolled away from the tail
// AND (b) visible under the active filter — so the pill matches what `G` reveals.
// Ring-buffer eviction is intentionally independent of this counter.
if !self.log_view_state.auto_scroll && self.filter_state.matches(&entry) {
    self.unseen_log_count = self.unseen_log_count.saturating_add(1);
}
```

**Borrow note:** ensure `entry` is still in scope (not moved) at the increment site. `add_log` pushes via `self.logs.push_back(entry)` earlier; if `entry` is moved by the push, read the predicate against the just-pushed back element instead (e.g. `self.logs.back()`), or evaluate `filter_state.matches(&entry)` into a `let visible = ...;` **before** the `push_back` and reuse the bool after the eviction loop. Pick whichever keeps a single clear borrow; prefer capturing `let passes_filter = self.filter_state.matches(&entry);` before `push_back` and gating on `passes_filter` after the eviction loop.

Update the field doc comment on `unseen_log_count` to document filter-gating next to the existing eviction note, including the accepted limitation that changing the filter while scrolled up does not retroactively recompute the count.

### Acceptance Criteria

1. After `clear_logs()` the `unseen_log_count` is 0 regardless of prior scroll state or count.
2. `handle_page_down` clears `unseen_log_count` iff `auto_scroll` transitions false→true during the call (pre-true→true: no-op; pre-false→still-false: no-op; pre-false→now-true: reset).
3. `add_log` while `!auto_scroll` increments the counter **only** when `filter_state.matches(&entry)` is true; entries filtered out do not increment it.
4. `add_log` while `!auto_scroll` with a default (match-all) filter still increments by exactly 1 per entry (no regression to existing Phase 4 behavior).
5. The `unseen_log_count` field doc comment documents both eviction-independence (existing) and filter-gating (new), including the no-retroactive-recompute limitation.
6. `cargo test -p fdemon-app`, `cargo fmt --all -- --check`, and `cargo clippy -p fdemon-app -- -D warnings` pass.

### Testing

Add to the inline `#[cfg(test)] mod tests` in `session.rs`:

```rust
#[test]
fn clear_logs_resets_unseen_log_count() {
    let mut s = Session::new(/* … */);
    s.log_view_state.auto_scroll = false;
    s.add_log(make_log_entry("a"));
    s.add_log(make_log_entry("b"));
    assert!(s.unseen_log_count > 0);
    s.clear_logs();
    assert_eq!(s.unseen_log_count, 0);
    assert!(s.logs.is_empty());
}

#[test]
fn unseen_log_count_skips_filtered_out_entries() {
    let mut s = Session::new(/* … */);
    s.log_view_state.auto_scroll = false;
    // Configure filter to exclude the entries added below (mirror the
    // existing filter-test setup in this module — e.g. errors-only).
    set_filter_excluding(&mut s, /* … */);
    s.add_log(make_log_entry("info line that is filtered out"));
    assert_eq!(s.unseen_log_count, 0);
}

#[test]
fn unseen_log_count_counts_filter_matching_entries() {
    let mut s = Session::new(/* … */);
    s.log_view_state.auto_scroll = false;
    // Default match-all filter (or a filter that matches the entry below).
    s.add_log(make_log_entry("visible line"));
    assert_eq!(s.unseen_log_count, 1);
}
```

Add to the `handler/scroll.rs` test module:

```rust
#[test]
fn handle_page_down_clears_unseen_count_on_natural_follow() {
    let mut state = AppState::default();
    add_test_session(&mut state);
    let handle = state.session_manager.selected_mut().unwrap();
    // Position so that one page_down lands at max_offset.
    handle.session.log_view_state.total_lines = 10;
    handle.session.log_view_state.visible_lines = 8; // page_down jumps to bottom
    handle.session.log_view_state.offset = 0;
    handle.session.log_view_state.auto_scroll = false;
    handle.session.unseen_log_count = 4;

    let _ = handle_page_down(&mut state);

    let handle = state.session_manager.selected_mut().unwrap();
    assert!(handle.session.log_view_state.auto_scroll);
    assert_eq!(handle.session.unseen_log_count, 0);
}

#[test]
fn handle_page_down_preserves_unseen_count_when_not_yet_at_bottom() {
    let mut state = AppState::default();
    add_test_session(&mut state);
    let handle = state.session_manager.selected_mut().unwrap();
    handle.session.log_view_state.total_lines = 1000;
    handle.session.log_view_state.visible_lines = 5;
    handle.session.log_view_state.offset = 0; // far from bottom after one page
    handle.session.log_view_state.auto_scroll = false;
    handle.session.unseen_log_count = 4;

    let _ = handle_page_down(&mut state);

    let handle = state.session_manager.selected_mut().unwrap();
    assert!(!handle.session.log_view_state.auto_scroll);
    assert_eq!(handle.session.unseen_log_count, 4);
}
```

Match the helper names already used in each test module (`make_log_entry`, `add_test_session`, the existing filter-setup helper) — names above are illustrative. Verify the page-down geometry against `LogViewState::page_down`'s actual step (`scroll_down(visible_lines.saturating_sub(...))`) so the "natural follow" case truly reaches `max_offset`.

### Notes

- **Do not edit any `fdemon-tui` file** — the pill render/click fix is task 02.
- **Do not flip `auto_scroll` in `clear_logs`** — out of scope; resetting the counter is the fix.
- **Preserve `saturating_add`** on the increment (overflow safety, unchanged).
- **Mouse wheel-down** is covered transitively via `Message::PageDown`/`ScrollDown`; no `handler/mouse/` change is in scope.

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/session.rs` | _(pending)_ |
| `crates/fdemon-app/src/handler/scroll.rs` | _(pending)_ |

### Notable Decisions/Tradeoffs

_(filled on completion)_

### Testing Performed

_(filled on completion)_

### Risks/Limitations

_(filled on completion)_
