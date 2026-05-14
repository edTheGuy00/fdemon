## Task: Update KEYBINDINGS.md for Performance Tab

**Objective**: Document all new Performance-tab key bindings.

**Depends on**: Phase 3

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**
- `docs/KEYBINDINGS.md`: Add a "DevTools — Performance Tab" subsection listing all new bindings.

**Files Read (Dependencies):**
- `docs/KEYBINDINGS.md`: Existing format.

### Details

Add a section in the DevTools area listing:

| Key | Action |
|-----|--------|
| `Tab` | Focus next section (frame → memory → alloc list → frame) |
| `Shift+Tab` | Focus previous section |
| `↑` / `k` | Scroll focused section up (or move row up in alloc list) |
| `↓` / `j` | Scroll focused section down (or move row down in alloc list) |
| `PageUp` | Scroll one viewport up |
| `PageDown` | Scroll one viewport down |
| `Home` | Jump to oldest sample / first row |
| `End` | Jump to live edge / first row |
| `←` / `→` | (existing) Select previous / next frame |
| `s` | (existing) Toggle allocation sort column |
| Click section | Focus that section |
| Click alloc row | Focus alloc list + select row |
| Click frame bar | Select that frame |

### Acceptance Criteria

1. `docs/KEYBINDINGS.md` contains the new section.
2. Format consistent with existing entries.
3. All bindings from the PLAN.md `Keyboard Shortcuts Summary` are present.

### Notes

- Unmanaged doc — implementor can edit directly.
- Keep `Home`/`End` semantics clear: Home = back-in-time, End = present.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a7c743386c915fe96

### Files Modified

| File | Changes |
|------|---------|
| `docs/KEYBINDINGS.md` | Expanded "Performance Panel" section with four subsections: Section Focus, Scrolling, Frame Selection, Allocation List. All 13 bindings from PLAN.md Keyboard Shortcuts Summary documented. |
| `workflow/plans/features/devtools-performance-interactivity/phase-4-polish/tasks/09-update-keybindings-doc.md` | Added this completion summary. |

### Notable Decisions/Tradeoffs

1. **Subsection structure**: Used `####` headers to group bindings by functional area (Section Focus, Scrolling, Frame Selection, Allocation List) rather than a flat table. This mirrors the pattern used in the Network Panel and Settings Panel sections, and makes the distinctions between behaviors clear (e.g., `↑`/`k` behavior differs by focused section).
2. **Mouse interactions in KEYBINDINGS.md**: Included click interactions (Click section, Click frame bar, Click alloc row) in the keyboard doc since they were specified in the task table. The existing doc already mixes mouse and keyboard in the Performance section header note ("Mouse interactions are documented separately") but these are interactive selection/focus triggers that belong alongside the keyboard equivalents.
3. **ToC not updated**: The new `####`-level subsections are below the granularity tracked in the Table of Contents, consistent with how the Network Filter Mode and Settings value-type subsections are handled.

### Testing Performed

- Visual inspection of KEYBINDINGS.md — all 13 bindings from PLAN.md Keyboard Shortcuts Summary present and correctly described.
- Format verified consistent with existing entries (three-column table: Key / Action / Description).
- `Home`/`End` semantics documented clearly: Home = oldest/back-in-time, End = live edge/present.

### Risks/Limitations

1. **Mouse doc overlap**: The doc header notes mouse interactions are in MOUSE.md, but click interactions are also listed here (matching the existing approach in the pre-existing Performance Panel section). If MOUSE.md is updated, it should cross-reference the Performance Panel click interactions.
