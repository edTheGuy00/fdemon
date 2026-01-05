## Task: Cursor Position and File Reference Tracking

**Objective**: Track which log entry and stack frame is currently "focused" in the log view, and extract file:line references that can be opened in an editor.

**Depends on**: None (can be developed in parallel with Tasks 01-02)

### Scope

- `src/tui/widgets/log_view.rs`: Track focused entry during rendering
- `src/app/session.rs`: Add focused file reference state
- `src/tui/hyperlinks.rs`: Use `FileReference` struct (from Task 01, or define here if Task 01 not complete)
- `src/core/types.rs`: Potentially add helper methods to LogEntry

### Background

For the `o` key to open a file at the cursor position, we need to:
1. Know which log entry is currently at the "focus" position (top of visible area or highlighted)
2. If that entry has a stack trace, identify which frame is visible
3. Extract the file path, line, and column from the focused element
4. Handle entries without stack traces by detecting file:line patterns in the message

### File Reference Type

```rust
// In src/tui/hyperlinks.rs or src/core/types.rs
#[derive(Debug, Clone, PartialEq)]
pub struct FileReference {
    /// File path (may be package:, dart:, or absolute/relative)
    pub path: String,
    
    /// Line number (1-based)
    pub line: u32,
    
    /// Column number (1-based, 0 if unknown)
    pub column: u32,
    
    /// Source of this reference (for display/debugging)
    pub source: FileReferenceSource,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileReferenceSource {
    /// From a parsed stack frame
    StackFrame,
    /// Detected in log message text
    LogMessage,
    /// From error source location
    ErrorLocation,
}

impl FileReference {
    pub fn from_stack_frame(frame: &StackFrame) -> Option<Self> {
        if frame.is_async_gap {
            return None;
        }
        
        Some(Self {
            path: frame.file_path.clone(),
            line: frame.line,
            column: frame.column,
            source: FileReferenceSource::StackFrame,
        })
    }
    
    /// Convert package: path to absolute path if possible
    pub fn resolve_path(&self, project_root: &Path) -> PathBuf {
        if self.path.starts_with("package:") {
            // Extract package path: package:app/src/main.dart -> lib/src/main.dart
            // This requires knowing the package name mapping
            // For now, assume package:app/... maps to lib/...
            let package_path = self.path.strip_prefix("package:").unwrap_or(&self.path);
            if let Some(rest) = package_path.split_once('/') {
                return project_root.join("lib").join(rest.1);
            }
        }
        
        // Already absolute or relative
        PathBuf::from(&self.path)
    }
}
```

### Session State Addition

```rust
// In src/app/session.rs - add to Session struct
pub struct Session {
    // ... existing fields ...
    
    /// Currently focused file reference (if any)
    /// Updated during rendering based on scroll position
    pub focused_file_ref: Option<FileReference>,
    
    /// Index of the focused log entry
    pub focused_entry_index: Option<usize>,
    
    /// Index of the focused frame within a stack trace (if applicable)
    pub focused_frame_index: Option<usize>,
}
```

### Focus Position Logic

The "focus" is determined by the current scroll position. Several strategies:

**Option A: First Visible Entry**
- The topmost fully visible entry is the focused one
- Simple to implement
- May not match user expectation

**Option B: Cursor Line (Recommended)**
- Track a "cursor" line that the user can move with j/k
- Focus is whatever entry/frame is at the cursor line
- More intuitive for file opening

**Option C: Center of Viewport**
- The entry at the vertical center is focused
- Good for "what am I looking at" but less precise

For Phase 3, implement **Option A** first with preparation for **Option B**:

