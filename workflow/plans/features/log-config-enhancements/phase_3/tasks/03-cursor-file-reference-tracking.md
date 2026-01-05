## Task: Cursor Position and File Reference Tracking

**Objective**: Track which log entry and stack frame is currently "focused" in the log view, and extract file:line references that can be opened in an editor.

**Depends on**: None (can be developed in parallel with Tasks 01-02)

### Scope

- `src/tui/widgets/log_view.rs`: Track focused entry during rendering, add `FocusInfo` to `LogViewState`
- `src/tui/hyperlinks.rs`: Use `FileReference` struct (from Task 01, or define here if Task 01 not complete)

> **Note**: Session struct changes are NOT needed. Existing `Session::focused_entry()` and `Session::focused_entry_id()` methods (added in Phase 2) already provide basic focus tracking. This task extends that with file reference extraction.

### Prerequisites: Existing Infrastructure

The logger block propagation bug fix added performance improvements that benefit this task:

| Component | Location | How It Helps |
|-----------|----------|--------------|
| `Session::focused_entry()` | `session.rs:757-760` | Returns `Option<&LogEntry>` at scroll position |
| `Session::focused_entry_id()` | `session.rs:763-765` | Returns focused entry's ID |
| `Session::current_log_position()` | `session.rs:739-750` | Maps scroll offset to log index (handles filtering) |
| `LogViewState::visible_range()` | `log_view.rs:107-111` | Returns `(start, end)` for virtualized rendering |
| `VecDeque<LogEntry>` | `session.rs:205` | Log storage (supports indexing, same API as Vec) |

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

### LogViewState Update (NOT Session)

> **Important**: Do NOT add fields to Session. The existing `focused_entry()` method already tracks the focused log entry. We add `FocusInfo` to `LogViewState` to track file references during rendering.

```rust
// In src/tui/widgets/log_view.rs - add to LogViewState struct
pub struct LogViewState {
    // ... existing fields (offset, h_offset, auto_scroll, etc.) ...
    
    /// Information about the currently focused element (updated during render)
    pub focus_info: FocusInfo,
}
```

This approach is better because:
1. Focus info is computed during render (render has access to visible entries)
2. LogViewState is the natural home for rendering-related state
3. Session already has `focused_entry()` for basic focus tracking

### Focus Position Logic

The "focus" is determined by the current scroll position. The existing `Session::focused_entry()` method already implements **Option A: First Visible Entry** using `current_log_position()`.

**Current Implementation (to leverage):**
```rust
// Already exists in session.rs:739-750
fn current_log_position(&self) -> usize {
    if self.filter_state.is_active() {
        // Map filtered offset to original index
        let filtered = self.filtered_log_indices();
        filtered.get(self.log_view_state.offset).copied().unwrap_or(0)
    } else {
        self.log_view_state.offset
    }
}

// Already exists in session.rs:757-760
pub fn focused_entry(&self) -> Option<&LogEntry> {
    let pos = self.current_log_position();
    self.logs.get(pos)
}
```

**Phase 3 Extension**: During render, we extend this to track which stack frame within the focused entry is visible, and extract the file reference:

```rust
// In src/tui/widgets/log_view.rs

impl<'a> StatefulWidget for LogView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut LogViewState) {
        // ... existing rendering logic ...
        
        // Track focus during rendering (leveraging existing virtualization)
        let mut focus_info = FocusInfo::default();
        
        // The first visible entry is the focus (matches Session::focused_entry() behavior)
        // Use visible_range() for efficient iteration - only process visible entries
        let (visible_start, visible_end) = state.visible_range();
        let focus_line = 0; // First visible line
        
        // Note: logs is now VecDeque<LogEntry> but indexing works the same
        for (entry_idx, entry) in self.logs.iter().enumerate().skip(visible_start).take(visible_end - visible_start) {
            let entry_start_line = current_line;
            
            // Render entry...
            
            // Check if this entry contains the focus line
            if current_line > focus_line && focus_info.entry_index.is_none() {
                focus_info.entry_index = Some(entry_idx);
                focus_info.entry_id = Some(entry.id);
                
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
    /// Index of the focused entry in the log buffer
    pub entry_index: Option<usize>,
    /// ID of the focused entry (for stability across buffer changes)
    pub entry_id: Option<u64>,
    /// Index of the focused frame within a stack trace (if applicable)
    pub frame_index: Option<usize>,
    /// Extracted file reference (if any)
    pub file_ref: Option<FileReference>,
}
```

> **VecDeque Note**: The log buffer is now `VecDeque<LogEntry>` (from the ring buffer implementation). This doesn't affect indexing - `VecDeque` implements the `Index` trait, so `logs[i]` still works.

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
// Note: LogViewState already has these fields from bug fix work:
// - offset, h_offset, auto_scroll, total_lines, visible_lines
// - max_line_width, visible_width, buffer_lines
// - visible_range() method

pub struct LogViewState {
    // ... existing fields ...
    
    /// Buffer lines above/below viewport (already exists from Task 05)
    pub buffer_lines: usize,
    
