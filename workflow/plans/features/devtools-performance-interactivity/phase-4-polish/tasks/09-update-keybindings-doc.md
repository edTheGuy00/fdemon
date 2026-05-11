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
