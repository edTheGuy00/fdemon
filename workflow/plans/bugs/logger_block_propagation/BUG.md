# Plan: Fix Logger Block Propagation and Performance

## TL;DR
Fix inconsistent coloring of structured log blocks (from `logger` package) where individual lines (like stack traces) don't inherit the block's error severity. Address false positive error detection on class names (e.g., "ErrorTestingPage"). Optimize log processing to reduce CPU usage during high-volume logging by minimizing redundant parsing and backward scanning.

## Affected Modules
- `src/app/session.rs`: Log storage and block propagation logic.
- `src/app/handler/helpers.rs`: Log level detection and block boundary identification.
- `src/core/ansi.rs`: ANSI code stripping and handling.

## Phases (Revised 2025-01-05)

### Phase 1: Stateful Block Tracking (Priority: HIGH)
Replace backward-scanning block propagation with incremental state tracking.
- **Current Problem:** `propagate_block_level()` scans backwards up to 50 lines on every block end - O(N*M) worst case.
- **Solution:** Track block state as lines arrive:
    - When `┌` is seen → record block start index, initialize max level
    - As lines arrive inside block → update max level if higher severity found
    - When `└` is seen → apply max level to known range (start..end)
- **Complexity:** O(B) where B is block size, only on block end - NOT on every line
- **Benefit:** Fixes both performance AND block propagation correctness in one change

### Phase 2: Fix False Positive Detection (Priority: MEDIUM)
Prevent class names like `ErrorTestingPage` from triggering Error-level detection.
- **Root Cause:** `lower.contains("error")` matches substrings within identifiers
- **Solution:** Use word boundary checks:
    - Match ` error `, `error:`, `Error:`, `[error]` patterns
    - Or use regex: `\berror\b` (word boundary)
- **Scope:** Only affects `app.log` events (stdout) - daemon messages have explicit level field
- **Note:** This is a minor fix once Phase 1 is complete

### ~~Phase 3: Leverage Daemon Level Field~~ (DEPRIORITIZED)
~~Trust the `level` field from Flutter daemon JSON events.~~
- **Reality Check:** Only `daemon.logMessage` events have a level field
- **App logs** (`app.log` from print/debugPrint/Logger) do NOT have level - they're raw stdout
- **Conclusion:** This optimization has limited value since most logs are app output
- **Action:** Keep content-based detection for app logs, can optionally trust daemon level for `daemon.logMessage` events

### Phase 3: Additional Performance Optimizations (Priority: LOW)
Further optimizations if CPU usage remains high after Phase 1:
- **Ring Buffer:** Cap log storage with `VecDeque` (fixed memory footprint)
- **Virtualized Display:** Only render visible logs + buffer
- **Coalesce Updates:** Batch rapid log arrivals, throttle re-renders to ~60fps

## Edge Cases & Risks
- **Interleaved Logs:** Logs from other sources (e.g., `LogSource::App`) appearing in the middle of a `LogSource::Flutter` block.
- **Incomplete Blocks:** Blocks that start but never end (e.g., app crash).
- **Huge Blocks:** Blocks larger than the scan limit (currently 50 lines).
- **Regex Performance:** Ensure any new regexes for detection are efficient.

## Further Considerations
- Should we move log processing to a background thread if CPU usage remains high?
- Can we cache the "clean" version of the log message to avoid repeated ANSI stripping?

---

## Research Findings: VS Code & Industry Approaches (2025-01-05)

### VS Code Debug Console Issues

VS Code's debug console has **known performance problems** with high-volume log output:

