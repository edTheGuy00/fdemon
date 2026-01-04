# Flutter Demon - Ideas & Future Considerations

This document contains ideas and features we're considering for future development. These are not on the immediate roadmap but represent interesting directions the project could take.

---

## Deferred Features

### 1. Pubspec Watcher

**Priority**: Low  
**Complexity**: Medium

Automatically watch `pubspec.yaml` and `pubspec.lock` for changes and prompt the user to run `flutter pub get` when dependencies change.

**Potential Implementation**:
- Add pubspec files to the existing file watcher
- Detect when dependency sections change (not just formatting)
- Show a non-blocking prompt: "Dependencies changed. Run pub get? [y/N]"
- Display `pub get` output in the log area
- Consider auto-running pub get as a configurable option

**Why Deferred**: 
- Current workflow of manually running `flutter pub get` is acceptable
- Adds complexity to the file watcher logic
- Risk of false positives when only comments or formatting change

---

### 2. Mouse Support

**Priority**: Low  
**Complexity**: Medium

Enable mouse interactions within the TUI for users who prefer point-and-click over keyboard shortcuts.

**Potential Features**:
- Clickable header buttons (Reload, Restart, DevTools, etc.)
- Scrollable log area with mouse wheel
- Log entry selection via mouse click
- Device selection via mouse in the device selector modal
- Session tab switching with mouse clicks

**Implementation Notes**:
- Ratatui + Crossterm support mouse events
- Need to track clickable regions and map mouse positions
- Must maintain full keyboard accessibility (mouse is optional)

**Why Deferred**:
- Terminal power users typically prefer keyboard-centric workflows
- Adds significant complexity to event handling
- Not all terminals have consistent mouse support

---

### 3. Remote Development

**Priority**: Low  
**Complexity**: High

Support running Flutter Demon on a remote machine with the Flutter project, connecting via SSH or other tunneling mechanisms.

**Potential Features**:
- SSH tunnel support for VM service connections
- Remote Flutter daemon connection
- Cloud device support (Firebase Test Lab, AWS Device Farm)
- Proxy mode for connecting to remote DevTools

**Use Cases**:
- Developing on a lightweight laptop with a remote build server
- Running on CI/CD machines with attached devices
- Connecting to cloud-hosted emulators/simulators

**Why Deferred**:
- Very high complexity with many edge cases
- Security considerations for remote connections
- Limited demand compared to local development workflows

---

### 4. Plugin System

**Priority**: Low  
**Complexity**: Very High

Allow users to extend Flutter Demon with custom plugins using Lua, WASM, or a Rust plugin API.

**Potential Features**:
- Custom commands and keyboard shortcuts
- Custom widgets and UI panels
- Theme customization and custom color schemes
- Custom log filters and formatters
- Integration with external tools (Sentry, Firebase Crashlytics, etc.)

**Architecture Considerations**:
- Lua embedding via `mlua` crate for scripting
- WASM plugins via `wasmtime` for sandboxed execution
- Rust dynamic libraries for high-performance extensions
- Plugin discovery from `~/.config/flutter-demon/plugins/`

**Why Deferred**:
- Extremely high complexity
- Need to stabilize core features first
- Plugin API design requires careful consideration for backwards compatibility
- Security implications of executing user code

---

## Ideas Under Consideration

### A. Build Runner Manager

**Priority**: Medium  
**Complexity**: Medium

Flutter projects using Riverpod, Freezed, or JSON Serializable rely heavily on code generation. Running `build_runner` is a constant friction point that typically requires a separate terminal.

**The Feature**: A dedicated background worker for `build_runner` integrated into Flutter Demon.

**UI Components**:
- Status bar indicator showing generation state:
  - 🟢 **Synced** - All generated files up to date
  - 🟡 **Building** - Generation in progress
  - 🔴 **Error** - Generation failed (with error count)

**Controls**:
| Command | Action |
|---------|--------|
| **Watch** | Spawns `dart run build_runner watch --delete-conflicting-outputs` |
| **Build** | One-off `dart run build_runner build --delete-conflicting-outputs` |
| **Clean** | Runs `flutter clean` + `flutter pub get` + `build_runner build` (the "fix everything" button) |

**Keyboard Shortcuts** (proposed):
- `b` - Toggle build_runner watch mode
- `B` - One-off build
- `Ctrl+b` - Full clean + rebuild

**Implementation Considerations**:
- Spawn build_runner as a separate managed process
- Parse build_runner output to detect errors/warnings
- Show generation errors in log view with file:line references
- Auto-detect if project uses build_runner (check dev_dependencies)
- Configuration option to auto-start watch mode

**Why It Helps**:
- Eliminates need for a separate terminal tab just for the watcher
- Flutter Demon manages the noise and only alerts on generation failures
- Quick access to the "nuclear option" (clean + rebuild) when things break
- Consistent with Flutter Demon's goal of being a complete development sidecar

---

### B. Log Persistence to Disk

Save logs to disk for post-mortem analysis. Could be useful for debugging issues that occur when the developer isn't watching.

- Automatic log rotation
- Configurable retention period
- Export to common formats (JSON, plain text)
- Integration with external log viewers

### C. Multiple Flutter SDK Support

Detect and allow switching between multiple Flutter SDK installations (including FVM integration).

- Show current SDK version in status bar
- Quick switch command
- Per-project SDK pinning via `.fvmrc` or similar

### D. Performance Profiling Mode

A dedicated mode for performance profiling that shows real-time CPU/GPU flame charts in the terminal.

- ASCII-art flame graphs
- Memory allocation timeline
- Jank detection and highlighting
- Export profiles for external analysis

### E. Test Runner Integration

Run and display Flutter tests directly within Flutter Demon.

- Watch mode for tests
- Coverage display
- Failure highlighting with stack trace navigation
- Integration with golden tests

### F. Build Size Analyzer

Analyze and display Flutter app build sizes with a breakdown by package/asset.

- Tree-map visualization in terminal (ASCII art)
- Compare sizes between builds
- Suggestions for size reduction

---

## Contributing Ideas

Have an idea for Flutter Demon? We'd love to hear it! Please open an issue on GitHub with the `idea` label and include:

1. **Problem**: What problem does this solve?
2. **Solution**: How would you implement it?
3. **Use Case**: Who would benefit from this feature?
4. **Complexity**: Your estimate of implementation effort

---

*Last updated: See git history*
