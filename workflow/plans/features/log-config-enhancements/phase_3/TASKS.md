# Phase 3: Terminal Hyperlinks (OSC 8) - Task Index

## Overview

Phase 3 implements terminal hyperlinks for Flutter Demon, enabling clickable file:line references in stack traces and log messages. When the terminal supports OSC 8 hyperlinks, users can click on file references to open them. Additionally, the `o` key provides a keyboard-driven way to open files at the cursor position in the configured editor.

**Estimated Duration:** 1 week  
**Total Tasks:** 6  
**Estimated Hours:** 15-20 hours (reduced due to performance foundation)

## Foundation: Completed Performance Work

Phase 3 builds on significant performance improvements completed in the logger block propagation bug fix (see `workflow/plans/bugs/logger_block_propagation/`). These changes provide a solid foundation for hyperlink features:

### Key Changes Affecting Phase 3

| Component | Change | Impact on Phase 3 |
|-----------|--------|-------------------|
| **Log Storage** | `Vec<LogEntry>` → `VecDeque<LogEntry>` | VecDeque supports indexing; code accessing logs works unchanged |
| **Virtualization** | Only visible entries rendered | Hyperlink tracking limited to ~30-50 entries per frame |
| **Visible Range** | `LogViewState::visible_range()` method | Focus/hyperlink code should use this for efficient iteration |
| **Buffer Lines** | `buffer_lines` field in LogViewState | Pre-renders entries above/below viewport |
| **Session Methods** | `focused_entry()`, `focused_entry_id()` already exist | Task 03 can leverage existing focus infrastructure |
| **Log Batching** | `LogBatcher` coalesces rapid updates | Hyperlink rebuild happens at render time, not per-log |

### Implications for Implementation

1. **Focus Tracking (Task 03)**: Session already has `focused_entry()` and `focused_entry_id()` methods. Task 03 extends this with file reference extraction.

2. **Hyperlink Map (Task 06)**: With virtualization, `HyperlinkMap` only needs to track visible entries. This simplifies implementation and improves performance.

3. **VecDeque Compatibility**: All code accessing `session.logs` works unchanged since `VecDeque` implements the `Index` trait.

4. **Range-Based Access**: Use `session.get_logs_range(start, end)` for efficient iteration over log subsets.

## Parent IDE Detection & Instance Reuse

Flutter Demon will often run from within an IDE's integrated terminal (VS Code, Cursor, Zed, IntelliJ, etc.). Two important considerations:

### 1. Opening Files in the Current IDE Instance

When the user presses `o` to open a file, we should open it in the **running IDE instance**, not spawn a new window. This requires:
- Detecting which IDE terminal we're running inside
- Using IDE-specific commands/flags for window reuse

### 2. Ctrl+Click / Cmd+Click Support

OSC 8 hyperlinks support native terminal click handling. However, `file://` URLs often just reveal files in Finder/Explorer. To open in the correct IDE:
- Use **IDE-specific URL schemes** (e.g., `vscode://file/path:line:col`)
- Ctrl+click (or Cmd+click on macOS) will open directly in that IDE

### IDE Detection via Environment Variables

| IDE | Detection | URL Scheme | Reuse Command |
|-----|-----------|------------|---------------|
| VS Code | `$TERM_PROGRAM=vscode` | `vscode://file/path:line:col` | `code --reuse-window --goto file:line:col` |
| VS Code Insiders | `$TERM_PROGRAM=vscode-insiders` | `vscode-insiders://file/...` | `code-insiders --reuse-window --goto ...` |
| Cursor | `$TERM_PROGRAM=cursor` | `cursor://file/path:line:col` | `cursor --reuse-window --goto ...` |
| Zed | `$ZED_TERM` or `$TERM_PROGRAM=Zed` | `zed://file/path:line` | `zed file:line` (reuses by default) |
| IntelliJ/Android Studio | `$TERMINAL_EMULATOR=JetBrains-*` | `idea://open?file=path&line=N` | `idea --line N file` |
| Neovim (inside `:terminal`) | `$NVIM` (socket path) | N/A (use RPC) | `nvim --server $NVIM --remote file` |

### Implementation Strategy

**Task 02 (Editor Configuration):**
- Add `detect_parent_ide()` function
- Store detected IDE in settings/context

**Task 04 (Open File Action):**
- Check for parent IDE before using configured editor
- Use IDE-specific reuse flags (`--reuse-window`, etc.)

