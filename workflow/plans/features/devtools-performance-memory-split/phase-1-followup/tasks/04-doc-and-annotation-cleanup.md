## Task: Documentation and annotation cleanup

**Objective:** Close out the remaining Major doc-hygiene findings (M2, M3) and the minor doc-adjacent items (m4, m6, m10) in one coherent edit. After Wave 1 lands its structural changes, this task ensures `docs/REVIEW_FOCUS.md` reflects the new `MemoryState` Cell fields, the stale Performance docstrings describe the post-split single-section reality, the EXCEPTION annotations carry consistent cross-references, and the Performance tab footer hint matches the actual key bindings.

**Depends on:** 02, 03 (both must be merged so this task edits the post-Wave-1 file contents)

**Agent:** implementor

**Estimated Time:** 1 hour

### Scope

**Files Modified (Write):**
- `docs/REVIEW_FOCUS.md` — add two bullets under the "Current usage" list for the two new `MemoryState` Cell render-hint fields (M3).
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` — rewrite the module-level `//!` docstring to describe the current single-section frame-chart-only layout (M2). No other code changes.
- `crates/fdemon-app/src/handler/devtools/performance.rs` — rewrite the module-level `//!` docstring to remove references to "allocation profile updates" and "rich memory samples" that moved to `memory.rs` (M2). Cross-link `super::memory`.
- `crates/fdemon-app/src/session/memory.rs` — update EXCEPTION annotation cross-references on the two Cell field declarations (m4); add a co-set invariant doc comment to `monitoring_active` (m10).
- `crates/fdemon-app/src/session/performance.rs` — update EXCEPTION annotation cross-reference on the `frame_chart_visible_width` field (m4); add a co-set invariant doc comment to `monitoring_active` (m10).
- `crates/fdemon-tui/src/widgets/devtools/mod.rs` — update the Performance tab footer hint string to reflect actual bindings after T03's option decision (m6). **Only the footer string** — no other edits to this file.

**Files Read (Dependencies):**
- T02's Completion Summary (will indicate whether the test_tab_bar test and Memory dispatch arm were modified — this task should not re-touch them).
- T03's Completion Summary (will indicate which Option A/B was chosen — this drives the footer hint string content).

### Background

The Phase 1 review surfaced multiple doc-hygiene items that all touch files already-edited-or-about-to-be-edited by Wave 1 tasks. Bundling them into a single Wave 2 task lets Wave 1 stay focused on structural fixes and avoids three separate small commits to the same files.

- **M2** — Two module-level `//!` headers still describe pre-T03 behaviour:
  - `widgets/devtools/performance/mod.rs:1-22` shows a dual-section "Frame Timing (~45%) / Memory (~55%)" ASCII diagram that no longer exists.
  - `handler/devtools/performance.rs:1-6` mentions handling "allocation profile updates and rich memory samples", but those handlers moved to `memory.rs` in T03 of Phase 1.

- **M3** — `docs/REVIEW_FOCUS.md:34` requires registration of new `Cell<usize>` render-hint fields: "New `Cell`-based render-hint fields require explicit review and documentation here." T02 of Phase 1 introduced `MemoryState::memory_chart_visible_width` and `MemoryState::alloc_table_visible_height` (migrated from `PerformanceState`) but the doc was never updated.

- **m4** — `CODE_STANDARDS.md` Principle 3 prescribes EXCEPTION annotations of the form `// EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md`. The current annotations on `session/memory.rs:91,97` and `session/performance.rs:73` cite `CODE_STANDARDS.md` but omit the `REVIEW_FOCUS.md` cross-reference.

- **m6** — The Performance tab footer hint in `widgets/devtools/mod.rs:373-375` lists `[Esc] Logs [i] Inspector [b] Browser [←/→] Frames [Ctrl+p] PerfOverlay` — but the panel actually supports Tab and j/k for scrolling. Bindings should match `docs/KEYBINDINGS.md` and the option chosen in T03.

- **m10** — `performance.monitoring_active` and `memory.monitoring_active` always flip in lockstep (only places they're set: T01's edit to `VmServicePerformanceMonitoringStarted` and the `VmServiceConnected` reset). The reviewer flagged "coupling-by-convention" — future maintainers could forget one. Documenting the invariant in code comments is the minimum-cost mitigation.

### Details

#### 1. `docs/REVIEW_FOCUS.md` — register the new Cell fields (M3)

Open the "Current usage" bulleted list (around line 29-34). It currently has three bullets (`TargetSelectorState`, `AppState::mouse_regions`, `TagFilterUiState`). Add two new bullets, matching the existing style:

```markdown
- `MemoryState::memory_chart_visible_width` — the renderer writes the actual chart plot width (in columns) each frame; the chart-scroll handler reads it to clamp `memory_chart_scroll_offset` against the latest geometry. Default 0 (safe fallback when no render has happened yet).
- `MemoryState::alloc_table_visible_height` — the renderer writes the visible data-row count (excluding header) each frame; the alloc-table page and jump handlers read it to size page-step and end-of-list navigation. Default 0 (safe fallback when no render has happened yet).
```

