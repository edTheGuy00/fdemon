## Task: Website Native Logs Documentation Page

**Objective**: Create a new Leptos documentation page at `/docs/native-logs` on the fdemon website, covering the full native platform logs feature including platform support, configuration, tag filtering, and custom sources.

**Depends on**: 04-app-custom-source-integration (implementation should be complete so docs match reality)

### Scope

- `website/src/pages/docs/native_logs.rs` — **NEW** Leptos component
- `website/src/pages/docs/mod.rs` — Add `pub mod native_logs;` + sidebar entry
- `website/src/lib.rs` — Register route
- `website/src/data.rs` — Add `T` key to keybindings data

### Details

#### 1. New Page Component (`native_logs.rs`)

Follow the established Leptos docs page pattern. Use the same private sub-components (`Section`, `SettingsTable`, `Tip`, `CodeBlock`) as other pages.

**Page sections:**

1. **Header + Overview**
   - "Native Platform Logs" title
   - Brief description: fdemon automatically captures native platform logs alongside Flutter's Dart output
   - Platform support card grid (3 cards):
     - Android: via `adb logcat` — captures Kotlin, Java, Go plugin logs
     - iOS: via `idevicesyslog` / `xcrun simctl log stream` — captures Swift, ObjC plugin logs
     - macOS: via `log stream` — captures `NSLog`, `os_log` from native plugins

2. **How It Works**
   - Automatic: native log capture starts when your Flutter app starts (after `AppStarted`)
   - Platform detection: fdemon detects the target platform and spawns the appropriate capture tool
   - Tag discovery: as native logs arrive, tags are automatically discovered and shown in the tag filter
   - Filtering: two-tier — config-level (`min_level`, `exclude_tags`) and UI-level (tag filter overlay)

3. **Supported Platforms**
   - **Android**: PID-based filtering via `adb logcat --pid`. Captures all tags from the app process. `flutter` tag excluded by default to avoid duplication.
   - **iOS Physical**: `idevicesyslog -u <udid> -p Runner`. Requires `libimobiledevice` installed.
   - **iOS Simulator**: `xcrun simctl spawn <udid> log stream`. Uses macOS unified logging format.
   - **macOS Desktop**: `log stream --predicate 'process == "<app>"'`. Captures `NSLog`, `os_log`.
   - **Linux/Windows/Web**: Already covered by stdout/stderr pipes — no additional capture needed.
   - Info callout: "fdemon automatically detects the platform and spawns the right tool. If the tool is not available, native log capture is silently skipped."

4. **Tag Filter UI**
   - Press `T` in the log view to open the tag filter overlay
   - Shows all discovered tags with entry counts
   - Toggle individual tags with Space
   - `a` = show all, `n` = hide all, `Esc` = close
   - Screenshot/description of the overlay appearance

5. **Configuration**
   - `SettingsTable` for `[native_logs]` settings
   - `CodeBlock` for per-tag overrides example
   - `CodeBlock` for custom sources example

6. **Custom Log Sources**
   - Explain the concept: define arbitrary commands whose output is parsed as log entries
   - Format options table: raw, json, logcat-threadtime, syslog
   - Examples with `CodeBlock`:
     - Tail a log file
     - JSON log stream
     - Filtered logcat for specific tags
   - Tip: "Custom source tags appear in the tag filter overlay alongside platform tags."

7. **Troubleshooting**
   - "Native logs not appearing?" — check tool availability, platform support, `enabled` setting
   - "Too many tags?" — use `exclude_tags` or per-tag `min_level` overrides
   - "Duplicate logs?" — `flutter` tag excluded by default, but check if `exclude_tags` was modified

#### 2. Sidebar Entry (`mod.rs`)

Add to the `doc_items()` vec in `mod.rs`:

```rust
DocItem {
    title: "Native Logs",
    path: "/docs/native-logs",
    icon: view! { <Terminal size=16 /> },  // or Layers icon
    description: "Native platform log capture",
},
```

Position it after "DevTools" and before "Debugging" (or wherever feature docs are grouped).

Add `pub mod native_logs;` to the module declarations.

#### 3. Route Registration (`lib.rs`)

Add to the docs `ParentRoute` in `lib.rs`:

```rust
use pages::docs::native_logs::NativeLogs;
// ...
<Route path=path!("/native-logs") view=NativeLogs />
```

#### 4. Keybindings Data Update (`data.rs`)

Add the `T` key to the "Log Filtering" section in `all_keybinding_sections()`:

```rust
Keybinding {
    key: "T",
    action: "Tag Filter Overlay",
    description: "Open/close native platform log tag filter overlay",
},
```

### Acceptance Criteria

1. `/docs/native-logs` route works and renders the page
2. Page appears in the sidebar navigation with an icon
3. All sections render correctly: overview, how it works, platforms, tag filter, config, custom sources, troubleshooting
4. `SettingsTable` shows all config options with correct types and defaults
5. `CodeBlock` examples contain valid TOML configuration
6. `T` key appears in the keybindings data (visible on `/docs/keybindings` page)
7. Internal links work (e.g., link to `/docs/keybindings`, `/docs/configuration`)
8. Page follows the visual style of existing docs pages (Tailwind classes, Section/Tip/CodeBlock components)

### Testing

- Manual: build the website (`cd website && trunk serve`) and verify:
  - Page accessible at `/docs/native-logs`
  - Sidebar link works
  - All sections render
  - Code blocks display correctly
  - Keybindings page shows `T` key
- No automated tests needed (Leptos component)

### Notes

- The website is a Leptos (Rust/WASM) SPA using Tailwind CSS
- Each docs page defines its own private `Section`, `SettingsTable`, etc. sub-components (they're not shared across pages)
- Use `CodeBlock` component from `crate::components::code_block::CodeBlock` for code examples
- Available icons in `components/icons.rs`: `Terminal` (line 30) is a good fit for native logs
- The docs sidebar in `mod.rs` uses `DocItem` structs with `title`, `path`, `icon`, `description` fields
- Keep the page content focused and scannable — use short paragraphs, tables, and code blocks over long prose
