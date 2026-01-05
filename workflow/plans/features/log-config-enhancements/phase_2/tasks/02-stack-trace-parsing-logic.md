## Task: Stack Trace Parsing Logic

**Objective**: Implement the parsing logic to extract structured `StackFrame` data from raw Dart/Flutter stack trace strings, handling multiple trace formats and edge cases.

**Depends on**: [01-stack-trace-parser-types](01-stack-trace-parser-types.md)

### Scope

- `src/core/stack_trace.rs`: Add parsing functions and format detection

### Parsing Functions to Implement

```rust
impl ParsedStackTrace {
    /// Parse a raw stack trace string into structured frames
    pub fn parse(raw: &str) -> Self {
        // 1. Detect format
        // 2. Apply appropriate parser
        // 3. Return ParsedStackTrace with frames
    }
}

/// Detect the format of a stack trace string
pub fn detect_format(trace: &str) -> StackTraceFormat {
    // Check for #N pattern (Dart VM)
    // Check for friendly format
    // Return Unknown if no match
}

/// Parse Dart VM format stack traces
/// Format: #0      function (file:line:col)
fn parse_dart_vm_trace(trace: &str) -> Vec<StackFrame> {
    // Apply regex, extract frames
}

/// Parse friendly format stack traces
/// Format: file line:col  function
fn parse_friendly_trace(trace: &str) -> Vec<StackFrame> {
    // Apply regex, extract frames
}

/// Determine if a file path is from a package/SDK (should be dimmed)
pub fn is_package_path(path: &str) -> bool {
    // Check dart:, package:flutter/, pub cache patterns
}

/// Extract the project name from pubspec or config
/// Used to identify project frames vs dependency frames
pub fn detect_project_package_name(project_path: Option<&Path>) -> Option<String> {
    // Read pubspec.yaml if available
    // Return package name
}
```

### Regex Patterns

```rust
use regex::Regex;
use std::sync::LazyLock;

/// Dart VM stack frame: #0      main (package:app/main.dart:15:3)
static DART_VM_FRAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"#(\d+)\s+(.+?)\s+\((.+?):(\d+):(\d+)\)").unwrap()
});

/// Dart VM frame without column: #0      main (package:app/main.dart:15)
static DART_VM_FRAME_NO_COL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"#(\d+)\s+(.+?)\s+\((.+?):(\d+)\)").unwrap()
});

/// Async suspension gap marker
static ASYNC_GAP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<asynchronous suspension>").unwrap()
});

/// Friendly format: package:app/main.dart 15:3  main
static FRIENDLY_FRAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(.+?)\s+(\d+):(\d+)\s+(.+)$").unwrap()
});

/// Package/SDK detection patterns
static PACKAGE_PATTERNS: &[&str] = &[
    "dart:",
    "package:flutter/",
    "package:flutter_test/",
    ".pub-cache",
    "pub.dev",
];
```

### Edge Cases to Handle

1. **Missing Column Number**
   - Some traces have only line: `#0 main (file.dart:15)`
   - Default column to 0

2. **Anonymous Closures**
   - Function name: `<anonymous closure>`
   - Preserve as-is

3. **Nested Closures**
   - Function name: `main.<anonymous closure>.<anonymous closure>`
   - Parse correctly without breaking on dots

4. **Async Suspension Markers**
   - Line: `<asynchronous suspension>`
   - Create frame with `is_async_gap = true`

5. **Mixed Format Traces**
   - Some Flutter errors mix formats
   - Parse line-by-line, detect format per line

6. **Empty/Whitespace Lines**
   - Skip empty lines
   - Trim whitespace

7. **Very Long Function Names**
   - Constructors: `_SomePrivateClass.someVeryLongMethodName`
   - Full capture without truncation

8. **File URI Format**
   - `file:///path/to/file.dart:15:3`
   - Extract path correctly

### Implementation Steps

1. **Format Detection**
   ```rust
   pub fn detect_format(trace: &str) -> StackTraceFormat {
       let first_line = trace.lines().next().unwrap_or("");
       
       if first_line.starts_with('#') && DART_VM_FRAME.is_match(first_line) {
           StackTraceFormat::DartVm
       } else if FRIENDLY_FRAME.is_match(first_line) {
           StackTraceFormat::Friendly
       } else {
           StackTraceFormat::Unknown
       }
   }
   ```

2. **Dart VM Parsing**
   ```rust
   fn parse_dart_vm_trace(trace: &str) -> Vec<StackFrame> {
       trace.lines()
           .filter_map(|line| {
               let line = line.trim();
               
               // Check for async gap
               if line.contains("<asynchronous suspension>") {
                   return Some(StackFrame::async_gap());
               }
               
               // Try with column
               if let Some(caps) = DART_VM_FRAME.captures(line) {
                   return Some(StackFrame::from_captures(&caps));
               }
               
               // Try without column
               if let Some(caps) = DART_VM_FRAME_NO_COL.captures(line) {
                   return Some(StackFrame::from_captures_no_col(&caps));
               }
               
               None
           })
           .collect()
   }
   ```

3. **Package Detection**
   ```rust
   pub fn is_package_path(path: &str) -> bool {
       PACKAGE_PATTERNS.iter().any(|p| path.contains(p))
   }
   
   // More sophisticated: check against project name
   pub fn is_package_path_with_project(path: &str, project_name: Option<&str>) -> bool {
       // If path starts with package:project_name/, it's a project frame
       if let Some(name) = project_name {
           if path.starts_with(&format!("package:{}/", name)) {
               return false; // Project frame, not package
           }
       }
       
       is_package_path(path)
   }
   ```

