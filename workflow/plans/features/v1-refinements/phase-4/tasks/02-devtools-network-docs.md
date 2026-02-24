## Task: Add Network Monitor Documentation to DevTools Page

**Objective**: Add a comprehensive "Network Monitor (n)" section to the DevTools documentation page, covering the request table, detail view with sub-tabs, recording toggle, filter mode, and history clearing.

**Depends on**: 01-fix-keybindings-data (needs correct keybinding data for the quick reference section)

### Scope

- `website/src/pages/docs/devtools.rs`: Add Network Monitor section + update panel navigation references

### Details

The `devtools.rs` page currently documents Widget Inspector, Layout Explorer, Performance Monitor, Debug Overlays, Browser DevTools, Connection States, and Configuration. The Network Monitor panel is fully implemented in the codebase but completely absent from the website documentation.

#### 1. Add "Network Monitor (n)" section

Insert a new `<Section title="Network Monitor (n)">` **between** the "Performance Monitor (p)" section and the "Debug Overlays" section. This follows the order of the DevToolsPanel enum: Inspector → Performance → Network.

Content structure:

**Opening paragraph**: Explain that pressing `n` opens the Network Monitor, which captures HTTP requests made by the Flutter app via the VM Service `ext.dart.io.getHttpProfile` extension. Recording must be enabled to capture requests.

**Request Table subsection** (`<h3>`):
- The main view shows a table of captured HTTP requests
- Columns: Method, URL (truncated), Status Code, Duration, Size
- Navigate with `j/k/↑/↓` (single) or `PgUp/PgDn` (page of 10)
- Color-coded status: green (2xx), yellow (3xx), red (4xx/5xx)

**ASCII art mockup** of the request table layout (same style as the Widget Inspector mockup):
```
┌─ Network Monitor ─────────────────────────────────────────────────┐
│ ● Recording   12 requests   Filter: none                         │
│───────────────────────────────────────────────────────────────────│
│ GET  /api/users           200  45ms   1.2 KB                     │
│ POST /api/login           200  120ms  0.3 KB                     │
│ GET  /api/posts?page=1    200  89ms   4.5 KB                     │
│▶GET  /api/posts/42        404  23ms   0.1 KB                     │
│ GET  /api/config           200  12ms   0.8 KB                     │
└───────────────────────────────────────────────────────────────────┘
```

**Request Detail View subsection** (`<h3>`):
- Press `Enter` on a request to open the detail view
- 5 sub-tabs accessible via single-key shortcuts:
  - `g` — **General**: Method, URL, status, start time, duration, content type
  - `h` — **Headers**: Request and response headers in key-value format
  - `q` — **Request Body**: The body sent with the request (if any)
  - `s` — **Response Body**: The response body content
  - `t` — **Timing**: Connection timing breakdown (DNS, connect, TLS, first byte, transfer)
- Press `Esc` to deselect the request and return to the list view

**Detail tabs table** (using `<KeyRow>`):

| Key | Tab |
|-----|-----|
| `g` | General — overview of the request |
| `h` | Headers — request and response headers |
| `q` | Request Body — payload sent |
| `s` | Response Body — response content |
| `t` | Timing — connection timing breakdown |

**Recording & Controls subsection** (`<h3>`):
- `Space` — Toggle recording on/off. When off, no new requests are captured.
- `Ctrl+X` — Clear all recorded requests from history
- `/` — Enter filter mode to filter requests by URL substring

**Filter Mode subsection** (`<h3>`):
- Pressing `/` enters a text input mode at the top of the panel
- Type to filter requests — only matching URLs are shown
- `Enter` to apply the filter, `Esc` to cancel
- The filter persists until cleared

**Blue callout box**: "Network profiling requires a debug-mode app with an active VM Service connection. The HTTP profile extension is not available in profile or release builds."

#### 2. Update "Entering and Exiting DevTools" section

The existing keybinding table at lines 64-72 lists `i`, `l`, `p` as panel keys. Update to:
- Change `l` → `n` (Network Monitor)
- Or better: list `i` (Inspector), `p` (Performance), `n` (Network) in that order
- Remove the `l` entry entirely

#### 3. Update "Overview" section

The overview shows a 3-panel grid: Widget Inspector, Layout Explorer, Performance Monitor. Update to a 4-panel grid:
- Widget Inspector (keep)
- ~~Layout Explorer~~ → Keep it but note it's accessed via Inspector panel
- Performance Monitor (keep)
- **Add**: Network Monitor card

Or change to: Widget Inspector, Performance Monitor, Network Monitor (3 panels matching the actual DevToolsPanel enum). The Layout Explorer is a sub-feature of the Inspector, not a separate panel.

#### 4. Update "Keybindings Quick Reference" section

The quick reference at the bottom currently has: Panel Navigation, Widget Inspector Navigation, Debug Overlays, Browser.

Add two new sub-tables:
- **Network Monitor** — navigation + detail tabs + controls (14 bindings)
- **Performance Monitor** — sort, frame navigation (3 bindings)

Update the **Panel Navigation** table to replace `l` with `n`.

### Acceptance Criteria

1. A "Network Monitor (n)" section exists between Performance Monitor and Debug Overlays
2. The section documents: request table, navigation, detail view with 5 sub-tabs, recording toggle, clear history, filter mode
3. An ASCII mockup shows the request table layout
4. Detail sub-tab key mappings are correct: `g/h/q/s/t`
5. The "Entering and Exiting" table uses `n` instead of `l`
6. The Overview grid reflects the actual 3-panel structure
7. The Keybindings Quick Reference includes Network Monitor and Performance Monitor tables
8. Website compiles: `cd website && trunk build`

### Testing

- Visual verification: `cd website && trunk serve` then navigate to `/docs/devtools`
- Verify the Network Monitor section renders correctly with all sub-sections
- Verify the overview grid is accurate
- Verify keybinding quick reference includes all panels

### Notes

- Use the same `Section`, `KeyRow` helper components already defined at the bottom of `devtools.rs`
- Match the existing documentation style: opening paragraph, keybinding tables, callout boxes
- The Layout Explorer section can remain — it describes a sub-feature of the Inspector. But panel navigation references must not list `l` as a panel key
- The `q` key quirk: in Network panel, `q` switches to Request Body tab instead of quitting. This should be noted
