## Task: Performance Handler Split + `]`/`[` Key Routing

**Objective**: Split the 1020-line `crates/fdemon-app/src/handler/devtools/performance.rs` into a directory module mirroring the inspector's structure, add the new `PerfCycleDetailsTab` / `PerfFocusDetailsTab` handlers, and route the new `]` / `[` keys when the user has `focused_section == Details` on the Performance panel. This is a structural refactor with one net behavioural change — details-tab cycling.

**Depends on**: 02 (state + message foundation)

**Estimated Time**: 3–5 hours

### Scope

**Files Modified (Write):**
- DELETE `crates/fdemon-app/src/handler/devtools/performance.rs`.
- **NEW** `crates/fdemon-app/src/handler/devtools/performance/mod.rs` — module declarations (`mod frame; mod details;`) and any cross-cutting helpers. Re-export the public handler functions consumed by `handler/update.rs` so existing call sites do not change.
- **NEW** `crates/fdemon-app/src/handler/devtools/performance/frame.rs` — all existing frame-selection / scroll / page / jump handlers moved verbatim from `performance.rs`. Function names unchanged (`handle_select_performance_frame`, `handle_perf_focus_section`, `handle_perf_scroll`, `handle_perf_page`, `handle_perf_jump_to_start`, `handle_perf_jump_to_end`). Only the file location changes. Tests inside the existing file move with it; if the existing test module is monolithic, split it along the same frame/details axis.
- **NEW** `crates/fdemon-app/src/handler/devtools/performance/details.rs` — new handlers `handle_perf_cycle_details_tab(state, forward) -> UpdateResult` and `handle_perf_focus_details_tab(state, tab) -> UpdateResult`. Inline `#[cfg(test)] mod tests` block.
- `crates/fdemon-app/src/handler/devtools/mod.rs` — verify `pub mod performance;` still works (it points at the new directory module; should require no edit if the directory layout follows Rust conventions — `performance/mod.rs` is auto-discovered).
- `crates/fdemon-app/src/handler/keys.rs` — inside the `if in_performance { ... }` block (around line 487):
  - When `focused_section == PerfSection::Details`:
    - `InputKey::Char(']')` → `Message::PerfCycleDetailsTab { forward: true }`.
    - `InputKey::Char('[')` → `Message::PerfCycleDetailsTab { forward: false }`.
  - Tab / Shift+Tab continue to emit `PerfFocusSection(next/prev)` — no change to those arms; T02's PerfSection cycling fix gives them functional behaviour automatically.
- `crates/fdemon-app/src/handler/update.rs` — add two new dispatch arms:
  - `Message::PerfCycleDetailsTab { forward } => devtools::performance::handle_perf_cycle_details_tab(state, forward),`
  - `Message::PerfFocusDetailsTab(tab) => devtools::performance::handle_perf_focus_details_tab(state, tab),`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/devtools/inspector/` — reference for directory-module layout pattern (`mod.rs` + sub-files).
- `crates/fdemon-app/src/session/performance.rs` (T02 outputs: `PerfDetailsTab`, the cycling-fixed `PerfSection`).
- `crates/fdemon-app/src/state.rs` (T02 outputs: `PerfDetailsTab` enum).
- `crates/fdemon-app/src/message.rs` (T02 outputs: `PerfCycleDetailsTab`, `PerfFocusDetailsTab`).

### Details

#### Directory layout after T03

```
crates/fdemon-app/src/handler/devtools/
├── debug.rs
├── inspector.rs
├── memory.rs
├── mod.rs                ← unchanged
├── network.rs
├── performance/          ← NEW DIR (replaces performance.rs)
│   ├── mod.rs            ← module decls + re-exports
│   ├── frame.rs          ← existing handlers (moved)
│   └── details.rs        ← NEW — Phase 2 cycle/focus handlers
└── scroll_helpers.rs
```

#### `performance/mod.rs`

```rust
//! Performance panel handlers.
//!
//! Split into:
//! - [`frame`] — frame selection, frame-chart scroll/page/jump, section focus.
//! - [`details`] — Phase 2 details-pane tab cycling/focus.
//!
//! Memory and allocation profile handlers live in [`super::memory`].

mod details;
mod frame;

pub(crate) use details::{handle_perf_cycle_details_tab, handle_perf_focus_details_tab};
pub(crate) use frame::{
    handle_perf_focus_section, handle_perf_jump_to_end, handle_perf_jump_to_start, handle_perf_page,
    handle_perf_scroll, handle_select_performance_frame,
};
```

The `pub(crate) use` re-exports preserve every existing call site in `handler/update.rs` — `devtools::performance::handle_perf_*(state, ...)` continues to work unchanged.

#### `performance/frame.rs`

Move the contents of the current `performance.rs` here, dropping nothing. Then:

- The Phase 1 `PerfSection::Details` arms in `handle_perf_scroll`, `handle_perf_page`, `handle_perf_jump_to_start`, `handle_perf_jump_to_end` are currently no-ops with a comment "No-op in Phase 1 — details pane content arrives in Phase 2." Update the comment to say "No-op in Phase 2 — Frame Analysis tab content fits on screen with no scrolling. Phase 3's Rebuild Stats / Timeline Events tabs will use `details_pane_visible_height` to scroll." Keep the no-op body; do not add per-tab scrolling yet.

#### `performance/details.rs`

```rust
//! Performance Details pane handlers — Phase 2 tab cycling.

