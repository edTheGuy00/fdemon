## Task: Performance Details State + Message Foundation

**Objective**: Add the state model and message variants needed for the Phase 2 Performance Details pane, plus fix the Phase 1 no-op `PerfSection::next/prev` so `Tab`/`Shift+Tab` actually cycles between `FrameChart` and `Details`. No handler or rendering code lands here — this task is the data-model scaffolding the rest of Phase 2 builds on.

**Depends on**: None

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/state.rs` — add `PerfDetailsTab` enum (variants: `FrameAnalysis` (default), `RebuildStats`, `TimelineEvents`); derive `Debug, Clone, Copy, PartialEq, Eq, Default`.
- `crates/fdemon-app/src/session/performance.rs`:
  - Fix `PerfSection::next` and `PerfSection::prev` to cycle properly (`FrameChart ↔ Details`). Remove the Phase 1 "visible no-op" comments; the variant cycling is now real.
  - Add `pub details_tab: PerfDetailsTab` to `PerformanceState` (default `FrameAnalysis`).
  - Add `pub details_pane_visible_height: Cell<usize>` render-hint Cell (default `0`). Annotate with the standard `// EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md Principle 3` comment.
  - Add `pub display_refresh_rate: f64` (default `60.0`). Doc-comment that Phase 2 hard-codes 60.0 and Phase 3 may parse `Display.Refresh` events.
  - Extend `PerformanceState::default()` to initialize the new fields.
