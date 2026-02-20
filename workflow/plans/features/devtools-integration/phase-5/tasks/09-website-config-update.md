## Task: Website — Update Configuration Page with Expanded DevTools Settings

**Objective**: Update the Configuration documentation page on the website to reflect all new `[devtools]` config fields added in Task 01, including the `[devtools.logging]` sub-section.

**Depends on**: 01-expand-devtools-config (to know final field names, types, and defaults)

**Estimated Time**: 3-5 hours

### Scope

- `website/src/pages/docs/configuration.rs`: Expand the DevTools Settings section with all new fields and the logging sub-section

### Details

#### 1. Current State

At `configuration.rs:85-92`, the DevTools Settings section currently shows only 2 fields:

```rust
<Section title="DevTools Settings">
    <CodeBlock language="toml" code="[devtools]\nauto_open = false          # Auto-open DevTools on app start\nbrowser = \"\"               # Browser command (empty = system default)" />
    <SettingsTable entries=vec![
        ("auto_open", "boolean", "false", "Automatically open DevTools in a browser when app starts"),
        ("browser", "string", "\"\"", "Browser command (e.g. \"chrome\", \"firefox\"). Empty = system default"),
    ] />
</Section>
```

#### 2. Expanded Code Block

Replace the `CodeBlock` with the full config:

```rust
<CodeBlock language="toml" code="[devtools]
auto_open = false              # Auto-open DevTools on app start
browser = \"\"                   # Browser command (empty = system default)
default_panel = \"inspector\"    # Default panel: \"inspector\", \"layout\", \"performance\"
performance_refresh_ms = 2000  # Performance data polling interval (ms)
memory_history_size = 60       # Memory snapshots to retain
tree_max_depth = 0             # Widget tree max depth (0 = unlimited)
auto_repaint_rainbow = false   # Auto-enable repaint rainbow on connect
auto_performance_overlay = false # Auto-enable performance overlay on connect

[devtools.logging]
hybrid_enabled = true          # Enable hybrid logging (VM Service + daemon)
prefer_vm_level = true         # Prefer VM Service log level when available
show_source_indicator = false  # Show [VM]/[daemon] tags on log entries
dedupe_threshold_ms = 100      # Dedup threshold for matching logs (ms)" />
```

#### 3. Expanded Settings Table

Replace the `SettingsTable` with all fields. Consider splitting into two tables — one for `[devtools]` and one for `[devtools.logging]`:

**Main `[devtools]` table:**

```rust
<SettingsTable entries=vec![
    ("auto_open", "boolean", "false", "Automatically open DevTools in a browser when app starts"),
    ("browser", "string", "\"\"", "Browser command (e.g. \"chrome\", \"firefox\"). Empty = system default"),
    ("default_panel", "string", "\"inspector\"", "Default panel when entering DevTools mode. Options: \"inspector\", \"layout\", \"performance\""),
    ("performance_refresh_ms", "integer", "2000", "Memory/performance data polling interval in milliseconds. Minimum 500"),
    ("memory_history_size", "integer", "60", "Number of memory snapshots to retain in the ring buffer"),
    ("tree_max_depth", "integer", "0", "Max depth when fetching widget tree. 0 = unlimited"),
    ("auto_repaint_rainbow", "boolean", "false", "Automatically enable repaint rainbow overlay when VM connects"),
    ("auto_performance_overlay", "boolean", "false", "Automatically enable performance overlay when VM connects"),
] />
```

**`[devtools.logging]` sub-section:**

Add a sub-heading and second table:

```rust
<h3 class="text-lg font-bold text-white mt-6">"Logging Settings"</h3>
<p class="text-slate-400 text-sm">
    "Configure hybrid logging behavior when both VM Service and daemon log sources are available."
</p>
<SettingsTable entries=vec![
    ("hybrid_enabled", "boolean", "true", "Enable hybrid logging. When true, merges VM Service logs with daemon stdout logs"),
    ("prefer_vm_level", "boolean", "true", "Use VM Service log level (accurate) over content-based level detection"),
    ("show_source_indicator", "boolean", "false", "Show [VM] or [daemon] tags next to each log entry to indicate its source"),
    ("dedupe_threshold_ms", "integer", "100", "Logs from both sources within this window (ms) with matching content are deduplicated"),
] />
```

#### 4. Add Explanatory Notes

Add helpful context about key settings:

```rust
<div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm">
    <p class="font-medium mb-2">"Notes"</p>
    <ul class="list-disc list-inside space-y-1">
        <li><code class="text-blue-400">"performance_refresh_ms"</code>" controls how often memory usage is polled. Lower values give more granular data but increase VM Service traffic. Frame timing and GC events are streamed in real-time regardless of this setting."</li>
        <li><code class="text-blue-400">"tree_max_depth"</code>" can improve performance for apps with very deep widget trees. A value of 0 (default) fetches the entire tree."</li>
        <li>"Auto-overlay settings ("<code class="text-blue-400">"auto_repaint_rainbow"</code>", "<code class="text-blue-400">"auto_performance_overlay"</code>") activate overlays on the device/emulator screen as soon as the VM Service connects."</li>
    </ul>
</div>
```

#### 5. Verify SettingsTable Component

The `SettingsTable` component is defined locally in `configuration.rs`. Check that it handles the new entries correctly — specifically:
- Long description text wraps properly
- The table doesn't overflow on narrow screens
- `"inspector"` with escaped quotes renders correctly in the default value column

### Acceptance Criteria

1. DevTools Settings section shows all config fields (not just `auto_open` and `browser`)
2. `[devtools.logging]` sub-section is documented with its own heading and table
3. Code block shows the full config example with all fields and comments
4. All field types, defaults, and descriptions are accurate
5. Explanatory notes provide context for key settings
6. Table renders correctly on both desktop and mobile viewports
7. `cd website && trunk build` succeeds

### Testing

- `cd website && trunk build` — compilation check
- Navigate to `/docs/configuration`, scroll to DevTools Settings
- Verify all fields are listed in the table
- Verify code block is syntax-highlighted and copy button works
- Test on narrow viewport (mobile) — table should scroll horizontally if needed
- Cross-reference field names/defaults with `config/types.rs` for accuracy

### Notes

- **This task depends on Task 01** because the config field names, types, and defaults must match the actual `DevToolsSettings` struct. If Task 01 changes any field names or defaults, this page must match.
- **Don't document the `[devtools]` section in the settings panel** — that's a TUI feature, not a website concern. The settings panel is already documented elsewhere on the configuration page.
- **The `SettingsTable` component** is a local helper in `configuration.rs`. If it needs modification (e.g., to support sub-sections), make the changes locally rather than extracting to a shared component.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `website/src/pages/docs/configuration.rs` | Expanded DevTools Settings section from 2 to 8 fields in main table; added `[devtools.logging]` sub-section with heading, description, and 4-field table; expanded CodeBlock with full 14-line TOML config; added explanatory notes callout with 3 bullets |

### Notable Decisions/Tradeoffs

1. **No SettingsTable component changes**: Existing component's overflow handling and responsive layout worked correctly for the expanded content
2. **Notes callout placement**: After both tables to provide context for the whole DevTools section

### Testing Performed

- `trunk build` — Passed
- Configuration page renders correctly with expanded tables

### Risks/Limitations

None
