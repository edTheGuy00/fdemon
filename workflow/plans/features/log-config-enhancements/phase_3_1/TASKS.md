# Phase 3.1: Link Highlight Mode - Task Index

## Overview

Phase 3.1 replaces the unreliable "auto-detect file reference at top of viewport" approach with an explicit **Link Highlight Mode**. When the user presses `L`, all file references in the visible viewport are detected, highlighted with shortcut keys (1-9, a-z), and the user can press the corresponding key to open that file in their editor.

This approach is inspired by VS Code's Cmd+Click behavior but uses a toggle key for better terminal compatibility.

**Estimated Duration:** 1 week  
**Total Tasks:** 10  
**Estimated Hours:** 17-24 hours

## Problem Being Solved

The current Phase 3 implementation has issues:
1. File reference detection only works when links are "perfectly aligned at the top"
2. OSC 8 terminal hyperlinks have limited terminal support
3. Auto-detection during render is computationally wasteful

## Task Dependency Graph

```
┌─────────────────────────────────────────────────────────────────┐
│  Phase A: Cleanup                                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────────────┐                                       │
│  │  01-cleanup-focus-   │                                       │
│  │  info-auto-detect    │                                       │
│  └──────────┬───────────┘                                       │
│             │                                                   │
│             ▼                                                   │
│  ┌──────────────────────┐                                       │
│  │  02-remove-osc8-code │                                       │
│  └──────────┬───────────┘                                       │
└─────────────┼───────────────────────────────────────────────────┘
              │
┌─────────────┼───────────────────────────────────────────────────┐
│  Phase B: Core Implementation                                   │
├─────────────┼───────────────────────────────────────────────────┤
│             │                                                   │
│             ▼                                                   │
│  ┌──────────────────────┐     ┌──────────────────────┐          │
│  │  03-link-highlight-  │     │  04-ui-mode-and-     │          │
│  │  state-types         │     │  messages            │          │
│  └──────────┬───────────┘     └──────────┬───────────┘          │
│             │                            │                      │
│             └────────────┬───────────────┘                      │
│                          │                                      │
│                          ▼                                      │
│             ┌──────────────────────┐                            │
│             │  05-viewport-        │                            │
│             │  scanning            │                            │
│             └──────────┬───────────┘                            │
│                        │                                        │
│                        ▼                                        │
│             ┌──────────────────────┐                            │
│             │  06-key-and-update-  │                            │
│             │  handlers            │                            │
│             └──────────┬───────────┘                            │
└────────────────────────┼────────────────────────────────────────┘
                         │
┌────────────────────────┼────────────────────────────────────────┐
│  Phase C: Rendering & Polish                                    │
├────────────────────────┼────────────────────────────────────────┤
│                        │                                        │
│                        ▼                                        │
│             ┌──────────────────────┐                            │
│             │  07-link-highlight-  │                            │
│             │  rendering           │                            │
│             └──────────┬───────────┘                            │
│                        │                                        │
│                        ▼                                        │
│             ┌──────────────────────┐                            │
│             │  08-instruction-bar  │                            │
│             └──────────┬───────────┘                            │
│                        │                                        │
│                        ▼                                        │
│             ┌──────────────────────┐                            │
│             │  09-remove-o-key     │                            │
│             └──────────┬───────────┘                            │
│                        │                                        │
│                        ▼                                        │
│             ┌──────────────────────┐                            │
│             │  10-testing-and-docs │                            │
│             └──────────────────────┘                            │
└─────────────────────────────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-cleanup-focus-info-auto-detect](tasks/01-cleanup-focus-info-auto-detect.md) | Done | - | 1-2h | `tui/widgets/log_view.rs`, `app/handler/update.rs` |
| 2 | [02-remove-osc8-code](tasks/02-remove-osc8-code.md) | Done | 01 | 2-3h | `tui/hyperlinks.rs`, `config/types.rs`, `tui/mod.rs` |
| 3 | [03-link-highlight-state-types](tasks/03-link-highlight-state-types.md) | Done | 02 | 2-3h | `tui/hyperlinks.rs` |
| 4 | [04-ui-mode-and-messages](tasks/04-ui-mode-and-messages.md) | Done | - | 1h | `app/state.rs`, `app/message.rs` |
| 5 | [05-viewport-scanning](tasks/05-viewport-scanning.md) | Done | 03, 04 | 3-4h | `tui/hyperlinks.rs`, `app/session.rs` |
| 6 | [06-key-and-update-handlers](tasks/06-key-and-update-handlers.md) | Done | 05 | 2-3h | `app/handler/keys.rs`, `app/handler/update.rs` |
| 7 | [07-link-highlight-rendering](tasks/07-link-highlight-rendering.md) | Done | 06 | 3-4h | `tui/widgets/log_view.rs` |
| 8 | [08-instruction-bar](tasks/08-instruction-bar.md) | Done | 07 | 1-2h | `tui/render.rs` |
| 9 | [09-remove-o-key-functionality](tasks/09-remove-o-key-functionality.md) | Done | 06 | 0.5h | `app/handler/keys.rs`, `app/message.rs`, `app/handler/update.rs` |
| 10 | [10-testing-and-docs](tasks/10-testing-and-docs.md) | Done | 08, 09 | 2h | Tests, README |

## Phase Breakdown

### Phase A: Cleanup (Tasks 01-02)

Remove the current auto-detection approach that's causing the "wonky" behavior:
- Remove `file_ref` field from `FocusInfo` and its auto-detection during render
- Remove all OSC 8 terminal hyperlink code (~500+ lines)
- Remove `ui.hyperlinks` configuration option

### Phase B: Core Implementation (Tasks 03-06)

Build the new Link Highlight Mode:
- Add `DetectedLink` and `LinkHighlightState` types
- Add `UiMode::LinkHighlight` and related messages
- Implement viewport scanning to find all file references
- Implement key handlers and update logic

### Phase C: Rendering & Polish (Tasks 07-10)

Complete the visual experience and cleanup:
- Render highlighted links with shortcut indicators
- Add instruction bar when in link mode
- Remove redundant `o` key functionality (Link Mode is superior)
- Testing and documentation

## Keyboard Shortcuts

| Key | Context | Action |
|-----|---------|--------|
| `L` or `Shift+L` | Normal mode | Enter link highlight mode |
| `L` | Link mode | Exit link highlight mode |
| `Esc` | Link mode | Exit link highlight mode |
| `1`-`9` | Link mode | Select and open link 1-9 |
| `a`-`z` | Link mode | Select and open link 10-35 |
| `j`/`k`/↑/↓ | Link mode | Scroll (re-scans on scroll) |

## Code Removed vs Kept

### Removed (OSC 8 Related)
- `HyperlinkMode`, `HyperlinkSupport` enums
- `HYPERLINK_SUPPORT` static, `hyperlink_support()`, `detect_hyperlink_support()`
- `is_unsupported_terminal()`, `is_supported_terminal()`, `is_terminal_multiplexer()`
- `TerminalInfo` struct, `terminal_info()`
- `file_url()`, `file_url_with_position()`
- `osc8` module, `osc8_wrap()`, `osc8_wrap_file()`, `contains_osc8()`
- `HyperlinkRegion`, `HyperlinkMap` structs
- `ide_aware_file_url()`, `percent_encode_path()`, `osc8_wrap_ide_aware()`
- `UiSettings.hyperlinks` config field
- `FocusInfo.file_ref` and auto-detection during render

### Removed (Redundant with Link Mode)
- `o` key binding and `Message::OpenFileAtCursor`
- `OpenFileAtCursor` handler in update.rs

### Kept (Core Functionality)
- `FileReference` struct and all methods
- `FileReferenceSource` enum
- `extract_file_ref_from_message()` - core scanning function
- `FILE_LINE_PATTERN` regex
- Entire `editor.rs` module (open_in_editor, resolve_file_path, sanitize_path)
- `EditorSettings`, `ParentIde` detection
- `FocusInfo.entry_index`, `entry_id`, `frame_index` (for stack trace toggle)

## Success Criteria

- [ ] Pressing `L` enters link highlight mode
- [ ] All visible file references are detected and numbered (1-9, a-z)
- [ ] Pressing shortcut key opens the corresponding file
- [ ] Files open in the correct editor (parent IDE if detected)
- [ ] Pressing `Esc` or `L` exits link mode
- [ ] Scrolling in link mode re-scans the viewport
- [ ] No more "wonky" behavior - links work reliably every time
- [ ] Removed ~500+ lines of unused OSC 8 code
- [ ] Removed redundant `o` key functionality
- [ ] Stack trace toggle with `Enter` still works
- [ ] All existing tests pass
- [ ] New tests for link highlight mode

## Testing Strategy

### Unit Tests
- `DetectedLink` shortcut assignment (1-9, a-z)
- `LinkHighlightState::scan_viewport()` detection
- `link_by_shortcut()` lookup
- Edge cases: empty logs, no links, >35 links

### Integration Tests
- Enter/exit link mode state transitions
- Link selection opens correct file
- Scroll re-scan behavior
- Filter interaction (only scan filtered entries)

### Manual Testing
1. Enter link mode in logs with file references
2. Verify all visible links highlighted and numbered
3. Press number to open file
4. Verify file opens in correct editor
5. Test with VS Code, Zed, Neovim
6. Test exit with `Esc` and `L`
7. Test scroll re-scan
8. Verify `o` key no longer does anything

## Notes

- This replaces Phase 3's Task 06 (OSC 8 rendering) approach entirely
- The `o` key functionality is removed - Link Highlight Mode is the only way to open files
- Terminal detection code is removed since we no longer need OSC 8 support detection
- Performance is better since we only scan when user explicitly enters link mode
- Maximum 35 links supported per viewport (1-9, a-z)

## References

- [Phase 3.1 PLAN.md](PLAN.md)
- [Original Phase 3 TASKS.md](../phase_3/TASKS.md)
- [VS Code Terminal Link Provider](https://code.visualstudio.com/api/references/vscode-api#TerminalLinkProvider)