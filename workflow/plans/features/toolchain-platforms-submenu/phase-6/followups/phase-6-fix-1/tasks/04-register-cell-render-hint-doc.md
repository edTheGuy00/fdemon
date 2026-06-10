# Task 04: Register the picker Cell render-hint in docs/REVIEW_FOCUS.md

**Status:** Not Started
**Agent:** implementor
**Complexity:** low
**Depends On:** —
**Estimated Hours:** 0.5

## Objective

Satisfy the project policy "New `Cell`-based render-hint fields require explicit review and
documentation here" for `VersionPickerState::last_known_visible_height` (review finding M4).
`docs/REVIEW_FOCUS.md` is an unmanaged doc — implementor may edit it directly.

## Root Cause (verified)

The "Approved TEA Exception: Render-Hint Feedback" → Current-usage list in `docs/REVIEW_FOCUS.md`
ends at `InstallWizardState::last_known_visible_height`; the Phase 6 field is absent despite
following the approved pattern (default 0, `// EXCEPTION:` annotation at the TUI write site).

## Required Changes

Append one bullet to the Current-usage list (after the `InstallWizardState::last_known_visible_height`
entry), matching the established entry format:

```markdown
- `VersionPickerState::last_known_visible_height` — the renderer writes the visible list-row count
  each frame inside `VersionPickerOverlay`'s list render; the picker's scroll-adjust logic
  (`adjust_scroll`, used by `move_up`/`move_down`/`apply_manifest`) reads it to keep the selected
  row visible. Default 0 (safe fallback when no render has happened yet). Write site annotated in
  `widgets/install_wizard/version_picker.rs`.
```

Before writing, verify the exact reader/writer symbol names against the merged code
(`crates/fdemon-app/src/install_wizard/version_picker.rs` and
`crates/fdemon-tui/src/widgets/install_wizard/version_picker.rs`) and adjust the wording if the
symbols differ — the entry must name the real write site and reader.

## Acceptance Criteria

- [ ] The bullet exists in the Current-usage list, names the write site and the reader, and the
      symbol names match the code.
- [ ] No other REVIEW_FOCUS.md content changed.

## Files

**Write:** `docs/REVIEW_FOCUS.md`
**Read:** `crates/fdemon-app/src/install_wizard/version_picker.rs`,
`crates/fdemon-tui/src/widgets/install_wizard/version_picker.rs` (read-only)

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-aae4ffe5c43f47aea

### Files Modified

| File | Changes |
|------|---------|
| `docs/REVIEW_FOCUS.md` | Added `VersionPickerState::last_known_visible_height` entry to the Approved TEA Exception Current-usage list at line 39 |

### Notable Decisions/Tradeoffs

1. **Symbol name verification**: Confirmed exact reader and writer functions against the merged code:
   - Reader: `adjust_scroll()` method in `crates/fdemon-app/src/install_wizard/version_picker.rs` (line 326)
   - Writer: `render_list()` method in `crates/fdemon-tui/src/widgets/install_wizard/version_picker.rs` (line 129)
   - Widget: `VersionPickerOverlay` in the TUI module

### Testing Performed

- Manual verification of symbol names in the source code
- Verified document placement and formatting match established pattern
- No REVIEW_FOCUS.md content outside the target bullet was modified

### Risks/Limitations

None. This is a documentation-only task with clear acceptance criteria met.