**Task 05 (Terminal Detection):**
- Expand to detect parent IDE alongside terminal capabilities
- Return `TerminalContext` with both terminal type and parent IDE

**Task 06 (OSC 8 Hyperlinks):**
- Use IDE-specific URL schemes when parent IDE detected
- Fall back to `file://` when no IDE detected

## Task Dependency Graph

```
┌─────────────────────────┐
│  01-hyperlink-module-   │
│  url-generation         │
└───────────┬─────────────┘
            │
            ├──────────────────┬──────────────────┐
            │                  │                  │
            ▼                  ▼                  │
┌─────────────────────┐ ┌─────────────────────┐   │
│  02-editor-         │ │  05-terminal-       │   │
│  configuration      │ │  capability-        │   │
│                     │ │  detection          │   │
└──────────┬──────────┘ └──────────┬──────────┘   │
           │                       │              │
           │     ┌─────────────────┘              │
           │     │                                │
           ▼     │                                │
┌─────────────────────────┐                       │
│  03-cursor-file-        │                       │
│  reference-tracking     │                       │
│  (Independent)          │                       │
└───────────┬─────────────┘                       │
            │                                     │
            ▼                                     │
┌─────────────────────────┐                       │
│  04-open-file-          │                       │
│  editor-action          │◄──────────────────────┤
└─────────────────────────┘                       │
                                                  │
                          ┌───────────────────────┘
                          │
                          ▼
            ┌─────────────────────────┐
            │  06-osc8-hyperlink-     │
            │  rendering              │
            │  (Experimental)         │
            └─────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-hyperlink-module-url-generation](tasks/01-hyperlink-module-url-generation.md) | Not Started | - | 2-3h | `tui/hyperlinks.rs` (NEW) |
| 2 | [02-editor-configuration](tasks/02-editor-configuration.md) | Not Started | - | 2-3h | `config/types.rs`, `config/settings.rs` |
| 3 | [03-cursor-file-reference-tracking](tasks/03-cursor-file-reference-tracking.md) | Not Started | - | 2-3h | `tui/widgets/log_view.rs` |
| 4 | [04-open-file-editor-action](tasks/04-open-file-editor-action.md) | Not Started | 2, 3 | 3-4h | `app/message.rs`, `app/handler/keys.rs`, `tui/editor.rs` (NEW) |
| 5 | [05-terminal-capability-detection](tasks/05-terminal-capability-detection.md) | Not Started | 1 | 2-3h | `tui/hyperlinks.rs` |
| 6 | [06-osc8-hyperlink-rendering](tasks/06-osc8-hyperlink-rendering.md) | Not Started | 1, 2, 5 | 3-4h | `tui/hyperlinks.rs`, `tui/render.rs`, `config/types.rs` |

## Core vs Experimental

### Core Tasks (Must Complete)
- **Task 01**: Foundation for all hyperlink functionality
- **Task 02**: Enables editor configuration
- **Task 03**: Tracks what file is "focused" for opening
- **Task 04**: The `o` key - primary way to open files from the TUI

### Experimental Tasks (Best Effort)
- **Task 05**: Terminal detection for OSC 8 support
- **Task 06**: Actual clickable hyperlinks in terminal (complex integration with Ratatui)

If Task 06 proves too complex, the `o` key functionality (Task 04) provides a reliable alternative for opening files.

## New Modules

| Module | Purpose |
|--------|---------|
| `src/tui/hyperlinks.rs` | OSC 8 sequence generation, URL creation, terminal detection, FileReference type |
| `src/tui/editor.rs` | Editor command execution, path resolution, sanitization |

## Existing Infrastructure to Leverage

| Component | Location | Purpose |
|-----------|----------|---------|
| `Session::focused_entry()` | `app/session.rs:757-760` | Returns currently focused LogEntry |
| `Session::focused_entry_id()` | `app/session.rs:763-765` | Returns focused entry's ID |
| `Session::current_log_position()` | `app/session.rs:739-750` | Maps scroll offset to log index (handles filtering) |
| `LogViewState::visible_range()` | `tui/widgets/log_view.rs:107-111` | Returns (start, end) for virtualized rendering |
| `Session::get_logs_range()` | `app/session.rs:497-501` | Efficient VecDeque range access |

## Keyboard Shortcuts (Phase 3)

| Key | Action |
|-----|--------|
| `o` | Open file at cursor in configured editor (or parent IDE if detected) |

## Configuration Additions

```toml
# .fdemon/config.toml

