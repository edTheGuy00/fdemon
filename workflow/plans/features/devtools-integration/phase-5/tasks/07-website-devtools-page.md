## Task: Website — New DevTools Documentation Page

**Objective**: Create a new `/docs/devtools` page on the Leptos website covering all DevTools features: panels, keybindings, debug overlays, browser fallback, connection states, and configuration.

**Depends on**: 01-expand-devtools-config, 02-connection-state-ui (to document final features)

**Estimated Time**: 6-8 hours

### Scope

- `website/src/pages/docs/devtools.rs`: **NEW** — Full DevTools documentation page as a Leptos component
- `website/src/pages/docs/mod.rs`: Register module, add sidebar entry
- `website/src/lib.rs`: Add route for `/docs/devtools`
- `website/src/components/icons.rs`: Add any needed icons (e.g., `Monitor` or `Wrench` for sidebar)

### Details

#### 1. Create the Page Component

Create `website/src/pages/docs/devtools.rs` following the conventions of existing pages:

```rust
use leptos::prelude::*;
use crate::components::code_block::CodeBlock;

#[component]
pub fn Devtools() -> impl IntoView {
    view! {
        <div class="animate-fade-in space-y-8">
            <h1 class="text-3xl font-bold text-white">"DevTools Integration"</h1>
            <p class="text-lg text-slate-400">
                "Built-in Flutter DevTools — inspect widgets, explore layouts, and monitor performance without leaving the terminal."
            </p>
            // ... sections below ...
        </div>
    }
}
```

#### 2. Page Sections

Structure the page with these sections (use local `Section`, `KeyRow`, `Tip` helper components as other pages do):

**Section 1: Overview**
- Brief explanation of what DevTools integration provides
- How it works: connects to Flutter VM Service via WebSocket
- Requirements: Flutter app running in debug mode
- Quick start: "Press `d` to enter DevTools mode"

**Section 2: Entering & Exiting DevTools**
- `d` in Normal mode → enters DevTools mode (replaces log view)
- `Esc` → returns to log view
- Requires at least one active session with VM Service connection
- The app header and session tabs remain visible above DevTools panels

**Section 3: Widget Inspector (`i`)**
- What it shows: Flutter widget tree from the running app
- Navigation: `↑`/`k` and `↓`/`j` to move, `→`/`Enter` to expand, `←`/`h` to collapse
- Details panel: shows widget type, description, creation location
- User-code highlighting: user widgets styled differently from framework widgets
- Refresh: `r` to re-fetch the tree from the VM
- Include a description or mockup of the inspector layout (60% tree / 40% details, or stacked in narrow terminals)

**Section 4: Layout Explorer (`l`)**
- What it shows: flex layout data for the widget selected in the Inspector
- Constraints box: min/max width and height with "tight" indicators
- Size visualization: proportional rendering of widget dimensions
- Flex properties: mainAxisAlignment, crossAxisAlignment, flex factor, fit
- Auto-fetches when switching to this panel (if a widget is selected)

**Section 5: Performance Monitor (`p`)**
- What it shows: real-time performance data from the running app
- FPS sparkline: 300-frame rolling window with color coding (green/yellow/red)
- Memory gauge: heap usage, capacity, external memory
- Stats: frame count, jank percentage, P95 frame time, average frame time
- GC history: recent garbage collection events
- Data is streamed continuously (no manual refresh needed)

**Section 6: Debug Overlays**
- `Ctrl+r` — Repaint rainbow: highlights widgets that are repainting
- `Ctrl+p` — Performance overlay: shows GPU/UI thread timing
- `Ctrl+d` — Debug paint: shows widget boundaries and padding
- Overlays are toggled on the device/emulator, not in the terminal
- Active overlays shown as indicators in the DevTools tab bar

**Section 7: Browser DevTools**
- `b` — Opens Flutter DevTools in a browser
- Uses the VM Service URI to construct a local DDS URL
- Configurable browser: `[devtools] browser = "chrome"` in config
- Useful for features not available in the TUI (timeline, network inspector, etc.)

**Section 8: Connection States**
- Connected: normal operation
- Reconnecting: auto-reconnects with exponential backoff, shows indicator
- Disconnected: panels show informative messages with retry hints
- Timeout: slow responses are handled gracefully

