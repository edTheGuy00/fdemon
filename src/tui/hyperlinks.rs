//! File reference extraction for Link Highlight Mode.
//!
//! This module provides types and utilities for detecting file references
//! in log messages and stack traces:
//! - `FileReference` for representing file:line:column references
//! - `FileReferenceSource` for tracking where references come from
//! - `extract_file_ref_from_message()` for scanning log text
//!
//! Note: OSC 8 terminal hyperlink code was removed in Phase 3.1 in favor of
//! the explicit Link Highlight Mode (L key).

use regex::Regex;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::app::session::CollapseState;
use crate::core::{FilterState, LogEntry, StackFrame};

// ─────────────────────────────────────────────────────────────────────────────
// FileReferenceSource
// ─────────────────────────────────────────────────────────────────────────────

/// Source of a file reference (for display/debugging).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileReferenceSource {
    /// From a parsed stack frame
    StackFrame,
    /// Detected in log message text
    LogMessage,
    /// From error source location
    ErrorLocation,
}

// ─────────────────────────────────────────────────────────────────────────────
// FileReference
// ─────────────────────────────────────────────────────────────────────────────

/// A reference to a file location (path, line, column).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReference {
    /// Absolute or relative file path (may be package:, dart:, or file path)
    pub path: String,
    /// Line number (1-based)
    pub line: u32,
    /// Column number (1-based, defaults to 1 if 0 provided)
    pub column: u32,
    /// Source of this reference (for display/debugging)
    pub source: FileReferenceSource,
}

impl FileReference {
    /// Create a new file reference.
    ///
    /// If column is 0, it defaults to 1.
    pub fn new(path: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            path: path.into(),
            line,
            column: if column == 0 { 1 } else { column },
            source: FileReferenceSource::LogMessage, // Default source
        }
    }

    /// Create a new file reference with explicit source.
    pub fn with_source(
        path: impl Into<String>,
        line: u32,
        column: u32,
        source: FileReferenceSource,
    ) -> Self {
        Self {
            path: path.into(),
            line,
            column: if column == 0 { 1 } else { column },
            source,
        }
    }

    /// Create from a `StackFrame`.
    ///
    /// Returns `None` if the frame is an async gap (which has no file location).
    pub fn from_stack_frame(frame: &StackFrame) -> Option<Self> {
        if frame.is_async_gap {
            return None;
        }
        Some(Self::with_source(
            &frame.file_path,
            frame.line,
            frame.column,
            FileReferenceSource::StackFrame,
        ))
    }

    /// Format as "path:line:column".
    pub fn display(&self) -> String {
        format!("{}:{}:{}", self.path, self.line, self.column)
    }

    /// Convert package: path to absolute path if possible.
    ///
    /// For `package:app/src/file.dart` paths, this resolves to `lib/src/file.dart`
    /// relative to the project root. For absolute paths, returns as-is.
    /// For `dart:` SDK paths, returns as-is (cannot be opened locally).
    pub fn resolve_path(&self, project_root: &Path) -> PathBuf {
        if self.path.starts_with("package:") {
            // Extract package path: package:app/src/main.dart -> lib/src/main.dart
            // The first segment after package: is the package name, rest is the path within lib/
            let package_path = self.path.strip_prefix("package:").unwrap_or(&self.path);
            if let Some((_package_name, rest)) = package_path.split_once('/') {
                return project_root.join("lib").join(rest);
            }
        }

        // dart: URIs point to SDK, return as-is
        if self.path.starts_with("dart:") {
            return PathBuf::from(&self.path);
        }

        // Already absolute
        let path = Path::new(&self.path);
        if path.is_absolute() {
            return path.to_path_buf();
        }

        // Relative path - resolve against project root
        project_root.join(&self.path)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DetectedLink
// ─────────────────────────────────────────────────────────────────────────────

/// A detected file reference link in the visible viewport.
///
/// Each detected link is assigned a shortcut key (1-9, a-z) that the user
/// can press to open that file in their editor.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectedLink {
    /// The file reference (path, line, column)
    pub file_ref: FileReference,

    /// Index of the log entry this link belongs to
    pub entry_index: usize,

    /// Stack frame index within the entry (None if from log message)
    pub frame_index: Option<usize>,

    /// Shortcut key to select this link ('1'-'9', 'a'-'z')
    pub shortcut: char,

    /// Display text shown to user (e.g., "lib/main.dart:42:5")
    pub display_text: String,

    /// Relative line number within the viewport (0-based)
    pub viewport_line: usize,
}

