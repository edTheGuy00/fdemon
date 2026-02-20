## Task: Website — Update Keybindings Data & Page

**Objective**: Fix the `d` key entry in the website's keybinding data (currently shows `+ / d` for "Start New Session" — `d` was reassigned to DevTools in Phase 4), and add a new DevTools Mode keybinding section to `data.rs`.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

- `website/src/data.rs`: Fix `d` key in Session Management section, add DevTools Mode keybinding sections
- `website/src/pages/docs/keybindings.rs`: Verify the page renders the new sections correctly (it's data-driven from `data.rs`)
- `website/src/pages/docs/introduction.rs`: Update the "Essential Keybindings" quick reference table if it mentions `d` for new session

### Details

#### 1. Fix `d` Key in Session Management

At `data.rs:74`, the Session Management section has:

```rust
Keybinding { key: "+ / d", action: "Start New Session", description: "Open New Session Dialog to configure and launch a session" },
```

**Fix**: Change to just `+` since `d` is now DevTools mode:

```rust
Keybinding { key: "+", action: "Start New Session", description: "Open New Session Dialog to configure and launch a session" },
```

#### 2. Add DevTools Mode Keybinding Sections

Add three new `KeybindingSection` entries to `all_keybinding_sections()` in `data.rs`. Place them after the "Log Search & Error Navigation" section and before the "New Session Dialog" section (to match the flow: normal mode → devtools mode → modal dialogs):

```rust
// ── DevTools Mode ─────────────────────────────────────────
KeybindingSection {
    title: "DevTools — Panel Navigation",
    color: "bg-cyan-500",
    key_color: "text-cyan-400",
    bindings: vec![
        Keybinding { key: "d", action: "Enter DevTools", description: "Enter DevTools mode (requires VM Service connection)" },
        Keybinding { key: "Esc", action: "Exit DevTools", description: "Return to Normal mode (log view)" },
        Keybinding { key: "i", action: "Inspector Panel", description: "Switch to Widget Inspector panel" },
        Keybinding { key: "l", action: "Layout Panel", description: "Switch to Layout Explorer panel" },
        Keybinding { key: "p", action: "Performance Panel", description: "Switch to Performance monitoring panel" },
        Keybinding { key: "b", action: "Browser DevTools", description: "Open Flutter DevTools in system browser" },
        Keybinding { key: "q", action: "Quit", description: "Quit the application" },
    ],
},
KeybindingSection {
    title: "DevTools — Debug Overlays",
    color: "bg-cyan-500",
    key_color: "text-cyan-400",
    bindings: vec![
        Keybinding { key: "Ctrl+r", action: "Repaint Rainbow", description: "Toggle repaint rainbow overlay on device" },
        Keybinding { key: "Ctrl+p", action: "Performance Overlay", description: "Toggle performance overlay on device" },
        Keybinding { key: "Ctrl+d", action: "Debug Paint", description: "Toggle debug paint overlay on device" },
    ],
},
KeybindingSection {
    title: "DevTools — Widget Inspector",
    color: "bg-cyan-500",
    key_color: "text-cyan-400",
    bindings: vec![
        Keybinding { key: "\u{2191} / k", action: "Move Up", description: "Move selection up in widget tree" },
        Keybinding { key: "\u{2193} / j", action: "Move Down", description: "Move selection down in widget tree" },
        Keybinding { key: "\u{2192} / Enter", action: "Expand", description: "Expand selected tree node" },
        Keybinding { key: "\u{2190} / h", action: "Collapse", description: "Collapse selected tree node" },
        Keybinding { key: "r", action: "Refresh", description: "Refresh widget tree from VM Service" },
    ],
},
```

**Color choice**: Use `bg-cyan-500` / `text-cyan-400` (cyan) for DevTools sections to distinguish them from Normal mode (blue) and modal dialogs (green). This matches the TUI's cyan DevTools tab bar border.

#### 3. Verify Introduction Page

Check `pages/docs/introduction.rs` for any quick reference table that mentions `d` for "New Session". The introduction page has an "Essential Keybindings" section — update it to show `d` for DevTools and `+` for New Session:

```rust
// Look for something like:
KeyRow { key: "d", desc: "New session" }
// Change to:
KeyRow { key: "d", desc: "DevTools mode" }
KeyRow { key: "+", desc: "New session" }
```

#### 4. Verify Keybindings Page Rendering

The keybindings page at `pages/docs/keybindings.rs` is data-driven — it iterates over `all_keybinding_sections()` and renders each section. Verify that:
1. The new DevTools sections render correctly
2. The cyan color scheme works (check that `bg-cyan-500` and `text-cyan-400` are valid Tailwind v4 classes)
3. Unicode arrows (`\u{2191}`, `\u{2193}`, `\u{2190}`, `\u{2192}`) render correctly in the browser

### Acceptance Criteria

1. Session Management section shows `+` (not `+ / d`) for "Start New Session"
2. Three new DevTools keybinding sections appear on the keybindings page
3. DevTools sections use cyan color scheme to distinguish from other modes
4. All DevTools keybindings match the actual implementation (`docs/KEYBINDINGS.md`)
5. Introduction page's quick reference table updated (if it mentions `d`)
6. Unicode arrow characters render correctly in all browsers
7. `cd website && trunk build` succeeds

### Testing

- `cd website && trunk build` — compilation check
- Manual browser testing: navigate to `/docs/keybindings`, verify DevTools sections appear
- Verify `d` no longer appears in Session Management row
- Compare website keybindings with `docs/KEYBINDINGS.md` for accuracy
- Test on mobile viewport to verify sections don't overflow

### Notes

- **The keybindings page is fully data-driven.** All changes go in `data.rs` — the page component itself (`keybindings.rs`) should not need modification unless the rendering logic needs adjustment for the new sections.
- **Unicode characters**: The existing data already uses `\u{2191}` etc. for arrow keys, so the rendering pipeline handles them. No special handling needed.
- **Tailwind v4 color classes**: `cyan-400` and `cyan-500` are standard Tailwind colors and should work. If the site uses a custom color palette, check `tailwind.config.js` or `Trunk.toml` for overrides.
