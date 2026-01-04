# Plan: Log & Config Enhancements

## TL;DR

Enhance Flutter Demon's logging capabilities with filtering, search, and error highlighting. Add terminal hyperlinks (OSC 8) for clickable file:line references. Create an in-app settings UI for managing configuration. Improve the startup flow to allow users to select build mode, flavor, and other launch options interactively.

---

## Background

As Flutter Demon matures, users need more control over log output and application configuration. Currently:

- Logs display all messages without filtering options
- Error stack traces are shown as plain text without visual distinction
- No way to click on file:line references to open in an editor
- Configuration requires manually editing `.fdemon/config.toml`
- Startup always uses the default or auto-start configuration

This feature addresses these usability gaps with a cohesive set of enhancements.

---

## Affected Modules

- `src/core/log.rs` - Add filter and search types
- `src/core/stack_trace.rs` - **NEW** Stack trace parsing
- `src/app/state.rs` - Add filter/search state
- `src/app/message.rs` - Add filter/settings messages
- `src/tui/widgets/log_view.rs` - Enhanced rendering with filters
- `src/tui/widgets/settings_panel.rs` - **NEW** Settings UI widget
- `src/tui/widgets/startup_config.rs` - **NEW** Startup configuration widget
- `src/tui/render.rs` - Add settings/config panel rendering
- `src/tui/actions.rs` - Add new keyboard handlers
- `src/tui/hyperlinks.rs` - **NEW** OSC 8 hyperlink support
- `src/config/types.rs` - Add new configuration options
- `src/config/settings.rs` - Add settings save functionality

---

## Development Phases

### Phase 1: Log Filtering & Search

**Goal**: Allow users to filter logs by level/source and search within logs.

**Duration**: 1-1.5 weeks

#### Steps

1. **Log Filter Types**
   - Add `LogFilter` enum: All, Errors, Warnings, Info, Debug
   - Add `LogSourceFilter`: All, App, Daemon, Watcher
   - Store current filter in `AppState`

2. **Filter UI**
   - Display current filter in log panel header
   - Keyboard shortcuts:
     - `f` - Cycle through level filters
     - `F` - Cycle through source filters
     - `Shift+f` - Reset to All
   - Show filter indicator: `[Errors only]`, `[App logs]`, etc.

3. **Search Functionality**
   - `/` - Open search prompt (like vim)
   - Regex-based search using `regex` crate
   - Highlight matched terms in log entries
   - `n` / `N` - Next/previous match
   - `Escape` - Clear search
   - Show match count: `[3/47 matches]`

4. **Filter Logic**
   - Filter applied on display, not on storage
   - Combine level and source filters
   - Search works on filtered results
   - Persist filter preference to config (optional)

5. **Performance**
   - Lazy evaluation of filters
   - Cache filtered view when filter unchanged
   - Efficient regex matching

**Milestone**: Users can filter and search logs efficiently.

---

### Phase 2: Error Highlighting & Stack Traces

**Goal**: Visually distinguish errors and make stack traces navigable.

**Duration**: 1-1.5 weeks

#### Steps

1. **Stack Trace Parser**
   - `src/core/stack_trace.rs` - Parse Dart stack traces
   - Regex patterns for Dart stack trace formats:
     - `#0      main (package:app/main.dart:15:3)`
     - `package:app/main.dart 15:3  main`
   - Extract: file path, line number, column, function name
   - Handle various formats (Flutter, Dart VM, async traces)

2. **Error Level Styling**
   - Color-code by log level:
     - Error: Red background or bold red text
     - Warning: Yellow text
     - Info: Default/cyan
     - Debug: Dim/gray text
   - Error icon prefix: `✖`, `⚠`, `ℹ`, `●`

3. **Stack Trace Formatting**
   - Indent stack frames under error message
   - Highlight file:line in different color (e.g., blue/underline)
   - Dim package frames (from pub cache)
   - Emphasize project frames (from lib/, test/)

4. **Collapsible Stack Traces**
   - Show first N frames by default (configurable)
   - `Enter` on error to expand/collapse full trace
   - Visual indicator: `▶ 3 more frames...` / `▼ Stack trace:`

5. **Error Summary**
   - Show error count in status bar
   - Quick jump to errors: `e` - next error, `E` - previous error
   - Error summary popup (optional): list of recent errors

**Milestone**: Errors are visually distinct and stack traces are readable.

---

### Phase 3: Terminal Hyperlinks (OSC 8)

**Goal**: Make file:line references clickable to open in editor.

**Duration**: 1 week

