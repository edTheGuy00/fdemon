## Task: Update KEYBINDINGS.md

**Objective**: Rewrite the DevTools Mode section of `docs/KEYBINDINGS.md` to reflect the post-Phase-4 state: remove the stale `l` (Layout Panel) reference, add `n` (Network), add full Performance panel and Network panel key binding sections, and update the table of contents.

**Depends on**: 01, 02, 03 (needs final key bindings from config, allocation sort, and filter input tasks)

### Scope

- `docs/KEYBINDINGS.md`: MODIFIED — Rewrite DevTools Mode section

### Details

#### Current state (stale)

The current DevTools Mode section at lines 389-423 contains:

| Issue | Current | Correct |
|-------|---------|---------|
| `l` binding | "Layout Panel" | **Removed in Phase 2** — `l` is unused in DevTools mode |
| `n` binding | Missing | Switch to Network panel |
| Panel list | "Inspector/Layout/Performance" | "Inspector/Performance/Network" |
| Network section | Missing entirely | Full section needed |
| Performance section | Missing | Frame selection keys needed |
| Table of Contents | No Network/Performance sub-sections | Add sub-sections |

#### Replacement content

Replace the entire `## DevTools Mode` section (from `## DevTools Mode` to just before `## Confirm Dialog Mode`) with the following structure:

```markdown
## DevTools Mode

Enter DevTools mode by pressing `d` in Normal mode (requires VM Service connection).

### Panel Navigation

| Key | Action | Description |
|-----|--------|-------------|
| `Esc` | Exit DevTools | Return to Normal mode (log view). In Performance panel, deselects frame first. In Network panel, deselects request first. |
| `i` | Inspector Panel | Switch to Widget Inspector panel |
| `p` | Performance Panel | Switch to Performance monitoring panel |
| `n` | Network Panel | Switch to Network monitor panel |
| `b` | Browser DevTools | Open Flutter DevTools in system browser |
| `q` | Quit | Quit the application |

### Debug Overlays

| Key | Action | Description |
|-----|--------|-------------|
| `Ctrl+r` | Repaint Rainbow | Toggle repaint rainbow overlay on device |
| `Ctrl+p` | Performance Overlay | Toggle performance overlay on device |
| `Ctrl+d` | Debug Paint | Toggle debug paint overlay on device |

### Widget Inspector Panel

When the Inspector panel is active:

| Key | Action | Description |
|-----|--------|-------------|
| `Up` / `k` | Move Up | Move selection up in widget tree |
| `Down` / `j` | Move Down | Move selection down in widget tree |
| `Enter` / `Right` | Expand | Expand selected tree node |
| `Left` / `h` | Collapse | Collapse selected tree node |
| `r` | Refresh | Refresh widget tree from VM Service |

The Inspector panel shows a 50/50 split: widget tree on one side, layout explorer on the other. Layout data auto-fetches when a tree node is selected.

### Performance Panel

When the Performance panel is active:

| Key | Action | Description |
|-----|--------|-------------|
| `Left` | Previous Frame | Select the previous frame in the bar chart |
| `Right` | Next Frame | Select the next frame in the bar chart |
| `Esc` | Deselect Frame | Clear frame selection (show summary instead of detail) |
| `s` | Toggle Sort | Toggle allocation table sort (Size / Instances) |

The Performance panel shows a frame timing bar chart (top) and memory time-series chart with class allocation table (bottom).

### Network Panel

When the Network panel is active:

| Key | Action | Description |
|-----|--------|-------------|
| `Up` / `k` | Navigate Up | Move up in request list |
| `Down` / `j` | Navigate Down | Move down in request list |
| `Page Up` | Page Up | Scroll request list up one page |
| `Page Down` | Page Down | Scroll request list down one page |
| `Enter` | Select / Refetch | Select request and fetch details (or refetch if already selected) |
| `Esc` | Deselect | Clear request selection |
| `Space` | Toggle Recording | Toggle network recording on/off |
| `Ctrl+x` | Clear Requests | Clear all recorded network requests |
| `/` | Filter | Enter filter mode to filter requests by text |
| `g` | General Tab | Switch to General detail sub-tab |
| `h` | Headers Tab | Switch to Headers detail sub-tab |
| `q` | Request Body Tab | Switch to Request Body detail sub-tab |
| `s` | Response Body Tab | Switch to Response Body detail sub-tab |
| `t` | Timing Tab | Switch to Timing detail sub-tab |

The Network panel shows HTTP/HTTPS requests in a scrollable table with detailed inspection.

#### Network Filter Mode

When filter input is active (after pressing `/`):

| Key | Action | Description |
|-----|--------|-------------|
| Type | Input | Type characters to build filter query |
| `Enter` | Apply Filter | Apply the filter and return to normal Network panel |
| `Esc` | Cancel | Discard filter input and return to normal Network panel |
| `Backspace` | Delete | Remove last character from filter |
```

#### Update Table of Contents

In the Table of Contents at the top of the file, update the DevTools Mode section to include new sub-sections:

```markdown
- [DevTools Mode](#devtools-mode)
  - [Panel Navigation](#panel-navigation)
  - [Debug Overlays](#debug-overlays)
  - [Widget Inspector Panel](#widget-inspector-panel)
  - [Performance Panel](#performance-panel)
  - [Network Panel](#network-panel)
```

#### Update Session Management description

In the Session Management section (line 72), change:

```markdown
| `d` | DevTools Mode | Enter DevTools mode (Inspector/Layout/Performance panels) |
```

to:

```markdown
| `d` | DevTools Mode | Enter DevTools mode (Inspector/Performance/Network panels) |
```

Also update the similar reference in the DevTools sub-section of Normal Mode (line 173):

```markdown
| `d` | DevTools Mode | Enter DevTools mode (Inspector/Performance/Network panels) |
```

### Acceptance Criteria

1. No reference to `l` (Layout Panel) exists anywhere in KEYBINDINGS.md
2. `n` (Network Panel) is documented in the Panel Navigation table
3. Full Performance Panel section with Left/Right/Esc/s bindings
4. Full Network Panel section with all key bindings
5. Network Filter Mode sub-section documented
6. Table of contents updated with new sub-sections
7. Session Management description updated (no "Layout" reference)
8. All key bindings match the actual implementation in `handler/keys.rs`

### Testing

No code tests — this is a documentation-only task. Verification:

1. Read `handler/keys.rs` and confirm every DevTools key binding has a corresponding row in KEYBINDINGS.md
2. Confirm no undocumented key bindings exist
3. Confirm no documented bindings reference removed functionality

### Notes

- **`q` in Network panel**: The `q` key has dual meaning — it's "Request Body" tab in the Network detail view, and "Quit" elsewhere in DevTools. The documentation should reflect this nuance (the `q` Quit binding is listed in Panel Navigation, and the `q` Request Body binding is listed in the Network Panel section).
- **`Esc` layered behavior**: In Performance panel, `Esc` deselects frame first, then exits DevTools on second press. In Network panel, `Esc` deselects request first. Document this "layered Esc" behavior.
