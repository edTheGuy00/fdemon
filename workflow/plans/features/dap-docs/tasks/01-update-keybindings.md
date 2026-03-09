## Task: Update KEYBINDINGS.md with DAP toggle

**Objective**: Add the `D` (Shift+D) keybinding to `docs/KEYBINDINGS.md` so users know how to toggle the DAP server from the TUI.

**Depends on**: None

**Estimated Time**: 0.5 hours

### Scope

- `docs/KEYBINDINGS.md`: Add DAP server keybinding

### Details

The `D` key in Normal mode sends `Message::ToggleDap` (see `crates/fdemon-app/src/handler/keys.rs:172-175`). This toggles the DAP server on/off regardless of session state. It is currently undocumented.

**Changes needed:**

1. In the **Session Management** table (around line 68-75), add a row after the `d` (DevTools Mode) entry:

   | Key | Action | Description |
   |-----|--------|-------------|
   | `D` | Toggle DAP Server | Start or stop the DAP debug adapter server |

2. Add a new **DAP Server** subsection under Normal Mode, after the existing DevTools subsection (around line 173-177). This mirrors how DevTools has both a table entry in Session Management AND its own subsection:

   ```markdown
   ### DAP Server

   | Key | Action | Description |
   |-----|--------|-------------|
   | `D` | Toggle DAP Server | Start or stop the DAP debug adapter server. When active, `[DAP :PORT]` appears in the status bar. Connect your IDE's debugger to this port. |
   ```

3. Update the **Table of Contents** to include the new DAP Server subsection.

### Acceptance Criteria

1. `D` keybinding appears in the Session Management table
2. New "DAP Server" subsection exists under Normal Mode
3. Table of Contents is updated
4. Document formatting is consistent with existing entries

### Notes

- `d` (lowercase) = DevTools mode, `D` (uppercase/Shift+D) = DAP toggle — make this distinction clear
- The DAP server keybinding works regardless of session state (no active session required to start the server)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `docs/KEYBINDINGS.md` | Added `D` row in Session Management table; added "DAP Server" subsection after DevTools subsection; added ToC entry for DAP Server |

### Notable Decisions/Tradeoffs

1. **ToC placement**: Added `[DAP Server](#dap-server)` after `[Link Navigation](#link-navigation)` in the Normal Mode ToC block. This matches document order (the DAP Server section appears after DevTools in the body, which itself comes after Settings and Link Navigation).

### Testing Performed

- Manual review of markdown formatting - Passed
- All three acceptance criteria confirmed present in the updated file

### Risks/Limitations

1. **None**: This is a documentation-only change with no code impact.