#### Steps

1. **OSC 8 Support Detection**
   - Detect terminal hyperlink support
   - Check `$TERM_PROGRAM`, `$COLORTERM`, or use heuristics
   - Graceful fallback if not supported
   - Configuration to force enable/disable

2. **Hyperlink Rendering**
   - `src/tui/hyperlinks.rs` - OSC 8 escape sequence helpers
   - Format: `\x1b]8;;URI\x1b\\TEXT\x1b]8;;\x1b\\`
   - Wrap file:line references in hyperlinks
   - URI format: `file:///path/to/file.dart:15:3`

3. **Editor Integration**
   - Configure editor command in settings
   - Common patterns:
     - VS Code: `code --goto file:line:col`
     - Zed: `zed file:line`
     - Neovim: `nvim +line file`
   - Custom command template: `$EDITOR +$LINE $FILE`

4. **Hyperlink Targets**
   - Stack trace file:line references
   - Error source locations
   - Log messages with file references
   - DevTools URLs (open in browser)

5. **Configuration**
   ```toml
   [ui]
   hyperlinks = true  # or "auto", false
   
   [editor]
   command = "zed"
   open_pattern = "$EDITOR $FILE:$LINE"
   ```

**Milestone**: Clicking on file references opens the file in the configured editor.

---

### Phase 4: Settings UI Panel

**Goal**: In-app interface for viewing and editing configuration.

**Duration**: 1.5-2 weeks

#### Steps

1. **Settings Panel Widget**
   - `src/tui/widgets/settings_panel.rs`
   - Full-screen or modal panel
   - Keyboard shortcut: `,` (comma) to open settings
   - `Escape` or `q` to close

2. **Settings Categories**
   - Organize by section: Behavior, Watcher, UI, DevTools
   - Collapsible category headers
   - Scroll through settings with j/k or arrows

3. **Setting Types & Editing**
   - Boolean: Toggle with Enter or Space
   - Number: Increment/decrement with +/- or type value
   - String: Inline text editing
   - Enum: Cycle through options
   - List: Add/remove items (for paths, extensions)

4. **Visual Feedback**
   - Highlight currently selected setting
   - Show current value and default value
   - Mark modified settings with indicator
   - Description text for each setting

5. **Persistence**
   - Add `save_settings()` to `config/settings.rs`
   - Write changes to `.fdemon/config.toml`
   - Preserve comments and formatting where possible
   - Apply changes immediately (where safe)

6. **Settings Preview**
   - For UI settings, show live preview
   - For behavior settings, explain impact
   - Warn if restart required for some changes

**Milestone**: Users can view and modify all settings from within Flutter Demon.

---

### Phase 5: Startup Configuration UI

**Goal**: Interactive launch configuration selection and creation.

**Duration**: 1-1.5 weeks

#### Steps

1. **Enhanced Device Selector**
   - Current selector shows devices only
   - Add launch config selection step
   - Flow: Select device → Select/create config → Launch

2. **Configuration Selector**
   - List existing configurations from `.fdemon/launch.toml` and `.vscode/launch.json`
   - Show: Name, mode, flavor, device preference
   - Quick launch with Enter
   - `n` - Create new configuration

3. **Quick Config Override**
   - Before launching, show mini-form:
     - Mode: [Debug] Profile Release
     - Flavor: [None] development staging production
     - Extra args: ____
   - Tab/arrow to navigate, Enter to confirm
   - Skip with `Shift+Enter` to use defaults

4. **New Configuration Wizard**
   - Step-by-step creation flow
   - Name, device, mode, flavor, dart-defines
   - Save to `.fdemon/launch.toml`
   - Option to set as default/auto-start

5. **Launch History**
   - Remember last N launches
   - Quick re-launch recent configuration
   - Show in device selector: "Recent: iPhone 15 (debug)"

**Milestone**: Users can select, create, and customize launch configurations at startup.

---

## Edge Cases & Risks

### Log Filtering
- **Risk**: Filtering hides important errors
- **Mitigation**: Always show error count; quick jump to errors regardless of filter

### Search Performance
- **Risk**: Regex on large log buffers is slow
- **Mitigation**: Limit search scope; use compiled regex; add search timeout

### OSC 8 Compatibility
- **Risk**: Escape sequences corrupt output in unsupported terminals
- **Mitigation**: Robust detection; easy disable option; test popular terminals

### Settings Persistence
- **Risk**: Overwriting config loses user comments/formatting
- **Mitigation**: Use TOML-preserving library or manual file updating