use crate::handler::UpdateResult;
use crate::session::performance::PerfSection;
use crate::state::{AppState, PerfDetailsTab};

/// Cycle the active tab in the Performance Details pane.
///
/// Only mutates state when the user actually has the Details section focused;
/// otherwise the key emission is a no-op. (The keys.rs guard already enforces
/// this, but the handler is defensive — a future mouse-driven dispatch path
/// could land here without the keyboard guard.)
pub(crate) fn handle_perf_cycle_details_tab(
    state: &mut AppState,
    forward: bool,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        if handle.session.performance.focused_section != PerfSection::Details {
            return UpdateResult::none();
        }
        let next = if forward {
            handle.session.performance.details_tab.next()
        } else {
            handle.session.performance.details_tab.prev()
        };
        handle.session.performance.details_tab = next;
    }
    UpdateResult::none()
}

/// Set the active tab in the Performance Details pane directly.
///
/// Phase 2: emitted only by tests. Phase 3 wires up mouse-click regions on the
/// tab strip that emit this variant.
pub(crate) fn handle_perf_focus_details_tab(
    state: &mut AppState,
    tab: PerfDetailsTab,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.performance.details_tab = tab;
    }
    UpdateResult::none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::update::update;
    use crate::message::Message;
    use crate::session::performance::PerfSection;
    use crate::state::{AppState, DevToolsPanel, PerfDetailsTab, UiMode};

    fn test_device() -> fdemon_daemon::Device {
        fdemon_daemon::Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    fn make_state_in_performance_details() -> AppState {
        let mut state = AppState::new();
        let _id = state.session_manager.create_session(&test_device()).unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Performance;
        if let Some(h) = state.session_manager.selected_mut() {
            h.session.performance.focused_section = PerfSection::Details;
        }
        state
    }

    #[test]
    fn cycle_forward_advances_details_tab() {
        let mut state = make_state_in_performance_details();
        update(&mut state, Message::PerfCycleDetailsTab { forward: true });
        assert_eq!(
            state.session_manager.selected().unwrap().session.performance.details_tab,
            PerfDetailsTab::RebuildStats,
        );
    }

    #[test]
    fn cycle_backward_wraps_to_timeline_events() {
        let mut state = make_state_in_performance_details();
        update(&mut state, Message::PerfCycleDetailsTab { forward: false });
        assert_eq!(
            state.session_manager.selected().unwrap().session.performance.details_tab,
            PerfDetailsTab::TimelineEvents,
        );
    }

    #[test]
    fn cycle_is_noop_when_frame_chart_focused() {
        let mut state = make_state_in_performance_details();
        if let Some(h) = state.session_manager.selected_mut() {
            h.session.performance.focused_section = PerfSection::FrameChart;
        }
        update(&mut state, Message::PerfCycleDetailsTab { forward: true });
        assert_eq!(
            state.session_manager.selected().unwrap().session.performance.details_tab,
            PerfDetailsTab::FrameAnalysis,
        );
    }

    #[test]
    fn focus_details_tab_sets_active_tab() {
        let mut state = make_state_in_performance_details();
        update(
            &mut state,
            Message::PerfFocusDetailsTab(PerfDetailsTab::TimelineEvents),
        );
        assert_eq!(
            state.session_manager.selected().unwrap().session.performance.details_tab,
            PerfDetailsTab::TimelineEvents,
        );
    }
}
```

#### `keys.rs` routing

In the `if in_performance { match key { ... } }` block (around line 487), add new arms **inside** the existing block, after the Home/End arms (so `Tab/Shift+Tab` continues to take precedence — `]` / `[` are only reachable when Tab/Shift+Tab aren't pressed):

```rust
// ── Details tab cycling (Phase 2) ─────────────────────────────────
// Only active when the Details section is focused.
InputKey::Char(']') => {
    let in_details = state
        .session_manager
        .selected()
        .is_some_and(|h| h.session.performance.focused_section == PerfSection::Details);
    if in_details {
        return Some(Message::PerfCycleDetailsTab { forward: true });
    }
}
InputKey::Char('[') => {
    let in_details = state
        .session_manager
        .selected()
        .is_some_and(|h| h.session.performance.focused_section == PerfSection::Details);
    if in_details {
        return Some(Message::PerfCycleDetailsTab { forward: false });
    }
}
```

Note: the `]` and `[` characters fall through to the outer match when the user is on the Performance panel but `focused_section == FrameChart`. The outer match has no binding for these keys, so they no-op silently. This matches the design intent ("`]/[` only cycle when DetailsTab focused").

### Acceptance Criteria

1. `crates/fdemon-app/src/handler/devtools/performance.rs` no longer exists; the directory module replaces it.
2. Every call site that previously called `devtools::performance::handle_*` continues to compile (re-exports in `performance/mod.rs` preserve the public path).
3. `cargo check --workspace --all-targets` is green.
4. `cargo test --workspace` passes — all previously passing tests still pass and the new handler tests in `details.rs` pass.
5. With `focused_section == Details`, pressing `]` updates `performance.details_tab` to the next variant; `[` to the previous. Both wrap.
6. With `focused_section == FrameChart`, pressing `]` or `[` is a no-op (the message is not emitted; the handler also early-returns defensively).
7. `Tab/Shift+Tab` cycles `focused_section` between `FrameChart` and `Details` (verified by an integration test in `frame.rs` test module — adapt the existing `test_tab_cycles_to_details` if it exists, or add one).
8. `Message::PerfCycleDetailsTab` and `Message::PerfFocusDetailsTab` have dispatch arms in `update.rs`.
9. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` are green.

### Testing

In addition to the inline tests in `details.rs` shown above, add an integration test in `performance/frame.rs` (or move it to `mod.rs`):

```rust
#[test]
fn tab_now_cycles_to_details_in_phase_2() {
    let mut state = make_state_in_performance_panel().0;
    // Initial state: focused_section == FrameChart (default)
    assert_eq!(
        state.session_manager.selected().unwrap().session.performance.focused_section,
        PerfSection::FrameChart,
    );
    // Tab should now flip to Details (Phase 1 was a no-op).
    dispatch(&mut state, Message::Key(crate::input_key::InputKey::Tab));
    assert_eq!(
        state.session_manager.selected().unwrap().session.performance.focused_section,
        PerfSection::Details,
    );
    // Tab again returns to FrameChart.
    dispatch(&mut state, Message::Key(crate::input_key::InputKey::Tab));
    assert_eq!(
        state.session_manager.selected().unwrap().session.performance.focused_section,
        PerfSection::FrameChart,
    );
}
```

Add keys.rs tests near the existing `make_state_in_performance_panel` helper:

```rust
#[test]
fn bracket_close_when_details_focused_emits_cycle_forward() {
    let mut state = make_state_in_performance_panel();
    if let Some(h) = state.session_manager.selected_mut() {
        h.session.performance.focused_section = PerfSection::Details;
    }
    let msg = handle_key_devtools(&state, InputKey::Char(']'));
    assert!(matches!(msg, Some(Message::PerfCycleDetailsTab { forward: true })));
}

#[test]
fn bracket_close_when_frame_chart_focused_is_noop() {
    let state = make_state_in_performance_panel();
    // focused_section defaults to FrameChart
    let msg = handle_key_devtools(&state, InputKey::Char(']'));
    // Falls through to the outer match, which has no binding for ']' — None.
    assert!(msg.is_none());
}
```

### Notes

- **Behaviour preservation**: every test in the current `performance.rs` test module must continue to pass after the file move. Run `cargo test -p fdemon-app handler::devtools::performance` before merging.
- **Why the no-op guard on `handle_perf_cycle_details_tab`?** The keys.rs guard ensures `]` / `[` only emit when `focused_section == Details`, but `Message::PerfCycleDetailsTab` is part of the public message bus — a future mouse-click path or scripted test might emit it without the keyboard guard. The defensive `if focused_section != Details { return UpdateResult::none(); }` keeps the invariant centralized.
- **Mouse handler**: `crates/fdemon-app/src/handler/mouse/devtools.rs` references `PerfScroll*` / `PerfPage*` but does NOT need editing for Phase 2 — mouse-click on the tab strip is a Phase 3 enhancement. The handler is untouched.
- **scroll_helpers.rs unchanged**: T03 does not touch `crates/fdemon-app/src/handler/devtools/scroll_helpers.rs` — the helpers (`clamp_chart_scroll`, `ScrollDir`) are shared between performance and memory and stay where they are.
- **Test module organisation**: the current `performance.rs` has ~600 lines of tests mixed with ~400 lines of handler code. After splitting:
  - frame-related tests (frame selection, scroll, page, jump, section focus) → `frame.rs` test module.
  - Phase 2 cycle/focus tests → `details.rs` test module.
  - Any cross-cutting tests (e.g. the `keys.rs` ↔ handler integration test for tab cycling) can live in either; place in `frame.rs` if they test frame-side state, in `details.rs` if they test details-side state.
- **`PerfSection::Details` arm in scroll/page/jump handlers**: stays a no-op in Phase 2. Phase 3 will replace the no-op with per-tab scroll dispatch (Rebuild Stats / Timeline Events). Do not add scroll logic for Frame Analysis content in Phase 2 — the content is short and fits without scrolling.

---

## Completion Summary

(Filled in by implementor after work completes.)
