## Task: Manual-test matrix execution + parent BUG.md success-criteria check-off

**Objective:** Execute the BUG.md manual-test matrix on at least one stand-alone macOS terminal and one stand-alone Linux terminal. Update parent BUG.md success-criteria checkboxes. Confirm the IDE-terminal limitations (Zed, VS Code, JetBrains, etc.) match the new MOUSE.md "IDE built-in terminals" matrix and require no further code changes.

**Depends on:** Tasks 01-08 (all code changes must be merged)

**Agent:** implementor

**Estimated time:** 1-2 hours (mostly manual)

### Scope

**Files Modified (Write):**
- `workflow/plans/bugs/log-text-selection-broken/BUG.md`: check off success-criteria boxes that are now verified; add a brief verification log section if useful.

**Files Read (Dependencies):**
- `workflow/plans/bugs/log-text-selection-broken/BUG.md`: §"Manual-Test Matrix" and §"Success Criteria".
- `docs/MOUSE.md`: §"IDE built-in terminals" — confirm reviewer-flagged Zed limitations match the documented matrix.

### Details

#### Step 1 — Stand-alone terminal verification

Pick at least one terminal from each row family in `BUG.md` §"Manual-Test Matrix":

**macOS:** at least one of macOS Terminal.app, iTerm2, kitty, Alacritty, Ghostty, Wezterm.
**Linux:** at least one of Alacritty, Ghostty, kitty, Wezterm, GNOME Terminal.
**(Optional) Windows:** Windows Terminal.

For each terminal, run `cargo run -- /path/to/flutter/project` and verify:

- [ ] **Plain drag** — terminal's native behavior (no selection while capture is on, OR scrollback movement, depending on terminal).
- [ ] **Shift+drag** — selects log text natively. Does NOT drift when new logs arrive (the user's reported regression in Zed should NOT reproduce here — buffer-anchored selection is the expected behavior in stand-alone terminals).
- [ ] **Right-click on a log row** — copies the row, shows `Copied: <preview>` toast.
- [ ] **Right-click off a log row** — shows `Right-click copies log lines; nothing to copy here.` hint toast.
- [ ] **`Alt+m` toggle** — flips the `[mouse]`/`[mouse-off]` badge in the status bar.
- [ ] **Verify the parent BUG.md success-criteria boxes** — check off the ones that pass.

#### Step 2 — IDE-terminal sanity check (informational)

Open fdemon inside Zed's built-in terminal and confirm:

- [ ] Right-click does nothing — matches MOUSE.md "IDE built-in terminals → Zed" entry.
- [ ] `Alt+m` is intercepted by Zed and does NOT toggle — matches MOUSE.md.
- [ ] Shift+drag drifts as logs arrive — matches MOUSE.md.

The MOUSE.md matrix sets correct user expectations for IDE-terminal limitations. **No code changes are required for IDE terminals** — this is a documentation contract.

If any IDE-terminal behavior contradicts MOUSE.md, file a separate issue (do NOT block this follow-up).

#### Step 3 — Update parent BUG.md

In `workflow/plans/bugs/log-text-selection-broken/BUG.md` §"Success Criteria", check off the boxes that are now verified:

```markdown
- [x] On macOS Terminal.app, iTerm2, Alacritty, Ghostty, kitty: Shift+drag selects log text natively while fdemon is running with `enable_mouse = true`.
- [x] Right-click on any log row copies the row's full text to the system clipboard; status-bar toast confirms.
- [x] `Ctrl+M` (or chosen chord) toggles mouse capture without restarting fdemon; status indicator reflects the current state; toggle is logged for debugging.
- [x] All existing mouse features (scroll wheel, click `[r]`, click tabs, double-click stack-trace) still work after the fix.
- [x] `cargo test --workspace` passes; new tests cover: (a) capture sequence excludes `?1003`, (b) right-click → clipboard write via mock, (c) toggle updates state, (d) status indicator renders both states.
- [x] `docs/MOUSE.md` rewritten to match the new reality; PLAN.md cross-references this BUG.md.
```

Add a brief verification log under §"Success Criteria" (or in a new "Verification Log" section):

```markdown
### Verification Log

**Date:** YYYY-MM-DD
**Terminals tested:**
- macOS: <terminal name + version>
- Linux: <terminal name + version>
**Result:** All checks passed. IDE-terminal limitations confirmed to match MOUSE.md "IDE built-in terminals" matrix; no code changes required.
```

### Acceptance Criteria

1. At least one stand-alone macOS terminal and one stand-alone Linux terminal verified.
2. All five behavior checks (Shift+drag, right-click on row, right-click off row, Alt+m toggle, status badge) pass on each verified terminal.
3. Parent BUG.md success-criteria boxes are checked off where verified.
4. Parent BUG.md has a Verification Log entry naming the terminals tested and the date.
5. IDE-terminal sanity check confirms MOUSE.md matrix is accurate (or filed as separate issue if not).

### Testing

This task is the manual gate — no new automated tests. Run `cargo test --workspace` once more to confirm nothing regressed since the last code change.

### Notes

- This task does NOT modify any code. It only updates `workflow/plans/bugs/log-text-selection-broken/BUG.md`.
- If a stand-alone terminal fails any check, that's a new bug — file it as a separate BUG.md and link from this Verification Log. Do NOT silently skip a failing check.
- The IDE-terminal sanity check is informational. If Zed/VS Code/etc. behavior matches MOUSE.md, document and move on. If it differs, that's a documentation drift to fix in MOUSE.md (separate small task) — not a blocker for this follow-up.