impl DetectedLink {
    /// Create a new detected link.
    ///
    /// The display text is automatically derived from the file reference.
    pub fn new(
        file_ref: FileReference,
        entry_index: usize,
        frame_index: Option<usize>,
        shortcut: char,
        viewport_line: usize,
    ) -> Self {
        let display_text = file_ref.display();
        Self {
            file_ref,
            entry_index,
            frame_index,
            shortcut,
            display_text,
            viewport_line,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LinkHighlightState
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum number of links we can assign shortcuts to (9 + 26 = 35).
///
/// - Links 0-8: assigned shortcuts '1'-'9'
/// - Links 9-34: assigned shortcuts 'a'-'z'
pub const MAX_LINK_SHORTCUTS: usize = 35;

/// State for link highlight mode.
///
/// When the user presses 'L' to enter link mode, this state is populated
/// by scanning the visible viewport for file references. Links are assigned
/// shortcut keys (1-9, then a-z) that the user can press to open files.
#[derive(Debug, Default, Clone)]
pub struct LinkHighlightState {
    /// Detected links in the current viewport (max 35: 1-9 + a-z)
    pub links: Vec<DetectedLink>,

    /// Whether link highlight mode is currently active
    pub active: bool,
}

impl LinkHighlightState {
    /// Create a new inactive link highlight state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if link mode is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Activate link mode (links should be populated first via scan).
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// Deactivate link mode and clear detected links.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.links.clear();
    }

    /// Get a link by its shortcut key.
    pub fn link_by_shortcut(&self, c: char) -> Option<&DetectedLink> {
        self.links.iter().find(|link| link.shortcut == c)
    }

    /// Get the number of detected links.
    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    /// Check if there are any detected links.
    pub fn has_links(&self) -> bool {
        !self.links.is_empty()
    }

    /// Clear links without deactivating (for re-scan on scroll).
    pub fn clear_links(&mut self) {
        self.links.clear();
    }

    /// Add a detected link (respects MAX_LINK_SHORTCUTS limit).
    pub fn add_link(&mut self, link: DetectedLink) {
        if self.links.len() < MAX_LINK_SHORTCUTS {
            self.links.push(link);
        }
    }

    /// Get the shortcut character for a given index (0-34).
    ///
    /// Returns '1'-'9' for indices 0-8, 'a'-'z' for indices 9-34.
    pub fn shortcut_for_index(index: usize) -> Option<char> {
        match index {
            0..=8 => Some((b'1' + index as u8) as char),
            9..=34 => Some((b'a' + (index - 9) as u8) as char),
            _ => None,
        }
    }

    /// Get all links on a specific viewport line.
    pub fn links_on_line(&self, line: usize) -> Vec<&DetectedLink> {
        self.links
            .iter()
            .filter(|link| link.viewport_line == line)
            .collect()
    }

    /// Scan visible log entries for file references and populate links.
    ///
    /// This method is called when entering link highlight mode. It scans
    /// the entries in the visible viewport range, extracts file references
    /// from both log messages and stack frames, and assigns shortcut keys.
    ///
    /// # Arguments
    ///
    /// * `logs` - The log entries
    /// * `visible_start` - Start index of visible range (in filtered indices)
    /// * `visible_end` - End index of visible range (exclusive, in filtered indices)
    /// * `filter_state` - Optional filter to apply
    /// * `collapse_state` - Stack trace collapse state for determining visible frames
    /// * `default_collapsed` - Whether stack traces are collapsed by default
    /// * `max_collapsed_frames` - Maximum frames to show when collapsed
    #[allow(clippy::too_many_arguments)]
    pub fn scan_viewport(
        &mut self,
        logs: &VecDeque<LogEntry>,
        visible_start: usize,
        visible_end: usize,
        filter_state: Option<&FilterState>,
        collapse_state: &CollapseState,
        default_collapsed: bool,
        max_collapsed_frames: usize,
    ) {
        self.clear_links();

        let mut link_index: usize = 0;
        let mut viewport_line: usize = 0;

        // Build filtered indices if filter is active
        let filtered_indices: Vec<usize> = if let Some(filter) = filter_state {
            if filter.is_active() {
                logs.iter()
                    .enumerate()
                    .filter(|(_, entry)| filter.matches(entry))
                    .map(|(i, _)| i)
                    .collect()
            } else {
                (0..logs.len()).collect()
            }
        } else {
            (0..logs.len()).collect()
        };

        // Iterate visible entries
        for &idx in filtered_indices
            .iter()
            .skip(visible_start)
            .take(visible_end.saturating_sub(visible_start))
        {
            if link_index >= MAX_LINK_SHORTCUTS {
                break;
            }

            let entry = &logs[idx];

            // Check log message for file reference
            if let Some(file_ref) = extract_file_ref_from_message(&entry.message) {
                if let Some(shortcut) = Self::shortcut_for_index(link_index) {
                    self.add_link(DetectedLink::new(
                        file_ref,
                        idx,
                        None, // No frame index - from message
                        shortcut,
                        viewport_line,
                    ));
                    link_index += 1;
                }
            }

            viewport_line += 1; // Count the message line

            // Check stack trace frames if present
            if let Some(ref trace) = entry.stack_trace {
                let is_expanded = collapse_state.is_expanded(entry.id, default_collapsed);

                let frames_to_scan = if is_expanded {
                    trace.frames.len()
                } else {
                    max_collapsed_frames.min(trace.frames.len())
                };

                for (frame_idx, frame) in trace.frames.iter().take(frames_to_scan).enumerate() {
                    if link_index >= MAX_LINK_SHORTCUTS {
                        break;
                    }

                    if let Some(file_ref) = FileReference::from_stack_frame(frame) {
                        if let Some(shortcut) = Self::shortcut_for_index(link_index) {
                            self.add_link(DetectedLink::new(
                                file_ref,
                                idx,
                                Some(frame_idx),
                                shortcut,
                                viewport_line,
                            ));
                            link_index += 1;
                        }
                    }

                    viewport_line += 1; // Count each frame line
                }

                // Account for collapsed indicator line if applicable
                if !is_expanded && trace.frames.len() > max_collapsed_frames {
                    viewport_line += 1;
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// File Reference Extraction
// ─────────────────────────────────────────────────────────────────────────────

/// Regex to detect file:line[:column] patterns in log messages.
///
/// Matches patterns like:
/// - `lib/main.dart:15:3`
/// - `package:app/main.dart:15`
/// - `/absolute/path/file.dart:100:5`
/// - `file.dart:42`
/// - `test/widget_test.dart:10`
static FILE_LINE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // Match: optional prefix (package:, dart:, or path segments) + .dart file + :line + optional :column
    Regex::new(r"(?:package:|dart:)?(?:[\w./\-]+/)?[\w\-]+\.dart:(\d+)(?::(\d+))?")
        .expect("File:line regex is valid")
});

/// Extract a file reference from a log message.
///
/// Searches the message for file:line[:column] patterns commonly found in
/// Dart/Flutter logs and stack traces.
///
/// # Examples
///
/// ```
/// use flutter_demon::tui::hyperlinks::extract_file_ref_from_message;
///
/// let msg = "Error at lib/main.dart:15:3";
/// let file_ref = extract_file_ref_from_message(msg);
/// assert!(file_ref.is_some());
/// ```
pub fn extract_file_ref_from_message(message: &str) -> Option<FileReference> {
    let caps = FILE_LINE_PATTERN.captures(message)?;

    // Get the full match to extract the file path
    let full_match = caps.get(0)?.as_str();

    // Find where the line number starts (first colon followed by digits)
    // We need to handle cases like "package:app/main.dart:15:3"
    let (path, _line_col) = split_path_and_location(full_match)?;

    // Parse line number (group 1)
    let line: u32 = caps.get(1)?.as_str().parse().ok()?;

    // Parse column number (group 2, optional)
    let column: u32 = caps
        .get(2)
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(0);

    Some(FileReference::with_source(
        path,
        line,
        column,
        FileReferenceSource::LogMessage,
    ))
}

/// Split a file:line:column string into (path, "line:column" or "line").
///
/// Handles the tricky case of `package:app/file.dart:15:3` where we need
/// to keep `package:app/file.dart` together.
fn split_path_and_location(s: &str) -> Option<(&str, &str)> {
    // Find .dart: which separates the file from line number
    let dart_pos = s.find(".dart:")?;
    let path_end = dart_pos + 5; // length of ".dart"

    if path_end >= s.len() {
        return None;
    }

    let path = &s[..path_end];
    let line_col = &s[path_end + 1..]; // skip the colon

    Some((path, line_col))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // FileReference tests

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
    fn test_file_reference_preserves_nonzero_column() {
        let reference = FileReference::new("file.dart", 10, 5);
        assert_eq!(reference.column, 5);
    }

    #[test]
    fn test_file_reference_from_stack_frame() {
        let frame = StackFrame::new(0, "main", "package:app/main.dart", 15, 3);
        let reference = FileReference::from_stack_frame(&frame).unwrap();

        assert_eq!(reference.path, "package:app/main.dart");
        assert_eq!(reference.line, 15);
        assert_eq!(reference.column, 3);
    }

    #[test]
    fn test_file_reference_from_async_gap_returns_none() {
        let frame = StackFrame::async_gap(0);
        assert!(FileReference::from_stack_frame(&frame).is_none());
    }

    #[test]
    fn test_file_reference_from_stack_frame_zero_column() {
        // StackFrame with column 0 should get normalized to 1
        let frame = StackFrame::new(0, "main", "file.dart", 10, 0);
        let reference = FileReference::from_stack_frame(&frame).unwrap();
        assert_eq!(reference.column, 1);
    }

    // FileReferenceSource tests

    #[test]
    fn test_file_reference_source_from_stack_frame() {
        let frame = StackFrame::new(0, "main", "package:app/main.dart", 15, 3);
        let reference = FileReference::from_stack_frame(&frame).unwrap();
        assert_eq!(reference.source, FileReferenceSource::StackFrame);
    }

    #[test]
    fn test_file_reference_with_source() {
        let reference =
            FileReference::with_source("lib/main.dart", 10, 5, FileReferenceSource::ErrorLocation);
        assert_eq!(reference.source, FileReferenceSource::ErrorLocation);
        assert_eq!(reference.line, 10);
        assert_eq!(reference.column, 5);
    }

    #[test]
    fn test_file_reference_new_default_source() {
        let reference = FileReference::new("lib/main.dart", 10, 5);
        assert_eq!(reference.source, FileReferenceSource::LogMessage);
    }

    // FileReference::resolve_path tests

    #[test]
    fn test_resolve_path_package() {
        let reference = FileReference::new("package:my_app/src/utils.dart", 10, 5);
        let project_root = Path::new("/home/user/my_app");
        let resolved = reference.resolve_path(project_root);
        assert_eq!(
            resolved,
            PathBuf::from("/home/user/my_app/lib/src/utils.dart")
        );
    }

    #[test]
    fn test_resolve_path_package_simple() {
        let reference = FileReference::new("package:app/main.dart", 10, 5);
        let project_root = Path::new("/project");
        let resolved = reference.resolve_path(project_root);
        assert_eq!(resolved, PathBuf::from("/project/lib/main.dart"));
    }

    #[test]
    fn test_resolve_path_absolute() {
        let reference = FileReference::new("/absolute/path/file.dart", 10, 5);
        let project_root = Path::new("/home/user/my_app");
        let resolved = reference.resolve_path(project_root);
        assert_eq!(resolved, PathBuf::from("/absolute/path/file.dart"));
    }

    #[test]
    fn test_resolve_path_relative() {
        let reference = FileReference::new("lib/main.dart", 10, 5);
        let project_root = Path::new("/home/user/my_app");
        let resolved = reference.resolve_path(project_root);
        assert_eq!(resolved, PathBuf::from("/home/user/my_app/lib/main.dart"));
    }

    #[test]
    fn test_resolve_path_dart_sdk() {
        let reference = FileReference::new("dart:core/object.dart", 100, 1);
        let project_root = Path::new("/project");
        let resolved = reference.resolve_path(project_root);
        // dart: URIs are returned as-is
        assert_eq!(resolved, PathBuf::from("dart:core/object.dart"));
    }

    // extract_file_ref_from_message tests

    #[test]
    fn test_extract_file_ref_basic() {
        let msg = "Error at lib/main.dart:15:3";
        let file_ref = extract_file_ref_from_message(msg);
        assert!(file_ref.is_some());
        let file_ref = file_ref.unwrap();
        assert_eq!(file_ref.path, "lib/main.dart");
        assert_eq!(file_ref.line, 15);
        assert_eq!(file_ref.column, 3);
        assert_eq!(file_ref.source, FileReferenceSource::LogMessage);
    }

    #[test]
    fn test_extract_file_ref_package() {
        let msg = "Error in package:my_app/utils.dart:42";
        let file_ref = extract_file_ref_from_message(msg);
        assert!(file_ref.is_some());
        let file_ref = file_ref.unwrap();
        assert_eq!(file_ref.path, "package:my_app/utils.dart");
        assert_eq!(file_ref.line, 42);
        assert_eq!(file_ref.column, 1); // Defaults to 1 when 0
    }

    #[test]
    fn test_extract_file_ref_package_with_column() {
        let msg = "package:app/main.dart:15:3";
        let file_ref = extract_file_ref_from_message(msg);
        assert!(file_ref.is_some());
        let file_ref = file_ref.unwrap();
        assert_eq!(file_ref.path, "package:app/main.dart");
        assert_eq!(file_ref.line, 15);
        assert_eq!(file_ref.column, 3);
    }

    #[test]
    fn test_extract_file_ref_simple_file() {
        let msg = "file.dart:42";
        let file_ref = extract_file_ref_from_message(msg);
        assert!(file_ref.is_some());
        let file_ref = file_ref.unwrap();
        assert_eq!(file_ref.path, "file.dart");
        assert_eq!(file_ref.line, 42);
    }

    #[test]
    fn test_extract_file_ref_test_path() {
        let msg = "test/widget_test.dart:10:5";
        let file_ref = extract_file_ref_from_message(msg);
        assert!(file_ref.is_some());
        let file_ref = file_ref.unwrap();
        assert_eq!(file_ref.path, "test/widget_test.dart");
        assert_eq!(file_ref.line, 10);
        assert_eq!(file_ref.column, 5);
    }

    #[test]
    fn test_extract_file_ref_no_match() {
        let msg = "Just a regular log message without file reference";
        assert!(extract_file_ref_from_message(msg).is_none());
    }

    #[test]
    fn test_extract_file_ref_no_dart_file() {
        let msg = "Error in config.yaml:10";
        // Should not match non-.dart files
        assert!(extract_file_ref_from_message(msg).is_none());
    }

    #[test]
    fn test_extract_file_ref_nested_path() {
        let msg = "Error at lib/src/widgets/my_widget.dart:100:20";
        let file_ref = extract_file_ref_from_message(msg);
        assert!(file_ref.is_some());
        let file_ref = file_ref.unwrap();
        assert_eq!(file_ref.path, "lib/src/widgets/my_widget.dart");
        assert_eq!(file_ref.line, 100);
        assert_eq!(file_ref.column, 20);
    }

    #[test]
    fn test_extract_file_ref_dart_sdk() {
        let msg = "dart:core/list.dart:50:3";
        let file_ref = extract_file_ref_from_message(msg);
        assert!(file_ref.is_some());
        let file_ref = file_ref.unwrap();
        assert_eq!(file_ref.path, "dart:core/list.dart");
        assert_eq!(file_ref.line, 50);
    }

    // DetectedLink tests

    #[test]
    fn test_detected_link_new() {
        let file_ref = FileReference::new("lib/main.dart", 42, 5);
        let link = DetectedLink::new(file_ref, 0, None, '1', 3);

        assert_eq!(link.entry_index, 0);
        assert_eq!(link.frame_index, None);
        assert_eq!(link.shortcut, '1');
        assert_eq!(link.viewport_line, 3);
        assert_eq!(link.display_text, "lib/main.dart:42:5");
    }

    #[test]
    fn test_detected_link_with_frame_index() {
        let file_ref = FileReference::new("lib/widget.dart", 10, 3);
        let link = DetectedLink::new(file_ref, 5, Some(2), 'a', 10);

        assert_eq!(link.entry_index, 5);
        assert_eq!(link.frame_index, Some(2));
        assert_eq!(link.shortcut, 'a');
        assert_eq!(link.viewport_line, 10);
    }

    // LinkHighlightState tests

    #[test]
    fn test_link_highlight_state_default_inactive() {
        let state = LinkHighlightState::new();
        assert!(!state.is_active());
        assert!(!state.has_links());
        assert_eq!(state.link_count(), 0);
    }

    #[test]
    fn test_link_highlight_state_activate_deactivate() {
        let mut state = LinkHighlightState::new();

        state.activate();
        assert!(state.is_active());

        state.deactivate();
        assert!(!state.is_active());
    }

    #[test]
    fn test_link_by_shortcut() {
        let mut state = LinkHighlightState::new();
        let link = DetectedLink::new(FileReference::new("test.dart", 10, 0), 0, None, 'a', 0);
        state.add_link(link);

        assert!(state.link_by_shortcut('a').is_some());
        assert!(state.link_by_shortcut('b').is_none());
    }

    #[test]
    fn test_link_by_shortcut_returns_correct_link() {
        let mut state = LinkHighlightState::new();
        state.add_link(DetectedLink::new(
            FileReference::new("first.dart", 1, 0),
            0,
            None,
            '1',
            0,
        ));
        state.add_link(DetectedLink::new(
            FileReference::new("second.dart", 2, 0),
            1,
            None,
            '2',
            1,
        ));

        let link = state.link_by_shortcut('2').unwrap();
        assert_eq!(link.file_ref.path, "second.dart");
        assert_eq!(link.file_ref.line, 2);
    }

    #[test]
    fn test_shortcut_for_index_digits() {
        assert_eq!(LinkHighlightState::shortcut_for_index(0), Some('1'));
        assert_eq!(LinkHighlightState::shortcut_for_index(1), Some('2'));
        assert_eq!(LinkHighlightState::shortcut_for_index(8), Some('9'));
    }

    #[test]
    fn test_shortcut_for_index_letters() {
        assert_eq!(LinkHighlightState::shortcut_for_index(9), Some('a'));
        assert_eq!(LinkHighlightState::shortcut_for_index(10), Some('b'));
        assert_eq!(LinkHighlightState::shortcut_for_index(34), Some('z'));
    }

    #[test]
    fn test_shortcut_for_index_out_of_range() {
        assert_eq!(LinkHighlightState::shortcut_for_index(35), None);
        assert_eq!(LinkHighlightState::shortcut_for_index(100), None);
    }

    #[test]
    fn test_max_link_limit() {
        let mut state = LinkHighlightState::new();

        for i in 0..40 {
            let shortcut = LinkHighlightState::shortcut_for_index(i).unwrap_or('?');
            let link = DetectedLink::new(
                FileReference::new("test.dart", i as u32, 0),
                i,
                None,
                shortcut,
                i,
            );
            state.add_link(link);
        }

        // Should cap at MAX_LINK_SHORTCUTS
        assert_eq!(state.link_count(), MAX_LINK_SHORTCUTS);
    }

    #[test]
    fn test_links_on_line() {
        let mut state = LinkHighlightState::new();
        state.add_link(DetectedLink::new(
            FileReference::new("a.dart", 1, 0),
            0,
            None,
            '1',
            0,
        ));
        state.add_link(DetectedLink::new(
            FileReference::new("b.dart", 2, 0),
            1,
            None,
            '2',
            0,
        ));
        state.add_link(DetectedLink::new(
            FileReference::new("c.dart", 3, 0),
            2,
            None,
            '3',
            1,
        ));

        let line_0_links = state.links_on_line(0);
        assert_eq!(line_0_links.len(), 2);

        let line_1_links = state.links_on_line(1);
        assert_eq!(line_1_links.len(), 1);

        let line_2_links = state.links_on_line(2);
        assert_eq!(line_2_links.len(), 0);
    }

    #[test]
    fn test_deactivate_clears_links() {
        let mut state = LinkHighlightState::new();
        state.add_link(DetectedLink::new(
            FileReference::new("test.dart", 1, 0),
            0,
            None,
            '1',
            0,
        ));
        state.activate();

        assert!(state.has_links());

        state.deactivate();
        assert!(!state.has_links());
        assert!(!state.is_active());
    }

    #[test]
    fn test_clear_links_preserves_active_state() {
        let mut state = LinkHighlightState::new();
        state.add_link(DetectedLink::new(
            FileReference::new("test.dart", 1, 0),
            0,
            None,
            '1',
            0,
        ));
        state.activate();

        state.clear_links();

        assert!(!state.has_links());
        assert!(state.is_active()); // Still active after clear_links
    }

    #[test]
    fn test_has_links() {
        let mut state = LinkHighlightState::new();
        assert!(!state.has_links());

        state.add_link(DetectedLink::new(
            FileReference::new("test.dart", 1, 0),
            0,
            None,
            '1',
            0,
        ));
        assert!(state.has_links());

        state.clear_links();
        assert!(!state.has_links());
    }

    #[test]
    fn test_link_count() {
        let mut state = LinkHighlightState::new();
        assert_eq!(state.link_count(), 0);

        state.add_link(DetectedLink::new(
            FileReference::new("a.dart", 1, 0),
            0,
            None,
            '1',
            0,
        ));
        assert_eq!(state.link_count(), 1);

        state.add_link(DetectedLink::new(
            FileReference::new("b.dart", 2, 0),
            1,
            None,
            '2',
            1,
        ));
        assert_eq!(state.link_count(), 2);
    }

    // scan_viewport tests

    fn make_entry(id: u64, message: &str) -> LogEntry {
        use crate::core::{LogLevel, LogSource};
        LogEntry {
            id,
            timestamp: chrono::Local::now(),
            level: LogLevel::Info,
            source: LogSource::App,
            message: message.to_string(),
            stack_trace: None,
        }
    }

    fn make_entry_with_trace(id: u64, message: &str, frames: Vec<&str>) -> LogEntry {
        use crate::core::{ParsedStackTrace, StackTraceFormat};
        let mut entry = make_entry(id, message);
        entry.stack_trace = Some(ParsedStackTrace {
            raw: String::new(),
            frames: frames
                .iter()
                .enumerate()
                .map(|(i, f)| StackFrame::new(i, "test", *f, 1, 0))
                .collect(),
            is_complete: true,
            format: StackTraceFormat::DartVm,
        });
        entry
    }

    #[test]
    fn test_scan_finds_message_links() {
        let mut logs = VecDeque::new();
        logs.push_back(make_entry(1, "Error at lib/main.dart:42"));
        logs.push_back(make_entry(2, "No link here"));
        logs.push_back(make_entry(3, "Another at lib/test.dart:10:5"));

        let mut state = LinkHighlightState::new();
        let collapse = CollapseState::default();
        state.scan_viewport(&logs, 0, 3, None, &collapse, true, 3);

        assert_eq!(state.link_count(), 2);
        assert_eq!(state.links[0].shortcut, '1');
        assert_eq!(state.links[1].shortcut, '2');
    }

    #[test]
    fn test_scan_finds_stack_frame_links() {
        let mut logs = VecDeque::new();
        logs.push_back(make_entry_with_trace(
            1,
            "Exception occurred",
            vec!["lib/widget.dart", "lib/app.dart"],
        ));

        let mut state = LinkHighlightState::new();
        let collapse = CollapseState::default();
        state.scan_viewport(&logs, 0, 1, None, &collapse, false, 10);

        // Should find 2 frame links (message has no dart file ref)
        assert_eq!(state.link_count(), 2);
        assert!(state.links[0].frame_index.is_some());
        assert!(state.links[1].frame_index.is_some());
    }

    #[test]
    fn test_scan_respects_collapsed_frames() {
        let mut logs = VecDeque::new();
        logs.push_back(make_entry_with_trace(
            1,
            "Error",
            vec![
                "lib/a.dart",
                "lib/b.dart",
                "lib/c.dart",
                "lib/d.dart",
                "lib/e.dart",
            ],
        ));

        let mut state = LinkHighlightState::new();
        let collapse = CollapseState::default();
        // max_collapsed_frames = 2, default_collapsed = true
        state.scan_viewport(&logs, 0, 1, None, &collapse, true, 2);

        // Should only find 2 frames (collapsed)
        assert_eq!(state.link_count(), 2);
    }

    #[test]
    fn test_scan_stops_at_max_shortcuts() {
        let mut logs = VecDeque::new();
        for i in 0..50 {
            logs.push_back(make_entry(i, &format!("Error at lib/file{}.dart:{}", i, i)));
        }

        let mut state = LinkHighlightState::new();
        let collapse = CollapseState::default();
        state.scan_viewport(&logs, 0, 50, None, &collapse, true, 3);

        assert_eq!(state.link_count(), MAX_LINK_SHORTCUTS);
    }

    #[test]
    fn test_scan_empty_logs() {
        let logs: VecDeque<LogEntry> = VecDeque::new();

        let mut state = LinkHighlightState::new();
        let collapse = CollapseState::default();
        state.scan_viewport(&logs, 0, 0, None, &collapse, true, 3);

        assert!(!state.has_links());
    }

    #[test]
    fn test_scan_respects_visible_range() {
        let mut logs = VecDeque::new();
        logs.push_back(make_entry(0, "lib/file0.dart:1")); // index 0
        logs.push_back(make_entry(1, "lib/file1.dart:2")); // index 1
        logs.push_back(make_entry(2, "lib/file2.dart:3")); // index 2
        logs.push_back(make_entry(3, "lib/file3.dart:4")); // index 3
        logs.push_back(make_entry(4, "lib/file4.dart:5")); // index 4

        let mut state = LinkHighlightState::new();
        let collapse = CollapseState::default();
        // Only scan indices 1-3 (visible_start=1, visible_end=4)
        state.scan_viewport(&logs, 1, 4, None, &collapse, true, 3);

        assert_eq!(state.link_count(), 3);
        // Should be files 1, 2, 3
        assert!(state.links[0].file_ref.path.contains("file1"));
        assert!(state.links[1].file_ref.path.contains("file2"));
        assert!(state.links[2].file_ref.path.contains("file3"));
    }

    #[test]
    fn test_scan_with_filter() {
        use crate::core::{LogLevel, LogLevelFilter};

        let mut logs = VecDeque::new();
        // Mix of info and error entries
        let mut entry0 = make_entry(0, "lib/info.dart:1");
        entry0.level = LogLevel::Info;
        logs.push_back(entry0);

        let mut entry1 = make_entry(1, "lib/error.dart:2");
        entry1.level = LogLevel::Error;
        logs.push_back(entry1);

        let mut entry2 = make_entry(2, "lib/info2.dart:3");
        entry2.level = LogLevel::Info;
        logs.push_back(entry2);

        let mut filter = FilterState::default();
        filter.level_filter = LogLevelFilter::Errors;

        let mut state = LinkHighlightState::new();
        let collapse = CollapseState::default();
        state.scan_viewport(&logs, 0, 3, Some(&filter), &collapse, true, 3);

        // Only the error entry should be scanned
        assert_eq!(state.link_count(), 1);
        assert!(state.links[0].file_ref.path.contains("error"));
    }

    #[test]
    fn test_scan_mixed_message_and_frames() {
        let mut logs = VecDeque::new();
        // Entry with file ref in message AND stack trace
        logs.push_back(make_entry_with_trace(
            1,
            "Error at lib/main.dart:42",
            vec!["lib/widget.dart", "lib/app.dart"],
        ));

        let mut state = LinkHighlightState::new();
        let collapse = CollapseState::default();
        state.scan_viewport(&logs, 0, 1, None, &collapse, false, 10);

        // Should find 3 links: 1 from message + 2 from frames
        assert_eq!(state.link_count(), 3);
        assert!(state.links[0].frame_index.is_none()); // From message
        assert!(state.links[1].frame_index.is_some()); // From frame
        assert!(state.links[2].frame_index.is_some()); // From frame
    }

    #[test]
    fn test_scan_shortcuts_in_order() {
        let mut logs = VecDeque::new();
        for i in 0..15 {
            logs.push_back(make_entry(i, &format!("Error at lib/file{}.dart:{}", i, i)));
        }

        let mut state = LinkHighlightState::new();
        let collapse = CollapseState::default();
        state.scan_viewport(&logs, 0, 15, None, &collapse, true, 3);

        // First 9 should be '1'-'9'
        for i in 0..9 {
            assert_eq!(state.links[i].shortcut, (b'1' + i as u8) as char);
        }
        // Next 6 should be 'a'-'f'
        for i in 9..15 {
            assert_eq!(state.links[i].shortcut, (b'a' + (i - 9) as u8) as char);
        }
    }

    #[test]
    fn test_scan_clears_previous_links() {
        let mut logs = VecDeque::new();
        logs.push_back(make_entry(0, "lib/first.dart:1"));

        let mut state = LinkHighlightState::new();
        let collapse = CollapseState::default();

        // First scan
        state.scan_viewport(&logs, 0, 1, None, &collapse, true, 3);
        assert_eq!(state.link_count(), 1);

        // Add more logs and scan again
        logs.push_back(make_entry(1, "lib/second.dart:2"));
        state.scan_viewport(&logs, 0, 2, None, &collapse, true, 3);

        // Should have 2 links now (previous cleared)
        assert_eq!(state.link_count(), 2);
    }
}