Insertion order: after the `TagFilterUiState` entry, in alphabetical order within the `MemoryState` group.

#### 2. `widgets/devtools/performance/mod.rs` — rewrite module docstring (M2)

The current `//!` header (lines 1-22) reads roughly:

```
//! Performance panel widget for the DevTools TUI mode.
//!
//! Displays real-time FPS and frame timing using data from `PerformanceState`.
//! Memory data appears in the bottom section using a `MemoryChart` widget.
//!
//! # Layout
//!
//! ```text
//! ┌─ Frame Timing (~45%) ───────────────────────────┐
//! │ ... bar chart and detail panel ...               │
//! ├──────────────────────────────────────────────────┤
//! │ Memory Chart (~55%)                              │
//! │ ... time-series + allocation table ...           │
//! └──────────────────────────────────────────────────┘
//! ```
```

Replace with a current-state version:

```
//! Performance panel widget for the DevTools TUI mode.
//!
//! Renders the Flutter Frames bar chart in the full inner area.
//!
//! Memory data and allocation profiling have moved to the dedicated Memory
//! panel (`DevToolsPanel::Memory`); see [`super::memory`].
//!
//! # Layout
//!
//! ```text
//! ┌─ Frame Timing ──────────────────────────────────┐
//! │                                                  │
//! │  [bar chart fills full inner area]               │
//! │                                                  │
//! └──────────────────────────────────────────────────┘
//! ```
```

**No code changes** — only the docstring. Do not modify the render path.

If T03 chose Option B (visible Details placeholder), append a short paragraph after the diagram noting that focus may move to the Details placeholder when the user presses Tab. If T03 chose Option A, no addition needed. T03's Completion Summary will identify which option to honour.

#### 3. `handler/devtools/performance.rs` — rewrite module docstring (M2)

The current `//!` header (lines 1-6) reads:

```
//! Performance panel handlers.
//!
//! Handles frame selection, allocation profile updates, and rich memory samples
//! for the Performance panel's bar chart and time-series views, plus the
//! Phase 2 keyboard interactivity handlers (section focus, scroll, page, jump,
//! alloc row selection).
```

The "allocation profile updates" and "rich memory samples" handlers moved to `super::memory` in T03 of Phase 1. Replace with:

```
//! Performance panel handlers.
//!
//! Handles frame selection and keyboard interactivity for the Performance
//! panel's frame bar chart (section focus, scroll, page, jump).
//!
//! Memory and allocation profile handlers moved to [`super::memory`]. See
//! [`crate::session::performance`] and [`crate::session::memory`] for the
//! data ownership split.
```

**No code changes** — only the docstring.

#### 4. EXCEPTION annotations — cross-reference fix (m4)

`docs/CODE_STANDARDS.md` Principle 3 prescribes the canonical form:

```rust
// EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md
```

Two files need updates:

**`crates/fdemon-app/src/session/memory.rs:91` and `:97`** — the two Cell fields. Current annotations (per the code quality reviewer):

```rust
// EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md Principle 3.
```

Replace with:

```rust
// EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md Principle 3 and
// docs/REVIEW_FOCUS.md "Approved TEA Exception → Current usage".
```

**`crates/fdemon-app/src/session/performance.rs:73`** — the `frame_chart_visible_width` Cell. Current annotation:

```rust
// EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md "Region Registry Pattern" and Principle 3.
```

The "Region Registry Pattern" reference is wrong — that pattern is for mouse regions, not scroll/width Cells. Replace with the same canonical form as above.

#### 5. `monitoring_active` co-set invariant (m10)