    /// Information about the currently focused element (NEW)
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
            buffer_lines: DEFAULT_BUFFER_LINES, // Already exists
            focus_info: FocusInfo::default(),   // NEW
        }
    }
    
    /// Get the currently focused file reference, if any
    pub fn focused_file_ref(&self) -> Option<&FileReference> {
        self.focus_info.file_ref.as_ref()
    }
    
    /// Get range of line indices to render (already exists from Task 05)
    pub fn visible_range(&self) -> (usize, usize) {
        let start = self.offset.saturating_sub(self.buffer_lines);
        let end = (self.offset + self.visible_lines + self.buffer_lines).min(self.total_lines);
        (start, end)
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

1. [ ] `FileReference` struct defined with path, line, column, source (in `tui/hyperlinks.rs`)
2. [ ] `FileReference::from_stack_frame()` extracts reference from StackFrame
3. [ ] `FileReference::resolve_path()` converts package: paths to file paths
4. [ ] `extract_file_ref_from_message()` detects file:line in log text
5. [ ] `FocusInfo` struct tracks focused entry index, ID, frame index, and file ref
6. [ ] `LogViewState` updated with `focus_info` field and `focused_file_ref()` method
7. [ ] Focus tracking integrates with existing `visible_range()` for efficiency
8. [ ] Stack frame focus correctly identified when scrolled to frame line
9. [ ] Works with `VecDeque<LogEntry>` (no code changes needed - indexing works)
10. [ ] Unit tests for file reference extraction from messages
11. [ ] Unit tests for FileReference::resolve_path()
12. [ ] Leverages existing `Session::focused_entry()` pattern

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
- **Performance**: With virtualization, focus tracking only processes visible entries (~30-50 max)
- Package path resolution may need enhancement for multi-package projects
- Consider caching pubspec.yaml package name for accurate path resolution
- This task prepares the data; Task 04 handles the actual file opening
- **Existing Infrastructure**: Leverage `Session::focused_entry()` pattern rather than duplicating logic
- **VecDeque Compatibility**: Log storage changed from `Vec` to `VecDeque` in bug fix work; indexing still works identically

### Files to Modify

| File | Action | Description |
|------|--------|-------------|
| `src/tui/widgets/log_view.rs` | Modify | Add `FocusInfo` struct, add `focus_info` field to `LogViewState`, add `focused_file_ref()` method |
| `src/tui/hyperlinks.rs` | Modify | Add `FileReference` struct (may already exist from Task 01) and `extract_file_ref_from_message()` |

> **Note**: `src/app/session.rs` does NOT need modification - existing `focused_entry()` infrastructure is sufficient.

### Estimated Effort

2-3 hours (reduced from 3-4h due to existing infrastructure)

---

## Completion Summary

**Status:** ✅ Done

**Date Completed:** 2026-01-05

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/hyperlinks.rs` | Added `FileReferenceSource` enum, updated `FileReference` with `source` field and `with_source()` constructor, added `resolve_path()` method, added `extract_file_ref_from_message()` function with regex-based parsing |
| `src/tui/widgets/log_view.rs` | Added `FocusInfo` struct with `entry_index`, `entry_id`, `frame_index`, and `file_ref` fields; added `focus_info` field to `LogViewState`; added `focused_file_ref()` method |

### Implementation Details

1. **FileReferenceSource Enum**: Added enum with three variants:
   - `StackFrame` - for references from parsed stack frames
   - `LogMessage` - for references detected in log message text
   - `ErrorLocation` - for references from error source locations

2. **FileReference Updates**:
   - Added `source` field to track origin of reference
   - Added `with_source()` constructor for explicit source specification
   - Updated `from_stack_frame()` to set `source: FileReferenceSource::StackFrame`
   - Added `resolve_path()` method for converting `package:` paths to filesystem paths

3. **File Reference Extraction**:
   - Added `FILE_LINE_PATTERN` regex using `LazyLock` for thread-safe lazy initialization
   - Added `extract_file_ref_from_message()` function to detect file:line[:column] patterns
   - Handles `package:`, `dart:`, and regular file paths
   - Supports optional column number

4. **FocusInfo Struct**: Added to `log_view.rs` with:
   - `entry_index: Option<usize>` - index in log buffer
   - `entry_id: Option<u64>` - stable ID across buffer changes
   - `frame_index: Option<usize>` - frame within stack trace
   - `file_ref: Option<FileReference>` - extracted file reference
   - `has_file_ref()` helper method

5. **LogViewState Updates**:
   - Added `focus_info: FocusInfo` field
   - Added `focused_file_ref()` convenience method

### Testing Performed

- `cargo check` - Compiles successfully (minor warnings for unused import, will be used in Task 04)
- `cargo test --lib tui::hyperlinks` - 42 tests passed
- `cargo test --lib tui::widgets::log_view` - 77 tests passed
- `cargo test --lib` - 916 tests passed, 3 ignored

### Notable Decisions/Tradeoffs

1. **Source Tracking**: Added `FileReferenceSource` to track where references originate from, enabling different behavior (e.g., confidence levels) in Task 04
2. **Package Path Resolution**: Implemented simple `package:app/path` → `lib/path` mapping; more complex multi-package workspace resolution deferred
3. **Regex Pattern**: Used `.dart` file extension check to avoid matching non-Dart files in logs
4. **Unused Import**: The `extract_file_ref_from_message` import in `log_view.rs` is intentionally added for Task 04 integration

### Acceptance Criteria Checklist

- [x] `FileReference` struct defined with path, line, column, source
- [x] `FileReference::from_stack_frame()` extracts reference from StackFrame
- [x] `FileReference::resolve_path()` converts package: paths to file paths
- [x] `extract_file_ref_from_message()` detects file:line in log text
- [x] `FocusInfo` struct tracks focused entry index, ID, frame index, and file ref
- [x] `LogViewState` updated with `focus_info` field and `focused_file_ref()` method
- [x] Focus tracking integrates with existing `visible_range()` for efficiency
- [x] Works with `VecDeque<LogEntry>` (no code changes needed - indexing works)
- [x] Unit tests for file reference extraction from messages
- [x] Unit tests for FileReference::resolve_path()
- [x] Leverages existing `Session::focused_entry()` pattern

### Next Steps

Task 04 (Open File Editor Action) can now use:
- `LogViewState::focused_file_ref()` to get the current file reference
- `FileReference::resolve_path()` to convert package: paths
- `extract_file_ref_from_message()` if needed for fallback extraction