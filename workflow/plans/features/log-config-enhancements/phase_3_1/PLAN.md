# Phase 3.1: Link Highlight Mode (VS Code-style)

## TL;DR

Replace the current "auto-detect file reference at top of viewport" approach with an explicit **Link Highlight Mode** inspired by VS Code's Cmd+Click behavior. When the user presses `L`, all file references in the visible viewport are detected, highlighted with shortcut keys (1-9, a-z), and the user can press the corresponding key to open that file in their editor. This approach is more reliable, intentional, and provides a better user experience than the current implementation which requires links to be "perfectly aligned at the top."

## Problem Statement

The current Phase 3 implementation (Tasks 01-06) has issues:
1. File reference detection only works for the element at the exact top of the viewport
2. Users report that "opening links does not work all the time"
3. OSC 8 terminal hyperlinks have limited terminal support and complex integration
4. The auto-detection during render is computationally wasteful when not needed

## Proposed Solution

Implement a **toggle-based Link Highlight Mode**:
1. User presses `L` to enter link mode
2. All file references in the visible viewport are scanned and displayed with numbered shortcuts
3. User presses `1-9` or `a-z` to select and open a link
4. User presses `Esc` or `L` again to exit link mode

This mirrors VS Code's approach where Cmd/Ctrl highlights links, but uses a toggle key for better terminal compatibility.

## Affected Modules

### Modified
- `src/app/state.rs`: Add `UiMode::LinkHighlight`
- `src/app/message.rs`: Add `EnterLinkMode`, `ExitLinkMode`, `SelectLink` messages
- `src/app/session.rs`: Add `link_highlight_state` field
- `src/app/handler/keys.rs`: Add link mode key handlers
- `src/app/handler/update.rs`: Handle link mode messages
- `src/tui/widgets/log_view.rs`: Simplify `FocusInfo`, add link highlighting render
- `src/tui/hyperlinks.rs`: Remove OSC 8 code, add `LinkHighlightState`, `DetectedLink`
- `src/tui/render.rs`: Add link mode instruction bar
- `src/config/types.rs`: Remove `ui.hyperlinks` setting

### Unchanged (Keep)
- `src/tui/editor.rs`: All functionality (open_in_editor, resolve_file_path, etc.)
- `src/config/types.rs`: `EditorSettings`, `ParentIde` detection

## Phases

### Phase A: Cleanup (Tasks 01-02)
Remove the current auto-detection approach and simplify existing code.

### Phase B: Core Implementation (Tasks 03-06)
Implement the new Link Highlight Mode state, messages, scanning, and handlers.

### Phase C: Rendering & Polish (Tasks 07-10)
Implement visual highlighting, instruction bar, final cleanup, and testing.

## Code to Remove

### From `src/tui/hyperlinks.rs`
| Item | Reason |
|------|--------|
| `HyperlinkMode` enum | OSC 8 configuration no longer needed |
| `HyperlinkSupport` enum | OSC 8 detection no longer needed |
| `HYPERLINK_SUPPORT` static | Cached OSC 8 detection |
| `hyperlink_support()` | OSC 8 detection |
| `detect_hyperlink_support()` | OSC 8 terminal detection |
| `is_unsupported_terminal()` | OSC 8 specific |
| `is_supported_terminal()` | OSC 8 specific |
| `is_terminal_multiplexer()` | OSC 8 specific |
| `TerminalInfo` struct | OSC 8 debug info |
| `terminal_info()` | OSC 8 debug |
| `file_url()` | OSC 8 URL generation |
| `file_url_with_position()` | OSC 8 URL generation |
| `osc8` module | OSC 8 constants |
| `osc8_wrap()` | OSC 8 wrapping |
| `osc8_wrap_file()` | OSC 8 wrapping |
| `contains_osc8()` | OSC 8 detection |
| `HyperlinkRegion` struct | OSC 8 region tracking |
| `HyperlinkMap` struct | OSC 8 region tracking |
| `ide_aware_file_url()` | OSC 8 IDE URLs |
| `percent_encode_path()` | OSC 8 URL encoding |
| `osc8_wrap_ide_aware()` | OSC 8 IDE wrapping |

### From `src/tui/widgets/log_view.rs`
| Item | Reason |
|------|--------|
| `FocusInfo.file_ref` field | Auto-detection approach being removed |
| File ref extraction during render | Auto-detection approach being removed |

### From `src/config/types.rs`
| Item | Reason |
|------|--------|
| `UiSettings.hyperlinks` field | OSC 8 configuration no longer needed |

## Code to Keep

### From `src/tui/hyperlinks.rs`
| Item | Reason |
|------|--------|
| `FileReference` struct | Core data type for link mode |
| `FileReferenceSource` enum | Tracking where refs come from |
| `FileReference::new()` | Construction |
| `FileReference::with_source()` | Construction with source |
| `FileReference::from_stack_frame()` | Stack frame conversion |
| `FileReference::display()` | Display formatting |
| `FileReference::resolve_path()` | Path resolution |
| `extract_file_ref_from_message()` | Core scanning function |
| `split_path_and_location()` | Helper function |
| `FILE_LINE_PATTERN` static | Regex for detection |