- `crates/fdemon-app/src/message.rs`:
  - Add `Message::PerfCycleDetailsTab { forward: bool }` — emitted by `]` (forward = true) and `[` (forward = false).
  - Add `Message::PerfFocusDetailsTab(PerfDetailsTab)` — reserved for mouse-click region forwarding (Phase 3 wires this up; T02 only defines the variant so Phase 3 doesn't need a second message-bus migration).
  - Import path: `use crate::state::PerfDetailsTab;` near other imports.
- `crates/fdemon-app/src/session/mod.rs` — re-export `PerfDetailsTab` from `session::` only if the existing pattern re-exports state-layer enums through `session` (verify by reading the file before editing; the inspector's `DetailsTab` is **not** re-exported through `session::`, so `PerfDetailsTab` likely belongs in `state::` alone).

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` (existing `DetailsTab` enum — model after this; same derive set; same default-variant ordering).
- `crates/fdemon-app/src/session/performance.rs` (existing `PerformanceState` shape; existing `frame_chart_visible_width` Cell as a reference for the EXCEPTION annotation).

### Details

#### `PerfDetailsTab` enum (state.rs)

Place alongside the existing inspector `DetailsTab` enum at `crates/fdemon-app/src/state.rs:170`:

```rust
/// Which tab is active within the Performance panel's Details pane.
///
/// Phase 2 populates `FrameAnalysis`; `RebuildStats` and `TimelineEvents`
/// render "Coming soon" stubs until Phase 3 adds the underlying VM Service
/// flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PerfDetailsTab {
    /// Per-frame phase breakdown + refresh-rate-aware hints. Default.
    #[default]
    FrameAnalysis,
    /// Widget rebuild counts per frame (Phase 3 stub in Phase 2).
    RebuildStats,
    /// UI / Raster thread timeline events (Phase 3 stub in Phase 2).
    TimelineEvents,
}

impl PerfDetailsTab {
    /// Next tab in display order (wraps from TimelineEvents → FrameAnalysis).
    pub fn next(self) -> Self {
        match self {
            PerfDetailsTab::FrameAnalysis => PerfDetailsTab::RebuildStats,
            PerfDetailsTab::RebuildStats => PerfDetailsTab::TimelineEvents,
            PerfDetailsTab::TimelineEvents => PerfDetailsTab::FrameAnalysis,
        }
    }

    /// Previous tab in display order (wraps from FrameAnalysis → TimelineEvents).
    pub fn prev(self) -> Self {
        match self {
            PerfDetailsTab::FrameAnalysis => PerfDetailsTab::TimelineEvents,
            PerfDetailsTab::RebuildStats => PerfDetailsTab::FrameAnalysis,
            PerfDetailsTab::TimelineEvents => PerfDetailsTab::RebuildStats,
        }
    }
}
```

Unlike the inspector's `DetailsTab::visible_tabs()` (which conditionally hides tabs based on widget type), all three Performance details tabs are **always visible** in Phase 2 — the conditional hiding logic the plan describes for Rebuild Stats (extension-gated) is deferred to Phase 3. T04 renders all three tab labels unconditionally; T05 / Phase 3 may revisit visibility.

#### `PerfSection::next/prev` fix (session/performance.rs:31–43)

Replace the no-op implementations with proper cycling:

```rust
impl PerfSection {
    /// Return the next section in Tab order — wraps `FrameChart → Details → FrameChart`.
    pub fn next(self) -> Self {
        match self {
            PerfSection::FrameChart => PerfSection::Details,
            PerfSection::Details => PerfSection::FrameChart,
        }
    }

    /// Return the previous section in Tab order — wraps the other way.
    pub fn prev(self) -> Self {
        match self {
            PerfSection::FrameChart => PerfSection::Details,
            PerfSection::Details => PerfSection::FrameChart,
        }
    }
}
```

The Phase 1 doc-comments warning about "visible no-op" must be removed — Tab is fully functional from Phase 2 onward.

Update the matching tests (`perf_section_next_is_noop_in_phase_1`, `perf_section_prev_is_noop_in_phase_1`) to assert the new cycling behaviour. Rename:

- `perf_section_next_is_noop_in_phase_1` → `perf_section_next_cycles_between_frame_chart_and_details`
- `perf_section_prev_is_noop_in_phase_1` → `perf_section_prev_cycles_between_frame_chart_and_details`

#### `PerformanceState` new fields (session/performance.rs)

```rust
pub struct PerformanceState {
    // ... existing fields stay ...

    /// Which tab is active within the Performance Details pane.
    ///
    /// Defaults to `PerfDetailsTab::FrameAnalysis`. Cycled by `]`/`[` when
    /// `focused_section == PerfSection::Details`. The renderer reads this
    /// value to dispatch to the correct tab module.
    pub details_tab: PerfDetailsTab,

    /// Render-hint: visible height (in rows) of the Details pane content area
    /// from the last rendered frame.
    ///
    /// Defaults to `0`, signalling "not yet rendered — use fallback". Phase 3
    /// consumes this for Rebuild Stats / Timeline Events scrolling; Phase 2 sets
    /// it but does not read it.
    // EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md Principle 3 and
    // docs/REVIEW_FOCUS.md "Approved TEA Exception → Current usage".
    pub details_pane_visible_height: Cell<usize>,

    /// Refresh rate (Hz) used to compute the per-frame budget in
    /// [`fdemon_core::frame_hints::frame_hints`].
    ///
    /// Phase 2 hard-codes `60.0`. Phase 3 may parse the `Display.Refresh`
    /// Extension event stream to detect 90 / 120 Hz devices. A conservative
    /// 60 Hz default never reports a non-janky frame as janky.
    pub display_refresh_rate: f64,
}

impl Default for PerformanceState {
    fn default() -> Self {
        Self {
            // ... existing field defaults ...
            details_tab: PerfDetailsTab::default(),
            details_pane_visible_height: Cell::new(0),
            display_refresh_rate: 60.0,
        }
    }
}
```

The `PerfDetailsTab` import: add `use crate::state::PerfDetailsTab;` at the top of `session/performance.rs`.

#### Message variants (message.rs)

After the existing `Perf*` block around `crates/fdemon-app/src/message.rs:1155–1170`:

```rust
// --- Performance details pane (Phase 2) ---

/// Cycle the active tab in the Performance Details pane.
///
/// Emitted by `]` (forward = true) and `[` (forward = false) when
/// `PerformanceState::focused_section == PerfSection::Details`.
PerfCycleDetailsTab { forward: bool },

/// Focus a specific tab in the Performance Details pane.
///
/// Phase 2 only emits this from tests; Phase 3 wires up mouse-click
/// regions on the tab strip that emit this variant.
PerfFocusDetailsTab(PerfDetailsTab),
```

Import: `use crate::state::PerfDetailsTab;` near the top of the file.

#### `update.rs` dispatch (NOT in scope for T02)

T03 owns the dispatch arms for these new messages. T02 only adds the variants so the enum compiles in dependent crates. Adding unhandled variants will not produce a compiler error because `update.rs` uses `match message { ... }` with no catch-all (verify before merging — if it has a catch-all, T02 is safe; if it doesn't, T02 must either add stub `Message::PerfCycleDetailsTab { .. } => UpdateResult::none()` arms or coordinate with T03).

> **Check before writing**: read `crates/fdemon-app/src/handler/update.rs` around the existing `Message::PerfFocusSection` dispatch (~line 2336). If the outer `match` has a wildcard, T02 needs no change to `update.rs`. Otherwise, add `_ => UpdateResult::none()` arms for the two new variants in T02 and let T03 replace them with the real handlers.

### Acceptance Criteria

1. `PerfDetailsTab` enum exists in `state.rs`, derives `Debug, Clone, Copy, PartialEq, Eq, Default`, with `FrameAnalysis` as the default.
2. `PerfDetailsTab::next()` cycles `FrameAnalysis → RebuildStats → TimelineEvents → FrameAnalysis`. `prev()` cycles the other way. Both are pure (`Copy` self, no `&mut`).
3. `PerfSection::next()` returns `Details` when called on `FrameChart`, and vice versa — no longer a no-op.
4. `PerformanceState::default()` constructs with `details_tab == PerfDetailsTab::FrameAnalysis`, `details_pane_visible_height.get() == 0`, `display_refresh_rate == 60.0`.
5. `Message::PerfCycleDetailsTab` and `Message::PerfFocusDetailsTab` variants exist in `message.rs`.
6. `cargo check --workspace --all-targets` is green.
7. `cargo test --workspace` passes — the two renamed `perf_section_*_cycles_between_*` tests pass.
8. Existing tests do not regress (specifically the inspector `DetailsTab` tests, which share the file).

### Testing

Add the following to `crates/fdemon-app/src/session/performance.rs` test module:

```rust
#[test]
fn perf_section_next_cycles_between_frame_chart_and_details() {
    assert_eq!(PerfSection::FrameChart.next(), PerfSection::Details);
    assert_eq!(PerfSection::Details.next(), PerfSection::FrameChart);
}

#[test]
fn perf_section_prev_cycles_between_frame_chart_and_details() {
    assert_eq!(PerfSection::FrameChart.prev(), PerfSection::Details);
    assert_eq!(PerfSection::Details.prev(), PerfSection::FrameChart);
}

#[test]
fn performance_state_defaults_phase_2_fields() {
    let s = PerformanceState::default();
    assert_eq!(s.details_tab, PerfDetailsTab::FrameAnalysis);
    assert_eq!(s.details_pane_visible_height.get(), 0);
    assert_eq!(s.display_refresh_rate, 60.0);
}
```

Add the following to `crates/fdemon-app/src/state.rs` test module (near the existing `DetailsTab` tests):

```rust
#[test]
fn perf_details_tab_default_is_frame_analysis() {
    assert_eq!(PerfDetailsTab::default(), PerfDetailsTab::FrameAnalysis);
}

#[test]
fn perf_details_tab_next_wraps() {
    assert_eq!(PerfDetailsTab::FrameAnalysis.next(), PerfDetailsTab::RebuildStats);
    assert_eq!(PerfDetailsTab::RebuildStats.next(), PerfDetailsTab::TimelineEvents);
    assert_eq!(PerfDetailsTab::TimelineEvents.next(), PerfDetailsTab::FrameAnalysis);
}

#[test]
fn perf_details_tab_prev_wraps() {
    assert_eq!(PerfDetailsTab::FrameAnalysis.prev(), PerfDetailsTab::TimelineEvents);
    assert_eq!(PerfDetailsTab::TimelineEvents.prev(), PerfDetailsTab::RebuildStats);
    assert_eq!(PerfDetailsTab::RebuildStats.prev(), PerfDetailsTab::FrameAnalysis);
}
```

### Notes

- **DO NOT** modify any handler files in T02 — that's T03's job. T02 may add `_ => UpdateResult::none()` placeholder arms in `update.rs` ONLY if needed to keep the build green; T03 replaces them.
- **DO NOT** touch any widget files in T02 — that's T04's job. The fields exist in state but the renderer doesn't read them yet.
- **PerfSection::Details rename — REJECTED.** The variant stays named `Details` (decided in Phase 1-followup T03). Don't rename to `DetailsTab` or `DetailsPane` even though the plan's prose uses those words for the destination region.
- **Why a separate `display_refresh_rate` field?** Putting refresh rate on `PerformanceState` (not `DevToolsViewState`) keeps it per-session — different connected devices may have different refresh rates in a multi-session future. Phase 2 only ever writes the default `60.0`, but the field placement scales.
- **`details_pane_visible_height` is set in T04 / T05 but read only in Phase 3.** Phase 2's three details tabs render fixed-height content with no scrolling. Adding the cell now avoids a second state-shape migration when Phase 3 wires up scrolling.
- Module-level docstrings stay as-is — only the inline doc comments on the changed enum / fields need updating.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `PerfDetailsTab` enum (3 variants + `next`/`prev` impl) and 3 new tests |
| `crates/fdemon-app/src/session/performance.rs` | Added `use crate::state::PerfDetailsTab` import; fixed `PerfSection::next/prev` to cycle properly; added 3 new fields to `PerformanceState` + `Default`; renamed Phase 1 noop tests; added `performance_state_defaults_phase_2_fields` test |
| `crates/fdemon-app/src/message.rs` | Added `PerfDetailsTab` to import; added `PerfCycleDetailsTab { forward: bool }` and `PerfFocusDetailsTab(PerfDetailsTab)` variants |
| `crates/fdemon-app/src/handler/update.rs` | Added stub dispatch arms for both new message variants (T03 replaces with real handlers) |
| `crates/fdemon-app/src/handler/devtools/performance.rs` | Updated 3 tests that encoded Phase 1 no-op behavior to reflect Phase 2 cycling |
| `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` | Updated 1 test asserting Phase 1 no-op behavior to reflect Phase 2 cycling |

### Notable Decisions/Tradeoffs

1. **Stub arms in update.rs**: The match in `update.rs` has no catch-all wildcard, so both new `Message` variants required stub `=> UpdateResult::none()` arms. These are annotated with a comment that T03 will replace them with real handlers.
2. **Phase 1 test migration**: Six tests across three files (performance.rs handler, performance.rs session, performance/tests.rs TUI) encoded the Phase 1 no-op behavior. All were updated to assert the new Phase 2 cycling semantics rather than deleted, preserving test intent.
3. **`PerfDetailsTab` in `state::` only**: Confirmed that `DetailsTab` (inspector) is not re-exported through `session::`, so `PerfDetailsTab` stays in `state::` alone — not added to session/mod.rs re-exports.

### Testing Performed

- `cargo check --workspace --all-targets` - Passed (green)
- `cargo test --workspace` - Passed (all test suites pass, zero failures)
- `cargo fmt --all` - Applied, no compilation regressions

### Risks/Limitations

1. **Stub handlers**: `PerfCycleDetailsTab` and `PerfFocusDetailsTab` dispatch to `UpdateResult::none()` until T03 lands. Any test that emits these messages will see no state change — acceptable since no tests emit them yet except the new ones defined by this task.
