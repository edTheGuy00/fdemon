## Task: Resolve the `PerfSection::DetailsTab` keypress trap + rename to disambiguate

**Objective:** Fix M4 — pressing Tab on the Performance panel moves focus to `PerfSection::DetailsTab`, but the renderer doesn't visualize the change and all subsequent j/k/PgUp/PgDn/Home/End silently no-op. The user sees scrolling break. Choose between Option A (collapse the Tab cycle) and Option B (render a visible Phase-2 stub) and document the choice. Also rename `PerfSection::DetailsTab` → `PerfSection::Details` (m11) to disambiguate from the unrelated `state::DetailsTab` used by the Inspector subsystem.

**Depends on:** None (Wave 1)

**Agent:** implementor

**Estimated Time:** 1.5–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/session/performance.rs` — rename `PerfSection::DetailsTab` → `PerfSection::Details` and adjust `next()` / `prev()` per the chosen option.
- `crates/fdemon-app/src/handler/devtools/performance.rs` — update the four no-op match arms (lines 111, 150, 180, 203) for the rename; if Option B is chosen, the section-focus handler may need to surface focus changes that drive the new visible render.
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` — if Option B is chosen, add a visible Details placeholder render path; rename references regardless.
- `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` — add a regression test verifying Tab on Performance does not silently disable scroll keys.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` — confirm the *Inspector* `DetailsTab` is a different type, so the rename does not introduce a new collision elsewhere.
- `crates/fdemon-app/src/handler/devtools/inspector/` — naming context for the Inspector's DetailsTab (do not modify).

### Background

`PerfSection` is a 2-variant enum that Phase 1 introduced as a forward-compat anchor:

```rust
pub enum PerfSection {
    FrameChart,        // current single section
    DetailsTab,        // Phase 2 anchor — no content yet
}
```

`next()` and `prev()` cycle between the two. The handler's scroll/page/jump bodies match on `focused_section`:

```rust
match handle.session.performance.focused_section {
    PerfSection::FrameChart => /* real scroll */,
    PerfSection::DetailsTab => { /* no-op */ }
}
```

The renderer only checks `focused_section == FrameChart` for border-colour decisions; `DetailsTab` is invisible. Result: pressing Tab once moves focus into `DetailsTab` → arrow keys go silent → user thinks scrolling broke. The footer doesn't advertise Tab, so the trap is also undiscoverable.

Three reviewers flagged this. The risks reviewer specifically called it "premature reservation" and recommended dropping the variant under YAGNI; the logic reviewer agreed that the keypress trap is unacceptable. **T03 must pick a path and commit to it.**

### Decision: Option A vs Option B

The implementor of this task chooses one of the two options below. Both options satisfy the acceptance criteria. Whichever is chosen, document the choice and rationale in the Completion Summary.

#### Option A — YAGNI (Tab no-ops until Phase 2 has real content)

Keep `PerfSection` as a 2-variant enum (renamed `FrameChart` + `Details`), but change `next()` and `prev()` to **return `FrameChart` unconditionally**. Tab becomes a visible no-op. The variant is reserved for Phase 2 but unreachable via Tab today.

Pros: smallest diff; no render changes; no footer changes; no docstring updates beyond the rename. Phase 2 just re-introduces the cycle logic when content lands.

Cons: keeps a dead-but-reachable variant in the enum if Phase 2 ever directly assigns to it (it shouldn't, but the foot-gun remains).

Implementation:

```rust
impl PerfSection {
    pub fn next(self) -> Self {
        // Phase 2 will reintroduce cycling when Details has real content.
        // For Phase 1: Tab is a visible no-op.
        PerfSection::FrameChart
    }
    pub fn prev(self) -> Self {
        self.next()
    }
}
```

The handler's no-op match arms become unreachable in practice but keep them with `#[allow(unreachable_patterns)]` or, cleaner, drop them since the match will compile without them (the compiler may complain about non-exhaustive matches; if so, convert to `if let` checks against `FrameChart` only or keep the arm as a guard against accidental future assignment).

#### Option B — Visible Phase-2 stub

Keep the Tab cycle. Render a "Details (Phase 2)" placeholder pane when `focused_section == Details`. Update the Performance footer hint to advertise Tab.

Pros: Tab feels alive; users see what's coming; better discoverability of the future feature.

Cons: bigger diff; adds a render path that exists only to render a placeholder; the footer hint string lives in `widgets/devtools/mod.rs` (which T04 of Wave 2 also touches — sequence accordingly).

Implementation outline:

1. In `widgets/devtools/performance/mod.rs::render_impl`, branch on `focused_section`:
   ```rust
   match performance.focused_section {
       PerfSection::FrameChart => {
           // existing frame chart fills inner area
       }
       PerfSection::Details => {
           self.render_details_placeholder(area, buf);
       }
   }
   ```

2. `render_details_placeholder` renders a centred Paragraph: "Details — Phase 2 (Frame Analysis · Rebuild Stats · Timeline Events)" with a subdued style. Use the exact style class that "Memory panel placeholder" used during Phase 1's T01 if it's still around, or roll a fresh `Style::default().add_modifier(Modifier::DIM)`.

3. Update the Performance footer hint string in `widgets/devtools/mod.rs` (line ~374) to include `[Tab] Details`. **Note:** this change overlaps with T04's footer update (m6). T04 will land after T03; signal in the Completion Summary that the footer was already adjusted so T04 doesn't double-edit.

#### Recommendation

Option A is the simpler path and most reviewers leaned that way. **The implementor is free to choose either.** Document the choice in the Completion Summary with one sentence of rationale.

### Details

