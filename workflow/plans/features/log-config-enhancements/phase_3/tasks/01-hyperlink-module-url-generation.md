## Task: Hyperlink Module and URL Generation

**Objective**: Create the foundational hyperlink module with types for file references, URL generation utilities for cross-platform file:// URLs, and OSC 8 escape sequence generation functions.

**Depends on**: None

### Scope

- `src/tui/hyperlinks.rs`: **NEW** - Core hyperlink types and utilities
- `src/tui/mod.rs`: Export the new hyperlinks module

### Current State

No hyperlink infrastructure exists. Stack trace file references are rendered with styling but cannot be opened or clicked.

### Target State

A new `hyperlinks` module providing:
1. `HyperlinkMode` enum for configuration
2. `FileReference` struct to represent file:line:column references
3. URL generation for file:// protocol (cross-platform)
4. OSC 8 escape sequence generation utilities

### Implementation Details

#### 1. HyperlinkMode Enum

```rust
// src/tui/hyperlinks.rs

use serde::{Deserialize, Serialize};

/// Mode for terminal hyperlink support
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HyperlinkMode {
    /// Automatically detect terminal support
    #[default]
    Auto,
    /// Always enable hyperlinks
    Enabled,
    /// Disable hyperlinks
    Disabled,
}

impl HyperlinkMode {
    /// Check if hyperlinks should be rendered based on mode and detection
    pub fn is_enabled(&self, detected_support: bool) -> bool {
        match self {
            HyperlinkMode::Auto => detected_support,
            HyperlinkMode::Enabled => true,
            HyperlinkMode::Disabled => false,
        }
    }
}
```

#### 2. FileReference Struct

```rust
/// A reference to a file location (path, line, column)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReference {
    /// Absolute or relative file path
    pub path: String,
    /// Line number (1-based)
    pub line: u32,
    /// Column number (1-based, optional - defaults to 1)
    pub column: u32,
}

impl FileReference {
    /// Create a new file reference
    pub fn new(path: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            path: path.into(),
            line,
            column: if column == 0 { 1 } else { column },
        }
    }

    /// Create from a StackFrame
    pub fn from_stack_frame(frame: &crate::core::StackFrame) -> Option<Self> {
        if frame.is_async_gap {
            return None;
        }
        Some(Self::new(&frame.file_path, frame.line, frame.column))
    }

    /// Format as "path:line:column"
    pub fn display(&self) -> String {
        format!("{}:{}:{}", self.path, self.line, self.column)
    }
}
```

#### 3. File URL Generation

```rust
use std::path::Path;

/// Generate a file:// URL for the given file reference
/// 
/// The URL format varies by platform:
/// - Unix: file:///absolute/path/to/file.dart
/// - Windows: file:///C:/path/to/file.dart
/// 
/// Note: Most terminals and editors expect absolute paths in file:// URLs.
pub fn file_url(reference: &FileReference) -> String {
    let path = &reference.path;
    
    // Handle package: URIs (convert to placeholder - actual resolution needed)
    if path.starts_with("package:") {
        // For package: URIs, we return as-is since resolution requires project context
        // The editor integration will handle this
        return format!("file:///{}", path);
    }
    
    // Handle dart: URIs (SDK paths)
    if path.starts_with("dart:") {
        return format!("file:///{}", path);
    }
    
    // For absolute paths, ensure proper file:// format
    let absolute_path = if Path::new(path).is_absolute() {
        path.to_string()
    } else {
        // Relative paths should be resolved by the caller
        path.to_string()
    };
    
    // Unix-style path
    #[cfg(unix)]
    {
        format!("file://{}", absolute_path)
    }
    
    // Windows-style path (convert backslashes and add leading slash)
    #[cfg(windows)]
    {
        let normalized = absolute_path.replace('\\', "/");
        if normalized.starts_with('/') {
            format!("file://{}", normalized)
        } else {
            format!("file:///{}", normalized)
        }
    }
}

/// Generate a file URL with line and column anchors
/// Some terminals/editors support opening at specific line:column
pub fn file_url_with_position(reference: &FileReference) -> String {
    let base_url = file_url(reference);
    // Line and column as URL fragment (common convention)
    format!("{}#L{}C{}", base_url, reference.line, reference.column)
}
```

#### 4. OSC 8 Sequence Generation

```rust
/// OSC 8 escape sequence constants
pub mod osc8 {
    /// Start of OSC 8 hyperlink: ESC ] 8 ; params ; URI ST
    /// ST (String Terminator) is ESC \
    pub const START: &str = "\x1b]8;;";
    
    /// End of OSC 8 hyperlink (empty URI closes it)
    pub const END: &str = "\x1b]8;;\x1b\\";
    
    /// String Terminator
    pub const ST: &str = "\x1b\\";
}

/// Wrap text with OSC 8 hyperlink escape sequences
/// 
/// Format: ESC ] 8 ; ; URI ST text ESC ] 8 ; ; ST
/// 
/// # Example
/// ```
/// use flutter_demon::tui::hyperlinks::osc8_wrap;
/// 
/// let linked = osc8_wrap("Click here", "https://example.com");
/// assert!(linked.contains("https://example.com"));
/// assert!(linked.contains("Click here"));
/// ```
pub fn osc8_wrap(text: &str, url: &str) -> String {
    format!(
        "{}{}{}{}{}",
        osc8::START,
        url,
        osc8::ST,
        text,
        osc8::END
    )
}

