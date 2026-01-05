## Task: Integrate Stack Trace Parsing into Log Processing

**Objective**: Connect the stack trace parser to the log entry processing pipeline so that stack traces received from Flutter are automatically parsed and stored alongside log entries.

**Depends on**: [02-stack-trace-parsing-logic](02-stack-trace-parsing-logic.md)

### Scope

- `src/core/types.rs`: Extend `LogEntry` to optionally hold parsed stack trace
- `src/app/handler/session.rs`: Parse stack traces when processing logs
- `src/daemon/protocol.rs`: Ensure stack trace data flows through correctly

### Current State

Currently, stack traces are handled in `session.rs`:

```rust
// Current implementation - stack traces added as separate log entries
if let Some(trace) = entry_info.stack_trace {
    for trace_line in trace.lines().take(10) {
        handle.session.add_log(LogEntry::new(
            LogLevel::Debug,
            LogSource::FlutterError,
            format!("    {}", trace_line),
        ));
    }
}
```

### Target State

Stack traces should be parsed and attached to the originating error log entry:

```rust
// New implementation - stack traces parsed and attached to log entry
let mut log_entry = LogEntry::new(
    entry_info.level,
    entry_info.source,
    entry_info.message,
);

if let Some(trace) = entry_info.stack_trace {
    log_entry.stack_trace = Some(ParsedStackTrace::parse(&trace));
}

handle.session.add_log(log_entry);
```

### Changes to LogEntry

```rust
// In src/core/types.rs

use crate::core::stack_trace::ParsedStackTrace;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub level: LogLevel,
    pub source: LogSource,
    pub message: String,
    
    /// Parsed stack trace, if this is an error with a trace
    pub stack_trace: Option<ParsedStackTrace>,
    
    /// Unique ID for this entry (for collapse state tracking)
    pub id: u64,
}

impl LogEntry {
    /// Create a new log entry with current timestamp
    pub fn new(level: LogLevel, source: LogSource, message: impl Into<String>) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        
        Self {
            timestamp: Local::now(),
            level,
            source,
            message: message.into(),
            stack_trace: None,
            id: COUNTER.fetch_add(1, Ordering::Relaxed),
        }
    }
    
    /// Create a new log entry with a stack trace
    pub fn with_stack_trace(
        level: LogLevel,
        source: LogSource,
        message: impl Into<String>,
        trace: ParsedStackTrace,
    ) -> Self {
        let mut entry = Self::new(level, source, message);
        entry.stack_trace = Some(trace);
        entry
    }
    
    /// Check if this entry has a stack trace
    pub fn has_stack_trace(&self) -> bool {
        self.stack_trace.is_some()
    }
    
    /// Get stack trace frame count
    pub fn stack_trace_frame_count(&self) -> usize {
        self.stack_trace.as_ref().map(|t| t.frames.len()).unwrap_or(0)
    }
}
```

### Changes to Session Handler

```rust
// In src/app/handler/session.rs

use crate::core::stack_trace::ParsedStackTrace;

pub fn handle_session_stdout(state: &mut AppState, session_id: SessionId, line: &str) {
    // ... existing parsing logic ...
    
    if let Some(entry_info) = msg.to_log_entry() {
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            // Create log entry with parsed stack trace
            let log_entry = if let Some(trace_str) = entry_info.stack_trace {
                let parsed_trace = ParsedStackTrace::parse(&trace_str);
                LogEntry::with_stack_trace(
                    entry_info.level,
                    entry_info.source,
                    entry_info.message,
                    parsed_trace,
                )
            } else {
                LogEntry::new(
                    entry_info.level,
                    entry_info.source,
                    entry_info.message,
                )
            };
            
            handle.session.add_log(log_entry);
        }
    }
    
    // ... rest of handler ...
}
```

### Project Name Detection

To properly distinguish project frames from package frames, pass project name:

```rust
// In session handler or app state
let project_name = state.project_name.clone();

// When parsing
let parsed_trace = ParsedStackTrace::parse_with_project(&trace_str, project_name.as_deref());
```

### Backward Compatibility

Ensure existing code continues to work:

1. `LogEntry::new()` works without stack trace (stack_trace = None)
2. `LogEntry::info()`, `LogEntry::error()`, `LogEntry::warn()` still work
3. Existing log display functions handle `stack_trace: None` gracefully

### Acceptance Criteria

1. [ ] `LogEntry` struct extended with `stack_trace: Option<ParsedStackTrace>`
2. [ ] `LogEntry` has unique `id` field for tracking collapse state
3. [ ] `LogEntry::with_stack_trace()` constructor added
4. [ ] `LogEntry::has_stack_trace()` helper added
5. [ ] Stack traces parsed when processing `app.log` events with traces
6. [ ] Stack traces parsed when processing `daemon.logMessage` events
7. [ ] Project name passed to parser for frame classification
8. [ ] Existing log creation code unchanged (backward compatible)
9. [ ] No stack trace lines added as separate log entries anymore
10. [ ] All existing tests pass

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_log_entry_with_stack_trace() {
        let trace = ParsedStackTrace::parse("#0 main (package:app/main.dart:15:3)");
        let entry = LogEntry::with_stack_trace(
            LogLevel::Error,
            LogSource::App,
            "Something went wrong",
            trace,
        );
        
        assert!(entry.has_stack_trace());
        assert_eq!(entry.stack_trace_frame_count(), 1);
    }
    
    #[test]
    fn test_log_entry_without_stack_trace() {
        let entry = LogEntry::new(LogLevel::Info, LogSource::App, "Hello");
        
        assert!(!entry.has_stack_trace());
        assert_eq!(entry.stack_trace_frame_count(), 0);
    }
    
    #[test]
    fn test_log_entry_id_uniqueness() {
        let entry1 = LogEntry::new(LogLevel::Info, LogSource::App, "First");
        let entry2 = LogEntry::new(LogLevel::Info, LogSource::App, "Second");
        
        assert_ne!(entry1.id, entry2.id);
    }
    
    #[test]
    fn test_backward_compatibility() {
        // These should all compile and work
        let _ = LogEntry::info(LogSource::App, "Info message");
        let _ = LogEntry::error(LogSource::App, "Error message");
        let _ = LogEntry::warn(LogSource::App, "Warning message");
    }
}
```

### Integration Testing

Test with sample app:

1. Run Flutter Demon with `sample/` project
2. Trigger an error with stack trace
3. Verify log entry has `stack_trace.is_some()`
4. Verify frames are correctly parsed
5. Verify project frames identified correctly

### Files to Modify

| File | Action | Description |
|------|--------|-------------|
| `src/core/types.rs` | Modify | Add `stack_trace` and `id` fields to `LogEntry` |
| `src/core/mod.rs` | Modify | Ensure `stack_trace` module is accessible |
| `src/app/handler/session.rs` | Modify | Parse and attach stack traces to log entries |
| `src/daemon/protocol.rs` | Review | Ensure `LogEntryInfo.stack_trace` flows correctly |

### Estimated Time

3-4 hours

### Notes

- The `id` field is needed for Phase 2 Task 6 (collapsible stack traces) to track which entries are collapsed/expanded
- Using `AtomicU64` for ID generation ensures thread-safety and uniqueness
- Remove the old code that adds stack trace lines as separate `LogEntry` items
- Keep the raw trace string in `ParsedStackTrace` for debugging/fallback display