**Section 9: Configuration**
- Show the full `[devtools]` config block with all options
- Brief table of all settings with types and defaults
- Link to the Configuration page for full details
- Mention the settings panel (`comma` key) for live editing

**Section 10: Keybindings Quick Reference**
- Table of all DevTools keybindings organized by category
- Link to the full Keybindings page

#### 3. Register the Page

**In `pages/docs/mod.rs`:**

Add module declaration:
```rust
pub mod devtools;
```

Add sidebar entry to `doc_items()` — insert between "Keybindings" and "Configuration":
```rust
DocItem {
    href: "/docs/devtools",
    label: "DevTools",
    icon: || view! { <Eye class="w-4 h-4 mr-3" /> }.into_any(),
},
```

The `Eye` icon is already available in `components/icons.rs`. Alternatively, consider `Cpu` (already used for Architecture) or add a new icon like `Monitor`.

**In `lib.rs`:**

Add import:
```rust
use pages::docs::devtools::Devtools;
```

Add route inside `<ParentRoute path=path!("/docs") ...>`:
```rust
<Route path=path!("/devtools") view=Devtools />
```

Place it after `/keybindings` and before `/configuration` to match sidebar order.

#### 4. Design Conventions to Follow

From the existing pages:

- **Root div**: `<div class="animate-fade-in space-y-8">`
- **Headings**: `text-3xl font-bold text-white` (h1), `text-xl font-bold text-white` (h2)
- **Section pattern**: Use a local `Section` component with blue left-border indicator:
  ```rust
  #[component]
  fn Section(title: &'static str, children: Children) -> impl IntoView {
      view! {
          <section class="space-y-4">
              <h2 class="text-xl font-bold text-white flex items-center">
                  <div class="w-2 h-6 bg-blue-500 mr-3 rounded-full"></div>
                  {title}
              </h2>
              {children()}
          </section>
      }
  }
  ```
- **Tables**: Overflow-hidden rounded-lg, `border-slate-800`, thead `bg-slate-900`, tbody `bg-slate-950 divide-y divide-slate-800`
- **Code blocks**: `<CodeBlock language="toml" code="..." />`
- **Info callouts**: `bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm`
- **Inline code**: `<code class="text-blue-400">`
- **Body text**: `text-slate-400`
- **All strings quoted**: Use `"text"` syntax in `view!` macros (Leptos convention)

#### 5. Add New Icon (If Needed)

If you want a `Monitor` or `Wrench` icon not in `icons.rs`, add it using the `lucide_icon!` macro:

```rust
lucide_icon!(Monitor,
    <rect x="2" y="3" width="20" height="14" rx="2"></rect>
    <line x1="8" y1="21" x2="16" y2="21"></line>
    <line x1="12" y1="17" x2="12" y2="21"></line>
);
```

Lucide icons can be found at https://lucide.dev. Use viewBox 24x24, stroke-based.

### Acceptance Criteria

1. `/docs/devtools` route renders a full DevTools documentation page
2. Page appears in the sidebar navigation between Keybindings and Configuration
3. All three panels (Inspector, Layout, Performance) are documented with their keybindings
4. Debug overlay section covers all three toggles
5. Browser DevTools fallback is documented
6. Configuration section shows the full `[devtools]` config block
7. Keybindings quick reference table is present
8. Connection states section explains reconnection and timeout behavior
9. Page follows the site's design conventions (dark theme, section pattern, code blocks)
10. Page renders correctly on mobile (responsive sidebar)
11. `cd website && trunk build` succeeds without errors

### Testing

- `cd website && trunk build` — verifies compilation
- Manual browser testing at `http://localhost:8080/docs/devtools`
- Verify sidebar active state highlights correctly
- Verify mobile menu toggle works
- Check responsive layout at various widths

### Notes

- **This is the largest website task** because it's a fully new page with ~10 sections. Consider structuring the component with sub-components to keep it organized.
- **All content is Rust code, not Markdown.** Every string, heading, and paragraph is a Leptos `view!` macro expression. This is more verbose than Markdown but provides full control over layout.
- **Don't create terminal mockup screenshots.** ASCII representations of the panels are fine for the page content. The existing `terminal_mockup.rs` component is only used on the home page.
- **The `CodeBlock` component supports `language` and `code` props.** Use `language="toml"` for config examples and omit language for key shortcut examples.