Both `PerformanceState::monitoring_active` and `MemoryState::monitoring_active` flip in lockstep (T01's edit to `VmServicePerformanceMonitoringStarted` sets both true; `VmServiceConnected` resets both to false via state replacement). Add a doc comment to **each** field documenting the invariant:

**`crates/fdemon-app/src/session/performance.rs:52`** (`monitoring_active: bool` field):

```rust
/// Whether performance monitoring is active.
///
/// **Invariant:** flipped in lockstep with [`super::memory::MemoryState::monitoring_active`].
/// Both flags are set true together in
/// [`crate::handler::update::update`]'s
/// `VmServicePerformanceMonitoringStarted` arm and both are reset on
/// `VmServiceConnected` (full struct replacement). If a future change
/// diverges these lifecycles, document the rationale here.
pub monitoring_active: bool,
```

**`crates/fdemon-app/src/session/memory.rs:77`** (`monitoring_active: bool` field): the symmetric comment, pointing at `super::performance::PerformanceState::monitoring_active`.

Keep the comments concise — five lines max each. The invariant is the important content; the exact code paths are a maintenance aid that may shift.

#### 6. Performance tab footer hint (m6)

Open `crates/fdemon-tui/src/widgets/devtools/mod.rs` around line 373-375 (the Performance arm of `render_footer`). The current hint string omits `Tab` and `j/k`.

**The exact final string depends on T03's choice:**

- **If T03 chose Option A** (Tab no-ops): keep the footer as Phase 1 shipped it. No edit needed beyond confirming the string matches `docs/KEYBINDINGS.md` Performance Panel section. If it doesn't, harmonise the wording.

- **If T03 chose Option B** (visible Details stub): the footer should advertise `[Tab] Details`. Insert it between `[←/→] Frames` and `[Ctrl+p] PerfOverlay`, matching the existing style.

For either option, ensure the footer string is consistent with `docs/KEYBINDINGS.md` (Phase 1's T04 documented the post-split keymap; the in-app footer should be a subset of the documented keys, not a divergent set).

**No new shortcuts.** This is purely a hint-string update.

#### 7. Quality gate

`cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`

All four green.

### Acceptance Criteria

- [ ] `cargo check`, `cargo test`, `cargo clippy` all green.
- [ ] `docs/REVIEW_FOCUS.md` "Current usage" section lists `MemoryState::memory_chart_visible_width` and `MemoryState::alloc_table_visible_height` with descriptions matching the existing entry style.
- [ ] `widgets/devtools/performance/mod.rs`'s `//!` header no longer mentions dual-section layout, memory chart, or "(~45%)" / "(~55%)" allocations.
- [ ] `handler/devtools/performance.rs`'s `//!` header no longer mentions "allocation profile updates" or "rich memory samples"; it cross-links `super::memory`.
- [ ] All three EXCEPTION annotations (`session/memory.rs:91`, `:97`, `session/performance.rs:73`) reference both `docs/CODE_STANDARDS.md` Principle 3 **and** `docs/REVIEW_FOCUS.md`.
- [ ] Both `monitoring_active` fields carry a doc comment naming the lockstep invariant.
- [ ] Performance tab footer hint matches T03's chosen option and is consistent with `docs/KEYBINDINGS.md`.

### Module Structure

No new modules. All edits are within existing files. This task introduces no new code — only doc comments, annotation tweaks, and a hint-string adjustment.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `docs/REVIEW_FOCUS.md` | Added two bullets for `MemoryState::memory_chart_visible_width` and `MemoryState::alloc_table_visible_height` under "Current usage" (M3) |
| `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` | Rewrote `//!` header to describe single-section frame-chart-only layout; removed dual-section / memory-chart / (~45%)/(~55%) references (M2) |
| `crates/fdemon-app/src/handler/devtools/performance.rs` | Rewrote `//!` header to remove "allocation profile updates" and "rich memory samples"; added cross-link to `super::memory` (M2) |
| `crates/fdemon-app/src/session/memory.rs` | Fixed both EXCEPTION annotations to reference both `CODE_STANDARDS.md` Principle 3 and `REVIEW_FOCUS.md` (m4); added `monitoring_active` lockstep invariant doc comment (m10) |
| `crates/fdemon-app/src/session/performance.rs` | Fixed EXCEPTION annotation to remove incorrect "Region Registry Pattern" reference; added `monitoring_active` lockstep invariant doc comment pointing at `MemoryState` (m4, m10) |
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Updated Performance footer hint to `[Esc] Logs  [←/→] Frames  [j/k] Scroll  [b] Browser  [Ctrl+p] PerfOverlay` — consistent with KEYBINDINGS.md; removed `[i] Inspector` (not panel-specific) and added missing `[j/k] Scroll` (m6) |

### Notable Decisions/Tradeoffs

1. **Option A footer (m6)**: T03 chose Option A (Tab is a no-op), so `[Tab] Details` was not added. The prior footer was missing `[j/k] Scroll` which KEYBINDINGS.md documents for the Performance Panel. Added it and reordered the hints to match KEYBINDINGS.md's natural grouping (navigation keys first, then control keys).

2. **`[i] Inspector` removed from footer**: The `[i]` key is a global panel-navigation shortcut documented under "Panel Navigation" in KEYBINDINGS.md, not specific to the Performance panel. The Inspector panel footer does not list `[p]` Performance, so symmetry supports removing `[i]` from the Performance footer. Replaced with `[b] Browser` which is listed in the Performance Panel section.

3. **Doc comment length (m10)**: Kept at four lines per field, matching the five-line max guideline. The invariant statement and the two set-points are the essential content.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test --workspace` — Passed (5,827+ tests, 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **Footer string truncation**: The footer truncates at `area.width - 2` columns. The new string `[Esc] Logs  [←/→] Frames  [j/k] Scroll  [b] Browser  [Ctrl+p] PerfOverlay` is 72 characters — typical terminal widths of 80+ will display it fully.