### Acceptance Criteria

1. [ ] `ParsedStackTrace::parse()` correctly parses Dart VM format
2. [ ] `ParsedStackTrace::parse()` correctly parses friendly format
3. [ ] `detect_format()` correctly identifies trace formats
4. [ ] Async suspension gaps parsed as special frames
5. [ ] Missing column numbers handled (default to 0)
6. [ ] Package frames correctly identified (`dart:`, `package:flutter/`)
7. [ ] Project frames correctly identified (app package name)
8. [ ] Empty/whitespace lines skipped
9. [ ] Regex patterns compile without panic (tested)
10. [ ] Performance acceptable for traces up to 100 frames

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_DART_VM_TRACE: &str = r#"
#0      main (package:sample/main.dart:15:3)
#1      _startIsolate.<anonymous closure> (dart:isolate-patch/isolate_patch.dart:307:19)
#2      _RawReceivePort._handleMessage (dart:isolate-patch/isolate_patch.dart:174:12)
"#;

    const SAMPLE_ASYNC_TRACE: &str = r#"
#0      someAsyncFunction (package:app/utils.dart:23:7)
<asynchronous suspension>
#1      main (package:app/main.dart:10:3)
"#;

    const SAMPLE_FLUTTER_TRACE: &str = r#"
#0      State.setState.<anonymous closure> (package:flutter/src/widgets/framework.dart:1187:9)
#1      State.setState (package:flutter/src/widgets/framework.dart:1222:6)
#2      _MyHomePageState._incrementCounter (package:sample/main.dart:45:5)
"#;

    #[test]
    fn test_detect_dart_vm_format() {
        let format = detect_format(SAMPLE_DART_VM_TRACE);
        assert!(matches!(format, StackTraceFormat::DartVm));
    }

    #[test]
    fn test_parse_dart_vm_trace() {
        let trace = ParsedStackTrace::parse(SAMPLE_DART_VM_TRACE);
        assert_eq!(trace.frames.len(), 3);
        
        let first = &trace.frames[0];
        assert_eq!(first.frame_number, 0);
        assert_eq!(first.function_name, "main");
        assert_eq!(first.file_path, "package:sample/main.dart");
        assert_eq!(first.line, 15);
        assert_eq!(first.column, 3);
        assert!(!first.is_package_frame);
    }

    #[test]
    fn test_parse_async_trace() {
        let trace = ParsedStackTrace::parse(SAMPLE_ASYNC_TRACE);
        assert_eq!(trace.frames.len(), 3);
        assert!(trace.frames[1].is_async_gap);
    }

    #[test]
    fn test_parse_flutter_trace_package_detection() {
        let trace = ParsedStackTrace::parse(SAMPLE_FLUTTER_TRACE);
        
        // Flutter framework frames are package frames
        assert!(trace.frames[0].is_package_frame);
        assert!(trace.frames[1].is_package_frame);
        
        // App frame is NOT a package frame
        assert!(!trace.frames[2].is_package_frame);
    }

    #[test]
    fn test_anonymous_closure_parsing() {
        let line = "#0      State.setState.<anonymous closure> (package:flutter/src/widgets/framework.dart:1187:9)";
        let trace = ParsedStackTrace::parse(line);
        assert_eq!(trace.frames[0].function_name, "State.setState.<anonymous closure>");
    }

    #[test]
    fn test_is_package_path() {
        // Dart SDK
        assert!(is_package_path("dart:isolate-patch/isolate_patch.dart"));
        assert!(is_package_path("dart:async/future.dart"));
        
        // Flutter SDK
        assert!(is_package_path("package:flutter/src/widgets/framework.dart"));
        assert!(is_package_path("package:flutter_test/flutter_test.dart"));
        
        // Pub cache
        assert!(is_package_path("/Users/user/.pub-cache/hosted/pub.dev/provider/lib/src/provider.dart"));
        
        // App package - NOT a package path
        assert!(!is_package_path("package:my_app/main.dart"));
        assert!(!is_package_path("package:sample/src/utils.dart"));
    }

    #[test]
    fn test_empty_trace() {
        let trace = ParsedStackTrace::parse("");
        assert!(trace.frames.is_empty());
        assert!(!trace.is_complete);
    }

    #[test]
    fn test_whitespace_handling() {
        let trace_with_whitespace = "  \n  #0      main (package:app/main.dart:15:3)  \n  \n";
        let trace = ParsedStackTrace::parse(trace_with_whitespace);
        assert_eq!(trace.frames.len(), 1);
    }

    #[test]
    fn test_frame_without_column() {
        let line = "#0      main (package:app/main.dart:15)";
        let trace = ParsedStackTrace::parse(line);
        assert_eq!(trace.frames[0].line, 15);
        assert_eq!(trace.frames[0].column, 0);
    }
}
```

### Files to Modify

| File | Action | Description |
|------|--------|-------------|
| `src/core/stack_trace.rs` | Modify | Add parsing functions and regex patterns |

### Estimated Time

4-5 hours

### References

- Task 01 types (StackFrame, ParsedStackTrace)
- Rust `regex` crate documentation
- Dart stack trace format examples from PLAN.md