```rust
// In src/tui/widgets/log_view.rs

impl<'a> StatefulWidget for LogView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut LogViewState) {
        // ... existing rendering logic ...
        
        // Track focus during rendering
        let mut focus_info = FocusInfo::default();
        
        // As we render each entry, check if it's the focus position
        let focus_line = 0; // First visible line (Option A)
        
        for (entry_idx, entry) in visible_entries.iter().enumerate() {
            let entry_start_line = current_line;
            
            // Render entry...
            
            // Check if this entry contains the focus line
            if current_line > focus_line && focus_info.entry_index.is_none() {
                focus_info.entry_index = Some(entry_idx);
                
                // If entry has stack trace, find focused frame
                if let Some(trace) = &entry.stack_trace {
                    let line_within_entry = focus_line - entry_start_line;
                    if line_within_entry > 0 {
                        // Focus is on a stack frame
                        let frame_idx = (line_within_entry - 1) as usize;
                        if frame_idx < trace.frames.len() {
                            focus_info.frame_index = Some(frame_idx);
                            focus_info.file_ref = FileReference::from_stack_frame(&trace.frames[frame_idx]);
                        }
                    } else {
                        // Focus is on the entry message itself
                        focus_info.file_ref = extract_file_ref_from_message(&entry.message);
                    }
                } else {
                    focus_info.file_ref = extract_file_ref_from_message(&entry.message);
                }
            }
            
            current_line += entry_line_count;
        }
        
        // Store focus info in state for retrieval
        state.focus_info = focus_info;
    }
}

#[derive(Debug, Default, Clone)]
pub struct FocusInfo {
    pub entry_index: Option<usize>,
    pub frame_index: Option<usize>,
    pub file_ref: Option<FileReference>,
}
```

### Extracting File References from Log Messages

For log entries without stack traces, detect file:line patterns:

```rust
// In src/tui/hyperlinks.rs or src/core/types.rs

use regex::Regex;
use std::sync::LazyLock;

/// Regex to detect file:line patterns in log messages
/// Matches:
/// - lib/main.dart:15:3
/// - package:app/main.dart:15
/// - /absolute/path/file.dart:100:5
/// - file.dart:42
static FILE_LINE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:(?:package:|lib/|test/|[\w./]+)?[\w/]+\.dart):(\d+)(?::(\d+))?")
        .expect("File:line regex is valid")
});

/// Extract file reference from a log message
pub fn extract_file_ref_from_message(message: &str) -> Option<FileReference> {
    if let Some(caps) = FILE_LINE_PATTERN.captures(message) {
        let full_match = caps.get(0)?.as_str();
        
        // Split on the first colon that precedes a digit
        if let Some((path_part, rest)) = full_match.rsplit_once(':') {
            // Check if path_part ends with .dart
            if !path_part.ends_with(".dart") {
                // The path includes more, need to find actual file path
                if let Some((path, _)) = rest.split_once(':') {
                    // path:line:col case handled differently
                }
            }
        }
        
        // Simplified parsing:
        let parts: Vec<&str> = full_match.rsplitn(3, ':').collect();
        match parts.len() {
            3 => {
                // file:line:col
                let column = parts[0].parse().unwrap_or(0);
                let line = parts[1].parse().ok()?;
                let path = parts[2].to_string();
                Some(FileReference {
                    path,
                    line,
                    column,
                    source: FileReferenceSource::LogMessage,
                })
            }
            2 => {
                // file:line
                let line = parts[0].parse().ok()?;
                let path = parts[1].to_string();
                Some(FileReference {
                    path,
                    line,
                    column: 0,
                    source: FileReferenceSource::LogMessage,
                })
            }
            _ => None,
        }
    } else {
        None
    }
}
```

### LogViewState Update

```rust
// Update LogViewState in log_view.rs
pub struct LogViewState {
    // ... existing fields ...
    
    /// Information about the currently focused element
    pub focus_info: FocusInfo,
}

impl LogViewState {
    pub fn new() -> Self {
        Self {
            offset: 0,
            h_offset: 0,
            auto_scroll: true,
            total_lines: 0,
            visible_lines: 0,
            max_line_width: 0,
            visible_width: 0,
            focus_info: FocusInfo::default(),
        }
    }
    
    /// Get the currently focused file reference, if any
    pub fn focused_file_ref(&self) -> Option<&FileReference> {
        self.focus_info.file_ref.as_ref()
    }
}
```

### Visual Focus Indicator (Optional Enhancement)

To help users know what will open with `o`:

