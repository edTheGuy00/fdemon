## Task: Deduplicate frame navigation logic

**Objective**: Remove duplicated prev/next frame index computation from `keys.rs` by delegating to pure methods on `PerformanceState`.

**Depends on**: Task 03 (selected_frame wrap fix) — both touch frame selection logic

**Source**: Review Major Issue #5 (Architecture Enforcer)

### Scope

- `crates/fdemon-app/src/handler/keys.rs:379-406`: Replace inline computation with method calls
- `crates/fdemon-app/src/session/performance.rs:118-148`: Add `compute_prev_frame_index()` / `compute_next_frame_index()` pure methods
- `crates/fdemon-app/src/handler/devtools/performance.rs`: Potentially simplify `handle_select_performance_frame`

### Details

#### Problem

The prev/next frame index computation exists in two places:

1. **`keys.rs:379-406`** — inline in key handler:
```rust
InputKey::Left if in_performance => Some(Message::SelectPerformanceFrame {
    index: state.session_manager.selected().and_then(|h| {
        let perf = &h.session.performance;
        let len = perf.frame_history.len();
        if len == 0 { return None; }
        Some(match perf.selected_frame {
            Some(i) if i > 0 => i - 1,
            Some(_) => 0,
            None => len - 1,
        })
    }),
}),
```

2. **`performance.rs:118-148`** — `select_prev_frame()` / `select_next_frame()` methods:
These are mutating methods (`&mut self`) that both compute the new index AND apply it. They duplicate the same boundary-clamping logic.

#### Fix

Add **pure computation methods** to `PerformanceState` that return the new index without mutating state:

```rust
impl PerformanceState {
    /// Compute the index of the previous frame without mutating state.
    pub fn compute_prev_frame_index(&self) -> Option<usize> {
        let len = self.frame_history.len();
        if len == 0 {
            return None;
        }
        Some(match self.selected_frame {
            Some(i) if i > 0 => i - 1,
            Some(_) => 0,       // already at first frame, stay
            None => len - 1,    // nothing selected, select newest
        })
    }

    /// Compute the index of the next frame without mutating state.
    pub fn compute_next_frame_index(&self) -> Option<usize> {
        let len = self.frame_history.len();
        if len == 0 {
            return None;
        }
        Some(match self.selected_frame {
            Some(i) if i + 1 < len => i + 1,
            Some(i) => i,       // already at last frame, stay
            None => len - 1,    // nothing selected, select newest
        })
    }
}
```

Then update `keys.rs` to call these:

```rust
InputKey::Left if in_performance => Some(Message::SelectPerformanceFrame {
    index: state.session_manager.selected()
        .and_then(|h| h.session.performance.compute_prev_frame_index()),
}),

InputKey::Right if in_performance => Some(Message::SelectPerformanceFrame {
    index: state.session_manager.selected()
        .and_then(|h| h.session.performance.compute_next_frame_index()),
}),
```

The existing `select_prev_frame()` / `select_next_frame()` mutating methods can be simplified to delegate to the pure methods:

```rust
pub fn select_prev_frame(&mut self) {
    self.selected_frame = self.compute_prev_frame_index();
}

pub fn select_next_frame(&mut self) {
    self.selected_frame = self.compute_next_frame_index();
}
```

### Acceptance Criteria

1. Frame navigation (Left/Right keys) behaves identically to before
2. `keys.rs` no longer contains inline frame index computation — delegates to `PerformanceState` methods
3. `select_prev_frame()` / `select_next_frame()` delegate to the pure `compute_*` methods
4. Single source of truth: all frame boundary logic lives in `PerformanceState`
5. Existing navigation tests pass

### Testing

The existing tests for `select_prev_frame` / `select_next_frame` in `crates/fdemon-app/src/session/tests.rs` should continue to pass. Add tests for the new pure methods:

```rust
#[test]
fn test_compute_prev_frame_index_from_middle() {
    let mut perf = PerformanceState::default();
    push_frames(&mut perf, 10);
    perf.selected_frame = Some(5);
    assert_eq!(perf.compute_prev_frame_index(), Some(4));
}

#[test]
fn test_compute_prev_frame_index_at_start() {
    let mut perf = PerformanceState::default();
    push_frames(&mut perf, 10);
    perf.selected_frame = Some(0);
    assert_eq!(perf.compute_prev_frame_index(), Some(0)); // clamp at 0
}

#[test]
fn test_compute_next_frame_index_at_end() {
    let mut perf = PerformanceState::default();
    push_frames(&mut perf, 10);
    perf.selected_frame = Some(9);
    assert_eq!(perf.compute_next_frame_index(), Some(9)); // clamp at end
}

#[test]
fn test_compute_prev_frame_index_none_selects_newest() {
    let mut perf = PerformanceState::default();
    push_frames(&mut perf, 10);
    perf.selected_frame = None;
    assert_eq!(perf.compute_prev_frame_index(), Some(9));
}
```

### Notes

- The `compute_*` methods are `&self` (immutable) while `select_*` methods are `&mut self`. Both should exist: the pure methods for `keys.rs` (which needs to return a `Message`), and the mutating methods for direct state manipulation in handlers.
- This task should be done after Task 03 lands, since Task 03 changes how `selected_frame` is managed on buffer wrap. The navigation logic should be consistent with the wrap-adjustment behavior.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/performance.rs` | Added `compute_prev_frame_index(&self)` and `compute_next_frame_index(&self)` pure methods; simplified `select_prev_frame` and `select_next_frame` to delegate to the new pure methods; added 8 tests for the new pure methods |
| `crates/fdemon-app/src/handler/keys.rs` | Replaced inline frame index computation in Left/Right key handlers with calls to `compute_prev_frame_index()` / `compute_next_frame_index()` |

### Notable Decisions/Tradeoffs

1. **`compute_next_frame_index` uses `Some(i) => i` (stay)**: The existing `select_next_frame` used `Some(_) => len - 1` (go to last frame). The task specifies `Some(i) => i`. These are semantically equivalent when `selected_frame` is always in-bounds (which it is, since all mutations clamp), so the behavioral change is nil in practice. The `Some(i) => i` form is more explicit about the "stay at current index" intent.

2. **Tests added inline in `performance.rs`**: The task mentioned both `performance.rs` and `session/tests.rs` as possible locations. The inline `#[cfg(test)]` block in `performance.rs` already contains all the frame selection tests and the `push_test_frames` helper, making it the natural home for these new tests.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (963 passed, 5 ignored; 8 new `compute_*` tests confirmed via `cargo test -p fdemon-app compute`)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)
- `cargo fmt -p fdemon-app` - Passed

### Risks/Limitations

1. **Behavioral equivalence assumption**: The `select_next_frame` behavior change (`Some(_) => len - 1` to `Some(i) => i`) is safe only because `selected_frame` is always kept in-bounds by all callers. If future code sets `selected_frame` to an out-of-bounds index, the old code would silently correct it to `len - 1` while the new code would return the out-of-bounds index. This is an acceptable tradeoff given that all existing mutations clamp correctly.
