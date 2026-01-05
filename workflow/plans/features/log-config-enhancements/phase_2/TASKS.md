# Phase 2: Error Highlighting & Stack Traces - Task Index

## Overview

Phase 2 implements enhanced error visualization and stack trace parsing for Flutter Demon. Users will see visually distinct error messages with color-coded severity levels, parsed and formatted stack traces with clickable file references, and collapsible stack trace views for better log readability.

**Estimated Duration:** 1-1.5 weeks  
**Total Tasks:** 12 (7 original + 5 follow-up fixes)  
**Estimated Hours:** 43-54 hours

## Task Dependency Graph

```
┌─────────────────────────┐     ┌─────────────────────────┐
│  01-stack-trace-        │     │  03-enhance-sample-     │
│  parser-types           │     │  apps-test-logs         │
└───────────┬─────────────┘     └─────────────────────────┘
            │                     (Independent - for testing)
            ▼
┌─────────────────────────┐
│  02-stack-trace-        │
│  parsing-logic          │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│  04-integrate-stack-    │
│  trace-parsing          │
└───────────┬─────────────┘
            │
     ┌──────┴──────┐
     ▼             ▼
┌─────────────┐ ┌─────────────┐
│ 05-stack-   │ │ 07-error-   │
│ trace-      │ │ count-      │
│ rendering   │ │ status-bar  │
└──────┬──────┘ └─────────────┘
       │
       ▼
┌─────────────────────────┐
│  06-collapsible-        │
│  stack-traces           │
└─────────────────────────┘

                    ┌─────────────────────────┐
                    │  08-strip-ansi-         │
                    │  escape-codes           │
                    │  (Follow-up Fix)        │
                    └───────────┬─────────────┘
                                │
                                ▼
                    ┌─────────────────────────┐
                    │  09-enhance-log-        │
                    │  level-detection        │
                    │  (Follow-up Fix)        │
                    └───────────┬─────────────┘
                                │
                                ▼
                    ┌─────────────────────────┐
                    │  10-strip-flutter-      │
                    │  prefix-raw-lines       │
                    │  (Follow-up Fix)        │
                    └───────────┬─────────────┘
                                │
                                ▼
                    ┌─────────────────────────┐
                    │  11-logger-block-       │
                    │  level-propagation      │
                    │  (Follow-up Fix)        │
                    └─────────────────────────┘

┌─────────────────────────┐
│  12-horizontal-scroll-  │
│  line-truncation        │
│  (Independent Fix)      │
└─────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-stack-trace-parser-types](tasks/01-stack-trace-parser-types.md) | Done | - | 3-4h | `core/stack_trace.rs` (NEW) |
| 2 | [02-stack-trace-parsing-logic](tasks/02-stack-trace-parsing-logic.md) | Done | 1 | 4-5h | `core/stack_trace.rs` |
| 3 | [03-enhance-sample-apps-test-logs](tasks/03-enhance-sample-apps-test-logs.md) | Done | - | 3-4h | `sample/lib/`, `sample2/lib/` |
| 4 | [04-integrate-stack-trace-parsing](tasks/04-integrate-stack-trace-parsing.md) | Done | 2 | 3-4h | `app/handler/session.rs`, `core/types.rs` |
| 5 | [05-stack-trace-rendering](tasks/05-stack-trace-rendering.md) | Done | 4 | 5-6h | `tui/widgets/log_view.rs` |
| 6 | [06-collapsible-stack-traces](tasks/06-collapsible-stack-traces.md) | Done | 5 | 4-5h | `tui/widgets/log_view.rs`, `app/session.rs`, `config/types.rs` |
| 7 | [07-error-count-status-bar](tasks/07-error-count-status-bar.md) | Done | 4 | 2-3h | `tui/widgets/status_bar.rs`, `app/session.rs` |
| 8 | [08-strip-ansi-escape-codes](tasks/08-strip-ansi-escape-codes.md) | Done | - | 2-3h | `core/ansi.rs` (NEW), `daemon/protocol.rs`, `app/handler/helpers.rs` |
| 9 | [09-enhance-log-level-detection](tasks/09-enhance-log-level-detection.md) | Done | 8 | 3-4h | `daemon/protocol.rs`, `app/handler/helpers.rs` |
| 10 | [10-strip-flutter-prefix-raw-lines](tasks/10-strip-flutter-prefix-raw-lines.md) | Done | 8 | 2-3h | `app/handler/helpers.rs` |
| 11 | [11-logger-block-level-propagation](tasks/11-logger-block-level-propagation.md) | Done | 8, 9 | 4-5h | `app/handler/helpers.rs`, `app/session.rs`, `core/types.rs` |
| 12 | [12-horizontal-scroll-line-truncation](tasks/12-horizontal-scroll-line-truncation.md) | Done | - | 4-5h | `tui/widgets/log_view.rs`, `app/handler/keys.rs`, `app/message.rs` |

## Dart Stack Trace Formats to Support

The stack trace parser must handle these common Dart/Flutter formats:

### Standard Dart VM Format
```
#0      main (package:app/main.dart:15:3)
#1      _startIsolate.<anonymous closure> (dart:isolate-patch/isolate_patch.dart:307)
#2      _RawReceivePort._handleMessage (dart:isolate-patch/isolate_patch.dart:174)
```

### Flutter Framework Format
```
#0      State.setState.<anonymous closure> (package:flutter/src/widgets/framework.dart:1187:9)
#1      State.setState (package:flutter/src/widgets/framework.dart:1222:6)
#2      _MyHomePageState._incrementCounter (package:sample/main.dart:45:5)
```

### Async/Await Format
```
#0      someAsyncFunction (package:app/utils.dart:23:7)
<asynchronous suspension>
#1      main (package:app/main.dart:10:3)
```

### Package Trace (Friendly) Format
```
package:app/main.dart 15:3                main
package:flutter/src/widgets/framework.dart 1187:9  State.setState.<anonymous closure>
```

## Success Criteria

Phase 2 is complete when:

- [x] Dart stack traces parsed correctly (all formats above)
- [x] `StackFrame` struct captures: file path, line, column, function name, package info
- [x] Error messages visually distinct (color-coded by level)
- [x] Error icon prefixes displayed (✗, ⚠, •, ·) - already exists
- [x] File:line references highlighted in distinct color (blue/underline)
- [x] Package frames dimmed (from `pub cache`, `dart:` prefixes)
- [x] Project frames emphasized (from `lib/`, `test/`, `package:app/`)
- [x] Stack traces collapsible with Enter key
- [x] Collapsed indicator shows: `▶ 3 more frames...`
- [x] Expanded indicator shows: `▼ Stack trace:`
- [x] Config option for default collapsed state
- [x] Config option for max visible frames when collapsed
- [x] Error count displayed in status bar
- [x] Sample apps contain diverse test logs and crashes
- [x] All new code has unit tests
- [x] No regressions in existing functionality

## Keyboard Shortcuts (Phase 2)

| Key | Action |
|-----|--------|
| `Enter` | Expand/collapse stack trace (when cursor on error) |
| `e` | Jump to next error (already implemented in Phase 1) |
| `E` | Jump to previous error (already implemented in Phase 1) |

## Configuration Additions

Add to `config/types.rs` and `.fdemon/config.toml`:

```toml
[ui]
# Stack trace display settings
stack_trace_collapsed = true    # Default collapsed state
stack_trace_max_frames = 5      # Max frames shown when collapsed
```

## Testing Notes

- Use `sample/` and `sample2/` Flutter apps for manual testing
- Add intentional crashes (null checks, divide by zero, assertion failures)
- Add various log levels (print, debugPrint, log with levels)
- Test with both iOS Simulator and Android emulator
- Verify stack traces render correctly for:
  - Synchronous exceptions
  - Async/await exceptions
  - Flutter framework errors (widget build errors)
  - Dart assertion failures

## Dependencies on Phase 1

Phase 2 builds on Phase 1 functionality:
- Error navigation (`e`/`E`) already implemented
- LogLevel and LogSource types exist
- Basic level styling exists in log_view.rs
- Filter state infrastructure can be reused for collapse state

## Notes

- Stack trace parsing uses the existing `regex` crate
- Parsed stack traces are stored alongside log entries, not replacing them
- Collapse state is per-entry and persists during the session
- File paths in stack traces will be prepared for Phase 3 (OSC 8 hyperlinks)

---

## Follow-up Fix Tasks

These tasks address issues discovered during testing with the sample apps:

### Task 08: Strip ANSI Escape Codes
**Issue**: The Logger package outputs ANSI escape codes for terminal coloring (e.g., `\x1b[38;5;244m`). These appear as garbage text like `^[[38;5;244m` in Flutter Demon's TUI because the raw escape codes are displayed as literal text instead of being interpreted or stripped.

**Solution**: Strip ANSI escape sequences from incoming log messages before processing and display.

### Task 09: Enhance Log Level Detection
**Issue**: Flutter Demon cannot correctly detect log levels from Logger/Talker package output. The packages use specific prefixes (`Trace:`, `Debug:`, `Info:`, `Warning:`, `Error:`, `Fatal:`) and emojis (🐛, 💡, ⚠️, ⛔, 🔥) that aren't recognized by the current detection logic.

**Solution**: Enhance `detect_log_level()` to recognize Logger/Talker package patterns, enabling accurate log filtering.

**Dependency**: Task 09 depends on Task 08 (ANSI codes must be stripped before level detection can work accurately).

### Task 10: Strip Redundant "flutter:" Prefix from Raw Lines
**Issue**: Raw stdout lines (non-JSON) display `flutter:` in the message content even though the `[flutter]` source tag is already shown, resulting in redundant output like `[flutter] flutter: message`.

**Root Cause**: `detect_raw_line_level()` in `helpers.rs` does not strip the `flutter: ` prefix like `parse_flutter_log()` does for JSON messages.

**Solution**: Add `flutter: ` prefix stripping to `detect_raw_line_level()`.

### Task 11: Logger Package Block-Level Propagation
**Issue**: Logger package outputs multi-line structured logs with box-drawing characters (┌│└├), but each line is processed independently. Only lines containing error indicators (like "Error" or ⛔) are styled red; the rest of the block remains white.

**Solution**: Detect Logger package structured log blocks (from ┌ to └) and propagate the highest severity level found within the block to all lines in that block.

**Dependency**: Tasks 08 and 09 should be completed first for accurate level detection.

### Task 12: Horizontal Scrolling and Line Truncation Fix
**Issue**: When terminal width is narrow, long log lines wrap but scroll calculations don't account for wrapped lines, causing content to be "cut off" and users unable to scroll to see all content.

**Solution**: Disable line wrapping, truncate lines at terminal width, and add horizontal scroll capability with ←/→ or h/l keys. Show truncation indicators when content extends beyond visible area.

**Dependency**: Independent fix - can be done in parallel with other tasks.