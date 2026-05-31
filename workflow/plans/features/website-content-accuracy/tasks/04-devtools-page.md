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
</content>
