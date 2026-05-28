## Task: Unseen log count state + reset wiring

**Objective**: Add a per-`Session` `unseen_log_count: usize` counter that increments on `add_log` while the view is not following the tail, and reset it to 0 on every path that re-engages tail-follow (`G`/`End` jump **and** natural `scroll_down`-to-bottom). Foundation for the render-time pill in task 02.

**Depends on**: None

**Estimated Time**: 0.5–1h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/session/session.rs`: Add `unseen_log_count: usize` field to `Session`, add increment in `add_log`, add `mark_tail_followed()` helper, add inline unit tests.
- `crates/fdemon-app/src/handler/scroll.rs`: Call `session.mark_tail_followed()` in `handle_scroll_to_bottom`; in `handle_scroll_down`, capture pre-state and call `mark_tail_followed()` on `false → true` transition. Add scroll-handler tests.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/log_view_state.rs`: Confirms `auto_scroll` transition semantics — `scroll_to_bottom()` always sets `auto_scroll = true`; `scroll_down(n)` sets `true` only when the resulting offset hits `max_offset` (lines 120–140).

### Details

#### 1. Session field + increment

In `crates/fdemon-app/src/session/session.rs`, add a `pub` field to `Session` (group it next to the other counter fields like `error_count`, `reload_count` — wherever the cluster lives). Default 0 in the constructor.

```rust
/// Count of log entries appended while the view was scrolled away from the
/// tail (i.e., `log_view_state.auto_scroll == false`). Advisory only — used
/// by the log view to render a "↓ N new · G to jump" indicator. Reset to
/// zero whenever auto-scroll re-engages via `mark_tail_followed()`.
///
/// Ring-buffer eviction does not decrement this counter: evicted entries
/// are old (front), unseen entries are new (back). The two are independent.
pub unseen_log_count: usize,
```

In `Session::add_log` (lines 277–362), **after** the existing `self.logs.push_back(entry)` call and the ring-buffer trim loop, increment the counter conditionally on the current follow state. Use `saturating_add` to defang `usize::MAX` overflow.

```rust
// Track unseen logs for the jump-to-latest indicator (issue #31).
// Only increment while the user is not following the tail; ring-buffer
// eviction is intentionally independent of this counter.
if !self.log_view_state.auto_scroll {
    self.unseen_log_count = self.unseen_log_count.saturating_add(1);
}
```