#### 1. Rename `PerfSection::DetailsTab` → `PerfSection::Details` (m11) — applies to BOTH options

The name collides with `state::DetailsTab` used by the Inspector subsystem (`widgets/devtools/inspector/details/`). The new name `PerfSection::Details` is unique within the project.

**Required edits:**

- `crates/fdemon-app/src/session/performance.rs:22` — rename the variant and update the doc comment immediately above it (line 20–21).
- `crates/fdemon-app/src/handler/devtools/performance.rs` — update every match arm referring to `PerfSection::DetailsTab` (lines 111, 150, 180, 203; rg for `DetailsTab` will surface them).
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` — update any reference (Option B adds new ones; Option A may have none).
- Any tests in `crates/fdemon-app/src/handler/devtools/performance.rs` and `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` referencing `DetailsTab`.

Run `rg "PerfSection::DetailsTab" -l` to enumerate. The rename is mechanical — no behaviour change.

#### 2. Apply the chosen option (A or B)

See "Decision" section above.

#### 3. Add a regression test

Regardless of option, add `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs::performance_tab_after_tab_does_not_break_scroll_keys`:

1. Set up a `PerformanceState` with several `FrameTiming` entries pushed.
2. Cycle Tab once (`focused_section = PerfSection::FrameChart.next()`).
3. Dispatch `Message::PerfScrollDown`.
4. Assert one of:
   - **Option A**: `focused_section` is back to `FrameChart` (Tab no-oped) AND `frame_chart_scroll_offset > 0` (scroll worked).
   - **Option B**: `focused_section == Details` AND the rendered output now contains "Phase 2" or similar identifying text from the placeholder.

Whichever option is taken, the test asserts the trap is gone: either Tab visibly did nothing and scroll still works, or Tab moved to a visible state and the user can navigate back.

#### 4. Quality gate

`cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`

### Acceptance Criteria

- [ ] `cargo check`, `cargo test`, `cargo clippy` all green.
- [ ] No reference to `PerfSection::DetailsTab` remains in the codebase (rename complete).
- [ ] If **Option A** chosen: `PerfSection::next()` and `prev()` return `FrameChart` for both input variants. Tab visibly no-ops.
- [ ] If **Option B** chosen: focusing on `PerfSection::Details` produces a visible placeholder. Performance footer hint advertises `Tab`. Coordination note added to Completion Summary for T04.
- [ ] `performance_tab_after_tab_does_not_break_scroll_keys` test passes.
- [ ] Completion Summary explicitly names the chosen option and gives one sentence of rationale.

### Module Structure

No new modules. The chosen option's render path (if Option B) lives inside the existing `widgets/devtools/performance/mod.rs` as a private `render_details_placeholder` method on `PerformancePanel`. Option A adds no new code.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Option Chosen

**Option A — YAGNI.** `PerfSection::next()` and `prev()` both return `FrameChart` unconditionally, making Tab a visible no-op; the `Details` variant is reserved for Phase 2 but unreachable via keyboard navigation. This is the smallest possible diff and avoids a render-path that exists only to show a placeholder.

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/performance.rs` | Renamed `DetailsTab` → `Details`; updated `next()` and `prev()` to always return `FrameChart` (Option A); updated tests to match new no-op semantics |
| `crates/fdemon-app/src/handler/devtools/performance.rs` | Renamed all four `PerfSection::DetailsTab` match arms to `PerfSection::Details`; updated doc comments; rewrote Tab/Shift+Tab tests to assert no-op; updated integration tests referencing the variant |
| `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` | Added `performance_tab_after_tab_does_not_break_scroll_keys` regression test |

### Notable Decisions/Tradeoffs

1. **Option A over Option B**: The three reviewers who flagged this and the "risks reviewer" explicitly recommended YAGNI. Option B would add a render path purely for a placeholder — overhead with no real user value until Phase 2 content arrives. Phase 2 simply re-introduces the cycle in `next()`/`prev()` when real content lands.

2. **`Details` match arms kept for exhaustiveness**: Even though `PerfSection::Details` is unreachable via Tab (Option A), the match arms in the four scroll/page/jump handlers are kept so the compiler continues to enforce exhaustiveness — any future direct assignment to `Details` (e.g., from a mouse click or message) will be handled gracefully (no-op) rather than panicking.

3. **Regression test placement**: The regression test (`performance_tab_after_tab_does_not_break_scroll_keys`) is placed in `fdemon-tui/src/widgets/devtools/performance/tests.rs` as the task requires. It tests at the state level (calling `next()` directly and simulating a scroll offset increment) since the TUI test file does not have full message dispatch infrastructure. The handler-level equivalent (`tab_is_noop_in_phase_1`) is in `fdemon-app`.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test --workspace` — Passed (all 5,800+ tests, zero failures)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
- `cargo test -p fdemon-tui performance_tab_after_tab_does_not_break_scroll_keys` — Passed (new test)
- `cargo test -p fdemon-app tab_is_noop_in_phase_1` — Passed
- `cargo test -p fdemon-app perf_section` — Passed (3 tests: `next_is_noop_in_phase_1`, `prev_is_noop_in_phase_1`, `default_is_frame_chart`)

### Risks/Limitations

1. **Dead variant**: `PerfSection::Details` cannot be reached via Tab in Phase 1. Code that directly assigns `focused_section = PerfSection::Details` (e.g. a future mouse handler or test setup) would enter the no-op match arms silently. This is the intended behavior — consistent with Option A's contract — and is documented in the variant's doc comment.

2. **T04 coordination**: Task T04 (doc and annotation cleanup) may update footer hint strings. No footer was changed in this task (Option A required no footer update), so there is no conflict with T04.
