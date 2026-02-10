# Plan: Widget Crash Detection & Collapsible Display

## TL;DR

Detect Flutter framework exception blocks (multi-line "EXCEPTION CAUGHT BY" banners) arriving on both stdout (raw non-JSON lines and `app.log` events) and stderr, buffer them into single cohesive `LogEntry` items with parsed stack traces, and display them as collapsible widget crash entries in the log view using the existing collapse infrastructure.

---

## Background

Flutter framework exceptions (widget build errors, assertion failures, rendering crashes) produce multi-line diagnostic banners like:

```
══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═════════════
The following assertion was thrown building _CodeLine(...):
'package:flutter/src/widgets/container.dart': Failed assertion: ...

The relevant error-causing widget was:
  _CodeLine
  _CodeLine:file:///Users/.../ide_code_viewer.dart:72:22

When the exception was thrown, this was the stack:
#2      new Container (package:flutter/src/widgets/container.dart:270:15)
#3      _CodeLine.build (package:zabin_app/.../ide_code_viewer.dart:141:16)
... (300+ frames)
(elided 2 frames from class _AssertionError)
════════════════════════════════════════════════════════
```

### Current Problem

fdemon does not surface these exceptions as recognizable crash entries:

1. **Fragmented display**: Each line of the exception block becomes a separate `LogEntry`, losing the structured context (which widget crashed, the assertion, the stack trace).
2. **No stack trace parsing**: The embedded `#N` frames in the exception block are never parsed by `ParsedStackTrace`, so they don't get collapsible rendering.
3. **Possible invisibility**: In `--machine` mode, some exception output may be suppressed or routed through unexpected paths. Flutter GitHub Issue [#92903](https://github.com/flutter/flutter/issues/92903) documents inconsistencies with error visibility in machine mode.

### How Flutter Outputs Exceptions

Flutter exception banners are rendered by `FlutterError.dumpErrorToConsole()` → `debugPrint()` → Dart's `print()` → **stdout**. In `--machine` mode, this output arrives via one of three paths:

| Path | How it arrives | Current handling |
|------|----------------|-----------------|
| **`app.log` JSON-RPC event** | Stdout JSON: `[{"event":"app.log","params":{"log":"...","error":true}}]` | Parsed by `protocol.rs` → `LogEntry` with optional `stackTrace` field. Multi-line `log` field may be split across events. |
| **Raw stdout text** | Non-JSON stdout lines between JSON-RPC messages | Fallback in `handle_session_stdout()` creates individual `LogEntry` per line. |
| **Stderr text** | Raw stderr lines | `daemon.rs` handler creates individual `LogEntry` per line. |

All three paths currently produce **per-line entries** without multi-line awareness. The solution must handle exception blocks from any path.

### Flutter Exception Block Format

**Structure** (from `FlutterError` / `TextTreeRenderer` in `diagnostics.dart`):

```
══╡ EXCEPTION CAUGHT BY <LIBRARY> ╞═══════════  ← Header (start marker)
<Exception description>                          ← Error message (multi-line)
                                                 ← Blank lines
The relevant error-causing widget was:           ← Widget info section
  <WidgetName>
  <WidgetName>:file:///<path>:<line>:<col>

When the exception was thrown, this was the stack: ← Stack trace section
#N      <function> (<file>:<line>:<col>)           ← Dart VM frames
...     Normal element mounting (N frames)         ← Elided frames
(elided N frames from class <ClassName>)           ← Elided summary
════════════════════════════════════════════════  ← Footer (end marker)
```

**Other exception variants:**
- `"Another exception was thrown: <summary>"` — Compact one-line follow-up for cascading errors
- `"═══ Exception caught by <library> ═══"` — Alternate header format in some Flutter versions

---

## Affected Modules

- `crates/fdemon-core/src/exception_block.rs` — **NEW** Exception block parser and types
- `crates/fdemon-core/src/lib.rs` — Export new types
- `crates/fdemon-app/src/handler/daemon.rs` — Route stderr lines through exception buffer
- `crates/fdemon-app/src/handler/session.rs` — Route stdout lines through exception buffer
- `crates/fdemon-app/src/session.rs` — Add `ExceptionBuffer` to `Session`
- `crates/fdemon-tui/src/widgets/log_view/mod.rs` — Minor styling for crash entries (optional)

---

## Development Phases

### Phase 1: Exception Block Detection, Buffering & Display

**Goal**: Detect multi-line Flutter exception blocks from any input path, buffer them into single `LogEntry` items with parsed stack traces, and display them as collapsible entries.

#### Steps

1. **Exception Block Parser (fdemon-core)**
   - New `crates/fdemon-core/src/exception_block.rs` module
   - `ExceptionBlock` struct: captures library name, error message, widget info, raw stack trace text
   - `ExceptionBlockParser` (line-by-line state machine):
     - States: `Idle` → `InHeader` → `InBody` → `InStackTrace` → `Complete`
     - Start marker: line contains `EXCEPTION CAUGHT BY` with `══╡` prefix
     - End marker: line is entirely `═` characters (the footer border)
     - Extracts: library name, error description, error-causing widget, stack trace frames
   - `feed_line(line) -> FeedResult` enum:
     - `Buffered` — line was consumed by the exception block
     - `NotConsumed` — line is not part of an exception block (pass through)
     - `Complete(ExceptionBlock)` — exception block is complete, here's the parsed result
   - Convert `ExceptionBlock` to `LogEntry` with:
     - `level: LogLevel::Error`
     - `message`: Compact summary, e.g. `"WIDGETS LIBRARY: Failed assertion: 'margin.isNonNegative' — _CodeLine"`
     - `stack_trace`: Parsed via existing `ParsedStackTrace::parse()`
   - Timeout safety: if no footer arrives within N lines (e.g., 500), flush as incomplete
   - Handle `"Another exception was thrown: <summary>"` one-liners (detect and create Error-level entry directly)

2. **Exception Buffer in Session (fdemon-app)**
   - Add `ExceptionBlockParser` field to `Session` struct (similar to existing `LogBlockState`)
   - New method `Session::process_stderr_line(line) -> Vec<LogEntry>`:
     - Feed line to exception parser
     - If `NotConsumed`: create normal `LogEntry` via existing `detect_raw_line_level()` path
     - If `Buffered`: return empty (line is being accumulated)
     - If `Complete(block)`: convert to `LogEntry` with stack trace, return it
   - New method `Session::process_raw_stdout_line(line) -> Vec<LogEntry>`:
     - Same logic as stderr, sharing the same `ExceptionBlockParser` instance
     - Only used for non-JSON stdout lines (fallback path in `handle_session_stdout`)

3. **Integrate with Daemon Handler (fdemon-app)**
   - **Stderr path** (`handler/daemon.rs`): Replace direct `LogEntry::new()` with `session.process_stderr_line()`
   - **Stdout fallback path** (`handler/session.rs`): Replace direct `LogEntry::new()` for non-JSON lines with `session.process_raw_stdout_line()`
   - **`app.log` path** (`handler/session.rs`): Check if `app.log` event's `log` field contains exception block markers; if so, feed through exception parser
   - Queue resulting entries through existing `queue_log()` / `flush_batched_logs()` pipeline

4. **Display with Existing Collapse Infrastructure**
   - Exception block `LogEntry` items will have `stack_trace: Some(ParsedStackTrace)` populated
   - The existing `LogView` rendering already handles:
     - Collapsible stack traces (default collapsed, 3 visible frames)
     - "N more frames..." indicator
     - Enter key to expand/collapse
     - Project vs package frame styling
   - No changes needed to `LogView` for basic functionality
   - Optional: Add distinct styling for crash entries (e.g., red left border or crash icon prefix)

5. **Flush on Session Exit**
   - When a session exits (`DaemonEvent::Exited`), flush any pending exception buffer as incomplete
   - Ensures no buffered lines are lost if the Flutter process crashes mid-exception

**Milestone**: Widget crash exceptions appear as single, collapsible log entries with parsed stack traces.

---

## Edge Cases & Risks

### Interleaved Output
- **Risk**: Other log lines arrive between exception block lines (from async output or concurrent isolates)
- **Mitigation**: The exception parser tracks state per-session. Lines that don't match expected patterns within the block trigger a flush of the incomplete buffer, emitting whatever was accumulated so far as a partial entry, then processing the new line normally.

### Incomplete Exception Blocks
- **Risk**: Flutter process crashes or is killed mid-exception, leaving a partial block
- **Mitigation**: Line count safety limit (500 lines). Session exit flushes pending buffer. Time-based flush (optional, via tick message checking buffer age).

### Very Large Stack Traces
- **Risk**: Widget exceptions can have 300+ frames, creating huge `ParsedStackTrace`
- **Mitigation**: The existing collapse infrastructure handles this well (shows 3 frames by default). The parser can also cap at a maximum frame count if needed.

### `app.log` Event Splitting
- **Risk**: Flutter may split exception blocks across multiple `app.log` events, each with a partial `log` field
- **Mitigation**: The exception parser is stateful and processes line-by-line regardless of which event delivered the line. Multiple `app.log` events with partial content will be correctly buffered.

### Logger Package Blocks Inside Exceptions
- **Risk**: A Logger package block (┌─┘) might contain an exception block or vice versa
- **Mitigation**: Exception detection takes priority over Logger block detection. The `══╡` pattern is unambiguous and distinct from Logger's `┌` pattern. If an exception block is detected, Logger block tracking pauses until the exception completes.

### Performance
- **Risk**: Additional parsing overhead per stderr/stdout line
- **Mitigation**: The start marker check (`contains("EXCEPTION CAUGHT BY")`) is a simple string scan, only performed on non-empty lines. The state machine adds minimal overhead when `Idle`. Existing batching (16ms/100 entries) is preserved.

---

## Task Dependency Graph

```
┌──────────────────────────────────┐
│  01-exception-block-parser       │  (fdemon-core)
│  Types + state machine + tests   │
└──────────────┬───────────────────┘
               │
               ▼
┌──────────────────────────────────┐
│  02-session-exception-buffer     │  (fdemon-app)
│  Session integration + methods   │
└──────────────┬───────────────────┘
               │
               ▼
┌──────────────────────────────────┐
│  03-handler-integration          │  (fdemon-app)
│  Wire into daemon + session      │
│  handlers, flush on exit         │
└──────────────┬───────────────────┘
               │
               ▼
┌──────────────────────────────────┐
│  04-crash-entry-styling          │  (fdemon-tui, optional)
│  Visual distinction for crashes  │
└──────────────────────────────────┘
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] Flutter widget exceptions (══╡ EXCEPTION CAUGHT BY ╞══) are detected in stderr and stdout
- [ ] Multi-line exception blocks are buffered into single `LogEntry` items
- [ ] Stack traces within exception blocks are parsed via `ParsedStackTrace::parse()`
- [ ] Exception entries appear collapsible in the log view (default collapsed, Enter to expand)
- [ ] "Another exception was thrown:" one-liners are detected and shown as Error level
- [ ] Incomplete exception blocks are flushed on session exit
- [ ] Line count safety limit prevents unbounded buffering
- [ ] No regression in existing log display (Logger blocks, regular stderr/stdout, JSON-RPC events)
- [ ] All new code has unit tests
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy` pass

---

## References

- [Flutter Daemon Protocol](https://github.com/flutter/flutter/blob/master/packages/flutter_tools/doc/daemon.md) — JSON-RPC protocol spec
- [FlutterError.dumpErrorToConsole](https://api.flutter.dev/flutter/foundation/FlutterError/dumpErrorToConsole.html) — Error output method
- [Flutter Error Handling Guide](https://docs.flutter.dev/testing/errors) — Error routing through FlutterError.onError
- [Flutter diagnostics.dart](https://github.com/flutter/flutter/blob/master/packages/flutter/lib/src/foundation/diagnostics.dart) — TextTreeRenderer and error banner formatting
- [GitHub Issue #92903](https://github.com/flutter/flutter/issues/92903) — Errors not shown in --machine mode
- Existing fdemon infrastructure: `stack_trace.rs` (parser), `session.rs` (CollapseState, LogBlockState), `log_view/mod.rs` (collapsible rendering)
