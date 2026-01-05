# Phase 4: Settings UI Panel - Task Index

## Overview

Build a full-screen settings panel with tabbed navigation for managing project settings, user preferences, launch configurations, and VSCode compatibility. The design prioritizes modularity and extensibility for future features (Build Runner Manager, Log Persistence, etc.).

**Total Tasks:** 13
**Estimated Hours:** 18-24 hours

## Architecture Overview

```
.fdemon/
├── config.toml          # Project settings (tracked in git, shared)
├── launch.toml          # Launch configurations (tracked in git, shared)
└── settings.local.toml  # User preferences (gitignored, per-developer)

.vscode/
└── launch.json          # VSCode launch configs (read-only display)
```

### File Purposes

| File | Shared? | Description |
|------|---------|-------------|
| `config.toml` | Yes | Project-wide settings (behavior, watcher, ui, devtools, editor) |
| `launch.toml` | Yes | Launch configurations (device, mode, flavor, dart-defines) |
| `settings.local.toml` | No | User-specific overrides (e.g., preferred editor, theme) |
| `.vscode/launch.json` | Yes | VSCode Dart configurations (read-only in fdemon) |

## Task Dependency Graph

```
┌─────────────────────────┐     ┌─────────────────────────┐
│  01-settings-types      │     │  02-local-settings-file │
│  (core types, traits)   │     │  (file format, gitignore)│
└──────────┬──────────────┘     └───────────┬─────────────┘
           │                                 │
           └────────────┬────────────────────┘
                        ▼
           ┌─────────────────────────┐
           │  03-ui-mode-settings    │
           │  (UiMode, `,` shortcut) │
           └──────────┬──────────────┘
                      ▼
           ┌─────────────────────────┐
           │  04-settings-widget     │
           │  (full-screen widget)   │
           └──────────┬──────────────┘
                      ▼
           ┌─────────────────────────┐
           │  05-tab-navigation      │
           │  (tab bar, switching)   │
           └──────────┬──────────────┘
                      │
     ┌────────────────┼────────────────┬────────────────┐
     ▼                ▼                ▼                ▼
┌─────────┐    ┌─────────┐    ┌─────────────┐    ┌─────────┐
│ 06-proj │    │ 07-user │    │ 08-launch   │    │ 09-vscode│
│ settings│    │ prefs   │    │ config tab  │    │ tab      │
└────┬────┘    └────┬────┘    └──────┬──────┘    └────┬────┘
     │              │                │                 │
     └──────────────┴────────┬───────┴─────────────────┘
                             ▼
                 ┌─────────────────────────┐
                 │  10-setting-editors     │
                 │  (bool, num, str, enum) │
                 └──────────┬──────────────┘
                            ▼
                 ┌─────────────────────────┐
                 │  11-settings-persistence│
                 │  (save to disk)         │
                 └──────────┬──────────────┘
                            ▼
                 ┌─────────────────────────┐
                 │  12-init-gitignore      │
                 │  (directory & gitignore)│
                 └──────────┬──────────────┘
                            ▼
                 ┌─────────────────────────┐
                 │  13-documentation       │
                 │  (KEYBINDINGS, CONFIG)  │
                 └─────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-settings-types](tasks/01-settings-types.md) | Not Started | - | 1.5-2h | `config/types.rs` |
| 2 | [02-local-settings-file](tasks/02-local-settings-file.md) | Not Started | - | 1-1.5h | `config/settings.rs` |
| 3 | [03-ui-mode-settings](tasks/03-ui-mode-settings.md) | Not Started | 1, 2 | 1-1.5h | `app/state.rs`, `app/message.rs`, `app/handler/keys.rs` |
| 4 | [04-settings-widget](tasks/04-settings-widget.md) | Not Started | 3 | 2-3h | `tui/widgets/settings_panel.rs` **NEW** |
| 5 | [05-tab-navigation](tasks/05-tab-navigation.md) | Not Started | 4 | 1.5-2h | `tui/widgets/settings_panel.rs` |
| 6 | [06-project-settings-tab](tasks/06-project-settings-tab.md) | Not Started | 5 | 1.5-2h | `tui/widgets/settings_panel.rs` |
| 7 | [07-user-preferences-tab](tasks/07-user-preferences-tab.md) | Not Started | 5 | 1.5-2h | `tui/widgets/settings_panel.rs` |
| 8 | [08-launch-config-tab](tasks/08-launch-config-tab.md) | Not Started | 5 | 1.5-2h | `tui/widgets/settings_panel.rs` |
| 9 | [09-vscode-config-tab](tasks/09-vscode-config-tab.md) | Not Started | 5 | 1-1.5h | `tui/widgets/settings_panel.rs` |
| 10 | [10-setting-editors](tasks/10-setting-editors.md) | Not Started | 6, 7, 8 | 2-3h | `tui/widgets/settings_panel.rs` |
| 11 | [11-settings-persistence](tasks/11-settings-persistence.md) | Not Started | 10 | 2-2.5h | `config/settings.rs` |
| 12 | [12-init-gitignore](tasks/12-init-gitignore.md) | Not Started | 11 | 0.5-1h | `config/settings.rs` |
| 13 | [13-documentation](tasks/13-documentation.md) | Not Started | 12 | 0.5-1h | `docs/` |

## Success Criteria

Phase 4 is complete when:

- [ ] Settings panel opens with `,` key
- [ ] Four tabs are visible: Project, User, Launch, VSCode
- [ ] Tab switching works with Tab/Shift+Tab and number keys
- [ ] All setting types are editable (bool, number, string, enum, list)
- [ ] Changes to config.toml save correctly
- [ ] User preferences save to settings.local.toml (gitignored)
- [ ] launch.toml configurations are viewable/editable
- [ ] .vscode/launch.json is read-only display
- [ ] Escape/q closes settings and returns to normal mode
- [ ] `.fdemon` directory is created on first run if missing
- [ ] `settings.local.toml` is added to `.gitignore` automatically
- [ ] All new code has unit tests
- [ ] No regressions in existing functionality

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `,` | Open settings panel |
| `Escape` / `q` | Close settings panel |
| `Tab` | Next tab |
| `Shift+Tab` | Previous tab |
| `1-4` | Jump to specific tab |
| `j` / `↓` | Next setting |
| `k` / `↑` | Previous setting |
| `Enter` / `Space` | Toggle boolean / Edit value |
| `+` / `-` | Increment/decrement number |
| `Ctrl+s` | Save settings |
| `Ctrl+r` | Reset to defaults |

## Future Extensibility

The settings architecture is designed to accommodate future features:

### From IDEAS.md

| Feature | Settings Section | Anticipated Options |
|---------|------------------|---------------------|
| Build Runner Manager | `[build_runner]` | `auto_start_watch`, `delete_conflicting_outputs` |
| Log Persistence | `[logging]` | `persist_to_disk`, `retention_days`, `format` |
| Multiple Flutter SDK | `[sdk]` | `fvm_integration`, `default_version` |
| Performance Profiling | `[profiling]` | `auto_record_jank`, `flame_chart_depth` |
| Test Runner | `[testing]` | `watch_mode`, `coverage_enabled`, `golden_tolerance` |
| Mouse Support | `[ui.mouse]` | `enabled`, `scroll_speed` |

### Implementation Pattern

Settings sections follow a consistent pattern:

```rust
// config/types.rs
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct BuildRunnerSettings {
    #[serde(default)]
    pub auto_start_watch: bool,

    #[serde(default = "default_true")]
    pub delete_conflicting_outputs: bool,
}

// Settings struct includes new section
pub struct Settings {
    // ... existing sections ...

    #[serde(default)]
    pub build_runner: BuildRunnerSettings,
}
```

The settings panel widget uses a trait-based approach for rendering:

```rust
trait SettingSection {
    fn name(&self) -> &'static str;
    fn items(&self) -> Vec<SettingItem>;
    fn apply_change(&mut self, item_id: &str, value: SettingValue);
}
```

This allows new sections to be added without modifying the core widget logic.

## Notes

- **Full-screen vs Modal**: This implementation uses full-screen to provide more space for complex settings. The header shows which tab is active.
- **Read-only VSCode tab**: We parse `.vscode/launch.json` for display but never write to it - users edit in VSCode directly.
- **Local settings priority**: `settings.local.toml` overrides values in `config.toml` for user-specific preferences.
- **Atomic saves**: Settings are saved atomically (write to temp, rename) to prevent corruption.
- **Debounced auto-save**: Consider adding optional auto-save with debouncing (future enhancement).
