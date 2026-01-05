# Phase 2: Error Highlighting & Stack Traces - Task Index

## Overview

Phase 2 implements enhanced error visualization and stack trace parsing for Flutter Demon. Users will see visually distinct error messages with color-coded severity levels, parsed and formatted stack traces with clickable file references, and collapsible stack trace views for better log readability.

**Estimated Duration:** 1-1.5 weeks  
**Total Tasks:** 7  
**Estimated Hours:** 25-32 hours

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
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-stack-trace-parser-types](tasks/01-stack-trace-parser-types.md) | Not Started | - | 3-4h | `core/stack_trace.rs` (NEW) |
| 2 | [02-stack-trace-parsing-logic](tasks/02-stack-trace-parsing-logic.md) | Not Started | 1 | 4-5h | `core/stack_trace.rs` |
| 3 | [03-enhance-sample-apps-test-logs](tasks/03-enhance-sample-apps-test-logs.md) | Not Started | - | 3-4h | `sample/lib/`, `sample2/lib/` |
| 4 | [04-integrate-stack-trace-parsing](tasks/04-integrate-stack-trace-parsing.md) | Not Started | 2 | 3-4h | `app/handler/session.rs`, `core/types.rs` |
| 5 | [05-stack-trace-rendering](tasks/05-stack-trace-rendering.md) | Not Started | 4 | 5-6h | `tui/widgets/log_view.rs` |
| 6 | [06-collapsible-stack-traces](tasks/06-collapsible-stack-traces.md) | Not Started | 5 | 4-5h | `tui/widgets/log_view.rs`, `app/session.rs`, `config/types.rs` |
| 7 | [07-error-count-status-bar](tasks/07-error-count-status-bar.md) | Not Started | 4 | 2-3h | `tui/widgets/status_bar.rs`, `app/session.rs` |

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

- [ ] Dart stack traces parsed correctly (all formats above)
- [ ] `StackFrame` struct captures: file path, line, column, function name, package info
- [ ] Error messages visually distinct (color-coded by level)
- [ ] Error icon prefixes displayed (✗, ⚠, •, ·) - already exists
- [ ] File:line references highlighted in distinct color (blue/underline)
- [ ] Package frames dimmed (from `pub cache`, `dart:` prefixes)
- [ ] Project frames emphasized (from `lib/`, `test/`, `package:app/`)
- [ ] Stack traces collapsible with Enter key
- [ ] Collapsed indicator shows: `▶ 3 more frames...`
- [ ] Expanded indicator shows: `▼ Stack trace:`
- [ ] Config option for default collapsed state
- [ ] Config option for max visible frames when collapsed
- [ ] Error count displayed in status bar
- [ ] Sample apps contain diverse test logs and crashes
- [ ] All new code has unit tests
- [ ] No regressions in existing functionality

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