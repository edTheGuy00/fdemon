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
