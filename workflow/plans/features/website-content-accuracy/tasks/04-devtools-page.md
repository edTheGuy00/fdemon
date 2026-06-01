## Task: Fix DevTools page

**Objective**: Remove fabricated keys/panels from the DevTools page and add the real
Memory panel, matching the actual DevTools key handler.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `website/src/pages/docs/devtools.rs`: remove Layout Explorer key, add Memory panel,
  fix `s` mapping and Inspector nav wording.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/keys.rs` (DevTools handler ~450-1087): source of truth.

### Details

- Remove the "Layout Explorer (l)" panel/key — pressing `l` in DevTools does nothing;
  layout info lives inside the Inspector details view. Reword as part of Inspector, not a
  top-level `l` panel. [D-28] (`devtools.rs:143-187`)
- Add the Memory panel (`m`) to panel navigation and keybinding tables: own scroll
  handler, `Tab/Shift+Tab`, `j/k`, `PageUp/PageDown`, `Home/End`, `s` (allocation sort),
  `Esc`. [D-26] (`keys.rs:800-826,878`)
- Fix `s` — it is the Memory allocation sort, not "sort frames by duration" under
  Performance. Remove the false Performance `s` entry. [D-27] (`keys.rs:1012`,
  `devtools.rs:611`)
- Inspector `→` vs `Enter` — `→` expands the node; `Enter` opens the Details view.
  Document them as distinct, not both "Expand". [D-29] (`keys.rs:964-974`)
- Ensure Panel Navigation matches code: `i` Inspector, `p` Performance, `m` Memory,
  `n` Network, `b` Browser/DevTools.

### Acceptance Criteria

1. No `l` Layout Explorer key remains; layout is described under Inspector.
2. Memory panel documented with its keys; `s` appears only under Memory.
3. Inspector `→`/`Enter` distinction is correct.
4. `cd website && trunk build` compiles.

### Notes

- Keep the DevTools keybinding tables consistent with the Keybindings page (T01 fixes the
  same bindings in `data.rs`).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a9d44ca232289acd7

### Files Modified

| File | Changes |
|------|---------|
| `website/src/pages/docs/devtools.rs` | Removed Layout Explorer (l) section; added Memory Panel (m) section with full keybinding table; fixed Inspector navigation (→ expands, Enter opens Details); removed false Performance `s` entry; updated all panel navigation tables to include `m` Memory and `b` browser; updated Overview grid to 4 panels; updated `default_panel` settings description to remove "layout" option |

### Notable Decisions/Tradeoffs

1. **Layout Explorer removal**: Replaced the standalone "Layout Explorer (l)" section with a note inside the Inspector Details Panel section explaining that layout info (constraints, size, flex properties) lives in the Details view. The `l` key does not exist in the actual DevTools handler.
2. **`s` key placement**: Removed the false "Sort frames by duration" entry from Performance keybindings. `s` now appears only under Memory (allocation sort toggle) and Network (Response Body sub-tab), matching keys.rs lines 1012 and 930 respectively.
3. **Browser key `b` consolidation**: Added `b` to the Panel Navigation entry table in both "Entering and Exiting DevTools" and "Keybindings Quick Reference > Panel Navigation". Removed the redundant standalone "Browser" sub-section from the quick reference to avoid duplication.
4. **Memory Panel section**: Added a complete "Memory Panel (m)" section between Performance and Network with Tab/Shift+Tab section cycling, j/k/PgUp/PgDn/Home/End scroll, `s` sort toggle, and Esc deselect — all matching keys.rs lines 800-826 and 1012.

### Testing Performed

- `cargo check` on website crate (via copy to main repo) - Passed (only pre-existing dead_code warning)
- Manual review of all changed keybinding tables against `crates/fdemon-app/src/handler/keys.rs` lines 450-1087

### Risks/Limitations

1. **Memory Panel content**: The panel description is accurate for the keys handled in keys.rs. Specific UI details (e.g. exact section names inside the Memory panel) were described generically since the task only required key documentation accuracy.
</content>
