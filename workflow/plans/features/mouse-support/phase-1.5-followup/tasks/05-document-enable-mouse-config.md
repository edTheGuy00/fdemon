## Task: Document `[ui] enable_mouse` in `docs/CONFIGURATION.md`

**Objective**: Add the new `enable_mouse` setting to the user-facing configuration documentation. The setting ships in Phase 1 but is currently undocumented anywhere users would discover it.

**Depends on**: None

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `docs/CONFIGURATION.md`: Add `enable_mouse` to the `[ui]` example block (around line 308), to the property table (around lines 318–326), and add a brief "what it does and when to disable" paragraph.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/config/types.rs`: Confirm the field's serde default (`default_true`) and `Default` impl (so the doc accurately states the default value).

### Details

`docs/CONFIGURATION.md` lines 303–326 currently document the `[ui]` section. The setting is missing from both the example block and the table.

Update the example block (around line 307) to add:

```toml
[ui]
icons = "nerd_fonts"
log_buffer_size = 10000
show_timestamps = true
compact_logs = false
theme = "default"
stack_trace_collapsed = true
stack_trace_max_frames = 3
enable_mouse = true              # Capture mouse events for clickable UI; restart required
```

Update the property table (around lines 318–326) by adding a row:

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `enable_mouse` | `boolean` | `true` | Enables terminal mouse capture for clickable UI elements (header shortcuts, tabs, log view, DevTools panels). When `false`, fdemon does not emit mouse-capture escape sequences, leaving native terminal behavior (text selection, wheel scrollback) intact. **Restart required after changing.** |

Then add a brief explanatory paragraph after the property table — something like:

> **When to disable mouse capture:** Most modern terminals (Windows Terminal, iTerm2, WezTerm, gnome-terminal) pass `Shift+drag` through to native text selection even when capture is on, so the default `true` works for most users. Disable (`enable_mouse = false`) if your terminal does not support `Shift+drag` for native selection, if you find the wheel-intercept-vs-host-scrollback behavior disorienting, or if you are running on legacy Windows conhost (which silently ignores mouse capture entirely). The setting is read at startup; restart fdemon after changing it.

Place this in the same style as the existing `> No Nerd Font?` callout that follows the icons table.

### Acceptance Criteria

1. `docs/CONFIGURATION.md` `[ui]` example block includes `enable_mouse = true` with a brief inline comment.
2. `docs/CONFIGURATION.md` property table includes a row for `enable_mouse` with type, default, and description.
3. The description mentions: (a) what mouse capture enables in fdemon, (b) the `false` opt-out and why someone might want it, (c) "restart required."
4. No other property rows or sections are altered.
5. Markdown renders cleanly (no broken table syntax).

### Testing

Visual review only. Optional: render the markdown locally to confirm table formatting (`grip docs/CONFIGURATION.md` or a markdown preview in your editor).

### Notes

- `docs/CONFIGURATION.md` is unmanaged (not subject to `doc_maintainer` content boundaries), so this can be done by the default implementor.
- The "Restart required" wording matches how the planner documented the setting in `workflow/plans/features/mouse-support/PLAN.md` — keep that consistent.
- A future `docs/MOUSE.md` (planned for Phase 6 of the parent feature) will provide the full interaction map. This task only documents the on/off setting.
