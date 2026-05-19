## Task: Consolidated minor cleanup — m1, m3, m5, m6, m7, m8, m12, m13

**Objective:** Bundle the remaining Phase 2 review minor findings into a single coherent edit. After Wave 1 lands its structural fixes, this task closes out the doc registry gap, stale comments, constant visibility workaround, footer hint UX issues, derivation comment math, 2-variant safety comment, and handler fallback alignment.

**Depends on:** 01 (must merge first — same-file overlap on `performance/mod.rs`)

**Agent:** implementor

**Estimated Time:** 1.5–2 hours

### Scope

**Files Modified (Write):**
- `docs/REVIEW_FOCUS.md` — register the two `PerformanceState::Cell<usize>` render-hint fields (m1).
- `crates/fdemon-app/src/handler/devtools/performance/frame.rs` — remove stale "Unreachable via Tab" comments (m3); align `handle_perf_jump_to_start` fallback with `handle_perf_page` (m13).
- `crates/fdemon-app/src/session/performance.rs` — add 2-variant safety comment near `PerfSection::next/prev` (m12).
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` — replace `const _: u16 = MIN_PHASE_BAR_WIDTH` workaround with `pub(super)` visibility (m5); correct or bump the `MIN_DUAL_PANE_HEIGHT` derivation comment (m7).
- `crates/fdemon-tui/src/widgets/devtools/mod.rs` — make Performance footer hint section-aware (m6); disambiguate `[]/[]` glyph (m8).
- `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` — IF m5 changes the import path for `MIN_PHASE_BAR_WIDTH`, update the import accordingly. No other edits to this file.

**Files Read (Dependencies):**
- T01 Completion Summary — confirm the `performance/mod.rs` callsite shape and which line ranges are stable to edit.

### Background

Phase 2 review surfaced a long tail of minor items spread across five files. Bundling them avoids five separate single-finding commits and lets the orchestrator track them as one logical unit. None has cross-dependency on the others; all are mechanical edits.

#### m1 — Missing `Cell<usize>` registry entries

`docs/REVIEW_FOCUS.md` "Approved TEA Exception → Current usage" lists Cell render-hint exceptions. Two `PerformanceState` fields are missing:

- `frame_chart_visible_width` (pre-existing gap — added in an earlier phase, never registered)
- `details_pane_visible_height` (new in Phase 2 T02; written by render in `performance/mod.rs:265-267`)

The doc's own policy at line 36 states: *"New `Cell`-based render-hint fields require explicit review and documentation here."* T07 (Phase 2 doc-maintainer task) did not address this.

#### m3 — Stale "Unreachable via Tab" comments

`crates/fdemon-app/src/handler/devtools/performance/frame.rs:164` and `:187` contain the comment:

```rust
PerfSection::Details => {
    // No-op in Phase 2. Unreachable via Tab; kept for exhaustiveness.
}
```

After Phase 2 T02 fixed `PerfSection::next/prev` cycling, the `Details` arm IS reachable via Tab — the comment now lies.

#### m5 — `const _` workaround for `MIN_PHASE_BAR_WIDTH`

`performance/mod.rs:358-361` contains:

```rust
// MIN_PHASE_BAR_WIDTH is consumed by `details::frame_analysis_tab` (T05).
// The child module can access this private constant because child modules
// may access private items of their ancestor modules in Rust.
const _: u16 = MIN_PHASE_BAR_WIDTH;
```

This suppresses an unused-constant warning because the constant is only referenced from the child submodule `details/frame_analysis_tab.rs`. Cleaner: declare the constant `pub(super)` so the child can import it explicitly. After the change, the child module should `use super::super::MIN_PHASE_BAR_WIDTH` (or whatever path resolves correctly given the module hierarchy).

#### m6 — Footer falsely advertises `[j/k] Scroll` on Details focus

`crates/fdemon-tui/src/widgets/devtools/mod.rs::render_footer` Performance arm emits a fixed hint string that includes `[j/k] Scroll`. When `focused_section == Details`, the `PerfScrollUp/Down/etc.` handlers in `frame.rs:90-94, 131-135, 163-165, 186-188` are explicit no-ops. The footer falsely promises functionality.

Mirror the pattern Memory panel uses (selection-aware hints): branch on `focused_section` and emit a hint set appropriate to the focused section. When Details is focused, drop `[j/k] Scroll` and `[←/→] Frames` (also a no-op on Details) and keep `[Tab] Section`, `]/[ Tabs`, `[b] Browser`, `[Esc] Logs`.

#### m7 — `MIN_DUAL_PANE_HEIGHT = 18` derivation comment math is wrong

`performance/mod.rs:53-58` documents:

```rust
/// Derivation: FrameChart requires ≥ `MIN_CHART_HEIGHT (4) + DETAIL_PANEL_HEIGHT (3) = 7`
/// rows internally. Details pane requires ≥ `MIN_DETAILS_HEIGHT (8)` rows. Inner area
/// is `area.height - 1` (footer) - 2 (chart block borders). So we need 10 inner
/// rows for the chart + 8 for details = 18 rows.
const MIN_DUAL_PANE_HEIGHT: u16 = 18;
```

The arithmetic `4 + 3 = 7` does not equal the comment's "10 inner rows for the chart" — the derivation is off. Either:

- **Option A (preferred):** Rewrite the comment to show the correct arithmetic that yields 18. Verify by checking the rendered chart-inner and details-inner at `usable.height = 18`.
- **Option B:** If the correct arithmetic yields a different threshold (e.g., 19), bump the constant value alongside the comment.

T04 should pick Option A unless the derivation forces a value change. Prefer comment correction over threshold change to avoid widening the diff.

#### m8 — Footer `[]/[] Tabs` is visually ambiguous

The footer string `[]/[] Tabs` reads as empty brackets. Disambiguate by either:

- Using `]/[ Tabs` (no surrounding brackets — the keys ARE the bracket characters), OR
- Using `[]` and `[[` notations explicitly, OR
- Using `<]> <[> Tabs` with angle bracket delimiters.

T04 should pick `]/[ Tabs` for consistency with the other footer entries that use `[Tab] Section` (where `[…]` denotes "the key inside is special"). Bracket keys are literal characters and don't need bracket delimiters.

#### m12 — `PerfSection::next/prev` assume 2 variants

`session/performance.rs:28-41` defines `next` and `prev` with identical bodies — correct only for n=2. A future 3rd variant (e.g., a Filters section) would silently make `next == prev`. Add a `///` doc comment on both methods warning that the bodies assume exactly 2 variants and must be rewritten if the enum grows.

A `#[test]` could enforce this at compile time using a const-eval trick (`const _: () = assert!(std::mem::variant_count::<PerfSection>() == 2);` — currently unstable), but pure documentation is the lower-risk option. Use a comment.

#### m13 — Handler fallback mismatch

`handler_perf_page` at `frame.rs:114-118`:

```rust
let visible = if visible_width == 0 { DEFAULT_PERF_PAGE_SIZE } else { visible_width };
```

`handle_perf_jump_to_start` at `frame.rs:155-160`:

```rust
let visible = handle.session.performance.frame_chart_visible_width.get().max(1);
```

The two fallbacks differ for pre-first-render keypresses: page uses 10, jump uses 1. Align both to `DEFAULT_PERF_PAGE_SIZE` for consistent behaviour.

### Details

#### 1. `docs/REVIEW_FOCUS.md` — register two Cell fields

Open the "Current usage" bulleted list under "Approved TEA Exception → Render-Hint Feedback". Add two new bullets, matching the existing style (modelled after the `MemoryState::memory_chart_visible_width` and `MemoryState::alloc_table_visible_height` entries from Phase 1-followup T04):

```markdown
- `PerformanceState::frame_chart_visible_width` — the renderer writes the visible bar count each frame; the chart-scroll, page, and jump handlers read it to clamp `frame_chart_scroll_offset` and size page-step navigation. Default 0 (safe fallback when no render has happened yet).
- `PerformanceState::details_pane_visible_height` — the renderer writes the inner details-pane height (excluding borders) each frame; Phase 3 Rebuild Stats and Timeline Events scroll handlers will read it. Default 0 (safe fallback when no render has happened yet; Phase 2 has no reader).
```

Insertion order: alphabetical within the `PerformanceState` group.

#### 2. `frame.rs` — remove stale comments

At `frame.rs:163-165` and `:186-188`, find both occurrences of:

```rust
PerfSection::Details => {
    // No-op in Phase 2. Unreachable via Tab; kept for exhaustiveness.
}
```

Replace the comment with a concise current-state version:

```rust
PerfSection::Details => {
    // No-op for scroll/jump. Details pane content (Phase 3 Rebuild Stats /
    // Timeline Events) will own its own scroll handlers.
}
```

#### 3. `frame.rs` — m13 fallback alignment

In `handle_perf_jump_to_start`, replace:

```rust
let visible = handle.session.performance.frame_chart_visible_width.get().max(1);
```

with the same pattern `handle_perf_page` uses:

```rust
let visible_width = handle.session.performance.frame_chart_visible_width.get();
let visible = if visible_width == 0 { DEFAULT_PERF_PAGE_SIZE } else { visible_width };
```

Audit `handle_perf_jump_to_end` for the same pattern and apply consistently if it exists. Add a regression test that verifies both `handle_perf_page` and `handle_perf_jump_to_start` produce the same scroll offset for a pre-first-render keypress with the same frame history.

#### 4. `session/performance.rs` — m12 PerfSection variant-count comment

Above `PerfSection::next()` and `PerfSection::prev()`, add:

```rust
/// # Caution: 2-variant assumption
///
/// This implementation assumes `PerfSection` has exactly 2 variants
/// (`FrameChart` and `Details`). The body returns the opposite variant
/// unconditionally — correct for n=2, silently wrong if a third variant
/// is added. If you add a variant, rewrite both `next` and `prev` to
/// cycle through all variants explicitly.
```

Apply to both methods (or use a single block-level comment above both).

#### 5. `performance/mod.rs` — m5 const visibility + m7 derivation comment

**m5:** Change

```rust
const MIN_PHASE_BAR_WIDTH: u16 = 40;
```

to

```rust
pub(super) const MIN_PHASE_BAR_WIDTH: u16 = 40;
```

Delete the `const _: u16 = MIN_PHASE_BAR_WIDTH;` line and its surrounding explanatory comment. Update the child module `details/frame_analysis_tab.rs` to import the constant explicitly:

```rust
use super::super::MIN_PHASE_BAR_WIDTH;
```

(Verify the path resolves; adjust if the actual module hierarchy is different.)

**m7:** Locate the `MIN_DUAL_PANE_HEIGHT` declaration at lines 53–58. Re-derive the correct arithmetic. The actual layout consumes:

- 1 row for the panel footer
- 2 rows for the chart block borders (top + bottom)
- Chart inner needs `MIN_CHART_HEIGHT (4) + DETAIL_PANEL_HEIGHT (3) = 7` rows
- 2 rows for the details block borders
- Details inner needs `MIN_DETAILS_HEIGHT (8)` rows

Total: `1 + 2 + 7 + 2 + 8 = 20` rows … which is GREATER than 18. Either:

- The current 18 is empirically correct because ratatui's `Constraint::Min` redistributes from `Constraint::Length` when space is tight, OR
- The constant should be bumped to 19 or 20.

Run the smoke test described in Phase 2 TASKS.md acceptance plan at `usable.height = 18` and visually verify the layout is acceptable. If acceptable, rewrite the comment to honestly describe the threshold ("18 rows is the empirically-tested minimum; below this, ratatui's Constraint::Min behaviour produces a chart inner < MIN_CHART_HEIGHT + DETAIL_PANEL_HEIGHT requirement"). If not acceptable, bump to the smallest value that satisfies all constraints exactly and update the comment.

#### 6. `widgets/devtools/mod.rs` — m6 + m8 footer

In `render_footer`'s Performance arm (around line 374-376), branch on `focused_section`:

```rust
DevToolsPanel::Performance => {
    let focused_section = state
        .session_manager
        .selected()
        .map(|h| h.session.performance.focused_section)
        .unwrap_or(PerfSection::FrameChart);

    match focused_section {
        PerfSection::FrameChart => "[Esc] Logs  [←/→] Frames  [Tab] Section  ]/[ Tabs  [j/k] Scroll  [b] Browser",
        PerfSection::Details => "[Esc] Logs  [Tab] Section  ]/[ Tabs  [b] Browser",
    }
}
```

Adjust the exact substrings to match the existing footer style (spacing, ordering, hint glyph conventions). The key requirements are:

- `[j/k] Scroll` and `[←/→] Frames` must NOT appear when `focused_section == Details`.
- `]/[ Tabs` (m8) replaces the previous `[]/[] Tabs`.
- The footer must fit within the standard footer width budget (test that the longest variant doesn't exceed it).

Add two new footer tests:

```rust
#[test]
fn performance_footer_hides_scroll_keys_when_details_focused() {
    // Build a state with focused_section = Details.
    // Render the footer.
    // Assert "[j/k] Scroll" is NOT in the rendered footer.
    // Assert "]/[ Tabs" IS in the rendered footer.
}

#[test]
fn performance_footer_shows_scroll_keys_when_frame_chart_focused() {
    // Build a state with focused_section = FrameChart.
    // Assert "[j/k] Scroll" IS in the rendered footer.
}
```

### Acceptance Criteria

1. `docs/REVIEW_FOCUS.md` "Current usage" lists both `PerformanceState::frame_chart_visible_width` and `PerformanceState::details_pane_visible_height` under the approved-exceptions section.
2. No "Unreachable via Tab" comments remain in `handler/devtools/performance/frame.rs`.
3. `handle_perf_jump_to_start` and `handle_perf_page` (and `handle_perf_jump_to_end` if applicable) use the same fallback for `frame_chart_visible_width.get() == 0`.
4. `PerfSection::next` and `prev` are preceded by a doc comment warning about the 2-variant assumption.
5. `MIN_PHASE_BAR_WIDTH` is declared `pub(super) const`. The `const _: u16 = MIN_PHASE_BAR_WIDTH` workaround line is gone. `details/frame_analysis_tab.rs` imports the constant explicitly.
6. The `MIN_DUAL_PANE_HEIGHT` derivation comment matches the actual threshold value (either by comment correction or by bumping the constant).
7. When `focused_section == Details`, the rendered Performance footer string does NOT contain `[j/k] Scroll`. The two new footer tests pass.
8. The footer string contains `]/[ Tabs` (or an equivalently unambiguous notation), not `[]/[] Tabs`.
9. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` is green.

### Testing

- `cargo test -p fdemon-app handler::devtools::performance` — runs frame.rs handler tests including any new fallback-alignment regression test.
- `cargo test -p fdemon-tui widgets::devtools` — runs footer tests including the two new ones.
- `cargo test --workspace` — full quality gate.

### Risk

- **m5 import path:** If the actual module hierarchy doesn't resolve `use super::super::MIN_PHASE_BAR_WIDTH` cleanly, adjust the path. The constant must remain accessible from `details/frame_analysis_tab.rs`.
- **m7 threshold bump:** If correcting the derivation forces a constant change, that ripples through the existing dual-pane / chart-only fallback tests. Most likely outcome: the constant is empirically fine at 18; the comment is wrong and the comment alone needs fixing.
- **m6 footer length:** Adding section-aware branching may push the longest footer variant past the available width on narrow terminals. Test at typical widths (80, 120, 200 columns) — if truncation occurs, drop one of the always-visible hints (e.g., `[b] Browser`).

### Out of Scope

- Do NOT touch `frame_analysis_tab.rs` beyond the m5 import-path update. T02 owns that file's content edits.
- Do NOT touch `frame_chart/*` — T01 owns that subtree.
- Do NOT modify the `OverBudget` or `PerfSection::Details` doc entries in ARCHITECTURE.md — T03 owns those.
- Do NOT change layout-threshold constant *values* unless m7's derivation strictly requires it. Prefer comment correction.
- Do NOT bundle M2 (`is_janky` migration) here. That is a Phase 3 prerequisite.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `docs/REVIEW_FOCUS.md` | Added two missing Cell entries: `PerformanceState::frame_chart_visible_width` and `PerformanceState::details_pane_visible_height` (m1) |
| `crates/fdemon-app/src/handler/devtools/performance/frame.rs` | Replaced stale "Unreachable via Tab" comments with accurate Phase 3 intent (m3); aligned `handle_perf_jump_to_start` fallback to use `DEFAULT_PERF_PAGE_SIZE` like `handle_perf_page` (m13); added regression test for fallback alignment |
| `crates/fdemon-app/src/session/performance.rs` | Added 2-variant assumption caution doc comments to `PerfSection::next` and `PerfSection::prev` (m12) |
| `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` | Promoted `MIN_PHASE_BAR_WIDTH` to `pub(super)` and removed the `const _: u16 = MIN_PHASE_BAR_WIDTH` workaround (m5); rewrote `MIN_DUAL_PANE_HEIGHT` derivation comment to accurately describe the empirically-tested threshold vs strict calculation (m7) |
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Made Performance footer section-aware: hides `[j/k] Scroll` and `[←/→] Frames` when `focused_section == Details` (m6); replaced `[]/[] Tabs` with `]/[ Tabs` (m8); added import for `PerfSection`; added two new footer tests |

### Notable Decisions/Tradeoffs

1. **m5 import path unchanged**: `frame_analysis_tab.rs` already used `use super::super::MIN_PHASE_BAR_WIDTH;` which is the correct path. No change needed to that file — the `pub(super)` promotion makes the constant properly visible to the parent (`devtools`) module while children already had access via Rust's private item inheritance.

2. **m7 comment rewrite (not constant bump)**: The strict bottom-up arithmetic yields 20 rows (1 footer + 2 chart borders + 7 chart inner + 2 details borders + 8 details inner). The constant stays at 18 since it is empirically correct — ratatui's `Constraint::Min` makes this work. The comment now honestly describes this empirical basis rather than presenting wrong math.

3. **m6 footer test with SessionHandle**: Built a minimal `SessionHandle` in tests via `Session::new` + `SessionHandle::new` to set `focused_section` for the Details test. This avoids mocking while staying within public API.

4. **m13 regression test clarifies semantics**: The test asserts `page_up` from 0 moves by `DEFAULT_PERF_PAGE_SIZE` and `jump_to_start` uses `buf_len - DEFAULT_PERF_PAGE_SIZE` as its max-back offset, making the shared-fallback semantics explicit.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-app handler::devtools::performance` - Passed (36 tests)
- `cargo test -p fdemon-tui widgets::devtools` - Passed (449 tests)
- `cargo test -p fdemon-tui performance_footer` - Passed (4 tests including 2 new)
- `cargo test --workspace` - Passed (all crates, 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **m5 pub(super) scope**: The constant is now visible to the parent `devtools` module. No other sibling panels reference it so there is no risk of unintended cross-panel access. If `devtools/mod.rs` ever needs to use it, it can now do so without re-exporting.

2. **m7 threshold unchanged at 18**: If the ratatui version changes its constraint-min behaviour, the 18-row threshold may become incorrect. The updated comment documents this empirical basis so future maintainers know to re-verify.
