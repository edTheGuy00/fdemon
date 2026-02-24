## Task: Fix Keybindings Data

**Objective**: Correct the keybindings data in `data.rs` — remove the phantom `l` Layout Panel binding, add the `n` Network Panel binding, add a full "DevTools — Network Monitor" keybinding section, add a "Network Filter Input" section, and add missing Performance panel bindings.

**Depends on**: None

### Scope

- `website/src/data.rs`: Modify `all_keybinding_sections()` to fix and add keybinding sections

### Details

The `data.rs` file defines all keybinding sections displayed on the `/docs/keybindings` page. Based on verified codebase analysis of `crates/fdemon-app/src/handler/keys.rs`, the following corrections are needed:

#### 1. Fix "DevTools — Panel Navigation" section

**Remove** the phantom entry:
```rust
Keybinding { key: "l", action: "Layout Panel", description: "Switch to Layout Explorer panel" },
```

The `DevToolsPanel` enum has only 3 variants: `Inspector`, `Performance`, `Network`. There is no `Layout` variant — pressing `l` in DevTools mode does nothing.

**Add** the Network panel entry:
```rust
Keybinding { key: "n", action: "Network Panel", description: "Switch to Network monitoring panel" },
```

The corrected section should list: `d` (enter), `Esc` (exit), `i` (Inspector), `p` (Performance), `n` (Network), `b` (Browser), `q` (Quit).

#### 2. Add "DevTools — Network Monitor" section

New `KeybindingSection` with **cyan** color (`bg-cyan-500` / `text-cyan-400`) to match other DevTools sections.

Exact bindings from the codebase (`handler/keys.rs` lines 322-449):

| Key | Action | Description |
|-----|--------|-------------|
| `j / ↓` | Navigate Down | Move to next request in the list |
| `k / ↑` | Navigate Up | Move to previous request in the list |
| `PgDn` | Page Down | Skip forward 10 requests |
| `PgUp` | Page Up | Skip back 10 requests |
| `Enter` | Select Request | Open request detail view for the selected request |
| `Esc` | Deselect / Exit | Deselect current request, or exit DevTools if nothing selected |
| `g` | General Tab | Switch detail view to General tab |
| `h` | Headers Tab | Switch detail view to Headers tab |
| `q` | Request Body Tab | Switch detail view to Request Body tab |
| `s` | Response Body Tab | Switch detail view to Response Body tab |
| `t` | Timing Tab | Switch detail view to Timing tab |
| `Space` | Toggle Recording | Start or stop recording network requests |
| `Ctrl+X` | Clear History | Clear all recorded network requests |
| `/` | Enter Filter Mode | Enter filter input mode to type a filter query |

#### 3. Add "Network Filter Input" section

New `KeybindingSection` with **cyan** color. When filter input mode is active, all keys are intercepted:

| Key | Action | Description |
|-----|--------|-------------|
| `Type` | Filter Input | Add character to filter query |
| `Backspace` | Delete Character | Remove last character from filter query |
| `Enter` | Apply Filter | Apply the filter and exit filter input mode |
| `Esc` | Cancel Filter | Discard filter changes and exit filter input mode |

#### 4. Add missing Performance panel bindings

Add a new "DevTools — Performance Monitor" section with **cyan** color:

| Key | Action | Description |
|-----|--------|-------------|
| `s` | Toggle Allocation Sort | Toggle allocation table sort between BySize and ByInstances |
| `←` | Previous Frame | Select the previous frame in the bar chart |
| `→` | Next Frame | Select the next frame in the bar chart |
| `Esc` | Deselect / Exit | Deselect current frame, or exit DevTools if no frame selected |

### Acceptance Criteria

1. The `l` → "Layout Panel" entry is removed from "DevTools — Panel Navigation"
2. The `n` → "Network Panel" entry is added to "DevTools — Panel Navigation"
3. A new "DevTools — Network Monitor" section exists with all 14 bindings listed above
4. A new "Network Filter Input" section exists with 4 bindings
5. A new "DevTools — Performance Monitor" section exists with 4 bindings
6. All new sections use cyan color (`bg-cyan-500`, `text-cyan-400`)
7. Section ordering is logical: Panel Navigation → Debug Overlays → Widget Inspector → Performance Monitor → Network Monitor → Network Filter Input → Search Input Mode
8. Website compiles: `cd website && trunk build`

### Testing

- Visual verification: `cd website && trunk serve` then navigate to `/docs/keybindings`
- Verify all sections render with correct colors and entries
- Verify the phantom `l` entry no longer appears

### Notes

- The `NetworkDetailTab` enum variants are: `General`, `Headers`, `RequestBody`, `ResponseBody`, `Timing` — mapped to keys `g`, `h`, `q`, `s`, `t`
- The `q` key in Network panel maps to "Request Body" tab, NOT quit — quit is suppressed when in Network panel to avoid conflicts
- Detail tab keys (`g/h/q/s/t`) are only functional when a request is selected
