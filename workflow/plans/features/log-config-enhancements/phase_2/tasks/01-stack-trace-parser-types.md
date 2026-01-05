## Task: Stack Trace Parser Types

**Objective**: Define the core data types and structures for parsing Dart/Flutter stack traces, laying the foundation for stack trace extraction and rendering.

**Depends on**: None

### Scope

- `src/core/stack_trace.rs`: **NEW** - Create new module with stack trace types
- `src/core/mod.rs`: Add `stack_trace` module export

### Types to Define

```rust
/// Represents a single frame in a stack trace
pub struct StackFrame {
    /// Frame number (e.g., 0, 1, 2 from #0, #1, #2)
    pub frame_number: usize,
    
    /// Function/method name (e.g., "main", "State.setState")
    pub function_name: String,
    
    /// File path (e.g., "package:app/main.dart", "dart:isolate-patch/...")
    pub file_path: String,
    
    /// Line number (1-based)
    pub line: u32,
    
    /// Column number (1-based)
    pub column: u32,
    
    /// Whether this is a package/SDK frame (should be dimmed)
    pub is_package_frame: bool,
    
    /// Whether this is an async suspension marker
    pub is_async_gap: bool,
}

/// Represents a parsed stack trace with multiple frames
pub struct ParsedStackTrace {
    /// Original raw stack trace string
    pub raw: String,
    
    /// Parsed frames
    pub frames: Vec<StackFrame>,
    
    /// Whether parsing was fully successful
    pub is_complete: bool,
}

/// Stack trace format variants for parsing
pub enum StackTraceFormat {
    /// Standard Dart VM: #0 function (file:line:col)
    DartVm,
    
    /// Flutter/package format
    Flutter,
    
    /// Friendly format: file line:col function
    Friendly,
    
    /// Unknown/unparseable format
    Unknown,
}
```

### Regex Patterns to Define

Define lazy-static or const regex patterns for:

1. **Dart VM Format**: `#(\d+)\s+(.+?)\s+\((.+?):(\d+):(\d+)\)`
   - Matches: `#0      main (package:app/main.dart:15:3)`
   
2. **Async Gap Marker**: `<asynchronous suspension>`
   - Simple string match
   
3. **Package Detection**: `^(dart:|package:flutter/|package:.*pub\.dev)`
   - Identifies SDK/package frames vs project frames

4. **Friendly Format**: `(.+?)\s+(\d+):(\d+)\s+(.+)`
   - Matches: `package:app/main.dart 15:3  main`

### Implementation Notes

1. **StackFrame Methods**:
   - `new()` - Constructor with all fields
   - `is_project_frame()` - Returns `!is_package_frame && !is_async_gap`
   - `display_location()` - Returns formatted "file:line:col"
   - `short_path()` - Extracts just filename from full path

2. **ParsedStackTrace Methods**:
   - `new(raw: &str)` - Create with raw string, empty frames
   - `add_frame(frame: StackFrame)` - Add a parsed frame
   - `project_frames()` - Iterator over non-package frames
   - `visible_frames(max: usize)` - First N frames for collapsed view
   - `hidden_count(max: usize)` - Count of frames beyond max

3. **Package Frame Detection**:
   - `dart:` prefix → package frame (Dart SDK)
   - `package:flutter/` → package frame (Flutter SDK)
   - Path contains `.pub-cache` → package frame
   - `package:app_name/` where app_name matches project → project frame
   - `lib/` or `test/` in path → project frame

### Acceptance Criteria

1. [ ] `StackFrame` struct defined with all fields
2. [ ] `ParsedStackTrace` struct defined with frame collection
3. [ ] `StackTraceFormat` enum defined for format detection
4. [ ] Regex pattern constants defined for Dart VM format
5. [ ] Regex pattern constants defined for friendly format
6. [ ] `is_package_frame()` detection logic implemented
7. [ ] `display_location()` helper returns "file:line:col"
8. [ ] `short_path()` extracts filename from package path
9. [ ] Module exported from `core/mod.rs`
10. [ ] All types derive `Debug, Clone`
11. [ ] `StackFrame` derives `PartialEq, Eq` for testing

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_frame_creation() {
        let frame = StackFrame {
            frame_number: 0,
            function_name: "main".to_string(),
            file_path: "package:app/main.dart".to_string(),
            line: 15,
            column: 3,
            is_package_frame: false,
            is_async_gap: false,
        };
        assert_eq!(frame.display_location(), "package:app/main.dart:15:3");
    }

    #[test]
    fn test_short_path_extraction() {
        let frame = StackFrame {
            file_path: "package:my_app/src/utils/helpers.dart".to_string(),
            // ... other fields
        };
        assert_eq!(frame.short_path(), "helpers.dart");
    }

    #[test]
    fn test_package_frame_detection() {
        // dart: prefix is package
        assert!(is_package_path("dart:isolate-patch/isolate_patch.dart"));
        
        // Flutter SDK is package
        assert!(is_package_path("package:flutter/src/widgets/framework.dart"));
        
        // App package is NOT package frame
        assert!(!is_package_path("package:my_app/main.dart"));
    }

    #[test]
    fn test_async_gap_frame() {
        let frame = StackFrame {
            is_async_gap: true,
            // ... other fields default
        };
        assert!(!frame.is_project_frame());
    }

    #[test]
    fn test_parsed_stack_trace_visible_frames() {
        let mut trace = ParsedStackTrace::new("#0 main...");
        // Add 10 frames...
        assert_eq!(trace.visible_frames(5).count(), 5);
        assert_eq!(trace.hidden_count(5), 5);
    }
}
```

### Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `src/core/stack_trace.rs` | Create | New module with all types and tests |
| `src/core/mod.rs` | Modify | Add `pub mod stack_trace;` export |

### Estimated Time

3-4 hours

### References

- [Dart Stack Trace Format](https://dart.dev/guides/language/language-tour#exceptions)
- Phase 1 `core/types.rs` for pattern reference
- `regex` crate documentation