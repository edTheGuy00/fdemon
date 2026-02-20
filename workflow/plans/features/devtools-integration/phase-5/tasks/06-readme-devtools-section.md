## Task: Add DevTools Section to README

**Objective**: Add a section to the project README.md documenting the DevTools integration feature — what it does, how to access it, key capabilities, and configuration basics.

**Depends on**: 01-expand-devtools-config (to document final config options)

**Estimated Time**: 1-2 hours

### Scope

- `README.md`: Add DevTools section with feature overview, usage, and key shortcuts

### Details

#### 1. Read Existing README

Read `README.md` to understand:
- Current structure and sections
- Tone and formatting style
- Where a DevTools section fits naturally (after features? after usage?)

#### 2. Write DevTools Section

Add a section covering:

**Feature Overview** (2-3 sentences):
- Built-in DevTools integration via VM Service
- Inspect widget tree, explore layouts, monitor performance
- All from the terminal — no browser needed (with browser fallback via `b`)

**Accessing DevTools**:
- Press `d` in normal mode to enter DevTools mode
- Requires a running Flutter app in debug mode with VM Service connection
- Press `Esc` to return to log view

**Three Panels**:
- **Widget Inspector** (`i`): Browse the widget tree, expand/collapse nodes, view widget details and source locations
- **Layout Explorer** (`l`): Visualize flex constraints, sizes, and layout properties for the selected widget
- **Performance** (`p`): Real-time FPS sparkline, memory usage gauge, jank detection, GC event history

**Debug Overlays**:
- `Ctrl+r` — Repaint rainbow
- `Ctrl+p` — Performance overlay
- `Ctrl+d` — Debug paint

**Browser Fallback**:
- Press `b` to open full Flutter DevTools in your system browser
- Configure browser with `[devtools] browser = "chrome"` in config

**Configuration** (brief):
- Point to the configuration docs or website for full details
- Show the basic `[devtools]` config block

Keep this section concise — the README should give a quick overview, not exhaustive documentation. Point to the website for full details.

### Acceptance Criteria

1. README has a "DevTools" or "Built-in DevTools" section
2. Section explains what DevTools does (inspect, layout, performance)
3. Section shows how to access it (`d` key)
4. All three panels are mentioned with their key shortcuts
5. Debug overlay shortcuts are listed
6. Browser fallback (`b`) is mentioned
7. Section points to website/docs for full configuration details
8. Formatting matches the existing README style

### Testing

- Visual review of README rendering
- No broken markdown links
- Section flows naturally within the existing document structure

### Notes

- **Keep it brief.** The README is for quick orientation. The website has the full documentation.
- **Don't duplicate the entire keybindings table.** Just mention the key shortcuts for entering DevTools and switching panels.
- **Include a small ASCII screenshot or description** of what the TUI looks like in DevTools mode if it fits the README's style.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `README.md` | Fixed `d` → `+` in "Adding More Sessions" text; updated Quick Reference table (`d` = DevTools, `+` = New Session); added "Built-in DevTools" section with panels table, debug overlays table, browser fallback, and link to website docs |

### Notable Decisions/Tradeoffs

1. **Section placement**: Added between "Keyboard Controls" and "Opening Files from Logs" to maintain logical flow
2. **Concise format**: Used tables for panels and overlays rather than prose to match existing README style

### Testing Performed

- Visual review of README rendering — Passed
- No broken markdown links

### Risks/Limitations

None
