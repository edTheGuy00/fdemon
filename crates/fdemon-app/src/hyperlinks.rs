//! Hyperlink detection and state management.
//!
//! Scans log output for file references (paths with line numbers)
//! and manages the link highlight mode state.

use regex::Regex;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::session::CollapseState;
use fdemon_core::{FilterState, LogEntry, StackFrame};

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
/// use fdemon_app::hyperlinks::extract_file_ref_from_message;
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