### Editor Command Execution
- **Risk**: Security risk with arbitrary command execution
- **Mitigation**: Sanitize file paths; whitelist known editors; show command before execution

### Cross-Platform Paths
- **Risk**: File paths in hyperlinks differ by platform
- **Mitigation**: Use platform-appropriate path formatting; handle symlinks

---

## Configuration Additions

Add to `.fdemon/config.toml`:

```toml
[ui]
# Enable terminal hyperlinks (OSC 8)
# Values: true, false, "auto" (detect support)
hyperlinks = "auto"

# Default log filter level
default_log_filter = "all"  # "all", "errors", "warnings", "info", "debug"

# Default log source filter
default_source_filter = "all"  # "all", "app", "daemon", "watcher"

# Stack trace display
stack_trace_collapsed = true
stack_trace_max_frames = 5

[editor]
# Editor name or path
command = "zed"

# Pattern for opening file at line
# Variables: $FILE, $LINE, $COLUMN
open_pattern = "$EDITOR $FILE:$LINE"

[startup]
# Show configuration selector at startup
show_config_selector = true

# Remember last used configuration
remember_last_config = true

# Quick launch (skip config selector if auto-start available)
quick_launch = false
```

---

## New Dependencies

No new crate dependencies required. Uses existing:
- `regex` (already available via other deps)
- `crossterm` for terminal detection
- `toml` for config editing

---

## Keyboard Shortcuts Summary

### Log Filtering & Search
| Key | Action |
|-----|--------|
| `f` | Cycle log level filter |
| `F` | Cycle log source filter |
| `Shift+f` | Reset all filters |
| `/` | Open search prompt |
| `n` | Next search match |
| `N` | Previous search match |
| `Escape` | Clear search |
| `e` | Jump to next error |
| `E` | Jump to previous error |

### Stack Traces
| Key | Action |
|-----|--------|
| `Enter` | Expand/collapse stack trace |
| `o` | Open file at cursor in editor |

### Settings
| Key | Action |
|-----|--------|
| `,` | Open settings panel |
| `j`/`k` | Navigate settings |
| `Enter`/`Space` | Toggle/edit setting |
| `Escape` | Close settings |
| `Ctrl+s` | Save settings |

### Startup
| Key | Action |
|-----|--------|
| `Tab` | Next field in config form |
| `Shift+Tab` | Previous field |
| `Enter` | Confirm and launch |
| `Shift+Enter` | Launch with defaults |
| `n` | New configuration |

---

## Success Criteria

### Phase 1 Complete When:
- [ ] Log filtering by level works (All/Errors/Warnings/Info/Debug)
- [ ] Log filtering by source works (All/App/Daemon/Watcher)
- [ ] Search with regex highlights matches
- [ ] Next/previous match navigation works
- [ ] Filter state shown in UI

### Phase 2 Complete When:
- [ ] Dart stack traces parsed correctly
- [ ] Error messages visually distinct (color-coded)
- [ ] File:line references highlighted
- [ ] Stack traces collapsible
- [ ] Quick jump to errors works

### Phase 3 Complete When:
- [ ] OSC 8 hyperlinks render in supported terminals
- [ ] Clicking file:line opens editor at correct location
- [ ] Graceful fallback in unsupported terminals
- [ ] Editor command configurable

### Phase 4 Complete When:
- [ ] Settings panel opens with `,` key
- [ ] All setting types editable (bool, number, string, enum)
- [ ] Changes saved to `.fdemon/config.toml`
- [ ] Settings apply immediately where applicable

### Phase 5 Complete When:
- [ ] Configuration selector shown at startup
- [ ] Existing configs loaded from .fdemon and .vscode
- [ ] Quick config override (mode/flavor) works
- [ ] New configuration creation wizard works
- [ ] Launch history tracked and accessible

---

## Future Enhancements

After core enhancements are complete, consider:

1. **Log Export** - Export filtered logs to file (JSON, plain text)
2. **Log Bookmarks** - Mark important log entries for quick reference
3. **Smart Grouping** - Auto-group related logs (e.g., HTTP request/response pairs)
4. **Log Diffing** - Compare logs between runs
5. **Theme Editor** - Visual theme customization in settings panel
6. **Keyboard Shortcut Customization** - Remap keys in settings

---

## References

- [OSC 8 Hyperlink Specification](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda)
- [Dart Stack Trace Format](https://dart.dev/guides/language/language-tour#exceptions)
- [Ratatui Input Handling](https://ratatui.rs/concepts/event-handling/)
- [TOML Format Specification](https://toml.io/en/)