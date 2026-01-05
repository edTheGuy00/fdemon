## Task: Documentation Update

**Objective**: Update KEYBINDINGS.md and CONFIGURATION.md to document the new settings panel.

**Depends on**: 12-init-gitignore

**Estimated Time**: 0.5-1 hour

### Scope

- `docs/KEYBINDINGS.md`: Add settings panel shortcuts
- `docs/CONFIGURATION.md`: Document file structure and settings

### Details

#### 1. KEYBINDINGS.md Additions

Add a new section for Settings Panel:

```markdown
## Settings Panel

| Key | Action |
|-----|--------|
| `,` | Open settings panel |
| `Escape` / `q` | Close settings panel |
| `Tab` | Next tab |
| `Shift+Tab` | Previous tab |
| `1` | Jump to Project Settings tab |
| `2` | Jump to User Preferences tab |
| `3` | Jump to Launch Config tab |
| `4` | Jump to VSCode Config tab |
| `j` / `↓` | Select next setting |
| `k` / `↑` | Select previous setting |
| `Enter` / `Space` | Toggle boolean / Edit value / Cycle enum |
| `+` / `=` | Increment number |
| `-` | Decrement number |
| `←` / `→` | Cycle enum options |
| `d` | Remove last list item |
| `Ctrl+S` | Save settings |
| `Backspace` | Delete character (when editing) |
| `Delete` | Clear edit buffer |
```

#### 2. CONFIGURATION.md Additions

Add sections for file structure and settings:

```markdown
## Configuration Files

Flutter Demon uses several configuration files in the `.fdemon/` directory:

### File Overview

| File | Purpose | Tracked in Git? |
|------|---------|-----------------|
| `.fdemon/config.toml` | Project settings (shared) | Yes |
| `.fdemon/launch.toml` | Launch configurations | Yes |
| `.fdemon/settings.local.toml` | User preferences | No (gitignored) |

### config.toml

Project-wide settings shared across the team:

```toml
[behavior]
auto_start = false      # Skip device selector at startup
confirm_quit = true     # Ask before quitting with running apps

[watcher]
paths = ["lib"]         # Directories to watch for changes
debounce_ms = 500       # Debounce delay in milliseconds
auto_reload = true      # Trigger hot reload on file changes
extensions = ["dart"]   # File extensions to watch

[ui]
log_buffer_size = 10000       # Maximum log entries
show_timestamps = true        # Show timestamps in logs
compact_logs = false          # Collapse similar logs
theme = "default"             # Color theme
stack_trace_collapsed = true  # Start stack traces collapsed
stack_trace_max_frames = 3    # Frames shown when collapsed

[devtools]
auto_open = false       # Auto-open DevTools on app start
browser = ""            # Browser for DevTools (empty = system default)

[editor]
command = ""            # Editor command (empty = auto-detect)
open_pattern = "$EDITOR $FILE:$LINE"  # File open pattern
```

### launch.toml

Launch configurations for different development scenarios:

```toml
[[configurations]]
name = "Development"
device = "auto"
mode = "debug"
auto_start = true

[configurations.dart_defines]
API_URL = "https://dev.api.com"

[[configurations]]
name = "Production"
device = "ios"
mode = "release"
flavor = "production"
```

**Launch Configuration Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Display name |
| `device` | string | Device ID, platform prefix, or "auto" |
| `mode` | string | "debug", "profile", or "release" |
| `flavor` | string | Build flavor (optional) |
| `entry_point` | path | Custom entry point (optional) |
| `dart_defines` | map | --dart-define values |
| `extra_args` | list | Additional flutter run arguments |
| `auto_start` | bool | Start automatically on launch |

### settings.local.toml

User-specific preferences that override project settings. This file is automatically added to `.gitignore`:

```toml
# User-specific preferences (not tracked in git)

[editor]
command = "nvim"
open_pattern = "nvim +$LINE $FILE"

# Theme override
theme = "dark"

# Last session memory (auto-populated)
last_device = "iPhone 15 Pro"
last_config = "Development"
```

**Note**: Local settings only override compatible fields. Not all project settings can be overridden locally.

## Settings Panel

Access the settings panel by pressing `,` from normal mode. The panel provides four tabs:

1. **Project** - Edit `config.toml` settings (shared with team)
2. **User** - Edit local preferences (gitignored)
3. **Launch** - Manage launch configurations
4. **VSCode** - View VSCode launch.json (read-only)

### Tab Navigation

- `Tab` / `Shift+Tab`: Cycle through tabs
- `1-4`: Jump directly to a tab
- `j/k` or arrows: Navigate settings within a tab
- `Enter`/`Space`: Edit selected setting

### Editing

- **Booleans**: Toggle with Enter/Space
- **Numbers**: Use +/- to increment, or type directly
- **Strings**: Type to replace, Backspace to delete
- **Enums**: Cycle with Enter/Space or arrow keys
- **Lists**: Enter to add item, 'd' to remove last

### Saving

- `Ctrl+S`: Save current tab's settings
- Closing with unsaved changes prompts for confirmation
```

#### 3. Update Section References

Ensure the table of contents and cross-references are updated:

```markdown
## Table of Contents

- [Global Shortcuts](#global-shortcuts)
- [Log View](#log-view)
- [Device Selector](#device-selector)
- [Settings Panel](#settings-panel)  <!-- NEW -->
- [Search](#search)
```

### Acceptance Criteria

1. KEYBINDINGS.md includes all settings panel shortcuts
2. CONFIGURATION.md documents file structure
3. All three config files documented with examples
4. Launch configuration fields documented
5. Settings panel usage documented
6. Table of contents updated
7. Cross-references working
8. Markdown renders correctly

### Testing

Documentation testing is manual:

1. Review markdown renders correctly in GitHub/VSCode preview
2. All links work
3. Code blocks have proper syntax highlighting
4. Tables are properly formatted
5. Examples are accurate and match implementation

### Notes

- Keep documentation concise - link to detailed examples
- Use tables for settings reference (scannable)
- Include examples showing common use cases
- Document which settings require restart (if any)
- Future: Consider auto-generating settings reference from code

---

## Completion Summary

**Status:** (Not Started)

**Files Modified:**
- (To be filled after implementation)

**Implementation Details:**

(To be filled after implementation)

**Testing Performed:**
- Markdown preview verified
- Links tested
- Tables render correctly

**Notable Decisions:**
- (To be filled after implementation)
