# Task: Documentation

## Summary

Update documentation to reflect the new NewSessionDialog and its keybindings.

## Files to Update

| File | Changes |
|------|---------|
| `docs/KEYBINDINGS.md` | Update dialog keybindings |
| `README.md` | Update screenshots/descriptions if needed |

## Implementation

### 1. Update KEYBINDINGS.md

```markdown
# Keybindings

## Global Keys

| Key | Action |
|-----|--------|
| `?` | Toggle help overlay |
| `q` | Quit (with confirmation if sessions running) |
| `Esc` | Close dialog/modal or cancel |

## Normal Mode (Log View)

| Key | Action |
|-----|--------|
| `d` | Open New Session Dialog (add device) |
| `1-9` | Switch to session tab |
| `[` | Previous session |
| `]` | Next session |
| `r` | Hot reload current session |
| `R` | Hot restart current session |
| `k`/`↑` | Scroll up |
| `j`/`↓` | Scroll down |
| `g` | Scroll to top |
| `G` | Scroll to bottom |
| `/` | Open search |

## New Session Dialog

The New Session Dialog has two panes:
- **Target Selector** (left): Choose a device or boot an emulator
- **Launch Context** (right): Configure launch settings

### General Navigation

| Key | Action |
|-----|--------|
| `Tab` | Switch focus between panes |
| `1` | Switch to Connected devices tab |
| `2` | Switch to Bootable devices tab |
| `Esc` | Close modal, or close dialog (if sessions exist) |

### Target Selector (Left Pane)

| Key | Action |
|-----|--------|
| `↑`/`↓` | Navigate device list |
| `Enter` | Select device (Connected) or Boot device (Bootable) |
| `r` | Refresh device list |

### Launch Context (Right Pane)

| Key | Action |
|-----|--------|
| `↑`/`↓` | Navigate between fields |
| `Enter` | Open selector modal or launch |
| `←`/`→` | Change mode (when Mode field focused) |

### Fuzzy Search Modal

Appears when selecting Configuration or Flavor.

| Key | Action |
|-----|--------|
| Type | Filter items / enter custom value |
| `↑`/`↓` | Navigate filtered results |
| `Enter` | Select highlighted item or use custom text |
| `Esc` | Cancel and close modal |
| `Backspace` | Delete character from query |

### Dart Defines Modal

Appears when editing Dart Defines.

| Key | Action |
|-----|--------|
| `Tab` | Switch between List and Edit panes |
| `↑`/`↓` | Navigate list |
| `Enter` | Load item for editing / Save / Delete |
| `Esc` | Save all and close modal |

In Edit pane:
| Key | Action |
|-----|--------|
| `Tab` | Cycle: Key → Value → Save → Delete |
| Type | Edit Key or Value field |
| `Enter` | Move to next field or activate button |

## Config Editability

| Config Source | Mode | Flavor | Dart Defines |
|---------------|------|--------|--------------|
| VSCode | Read-only | Read-only | Read-only |
| FDemon | Editable (auto-saves) | Editable (auto-saves) | Editable (auto-saves) |
| None | Editable (transient) | Editable (transient) | Editable (transient) |

When a VSCode config is selected, fields show "(from config)" and cannot be modified.
When an FDemon config is selected, changes are automatically saved to `.fdemon/launch.toml`.
```

### 2. Update README if needed

If the README contains screenshots or descriptions of the old dialogs:

```markdown
## Quick Start

1. Run `fdemon` in your Flutter project directory
2. The **New Session Dialog** appears automatically
3. Select a device from the **Target Selector** (left pane)
4. Configure launch settings in **Launch Context** (right pane)
5. Press `Enter` to launch

### Adding More Sessions

Press `d` to open the New Session Dialog and add another device.
```

### 3. Create/update feature documentation

If there's a docs/FEATURES.md or similar:

```markdown
## New Session Dialog

The New Session Dialog is the central interface for launching Flutter sessions.

### Two-Pane Layout

- **Target Selector (left)**: Lists available devices in two tabs:
  - **Connected**: Devices already running (from `flutter devices`)
  - **Bootable**: Offline simulators and emulators that can be booted

- **Launch Context (right)**: Configure how to launch:
  - **Configuration**: Select from `.fdemon/launch.toml` or `.vscode/launch.json`
  - **Mode**: Debug, Profile, or Release
  - **Flavor**: App flavor (e.g., dev, staging, prod)
  - **Dart Defines**: Environment variables

### Modals

- **Fuzzy Search Modal**: Type to filter configurations or flavors
- **Dart Defines Modal**: Add, edit, or delete key-value pairs

### Config Sources

- **FDemon configs** (`.fdemon/launch.toml`): Fully editable, changes auto-save
- **VSCode configs** (`.vscode/launch.json`): Read-only in fdemon
```

## Verification

1. Check documentation renders correctly:
```bash
# If using mdbook or similar
mdbook build
mdbook serve
```

2. Verify keybindings match implementation:
```bash
# Search for key handling code
rg "KeyCode::" src/app/handler/keys.rs
```

3. Verify documented features exist:
```bash
# Search for feature implementations
rg "Fuzzy" src/
rg "DartDefines" src/
```

## Notes

- Keep keybindings table in sync with actual implementation
- Update screenshots if using them
- Consider adding a "What's New" section for major changes
- Ensure README quick start reflects new dialog flow