### From `src/tui/editor.rs` (Keep All)
| Item | Reason |
|------|--------|
| `EditorError` enum | Error handling |
| `OpenResult` struct | Result type |
| `open_in_editor()` | Core action |
| `resolve_file_path()` | Path resolution |
| `substitute_pattern()` | Command building |
| `execute_command()` | Command execution |
| `sanitize_path()` | Security |

### From `src/tui/widgets/log_view.rs`
| Item | Reason |
|------|--------|
| `FocusInfo.entry_index` | Stack trace toggle needs this |
| `FocusInfo.entry_id` | Stack trace toggle needs this |
| `FocusInfo.frame_index` | Stack trace toggle needs this |

## New Types

### `DetectedLink` (in `src/tui/hyperlinks.rs`)
```rust
/// A detected link in the visible viewport
#[derive(Debug, Clone)]
pub struct DetectedLink {
    /// The file reference
    pub file_ref: FileReference,
    /// Which log entry this belongs to
    pub entry_index: usize,
    /// Optional stack frame index within entry
    pub frame_index: Option<usize>,
    /// Shortcut key to select this link ('1'-'9', 'a'-'z')
    pub shortcut: char,
    /// Display text (the file:line portion shown to user)
    pub display_text: String,
}
```

### `LinkHighlightState` (in `src/tui/hyperlinks.rs`)
```rust
/// State for link highlight mode
#[derive(Debug, Default, Clone)]
pub struct LinkHighlightState {
    /// Detected links in the current viewport
    pub links: Vec<DetectedLink>,
    /// Whether link mode is active
    pub active: bool,
}
```

## Keyboard Shortcuts

| Key | Context | Action |
|-----|---------|--------|
| `L` or `Shift+L` | Normal mode | Enter link highlight mode |
| `L` | Link mode | Exit link highlight mode |
| `Esc` | Link mode | Exit link highlight mode |
| `1`-`9` | Link mode | Select and open link 1-9 |
| `a`-`z` | Link mode | Select and open link 10-35 |
| `j`/`k`/↑/↓ | Link mode | Scroll (re-scans on scroll) |

## Visual Design

### Link Mode Active
```
┌─ Logs ──────────────────────────────────────────────────────────┐
│ [INFO] App started                                              │
│ [ERROR] Exception at [1]lib/main.dart:42:5                      │
│   #0  MyWidget.build ([2]lib/widgets/my_widget.dart:15:10)      │
│   #1  StatelessElement.build ([3]package:flutter/src/...dart:23)│
│ [DEBUG] Loading config from [4]lib/config/app_config.dart:8     │
└─────────────────────────────────────────────────────────────────┘
┌─ Link Mode: Press 1-4 to open, Esc to cancel ───────────────────┐
```

### Highlighting Style
- Link text: Cyan/blue background with the shortcut number in brackets `[1]`
- Instruction bar: Dark background at bottom of log area

## Edge Cases & Risks

| Risk | Mitigation |
|------|------------|
| No links in viewport | Show "No links found in viewport" message |
| Too many links (>35) | Only show first 35 (1-9, a-z) |
| Link mode + scroll | Re-scan viewport after scroll |
| Invalid file path | sanitize_path() rejects, show error |
| File doesn't exist | Show "File not found" error |
| Cross-platform differences | Use existing editor detection logic |
| Performance with many entries | Only scan visible viewport (virtualized) |

## Success Criteria

- [ ] Pressing `L` enters link highlight mode
- [ ] All visible file references are detected and numbered
- [ ] Pressing `1-9` or `a-z` opens the corresponding file
- [ ] Files open in the correct editor (parent IDE if detected)
- [ ] Pressing `Esc` or `L` exits link mode
- [ ] No more "wonky" behavior - links work reliably
- [ ] Removed unused OSC 8 code (~500+ lines)
- [ ] Existing `o` key behavior preserved (opens focused entry)
- [ ] Stack trace toggle with `Enter` still works

## Testing Strategy

### Unit Tests
- `DetectedLink` creation and shortcut assignment
- `LinkHighlightState::scan_viewport()` with various log content
- Shortcut key lookup (`link_by_shortcut`)
- Edge cases: empty logs, no links, >35 links

### Integration Tests
- Enter/exit link mode transitions
- Link selection opens correct file
- Scroll in link mode re-scans
- Filter interaction (only scan filtered entries)

### Manual Testing
1. Enter link mode with `L` in logs with file references
2. Verify all visible links are highlighted and numbered
3. Press number/letter to open file
4. Verify file opens in correct editor
5. Test with different editors (VS Code, Zed, Neovim)
6. Test exit with `Esc` and toggle with `L`

## Dependencies

- Existing `editor.rs` module (no changes needed)
- Existing `EditorSettings` configuration
- Existing `ParentIde` detection

## References

- [VS Code Terminal Link Provider](https://code.visualstudio.com/api/references/vscode-api#TerminalLinkProvider)
- [Current Phase 3 TASKS.md](../phase_3/TASKS.md)
- [Editor Configuration](../phase_3/tasks/02-editor-configuration.md)