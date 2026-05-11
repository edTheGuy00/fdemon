# Task 03: Docs & plan reconciliation

**Status:** Not Started
**Estimated Hours:** 0.25h
**Depends On:** —
**Crate / Area:** docs / planning artifacts

## Goal

Discharge three review findings that all involve documentation or planning artifacts (no source code changes):

1. **`docs/REVIEW_FOCUS.md` missing the `mouse_regions` exception** (review item 3): The doc explicitly states *"New `Cell`-based render-hint fields require explicit review and documentation here."* `MouseRegionsCell` was added to `AppState` but never registered in the "Current usage" list. Add a bullet.
2. **Phase 3 TASKS.md narrative drift** (review item 4): TASKS.md line 172 claims *"Settings mode does not render the header, so the header regions are not in the registry, so header clicks in Settings are silently dropped."* Reality: regions ARE recorded; the gate is at the dispatcher (`crates/fdemon-app/src/handler/mouse/mod.rs:54-58`'s `_ => None` arm), not at registration. Fix the wording so future maintainers know where the actual gate lives.
3. **Task 07 reconciliation audit trail** (review item 14): The completion summary in `tasks/07-tabs-and-device-pill-regions.md` does not mention that the first implementor's worktree was discarded in favor of a manual cherry-pick of `tabs.rs` plus a small wiring delta in `header.rs`. Append a "Reconciliation note" subsection.

## Files Modified (Write)

- `docs/REVIEW_FOCUS.md`
- `workflow/plans/features/mouse-support/phase-3-region-registry/TASKS.md`
- `workflow/plans/features/mouse-support/phase-3-region-registry/tasks/07-tabs-and-device-pill-regions.md`

## Files Read

- `crates/fdemon-app/src/handler/mouse/mod.rs` — confirm exact line numbers and arm wording for the dispatcher gate citation
- `crates/fdemon-tui/src/render/tests.rs` — confirm the probe-test name `view_header_regions_present_in_settings_mode_because_header_always_renders` to cite verbatim

## Implementation Steps

### 1. Update `docs/REVIEW_FOCUS.md` "Current usage"

Locate the "Approved TEA Exception: Render-Hint Feedback" section. The "Current usage" subsection currently reads:
> **Current usage:** `TargetSelectorState::last_known_visible_height` — the renderer writes the actual device list area height each frame; the handler reads it for scroll calculations.

Replace it with a multi-bullet list:
> **Current usage:**
> - `TargetSelectorState::last_known_visible_height` — the renderer writes the actual device list area height each frame; the handler reads it for scroll calculations.
> - `AppState::mouse_regions: MouseRegionsCell` — the renderer populates a fresh `MouseRegions` registry each frame (header shortcuts, session tabs, device pill); `handler/mouse/normal.rs::handle_press` reads it for click hit-tests. Wrapped in a `MouseRegionsCell` newtype to satisfy `#[derive(Debug)]` on `AppState` (since `Cell<T>: Debug` requires `T: Copy`, which `MouseRegions` cannot be).

### 2. Update Phase 3 TASKS.md narrative

Locate line 172 of `workflow/plans/features/mouse-support/phase-3-region-registry/TASKS.md`. The current text (within the "No coordinate gating for clicks" note) reads:
> (E.g., Settings mode does not render the header, so the header regions are not in the registry, so header clicks in Settings are silently dropped.)

Replace with:
> (E.g., Settings mode still populates header regions because `render::view` paints the header before the modal overlay. The actual gate is at the dispatcher: `crates/fdemon-app/src/handler/mouse/mod.rs::handle_press` returns `None` for `_ => ` (non-Normal) modes, so the click never reaches a hit-test. Confirmed by the integration test `view_header_regions_present_in_settings_mode_because_header_always_renders` in `crates/fdemon-tui/src/render/tests.rs`.)

### 3. Append reconciliation note to Phase 3 Task 07's completion summary

In `tasks/07-tabs-and-device-pill-regions.md`, after the existing "Completion Summary" / "Risks/Limitations" content, append a new subsection:

```markdown
### Reconciliation Note (Phase 3.5)

The first implementor's worktree (`worktree-agent-a99ad3bd2a8c920bd`) exceeded scope by re-implementing
`render_main_header`, `TitleRowHints`, `register_shortcut_clicks`, and the shortcut constants —
all of which were Task 06's deliverables. When merged after Task 06 had already landed, this caused
4-file conflicts (`header.rs`, `render/mod.rs`, `widgets/mod.rs`, `handler/mouse/normal.rs`).

The orchestrator aborted the squash-merge and resolved by cherry-picking only the *new* contributions
of the worktree:

**Kept (cherry-picked from worktree):**
- `crates/fdemon-tui/src/widgets/tabs.rs` — `render_session_tabs(...)` free function with multi-session
  tab regions and single-session device-pill region.

**Added during reconciliation (manual delta in `feat/mouse-support`):**
- `crates/fdemon-tui/src/widgets/header.rs` — replaced `let tabs = SessionTabs::new(...); tabs.render(...);`
  with `render_session_tabs(tabs_area, buf, session_manager, header.icons, ctx);` so `MouseCtx` threads
  into the multi-session tabs row.
- `cargo fmt` re-flow on `handler/mouse/normal.rs` and `render/mod.rs`.

**Discarded (from worktree, not merged):**
- The worktree's `header.rs` rewrite (used Task 06's version instead).
- The worktree's `render/mod.rs` rewrite (used Task 06's version instead).
- The worktree's `widgets/mod.rs` rewrite (used Task 06's version instead).
- The worktree's `handler/mouse/normal.rs` whitespace edits (already applied by Task 06's clippy fix).

The cherry-picked `tabs.rs` had been independently validated by `task_validator` before reconciliation,
so the discarded files do not represent unreviewed code.
```

## Acceptance Criteria

- [ ] `docs/REVIEW_FOCUS.md` "Current usage" lists both `TargetSelectorState::last_known_visible_height` and `AppState::mouse_regions`
- [ ] Phase 3 TASKS.md no longer claims "Settings mode does not render the header"; cites the dispatcher gate location and the probe test name
- [ ] Phase 3 Task 07's completion summary contains a "Reconciliation Note (Phase 3.5)" subsection enumerating kept / added / discarded changes
- [ ] No source code changes
- [ ] `cargo fmt --all -- --check` passes (unchanged — no source edits)

## Notes

- This is a documentation-only task. None of the three target files are managed docs (`docs/REVIEW_FOCUS.md` is explicitly listed as implementor-editable per the planner skill's "Other Docs" classification; the two `workflow/plans/...` files are planning artifacts).
- Cite line numbers and test names verbatim from the actual source files at the time of writing — these are stable references and should remain accurate.
- Keep the reconciliation note factual; do not editorialize about the worktree implementor's process.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `docs/REVIEW_FOCUS.md` | Expanded "Current usage" from a single-line entry to a bulleted list; added `AppState::mouse_regions: MouseRegionsCell` bullet alongside the existing `TargetSelectorState::last_known_visible_height` entry |
| `workflow/plans/features/mouse-support/phase-3-region-registry/TASKS.md` | Replaced the incorrect "Settings mode does not render the header" sentence at line 172 with accurate wording citing the dispatcher gate (`handle_press` `_ => None` arm) and the integration test name |
| `workflow/plans/features/mouse-support/phase-3-region-registry/tasks/07-tabs-and-device-pill-regions.md` | Appended "Reconciliation Note (Phase 3.5)" subsection after "Risks/Limitations", enumerating kept / added / discarded changes from the manual cherry-pick |

### Notable Decisions/Tradeoffs

1. **Verbatim test name**: Used `view_header_regions_present_in_settings_mode_because_header_always_renders` exactly as it appears in `crates/fdemon-tui/src/render/tests.rs` line 160. The test name itself describes the corrected understanding.
2. **Dispatcher line reference**: The `_ => None` arm in `handle_press` is at line 57 of `crates/fdemon-app/src/handler/mouse/mod.rs` at time of writing. The TASKS.md update cites the function name rather than a line number to be more stable against minor reformats.

### Testing Performed

- `cargo fmt --all -- --check` - Passed (no source changes)
- No source code was modified; no test suite run required

### Risks/Limitations

1. **Line number stability**: The TASKS.md update deliberately uses function-name citation (`handle_press` returns `None` for `_ =>`) rather than a bare line number, so it remains accurate if surrounding code is reformatted.