**Placement rationale:** The increment must be after `push_back` (so the entry is committed) and after the eviction loop (so the counter is not affected by trim — verified by the eviction-doesn't-decrement test). `add_logs_batch`, `queue_log → flush_batched_logs`, and any other batch path all funnel through `add_log` per entry, so the per-call increment correctly accumulates batches.

#### 2. Helper method

Add a `mark_tail_followed` method on `Session`:

```rust
/// Reset the unseen log counter, called when the view re-engages tail-follow
/// (either via `Message::ScrollToBottom` or by scrolling down to the natural
/// bottom). Idempotent — safe to call when `unseen_log_count` is already 0
/// or when `auto_scroll` is already true.
pub fn mark_tail_followed(&mut self) {
    self.unseen_log_count = 0;
}
```

Keep it dead-simple: a single assignment, no return value, no transition check. The caller decides when to invoke; the method is unconditional. This keeps the API obvious and unit-testable.

#### 3. Handler wiring — `handle_scroll_to_bottom`

In `crates/fdemon-app/src/handler/scroll.rs`, the existing handler (line 37):

```rust
pub fn handle_scroll_to_bottom(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.log_view_state.scroll_to_bottom();
        handle.session.mark_tail_followed(); // NEW
    }
    UpdateResult::no_op()
}
```

The order matters: call `scroll_to_bottom()` first (which sets `auto_scroll = true` and adjusts `offset`), then `mark_tail_followed()` (which clears the counter). Both touch the same `session` mutable borrow; no aliasing issue.

#### 4. Handler wiring — `handle_scroll_down` (natural follow re-engagement)

`LogViewState::scroll_down(n)` re-enables `auto_scroll` when the resulting offset reaches `max_offset`. Detect the transition by capturing pre-state:

```rust
pub fn handle_scroll_down(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        let was_following = handle.session.log_view_state.auto_scroll;
        handle.session.log_view_state.scroll_down(1);
        // If scroll_down naturally re-engaged auto-scroll (false → true),
        // also clear the unseen counter so the pill disappears in step.
        if !was_following && handle.session.log_view_state.auto_scroll {
            handle.session.mark_tail_followed();
        }
    }
    UpdateResult::no_op()
}
```

**Do not** apply the same transition check to `handle_page_down` unless that handler also has a natural-bottom auto-engage path. Inspect `scroll.rs` and apply the same pattern only where `LogViewState`'s scroll method can flip `auto_scroll` from false to true. (Confirmed callsites per research: `scroll_down` is the only such method; `page_down` calls `scroll_down(visible_lines)` so the same handler-level guard works there too — extend if so.)

**Negative case:** Do **not** add the transition check to `handle_scroll_up` / `handle_scroll_to_top` / `handle_page_up`. These can only flip `auto_scroll` true → false, never the other direction; the counter starts at 0 and only grows while `auto_scroll == false`, so there is nothing to reset on these paths.

### Acceptance Criteria

1. `Session::unseen_log_count` exists as a public `usize` field, default 0 in `Session::new` / `Session::default`.
2. Calling `add_log` while `log_view_state.auto_scroll == true` does **not** increment `unseen_log_count`.
3. Calling `add_log` while `log_view_state.auto_scroll == false` increments `unseen_log_count` by exactly 1.
4. Ring-buffer eviction (`logs.len() > max_logs`) does **not** decrement `unseen_log_count`. (Push to a Session with `max_logs = 2` and `auto_scroll = false`, then add 5 logs; counter is 5 even though only 2 entries remain in the buffer.)
5. `mark_tail_followed()` sets `unseen_log_count = 0` unconditionally.
6. `handle_scroll_to_bottom` always clears `unseen_log_count` on the selected session (via `mark_tail_followed`).
7. `handle_scroll_down` clears `unseen_log_count` **iff** `auto_scroll` transitioned `false → true` during the call. (Pre-true → still true: no-op. Pre-false → still false: no-op. Pre-false → now true: reset.)
8. `unseen_log_count = usize::MAX; add_log` while not following keeps it at `usize::MAX` (no panic — saturating add).
9. `cargo test -p fdemon-app`, `cargo fmt --all -- --check`, and `cargo clippy -p fdemon-app -- -D warnings` pass.

### Testing

Add to `Session`'s existing inline `#[cfg(test)] mod tests` block in `session/session.rs`:

```rust
#[test]
fn unseen_log_count_does_not_increment_while_following() {
    let mut s = Session::new(/* … */);
    assert!(s.log_view_state.auto_scroll);
    s.add_log(make_log_entry("a"));
    s.add_log(make_log_entry("b"));
    assert_eq!(s.unseen_log_count, 0);
}

#[test]
fn unseen_log_count_increments_while_scrolled_up() {
    let mut s = Session::new(/* … */);
    s.log_view_state.auto_scroll = false;
    s.add_log(make_log_entry("a"));
    s.add_log(make_log_entry("b"));
    s.add_log(make_log_entry("c"));
    assert_eq!(s.unseen_log_count, 3);
}

#[test]
fn unseen_log_count_unaffected_by_ring_buffer_eviction() {
    let mut s = Session::new(/* … */);
    s.max_logs = 2; // tight buffer
    s.log_view_state.auto_scroll = false;
    for i in 0..5 {
        s.add_log(make_log_entry(&format!("log {i}")));
    }
    assert_eq!(s.logs.len(), 2);
    assert_eq!(s.unseen_log_count, 5); // all 5 appends counted
}

#[test]
fn mark_tail_followed_resets_counter() {
    let mut s = Session::new(/* … */);
    s.log_view_state.auto_scroll = false;
    s.add_log(make_log_entry("a"));
    s.add_log(make_log_entry("b"));
    assert_eq!(s.unseen_log_count, 2);
    s.mark_tail_followed();
    assert_eq!(s.unseen_log_count, 0);
}

#[test]
fn unseen_log_count_saturates_at_max() {
    let mut s = Session::new(/* … */);
    s.log_view_state.auto_scroll = false;
    s.unseen_log_count = usize::MAX;
    s.add_log(make_log_entry("overflow"));
    assert_eq!(s.unseen_log_count, usize::MAX);
}
```

Add to the `handler/scroll.rs` test module (or wherever existing scroll-handler tests live):

```rust
#[test]
fn handle_scroll_to_bottom_clears_unseen_count() {
    let mut state = AppState::default();
    // Assume helper that pushes a session into session_manager.
    add_test_session(&mut state);
    let handle = state.session_manager.selected_mut().unwrap();
    handle.session.log_view_state.auto_scroll = false;
    handle.session.unseen_log_count = 7;

    let _ = handle_scroll_to_bottom(&mut state);

    let handle = state.session_manager.selected_mut().unwrap();
    assert!(handle.session.log_view_state.auto_scroll);
    assert_eq!(handle.session.unseen_log_count, 0);
}

#[test]
fn handle_scroll_down_clears_unseen_count_on_natural_follow() {
    let mut state = AppState::default();
    add_test_session(&mut state);
    let handle = state.session_manager.selected_mut().unwrap();
    // Position one line above the bottom with auto_scroll off.
    handle.session.log_view_state.total_lines = 10;
    handle.session.log_view_state.visible_lines = 5;
    handle.session.log_view_state.offset = 4; // max_offset = 5, so one down hits it
    handle.session.log_view_state.auto_scroll = false;
    handle.session.unseen_log_count = 3;

    let _ = handle_scroll_down(&mut state);

    let handle = state.session_manager.selected_mut().unwrap();
    assert!(handle.session.log_view_state.auto_scroll);
    assert_eq!(handle.session.unseen_log_count, 0);
}

#[test]
fn handle_scroll_down_preserves_unseen_count_when_not_yet_at_bottom() {
    let mut state = AppState::default();
    add_test_session(&mut state);
    let handle = state.session_manager.selected_mut().unwrap();
    handle.session.log_view_state.total_lines = 100;
    handle.session.log_view_state.visible_lines = 5;
    handle.session.log_view_state.offset = 10; // far from bottom
    handle.session.log_view_state.auto_scroll = false;
    handle.session.unseen_log_count = 3;

    let _ = handle_scroll_down(&mut state);

    let handle = state.session_manager.selected_mut().unwrap();
    assert!(!handle.session.log_view_state.auto_scroll);
    assert_eq!(handle.session.unseen_log_count, 3); // unchanged
}
```

If the existing test scaffold differs (e.g., no `make_log_entry` / `add_test_session` helpers), follow the conventions already in the test module — these names are illustrative.

### Notes

- **Why `saturating_add`?** Per CODE_STANDARDS.md "Common Anti-Patterns" — no panics in library code. A counter that hits `usize::MAX` on a sufficiently long unattended session must not crash; it must clamp.
- **Why reset in the handler, not in `LogViewState`?** Keeping `LogViewState` ignorant of `Session` avoids a cyclic-knowledge problem and keeps `LogViewState` pure of business state. The handler is the only place that already has a `&mut Session` borrow and can coordinate both updates atomically.
- **Why no transition check in `mark_tail_followed`?** It's a primitive reset. The caller (handler) does the transition check exactly once. Embedding the check inside `mark_tail_followed` would duplicate the guard and force the caller to still capture pre-state anyway.
- **Why advisory, not authoritative?** PLAN.md "Edge Cases & Risks → Jump-to-latest" already calls out: "the count is advisory ('12 new'); clamp/treat as best-effort and always reset on follow." The ring-buffer-eviction independence is consistent with this — if the buffer evicts 100 entries while the user is scrolled up, the counter still says "100 new" even though only the most recent slice is actually retained. That's fine; the user gets the right behavioral signal (lots of new stuff happened).
- **Do not edit `widgets/log_view/`, `render/mod.rs`, or `LogView` builders in this task.** Those are task 02.
- **Do not add a new `Message` variant** — `Message::ScrollToBottom` already exists and is what `G`/`End` and (in task 02) the pill mouse-click both emit.

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