/// Wrap text with a file:// hyperlink
pub fn osc8_wrap_file(text: &str, reference: &FileReference) -> String {
    let url = file_url_with_position(reference);
    osc8_wrap(text, &url)
}

/// Check if a string contains OSC 8 sequences
pub fn contains_osc8(text: &str) -> bool {
    text.contains("\x1b]8;")
}
```

### Update tui/mod.rs

```rust
// Add to src/tui/mod.rs

pub mod hyperlinks;

pub use hyperlinks::{FileReference, HyperlinkMode};
```

### Acceptance Criteria

1. [ ] `HyperlinkMode` enum with Auto, Enabled, Disabled variants
2. [ ] `HyperlinkMode::is_enabled()` correctly evaluates based on mode and detection
3. [ ] `FileReference` struct stores path, line, column
4. [ ] `FileReference::from_stack_frame()` extracts data from StackFrame
5. [ ] `FileReference::display()` formats as "path:line:column"
6. [ ] `file_url()` generates valid file:// URLs
7. [ ] `file_url()` handles Unix and Windows paths correctly
8. [ ] `file_url()` handles package: and dart: URIs gracefully
9. [ ] `osc8_wrap()` generates correct OSC 8 escape sequences
10. [ ] `osc8_wrap_file()` combines URL generation with OSC 8 wrapping
11. [ ] `contains_osc8()` detects OSC 8 sequences in text
12. [ ] Module exported from `tui/mod.rs`

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hyperlink_mode_auto_with_support() {
        assert!(HyperlinkMode::Auto.is_enabled(true));
    }

    #[test]
    fn test_hyperlink_mode_auto_without_support() {
        assert!(!HyperlinkMode::Auto.is_enabled(false));
    }

    #[test]
    fn test_hyperlink_mode_enabled_ignores_detection() {
        assert!(HyperlinkMode::Enabled.is_enabled(false));
    }

    #[test]
    fn test_hyperlink_mode_disabled_ignores_detection() {
        assert!(!HyperlinkMode::Disabled.is_enabled(true));
    }

    #[test]
    fn test_file_reference_display() {
        let reference = FileReference::new("/path/to/file.dart", 42, 10);
        assert_eq!(reference.display(), "/path/to/file.dart:42:10");
    }

    #[test]
    fn test_file_reference_zero_column() {
        let reference = FileReference::new("file.dart", 10, 0);
        assert_eq!(reference.column, 1);
    }

    #[test]
    fn test_file_url_absolute_unix() {
        let reference = FileReference::new("/home/user/project/lib/main.dart", 15, 3);
        let url = file_url(&reference);
        assert!(url.starts_with("file://"));
        assert!(url.contains("/home/user/project/lib/main.dart"));
    }

    #[test]
    fn test_file_url_package_uri() {
        let reference = FileReference::new("package:my_app/main.dart", 10, 1);
        let url = file_url(&reference);
        assert!(url.contains("package:my_app/main.dart"));
    }

    #[test]
    fn test_file_url_with_position() {
        let reference = FileReference::new("/path/to/file.dart", 42, 10);
        let url = file_url_with_position(&reference);
        assert!(url.contains("#L42C10"));
    }

    #[test]
    fn test_osc8_wrap() {
        let wrapped = osc8_wrap("link text", "https://example.com");
        assert!(wrapped.starts_with("\x1b]8;;"));
        assert!(wrapped.contains("https://example.com"));
        assert!(wrapped.contains("link text"));
        assert!(wrapped.ends_with("\x1b]8;;\x1b\\"));
    }

    #[test]
    fn test_osc8_wrap_file() {
        let reference = FileReference::new("/path/file.dart", 10, 5);
        let wrapped = osc8_wrap_file("file.dart:10", &reference);
        assert!(wrapped.contains("file://"));
        assert!(wrapped.contains("file.dart:10"));
    }

    #[test]
    fn test_contains_osc8() {
        assert!(contains_osc8("\x1b]8;;https://example.com\x1b\\text\x1b]8;;\x1b\\"));
        assert!(!contains_osc8("plain text"));
    }

    #[test]
    fn test_file_reference_from_stack_frame() {
        use crate::core::StackFrame;
        
        let frame = StackFrame::new(0, "main", "package:app/main.dart", 15, 3);
        let reference = FileReference::from_stack_frame(&frame).unwrap();
        
        assert_eq!(reference.path, "package:app/main.dart");
        assert_eq!(reference.line, 15);
        assert_eq!(reference.column, 3);
    }

    #[test]
    fn test_file_reference_from_async_gap_returns_none() {
        use crate::core::StackFrame;
        
        let frame = StackFrame::async_gap(0);
        assert!(FileReference::from_stack_frame(&frame).is_none());
    }
}
```

### Run Tests

```bash
# Run hyperlink module tests
cargo test tui::hyperlinks

# Verify module compiles
cargo check
```

### Notes

- The `file_url()` function generates URLs but doesn't resolve package: URIs - that requires project context and will be handled in Task 04 (editor execution)
- OSC 8 sequences are generated but not yet emitted to the terminal - that's Task 06
- This module provides the building blocks; integration happens in later tasks
- Cross-platform support uses conditional compilation (`#[cfg(unix)]`, `#[cfg(windows)]`)

### Estimated Time

2-3 hours

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/hyperlinks.rs` | **NEW** - Complete module implementation |
| `src/tui/mod.rs` | Add module export |