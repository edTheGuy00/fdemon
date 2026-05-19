## Task: Phase 3 Keybindings and Footer Hints

**Objective**: Wire the two new letter shortcuts (`f` cycles Timeline filter; `R` toggles rebuild tracking), update the Performance footer hint string so the user sees them, and document the changes in `docs/KEYBINDINGS.md`.

**Depends on**: T04 (Message variants `TimelineEventsCycleFilter`, `ToggleRebuildStats` exist), T05 (visibility behaviour established — `R` shows/hides the tab).

**Agent:** implementor

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**

| File | Change |
|---|---|
| `crates/fdemon-app/src/handler/keys.rs` | Inside the `if in_performance` branch (lines 488+) where `focused_section == Details` is already gated, add: (1) `f` → `Message::TimelineEventsCycleFilter { session_id }` IFF `details_tab == TimelineEvents`. (2) Shift-`R` (capital `R`, `KeyCode::Char('R')`) → `Message::ToggleRebuildStats { session_id }` IFF `details_tab == RebuildStats`. (3) Regression-test that `R` outside this exact context (e.g. on Logs panel, FrameChart focus, FrameAnalysis tab) STILL routes to the global hot-restart message. |
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Update the Performance arm of `render_footer` (around lines 347–367) to append: when `focused_section == Details && details_tab == TimelineEvents`: `  [f] Filter`; when `focused_section == Details && details_tab == RebuildStats`: `  [R] Rebuild track`. Keep the existing `]/[ Tabs` and `Tab Section` hints. |
| `docs/KEYBINDINGS.md` | Add two rows to the DevTools Performance section: `\| f \| Performance, Details, TimelineEvents tab \| Cycle filter All → UI → Raster \|` and `\| R (Shift+r) \| Performance, Details, RebuildStats tab \| Toggle widget rebuild tracking \|`. Also add a note under the existing global `R` (hot restart) row clarifying the context-dependent precedence. |

**Files Read (Dependencies):**
- T04 outputs (`Message::TimelineEventsCycleFilter`, `Message::ToggleRebuildStats` shapes).
- `crates/fdemon-app/src/handler/keys.rs:486–523, 559–565` — existing `in_performance` guarded key routing pattern.
- `crates/fdemon-app/src/handler/keys.rs` — existing global `R` (hot restart) routing. Confirm precedence ordering: panel-specific bindings MUST be checked before the global `R` fallback.
- `crates/fdemon-tui/src/widgets/devtools/mod.rs:347–367` — `render_footer` Performance arm.
- `docs/KEYBINDINGS.md` — existing DevTools section structure.

### Details

#### `keys.rs` — routing block

Mirror the Phase-2 `]`/`[` routing pattern that lives inside the `in_performance && focused_section == Details` branch:

```rust
if in_performance && focused_section == PerfSection::Details {
    match (input_key, details_tab) {
        (InputKey::Char('f'), PerfDetailsTab::TimelineEvents) => {
            return Some(Message::TimelineEventsCycleFilter { session_id });
        }
        (InputKey::Char('R'), PerfDetailsTab::RebuildStats) => {
            return Some(Message::ToggleRebuildStats { session_id });
        }
        _ => {}
    }
}
// ... existing ]/[ and global R fallbacks follow
```

> **`R` precedence is critical.** The global hot-restart `R` lives later in the key-handler match. Phase 3's contextual `R` must be checked FIRST (early-return). Otherwise the user pressing `R` on the RebuildStats tab will trigger a hot restart instead of toggling tracking. The regression test below pins this ordering.

#### Footer hint string