[ui]
# Enable terminal hyperlinks (OSC 8)
# Values: "auto" (detect terminal support), "enabled", "disabled"
hyperlinks = "auto"

[editor]
# Editor command (leave empty for auto-detection)
# Auto-detected from: $VISUAL, $EDITOR, or common editors in PATH
command = ""

# Pattern for opening file at line/column
# Variables: $EDITOR, $FILE, $LINE, $COLUMN
# Examples:
#   VS Code:  "code --goto $FILE:$LINE:$COLUMN"
#   Zed:      "zed $FILE:$LINE"
#   Neovim:   "nvim +$LINE $FILE"
#   Vim:      "vim +$LINE $FILE"
#   Emacs:    "emacs +$LINE:$COLUMN $FILE"
open_pattern = "$EDITOR $FILE:$LINE"
```

## Supported Terminals (OSC 8)

| Terminal | Status | Detection Method |
|----------|--------|------------------|
| iTerm2 | ✅ Supported | `$TERM_PROGRAM = "iTerm.app"` |
| Kitty | ✅ Supported | `$TERM = "xterm-kitty"` |
| WezTerm | ✅ Supported | `$TERM_PROGRAM = "WezTerm"` |
| Alacritty | ✅ Supported | `$TERM = "alacritty"` |
| Windows Terminal | ✅ Supported | `$WT_SESSION` exists |
| VS Code Terminal | ✅ Supported | `$TERM_PROGRAM = "vscode"` |
| GNOME Terminal | ✅ 3.26+ | `$VTE_VERSION >= 5000` |
| macOS Terminal.app | ❌ Unsupported | `$TERM_PROGRAM = "Apple_Terminal"` |

## Success Criteria

Phase 3 is complete when:

- [ ] `o` key opens file at cursor in configured editor
- [ ] Editor auto-detection works for VS Code, Zed, Neovim, Vim
- [ ] Editor command pattern substitution works ($FILE, $LINE, $COLUMN)
- [ ] File paths resolved correctly (package: URIs, relative paths)
- [ ] Path sanitization prevents security issues
- [ ] Terminal hyperlink support detected accurately (if Task 05 complete)
- [ ] OSC 8 hyperlinks clickable in supported terminals (if Task 06 complete)
- [ ] Graceful fallback in unsupported terminals
- [ ] Configuration options documented

## Dependencies on Phase 2

Phase 3 builds on Phase 2 functionality:
- Stack trace parsing provides `StackFrame` with file paths
- File:line references already highlighted in log view
- Collapsible stack traces provide frame-level navigation context

## Dependencies on Bug Fix Work

Phase 3 benefits from the logger block propagation bug fix:
- **VecDeque storage**: Log access via indexing unchanged (`logs[i]` works)
- **Virtualization**: `visible_range()` provides efficient iteration bounds
- **Existing focus methods**: `focused_entry()` and `focused_entry_id()` already implemented
- **Performance baseline**: High-volume logging no longer causes CPU spikes

## Testing Strategy

### Unit Tests
- URL generation (file://, package: handling)
- OSC 8 sequence generation
- Editor pattern substitution
- Path resolution and sanitization
- Terminal detection logic

### Manual Testing
1. Test `o` key with various editors (VS Code, Zed, Neovim)
2. Test in different terminals (iTerm2, Kitty, macOS Terminal)
3. Verify hyperlinks are clickable where supported
4. Verify no garbage output in unsupported terminals
5. Test with package: paths and absolute paths

## Security Considerations

- **Path Traversal**: Reject paths containing `..`
- **Shell Injection**: Reject paths with shell metacharacters
- **Command Injection**: Use `Command::new()` with args, not shell execution
- **File Validation**: Check file exists before opening

## Notes

- The `o` key functionality is the reliable core feature
- OSC 8 hyperlinks are "nice to have" and may be marked experimental
- Some terminals support OSC 8 for http:// but not file:// URLs
- Consider adding `--debug-terminal-info` CLI flag for troubleshooting
- **Performance Note**: With virtualization, hyperlink tracking is O(visible) not O(total_logs)
- **IDE Integration**: When running in an IDE terminal, use that IDE's URL scheme for Ctrl+click support
- **Instance Reuse**: The `o` key should open files in the current IDE instance, not spawn new windows

## References

- [OSC 8 Hyperlink Specification](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda)
- [PLAN.md Phase 3 Section](../PLAN.md#phase-3-terminal-hyperlinks-osc-8)