```rust
// In log_view.rs rendering

// Add a subtle indicator for the focused line
if is_focus_line {
    // Option 1: Left margin marker
    buf.set_string(area.x, y, "▸", Style::default().fg(Color::Yellow));
    
    // Option 2: Subtle background highlight
    for x in area.x..area.x + area.width {
        let cell = buf.get_mut(x, y);
        cell.set_bg(Color::Rgb(40, 40, 50)); // Very subtle highlight
    }
}
```

### Acceptance Criteria

1. [ ] `FileReference` struct defined with path, line, column, source
2. [ ] `FileReference::from_stack_frame()` extracts reference from StackFrame
3. [ ] `FileReference::resolve_path()` converts package: paths to file paths
4. [ ] `extract_file_ref_from_message()` detects file:line in log text
5. [ ] `FocusInfo` struct tracks focused entry and frame indices
6. [ ] `LogViewState` updated with focus_info during render
7. [ ] Focus determined by first visible entry (scroll position)
8. [ ] Stack frame focus correctly identified when scrolled to frame line
9. [ ] Session state holds focused_file_ref for action handlers to access
10. [ ] Unit tests for file reference extraction from messages
11. [ ] Unit tests for FileReference::resolve_path()

### Testing

**Unit Tests:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_file_ref_basic() {
        let msg = "Error at lib/main.dart:15:3";
        let ref_opt = extract_file_ref_from_message(msg);
        assert!(ref_opt.is_some());
        let file_ref = ref_opt.unwrap();
        assert_eq!(file_ref.path, "lib/main.dart");
        assert_eq!(file_ref.line, 15);
        assert_eq!(file_ref.column, 3);
    }
    
    #[test]
    fn test_extract_file_ref_package() {
        let msg = "package:my_app/utils.dart:42";
        let ref_opt = extract_file_ref_from_message(msg);
        assert!(ref_opt.is_some());
        let file_ref = ref_opt.unwrap();
        assert!(file_ref.path.contains("my_app/utils.dart"));
        assert_eq!(file_ref.line, 42);
    }
    
    #[test]
    fn test_extract_file_ref_no_match() {
        let msg = "Just a regular log message";
        assert!(extract_file_ref_from_message(msg).is_none());
    }
    
    #[test]
    fn test_file_reference_from_stack_frame() {
        let frame = StackFrame::new(0, "main", "package:app/main.dart", 15, 3);
        let file_ref = FileReference::from_stack_frame(&frame);
        assert!(file_ref.is_some());
        let file_ref = file_ref.unwrap();
        assert_eq!(file_ref.line, 15);
        assert_eq!(file_ref.column, 3);
    }
    
    #[test]
    fn test_file_reference_from_async_gap() {
        let frame = StackFrame::async_gap(0);
        assert!(FileReference::from_stack_frame(&frame).is_none());
    }
    
    #[test]
    fn test_resolve_path_package() {
        let file_ref = FileReference {
            path: "package:my_app/src/utils.dart".to_string(),
            line: 10,
            column: 5,
            source: FileReferenceSource::StackFrame,
        };
        let project_root = Path::new("/home/user/my_app");
        let resolved = file_ref.resolve_path(project_root);
        assert_eq!(resolved, PathBuf::from("/home/user/my_app/lib/src/utils.dart"));
    }
    
    #[test]
    fn test_resolve_path_absolute() {
        let file_ref = FileReference {
            path: "/absolute/path/file.dart".to_string(),
            line: 10,
            column: 5,
            source: FileReferenceSource::LogMessage,
        };
        let project_root = Path::new("/home/user/my_app");
        let resolved = file_ref.resolve_path(project_root);
        assert_eq!(resolved, PathBuf::from("/absolute/path/file.dart"));
    }
}
```

**Manual Testing:**
1. Run Flutter Demon with sample app
2. Trigger errors with stack traces
3. Scroll through logs
4. Verify focus indicator appears (if implemented)
5. Verify correct entry/frame is tracked as focus changes

### Notes

- Focus tracking happens during render pass (minimal performance impact)
- Package path resolution may need enhancement for multi-package projects
- Consider caching pubspec.yaml package name for accurate path resolution
- This task prepares the data; Task 04 handles the actual file opening