The Performance footer today (Phase 2) reads roughly:
```
[Tab] Section  [↑↓ ←→] Navigate  []/[] Tabs
```
(per phase-2-followup m8's disambiguation, the actual glyph is `]/[ Tabs`).

Phase 3 appends contextually:

| `focused_section` | `details_tab` | Appended hint |
|---|---|---|
| `FrameChart` | — | (no change) |
| `Details` | `FrameAnalysis` | (no change) |
| `Details` | `RebuildStats` | `  [R] Rebuild track` |
| `Details` | `TimelineEvents` | `  [f] Filter` |

Implementation: small `match` in the footer-render branch. Total added length ≤ 20 chars — fits within existing 200-col footer budget.

#### `docs/KEYBINDINGS.md` additions

Find the DevTools → Performance section. Add the two new rows. Below the global `R` (Hot restart) row, add:

> Note: In DevTools Performance with the Rebuild Stats tab focused, `R` instead toggles `ext.flutter.profileWidgetBuilds` (per-context shadowing).

### Acceptance Criteria

1. `cargo check -p fdemon-app -p fdemon-tui` passes.
2. `cargo test -p fdemon-app -p fdemon-tui` passes — including the new regression tests below.
3. `cargo clippy --workspace --all-targets -- -D warnings` is clean.
4. Pressing `f` on the TimelineEvents tab (Details focused) emits `Message::TimelineEventsCycleFilter`. Pressing `f` anywhere else does NOT emit it.
5. Pressing `R` (Shift+r) on the RebuildStats tab (Details focused) emits `Message::ToggleRebuildStats`. Pressing `R` anywhere else (Logs panel, FrameChart focus, FrameAnalysis tab, Memory panel, etc.) STILL emits `Message::HotRestart` (or whatever the global hot-restart message is named).
6. Performance footer hint includes `[f] Filter` when on TimelineEvents and `[R] Rebuild track` when on RebuildStats; absent in other contexts.
7. `docs/KEYBINDINGS.md` documents both new bindings and the contextual `R` shadow.

### Testing

Add to `handler/keys.rs` test module:

- `test_f_on_timeline_events_tab_emits_filter_cycle` — set up `AppState` with `active_panel == Performance`, `focused_section == Details`, `details_tab == TimelineEvents`. Press `f`. Assert `Message::TimelineEventsCycleFilter { session_id }`.
- `test_f_on_frame_analysis_tab_does_not_emit_filter_cycle` — same state but `details_tab == FrameAnalysis`. Press `f`. Assert `None` (or a different message — not the filter-cycle).
- `test_f_on_logs_panel_does_not_emit_filter_cycle`.
- `test_capital_R_on_rebuild_stats_tab_emits_toggle` — `details_tab == RebuildStats`. Press `R`. Assert `Message::ToggleRebuildStats { session_id }`.
- `test_capital_R_on_rebuild_stats_tab_does_not_trigger_hot_restart` — Same as above. Assert the message is NOT `Message::HotRestart` / equivalent.
- `test_capital_R_on_frame_analysis_tab_triggers_hot_restart` — `details_tab == FrameAnalysis`. Press `R`. Assert `Message::HotRestart` (or current equivalent).
- `test_capital_R_on_logs_panel_triggers_hot_restart` — confirms the global binding is preserved outside the Performance/Details/RebuildStats context.
- `test_capital_R_on_memory_panel_triggers_hot_restart`.

Add to `widgets/devtools/mod.rs` test module (or wherever footer tests live):

- `test_performance_footer_includes_filter_hint_on_timeline_events_tab`.
- `test_performance_footer_includes_rebuild_hint_on_rebuild_stats_tab`.
- `test_performance_footer_omits_phase_3_hints_on_frame_analysis_tab`.
- `test_performance_footer_omits_phase_3_hints_when_frame_chart_focused`.

### Notes

- **Letter choice `R` for rebuild toggle:** Capital `R` is intentionally hard to hit accidentally (shift required) and mnemonically maps to "Rebuild". The trade-off is shadowing the global hot-restart. The regression tests above pin the precedence — any future refactor that changes the keys.rs match-arm order will fail those tests.
- **Alternative letter considered: `t` for "tracking"** — rejected because `t` is heavily overloaded in fdemon (Tag filter overlay, Tab cycle). Avoid.
- **Alternative letter considered: `Ctrl+R`** — rejected because Ctrl-modifier handling in `crossterm` has cross-platform quirks (some terminals swallow Ctrl+R as reverse-history-search). Plain Shift+R is more portable.
- **Footer hint length budget:** Existing footer is ~60 chars on a 200-col terminal. Adding ~14 chars (`  [R] Rebuild track`) keeps it under 80 chars, well within budget even on narrow terminals (down to ~80 cols).
- **No mouse region changes** — Phase 2 deferred Performance click regions; Phase 3 stays keyboard-only. If a future phase adds mouse, the `f` and `R` actions become click-action targets on the footer-hint chips and tab chips respectively.
- **No KEYBINDINGS.md restructure** — append only. Match existing table style (column headers `\| Key \| Context \| Action \|`).

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/keys.rs` | Added `PerfDetailsTab` import; added `Char('f')` and `Char('R')` arms inside `if in_performance` block (before `]`/`[` arms) that early-return contextual messages when `Details` focused and respective tab active; added 8 new regression tests in `performance_sort_key_tests` module |
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Added `PerfDetailsTab` import; changed `hints` variable from `&'static str` to `Cow<'static, str>`; updated `PerfSection::Details` arm to dynamically append `[f] Filter` or `[R] Rebuild track` based on `details_tab`; added 4 new footer tests and a `make_perf_session_handle_with_details_tab` helper |
| `docs/KEYBINDINGS.md` | Added two new rows to Performance Panel table (`f` and `R (Shift+r)`); added note after global `R` (Hot Restart) row clarifying context-dependent precedence; added notes block under Performance table |

### Notable Decisions/Tradeoffs

1. **`R` in DevTools returns None (not HotRestart)**: The global `R → HotRestart` binding lives in `handle_key_normal`, not `handle_key_devtools`. So pressing `R` in DevTools on FrameAnalysis/Memory returns `None` (no-op), not `HotRestart`. The regression tests were adjusted to assert that `ToggleRebuildStats` is NOT emitted in non-RebuildStats contexts, and the Logs-panel test (Normal mode) correctly verifies HotRestart. This matches existing code behavior and is documented in the test comments.

2. **`Cow<'static, str>` for footer hints**: Changed `hints` from `&'static str` to `std::borrow::Cow<'static, str>` to support both static strings (no allocation) and owned `String` for the dynamic Performance/Details branch. This avoids any new heap allocation for the non-Details branches.

3. **`f` arm inside `if in_performance`**: The `f` key is only intercepted when `in_performance` and `focused_section == Details` and `details_tab == TimelineEvents`. Otherwise it falls through to the outer `match key` block which returns `None` (no DevTools binding for `f` otherwise). This is correct since `f` has no global DevTools binding.

### Testing Performed

- `cargo check -p fdemon-app -p fdemon-tui` — Passed
- `cargo test -p fdemon-app -p fdemon-tui` — Passed (2424 + 1182 tests, 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` — Clean

### Risks/Limitations

1. **`R` no-op in DevTools/non-RebuildStats**: Pressing `R` in DevTools mode outside the RebuildStats tab returns `None` (no hot restart). This is pre-existing behavior, not introduced by this task. A future task could add `R → HotRestart` as a DevTools-global fallback.