1. **No Built-in Virtualization**: The debug console does NOT use virtualized rendering. Each output event from DAP is rendered into the DOM, causing performance degradation with large outputs ([Issue #83393](https://github.com/microsoft/vscode/issues/83393), [Issue #19678](https://github.com/Microsoft/vscode/issues/19678)).

2. **DAP Output Events**: The [Debug Adapter Protocol](https://microsoft.github.io/debug-adapter-protocol/specification.html) sends discrete `output` events with no built-in batching or flow control. Each event has a `category` (stdout/stderr/console) and `output` field.

3. **Known Bottleneck**: "The entire VS Code UI can become very slow during debugging when the debug console is opened with several pages of output" - this suggests Flutter Demon's approach is not necessarily worse than VS Code.

4. **Mitigation**: VS Code auto-clears console on debug restart and limits scrollback. Android Studio halts logcat stream when not visible to conserve CPU.

### VS Code Terminal (xterm.js) Optimizations

VS Code's integrated terminal uses [xterm.js](https://github.com/xtermjs/xterm.js/issues/791) which has more sophisticated handling:

1. **Ring Buffer**: Uses circular buffer for scrollback to cap memory usage
2. **Rate-Limited Viewport Refresh**: Doesn't re-render on every line
3. **DOM Node Reuse**: Reduces garbage collection overhead
4. **WebGL Rendering**: But has limits (max ~220 lines with default texture size)
5. **Throttling**: Has throttling mechanism for large data pastes ([Issue #283056](https://github.com/microsoft/vscode/issues/283056))

### Flutter Daemon Protocol Insights

From the [Flutter Daemon documentation](https://github.com/flutter/flutter/blob/master/packages/flutter_tools/doc/daemon.md):

1. **JSON-RPC Protocol**: Communication uses JSON-RPC over stdin/stdout
2. **Debounce Removed**: Flutter 1.23+ removed automatic debouncing, meaning rapid-fire events are sent individually
3. **No Batching**: Events are sent one-at-a-time over stdout

#### Critical: Two Different Log Sources

| Event Type | Has Level Field? | Has Error Flag? | Source | Examples |
|------------|------------------|-----------------|--------|----------|
| `daemon.logMessage` | ✅ Yes (`level` field) | N/A | Flutter tooling | Build status, device events |
| `app.log` | ❌ No | ✅ Yes (`error: bool`) | App stdout/stderr | `print()`, Logger package |
| Raw process stderr | ❌ No | ❌ No | Direct stderr | Native crashes |

#### The `error: true` Flag (Already Implemented!)

The `app.log` event includes an `error: bool` field that indicates whether the log came from **stderr**:

```json
{"event":"app.log","params":{"appId":"abc","log":"Error message","error":true,"stackTrace":"..."}}
```

**Current behavior** (in `parse_flutter_log()`):
- If `error: true` → immediately return `LogLevel::Error` (trusted, no content parsing)
- If `error: false` → fall back to content-based detection (where false positives occur)

**Implication**: The false positive issue ("ErrorTestingPage") only affects **stdout logs** (`error: false`). True errors from stderr are already correctly detected via the `error: true` flag.

This reduces the scope of Task 02 (false positives) - it only needs to fix content-based detection for stdout logs, not all logs.

### Ratatui Performance Considerations

From [Ratatui documentation](https://ratatui.rs/concepts/rendering/):

1. **Immediate Mode Rendering**: UI redrawn every frame - efficient diffing handles what actually changes
2. **No Built-in Virtualization**: The List widget renders all items; virtualization must be implemented manually
3. **Third-party Options**: [rat-widget](https://github.com/ratatui/awesome-ratatui) provides table widgets designed for large datasets

### Ring Buffer Pattern (Industry Standard)

The [log_buffer](https://github.com/whitequark/rust-log_buffer) crate demonstrates the ideal pattern:
- Zero-allocation ring buffer
- Fixed memory footprint regardless of log volume
- O(1) insertions, automatic eviction of oldest entries
- Similar to Linux `dmesg` facility

---

## Recommended Architecture Changes

### Phase 3A: Stateful Block Tracking (O(1) per line)

**Current Problem**: `propagate_block_level()` scans backwards up to 50 lines on every block end - O(N*M) worst case.

**Solution**: Track block state incrementally:

```rust
pub struct LogBlockState {
    /// Index where current block started (if any)
    block_start: Option<usize>,
    /// Highest severity seen in current block
    block_max_level: LogLevel,
}

impl Session {
    fn add_log(&mut self, mut entry: LogEntry) {
        let idx = self.logs.len();

        // Track block state as we go
        if is_block_start(&entry.message) {
            self.block_state.block_start = Some(idx);
            self.block_state.block_max_level = entry.level;
        } else if self.block_state.block_start.is_some() {
            // Inside a block - update max level
            if entry.level.is_more_severe_than(&self.block_state.block_max_level) {
                self.block_state.block_max_level = entry.level;
            }

            // Block ended - apply max level to all block lines
            if is_block_end(&entry.message) {
                let start = self.block_state.block_start.take().unwrap();
                let max_level = self.block_state.block_max_level;

                // Update all block entries (single pass, known range)
                for i in start..=idx {
                    self.logs[i].level = max_level;
                }
            }
        }

        self.logs.push(entry);
    }
}
```

**Complexity**: O(B) where B is block size, only when block ends - NOT on every line.

### Phase 3B: Ring Buffer for Log Storage

**Current Problem**: `Vec<LogEntry>` grows unbounded, consuming RAM.

**Solution**: Use a ring buffer with configurable capacity:

```rust
use std::collections::VecDeque;

const MAX_LOG_ENTRIES: usize = 10_000; // Configurable

pub struct Session {
    logs: VecDeque<LogEntry>,
    // ... other fields
}

impl Session {
    fn add_log(&mut self, entry: LogEntry) {
        if self.logs.len() >= MAX_LOG_ENTRIES {
            self.logs.pop_front();
        }
        self.logs.push_back(entry);
    }
}
```

**Benefits**:
- Fixed memory footprint
- O(1) insertions
- Natural FIFO eviction of old logs
- Aligns with VS Code's scrollback limiting approach

### ~~Phase 3C: Trust Flutter Daemon Level Field~~ (DEPRIORITIZED)

**Why Deprioritized**: After deeper research, the `level` field only exists on `daemon.logMessage` events (build status, device info). App logs via `app.log` events are raw stdout with NO level field - and this is where Logger package output comes from.

**Limited Value**: Most log volume is app output, not daemon messages. Content-based detection is still required for:
- `print()` / `debugPrint()` output
- Logger package formatted blocks
- Talker package output
- Any stdout/stderr from the Flutter app

**Optional Enhancement**: Can still trust level field for `daemon.logMessage` events as a minor optimization, but this won't significantly impact CPU usage.

### Phase 3D: Virtualized Log Display

**Current Problem**: Rendering 10,000+ log entries every frame.

**Solution**: Only render visible viewport + buffer:

```rust
fn render_logs(&self, area: Rect, logs: &VecDeque<LogEntry>, scroll_offset: usize) {
    let visible_height = area.height as usize;
    let buffer_lines = 20; // Extra lines above/below viewport

    let start = scroll_offset.saturating_sub(buffer_lines);
    let end = (scroll_offset + visible_height + buffer_lines).min(logs.len());

    let visible_logs: Vec<_> = logs.range(start..end).collect();
    // Only pass visible_logs to List widget
}
```

### Phase 3E: Coalesce Rapid Updates

**Current Problem**: Processing each log line triggers immediate re-render.

**Solution**: Batch updates and throttle rendering:

```rust
pub struct LogBatcher {
    pending: Vec<LogEntry>,
    last_flush: Instant,
    flush_interval: Duration, // e.g., 16ms for 60fps
}

impl LogBatcher {
    pub fn add(&mut self, entry: LogEntry) {
        self.pending.push(entry);

        // Flush if interval elapsed or buffer full
        if self.last_flush.elapsed() >= self.flush_interval
           || self.pending.len() >= 100 {
            self.flush();
        }
    }

    fn flush(&mut self) {
        // Batch insert all pending entries
        // Trigger single re-render
        self.last_flush = Instant::now();
    }
}
```

---

## Priority Order (Revised)

| Priority | Phase | Impact | Effort |
|----------|-------|--------|--------|
| 1 | **Stateful Block Tracking** | Fixes O(N*M) → O(B), fixes block propagation | Medium |
| 2 | **False Positive Detection** | Fixes "ErrorTestingPage" issue | Low |
| 3 | **Ring Buffer** | Caps memory growth | Low |
| 4 | **Coalesce Updates** | Reduces render frequency | Medium |
| 5 | **Virtualized Display** | Final optimization for massive logs | High |

**Note:** "Trust Daemon Level" (original Phase 3C) has been **deprioritized** - it only applies to daemon messages, not app logs where the real volume comes from.

---

## References

- [VS Code Debug Console Issue #83393](https://github.com/microsoft/vscode/issues/83393)
- [VS Code Debug Console Issue #19678](https://github.com/Microsoft/vscode/issues/19678)
- [DAP Specification](https://microsoft.github.io/debug-adapter-protocol/specification.html)
- [xterm.js Buffer Performance](https://github.com/xtermjs/xterm.js/issues/791)
- [Flutter Daemon Protocol](https://github.com/flutter/flutter/blob/master/packages/flutter_tools/doc/daemon.md)
- [Ratatui Rendering](https://ratatui.rs/concepts/rendering/)
- [log_buffer Ring Buffer](https://github.com/whitequark/rust-log_buffer)
- [Android Studio Logcat Performance](https://alexzh.com/new-logcat-5-features-for-effective-android-app-debugging/)