//! # Frame and Variable Reference Stores
//!
//! Provides [`FrameStore`] and [`VariableStore`] for allocating and looking up
//! DAP frame IDs and variable references.
//!
//! ## Lifecycle
//!
//! Both stores are valid only while the debuggee is stopped. When the debuggee
//! resumes, both stores must be reset via [`FrameStore::reset`] and
//! [`VariableStore::reset`] (or equivalently via [`DapAdapter::on_resume`]).
//!
//! IDs are monotonically increasing integers starting at 1, allocated per stop
//! cycle. They do **not** persist across stop/resume transitions.
//!
//! ## Frame Mapping Helpers
//!
//! This module also provides free functions for mapping VM Service frame JSON to
//! DAP protocol types:
//!
//! - [`extract_source`] — extract a [`DapSource`] from a VM Service frame
//! - [`extract_line_column`] — extract line and column from a frame's location
//! - [`dart_uri_to_path`] — convert a Dart URI to a filesystem path

use std::collections::HashMap;

use crate::protocol::types::DapSource;

// ─────────────────────────────────────────────────────────────────────────────
// VariableRef
// ─────────────────────────────────────────────────────────────────────────────

/// What a DAP variable reference points to.
///
/// A variable reference is a compact integer (`i64`) that the DAP client uses
/// to request the children of a scope or object. The adapter maps these
/// integers to [`VariableRef`] values and uses them to make the right VM
/// Service call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableRef {
    /// A scope (locals, globals) associated with a specific frame.
    Scope {
        /// Index of the frame (0 = top of the stack).
        frame_index: i32,
        /// Which scope this represents.
        scope_kind: ScopeKind,
    },
    /// A VM Service object that can be expanded (e.g., a list or instance).
    Object {
        /// The isolate this object belongs to.
        isolate_id: String,
        /// The VM Service object ID.
        object_id: String,
    },
}

/// The kind of scope a [`VariableRef::Scope`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// Local variables visible in the current frame.
    Locals,
    /// Module-level (global) variables.
    Globals,
}

// ─────────────────────────────────────────────────────────────────────────────
// VariableStore
// ─────────────────────────────────────────────────────────────────────────────

/// Allocates and looks up variable references for a single stopped state.
///
/// Variable references are monotonically increasing integers starting at 1.
/// They are **invalidated** on every resume: calling [`VariableStore::reset`]
/// clears all mappings so that stale references from the previous stop cannot
/// be accidentally resolved.
///
/// # Design
///
/// - Uses a simple `HashMap` — cheap to create, cheap to clear.
/// - No complex allocator needed: even large Dart programs have at most a few
///   hundred variables visible at any one time.
pub struct VariableStore {
    references: HashMap<i64, VariableRef>,
    next_ref: i64,
}

impl VariableStore {
    /// Create a new empty [`VariableStore`]. The next reference will be 1.
    pub fn new() -> Self {
        Self {
            references: HashMap::new(),
            next_ref: 1,
        }
    }

    /// Allocate a new variable reference for the given target.
    ///
    /// Returns the allocated reference integer (always >= 1).
    pub fn allocate(&mut self, target: VariableRef) -> i64 {
        let r = self.next_ref;
        self.next_ref += 1;
        self.references.insert(r, target);
        r
    }

    /// Look up what a variable reference points to.
    ///
    /// Returns `None` if the reference was never allocated or if
    /// [`VariableStore::reset`] was called since it was allocated.
    pub fn lookup(&self, reference: i64) -> Option<&VariableRef> {
        self.references.get(&reference)
    }

    /// Invalidate all variable references.
    ///
    /// Must be called when the debuggee resumes. After a reset, any previously
    /// allocated reference will resolve to `None`.
    pub fn reset(&mut self) {
        self.references.clear();
        self.next_ref = 1;
    }

    /// Return the number of currently allocated references.
    pub fn len(&self) -> usize {
        self.references.len()
    }

    /// Return `true` if no references are allocated.
    pub fn is_empty(&self) -> bool {
        self.references.is_empty()
    }
}

impl Default for VariableStore {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FrameRef / FrameStore
// ─────────────────────────────────────────────────────────────────────────────

/// What a DAP frame ID points to.
///
/// A frame ID is a compact integer that identifies a specific stack frame
/// within a specific isolate. The adapter uses it to route `scopes` and
/// `variables` requests back to the correct VM Service call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameRef {
    /// The Dart VM isolate ID containing this frame.
    pub isolate_id: String,
    /// The 0-based index of this frame in the isolate's call stack.
    pub frame_index: i32,
}

impl FrameRef {
    /// Create a new [`FrameRef`].
    pub fn new(isolate_id: impl Into<String>, frame_index: i32) -> Self {
        Self {
            isolate_id: isolate_id.into(),
            frame_index,
        }
    }
}

/// Allocates and looks up frame IDs for a single stopped state.
///
/// Frame IDs are monotonically increasing integers starting at 1, allocated
/// per stop cycle. They are **invalidated** on every resume.
///
/// # Design
///
/// Symmetric to [`VariableStore`]: cheap `HashMap`, reset on resume.
pub struct FrameStore {
    frames: HashMap<i64, FrameRef>,
    next_id: i64,
}

impl FrameStore {
    /// Create a new empty [`FrameStore`]. The next frame ID will be 1.
    pub fn new() -> Self {
        Self {
            frames: HashMap::new(),
            next_id: 1,
        }
    }

    /// Allocate a new frame ID for the given frame reference.
    ///
    /// Returns the allocated frame ID (always >= 1).
    pub fn allocate(&mut self, frame: FrameRef) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        self.frames.insert(id, frame);
        id
    }

    /// Look up what a frame ID points to.
    ///
    /// Returns `None` if the ID was never allocated or if [`FrameStore::reset`]
    /// was called since it was allocated.
    pub fn lookup(&self, frame_id: i64) -> Option<&FrameRef> {
        self.frames.get(&frame_id)
    }

    /// Look up a [`FrameRef`] by its 0-based frame index (not its DAP frame ID).
    ///
    /// Scans all allocated frames to find the first one whose `frame_index`
    /// matches. This is used when resolving a [`VariableRef::Scope`] to find
    /// the isolate ID for a given frame position.
    ///
    /// Returns `None` if no frame with that index is allocated.
    pub fn lookup_by_index(&self, frame_index: i32) -> Option<&FrameRef> {
        self.frames
            .values()
            .find(|fr| fr.frame_index == frame_index)
    }

    /// Invalidate all frame IDs.
    ///
    /// Must be called when the debuggee resumes.
    pub fn reset(&mut self) {
        self.frames.clear();
        self.next_id = 1;
    }

    /// Return the number of currently allocated frame IDs.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Return `true` if no frame IDs are allocated.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

impl Default for FrameStore {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Frame mapping helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Extract a [`DapSource`] from a VM Service frame object.
///
/// The VM Service frame has a `location.script.uri` field that identifies the
/// source file. This function converts that URI to a [`DapSource`] with an
/// appropriate `presentation_hint`:
///
/// - `dart:` URIs (SDK sources) → `"deemphasize"`, no `path`
/// - `package:flutter/` URIs (framework sources) → `"deemphasize"`, no `path`
/// - `file://` URIs (user code) → no hint, absolute filesystem path
/// - Other URIs → no path (Phase 4 will add package resolution)
///
/// Returns `None` when the frame has no `location.script.uri` field.
pub fn extract_source(frame: &serde_json::Value) -> Option<DapSource> {
    let location = frame.get("location")?;
    let script = location.get("script")?;
    let uri = script.get("uri")?.as_str()?;

    // Determine presentation hint based on URI scheme.
    let hint = if uri.starts_with("dart:") {
        // SDK sources are de-emphasized — the developer usually does not want
        // to debug into the Dart SDK.
        Some("deemphasize".to_string())
    } else if uri.starts_with("package:flutter/") {
        // Flutter framework sources are similarly de-emphasized.
        Some("deemphasize".to_string())
    } else {
        // User code: normal emphasis (no hint).
        None
    };

    // Derive a human-readable name from the last path segment of the URI.
    let name = uri.rsplit('/').next().unwrap_or(uri).to_string();

    // Convert the URI to a filesystem path if possible.
    let path = dart_uri_to_path(uri);

    Some(DapSource {
        name: Some(name),
        path,
        source_reference: None,
        presentation_hint: hint,
    })
}

/// Extract line and column numbers from a VM Service frame's `location` field.
///
/// Both values are 1-based per the DAP specification. Returns `(None, None)`
/// when the frame has no `location` field.
pub fn extract_line_column(frame: &serde_json::Value) -> (Option<i32>, Option<i32>) {
    let location = match frame.get("location") {
        Some(loc) => loc,
        None => return (None, None),
    };
    let line = location
        .get("line")
        .and_then(|l| l.as_i64())
        .map(|l| l as i32);
    let column = location
        .get("column")
        .and_then(|c| c.as_i64())
        .map(|c| c as i32);
    (line, column)
}

/// Convert a Dart URI to an absolute filesystem path suitable for [`DapSource::path`].
///
/// DAP clients that use `pathFormat: "path"` (Zed, Helix) expect a plain
/// filesystem path, **not** a `file://` URI.
///
/// Uses [`url::Url::parse`] to correctly handle all `file://` URI forms,
/// including percent-encoded characters and Windows drive-letter paths.
///
/// | Input URI                        | Output (Unix)                    |
/// |----------------------------------|----------------------------------|
/// | `file:///home/app/main.dart`     | `Some("/home/app/main.dart")`    |
/// | `file:///C:/Users/app/main.dart` | `Some("C:\\Users\\app\\main.dart")` (Windows only) |
/// | `file:///home/my%20app/main.dart`| `Some("/home/my app/main.dart")` |
/// | `dart:core/list.dart`            | `None` (no local path in Phase 3) |
/// | `package:myapp/main.dart`        | `None` (deferred to Phase 4)     |
/// | anything else                    | `None`                           |
pub fn dart_uri_to_path(uri: &str) -> Option<String> {
    if uri.starts_with("file://") {
        // Use the `url` crate for correct cross-platform handling:
        // - Strips `file://` properly (three slashes for absolute paths)
        // - Decodes percent-encoded characters (e.g. %20 → space)
        // - Handles Windows drive letters (file:///C:/path → C:\path on Windows)
        url::Url::parse(uri)
            .ok()
            .and_then(|u| u.to_file_path().ok())
            .map(|p| p.to_string_lossy().into_owned())
    } else if uri.starts_with("dart:") || uri.starts_with("package:") {
        // SDK and package URIs cannot be resolved to a local path in Phase 3.
        // Phase 4 will add package resolution via .dart_tool/package_config.json.
        None
    } else {
        None
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── VariableStore ─────────────────────────────────────────────────────

    #[test]
    fn test_variable_store_starts_empty() {
        let store = VariableStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_variable_store_allocates_starting_at_one() {
        let mut store = VariableStore::new();
        let r = store.allocate(VariableRef::Scope {
            frame_index: 0,
            scope_kind: ScopeKind::Locals,
        });
        assert_eq!(r, 1, "First allocated reference must be 1");
    }

    #[test]
    fn test_variable_store_allocates_monotonic_references() {
        let mut store = VariableStore::new();
        let r1 = store.allocate(VariableRef::Scope {
            frame_index: 0,
            scope_kind: ScopeKind::Locals,
        });
        let r2 = store.allocate(VariableRef::Scope {
            frame_index: 0,
            scope_kind: ScopeKind::Globals,
        });
        let r3 = store.allocate(VariableRef::Object {
            isolate_id: "isolates/1".into(),
            object_id: "objects/42".into(),
        });
        assert_eq!(r1, 1);
        assert_eq!(r2, 2);
        assert_eq!(r3, 3);
    }

    #[test]
    fn test_variable_store_lookup_returns_allocated_target() {
        let mut store = VariableStore::new();
        let target = VariableRef::Object {
            isolate_id: "isolates/1".into(),
            object_id: "objects/7".into(),
        };
        let r = store.allocate(target.clone());
        assert_eq!(store.lookup(r), Some(&target));
    }

    #[test]
    fn test_variable_store_lookup_returns_none_for_unknown_reference() {
        let store = VariableStore::new();
        assert!(store.lookup(99).is_none());
    }

    #[test]
    fn test_variable_store_reset_clears_all_references() {
        let mut store = VariableStore::new();
        let r1 = store.allocate(VariableRef::Scope {
            frame_index: 0,
            scope_kind: ScopeKind::Locals,
        });
        let r2 = store.allocate(VariableRef::Object {
            isolate_id: "isolates/1".into(),
            object_id: "objects/1".into(),
        });

        assert!(store.lookup(r1).is_some());
        assert!(store.lookup(r2).is_some());

        store.reset();

        assert!(store.lookup(r1).is_none(), "r1 should be None after reset");
        assert!(store.lookup(r2).is_none(), "r2 should be None after reset");
        assert!(store.is_empty());
    }

    #[test]
    fn test_variable_store_reset_resets_counter_to_one() {
        let mut store = VariableStore::new();
        store.allocate(VariableRef::Scope {
            frame_index: 0,
            scope_kind: ScopeKind::Locals,
        });
        store.allocate(VariableRef::Scope {
            frame_index: 0,
            scope_kind: ScopeKind::Globals,
        });
        store.reset();

        // After reset, IDs should start at 1 again.
        let r = store.allocate(VariableRef::Scope {
            frame_index: 0,
            scope_kind: ScopeKind::Locals,
        });
        assert_eq!(r, 1, "After reset, first allocated reference should be 1");
    }

    #[test]
    fn test_variable_store_len_tracks_allocations() {
        let mut store = VariableStore::new();
        assert_eq!(store.len(), 0);
        store.allocate(VariableRef::Scope {
            frame_index: 0,
            scope_kind: ScopeKind::Locals,
        });
        assert_eq!(store.len(), 1);
        store.allocate(VariableRef::Scope {
            frame_index: 0,
            scope_kind: ScopeKind::Globals,
        });
        assert_eq!(store.len(), 2);
        store.reset();
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_scope_kind_locals_and_globals_are_distinct() {
        assert_ne!(ScopeKind::Locals, ScopeKind::Globals);
    }

    // ── FrameStore ────────────────────────────────────────────────────────

    #[test]
    fn test_frame_store_starts_empty() {
        let store = FrameStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_frame_store_allocates_starting_at_one() {
        let mut store = FrameStore::new();
        let id = store.allocate(FrameRef::new("isolates/1", 0));
        assert_eq!(id, 1, "First allocated frame ID must be 1");
    }

    #[test]
    fn test_frame_store_allocates_monotonic_ids() {
        let mut store = FrameStore::new();
        let id1 = store.allocate(FrameRef::new("isolates/1", 0));
        let id2 = store.allocate(FrameRef::new("isolates/1", 1));
        let id3 = store.allocate(FrameRef::new("isolates/1", 2));
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[test]
    fn test_frame_store_lookup_returns_allocated_frame() {
        let mut store = FrameStore::new();
        let frame = FrameRef::new("isolates/5", 3);
        let id = store.allocate(frame.clone());
        let found = store.lookup(id).expect("Frame should be found");
        assert_eq!(found, &frame);
    }

    #[test]
    fn test_frame_store_lookup_returns_none_for_unknown_id() {
        let store = FrameStore::new();
        assert!(store.lookup(99).is_none());
    }

    #[test]
    fn test_frame_store_reset_clears_all_frames() {
        let mut store = FrameStore::new();
        let id1 = store.allocate(FrameRef::new("isolates/1", 0));
        let id2 = store.allocate(FrameRef::new("isolates/1", 1));

        assert!(store.lookup(id1).is_some());
        assert!(store.lookup(id2).is_some());

        store.reset();

        assert!(
            store.lookup(id1).is_none(),
            "id1 should be None after reset"
        );
        assert!(
            store.lookup(id2).is_none(),
            "id2 should be None after reset"
        );
        assert!(store.is_empty());
    }

    #[test]
    fn test_frame_store_reset_resets_counter_to_one() {
        let mut store = FrameStore::new();
        store.allocate(FrameRef::new("isolates/1", 0));
        store.allocate(FrameRef::new("isolates/1", 1));
        store.reset();

        let id = store.allocate(FrameRef::new("isolates/1", 0));
        assert_eq!(id, 1, "After reset, first allocated frame ID should be 1");
    }

    #[test]
    fn test_frame_store_len_tracks_allocations() {
        let mut store = FrameStore::new();
        assert_eq!(store.len(), 0);
        store.allocate(FrameRef::new("isolates/1", 0));
        assert_eq!(store.len(), 1);
        store.allocate(FrameRef::new("isolates/1", 1));
        assert_eq!(store.len(), 2);
        store.reset();
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_frame_ref_new_stores_fields_correctly() {
        let f = FrameRef::new("isolates/99", 7);
        assert_eq!(f.isolate_id, "isolates/99");
        assert_eq!(f.frame_index, 7);
    }

    #[test]
    fn test_frame_store_multiple_isolates() {
        let mut store = FrameStore::new();
        let id_a = store.allocate(FrameRef::new("isolates/1", 0));
        let id_b = store.allocate(FrameRef::new("isolates/2", 0));

        let frame_a = store.lookup(id_a).unwrap();
        let frame_b = store.lookup(id_b).unwrap();

        assert_eq!(frame_a.isolate_id, "isolates/1");
        assert_eq!(frame_b.isolate_id, "isolates/2");
        assert_ne!(id_a, id_b);
    }

    // ── extract_source ────────────────────────────────────────────────────

    #[test]
    fn test_extract_source_from_file_uri() {
        let frame = serde_json::json!({
            "location": {
                "script": { "uri": "file:///home/user/app/lib/main.dart" },
                "line": 42,
                "column": 5
            }
        });
        let source = extract_source(&frame).unwrap();
        assert_eq!(source.path.as_deref(), Some("/home/user/app/lib/main.dart"));
        assert_eq!(source.name.as_deref(), Some("main.dart"));
        // User code — no presentation hint.
        assert!(source.presentation_hint.is_none());
        assert!(source.source_reference.is_none());
    }

    #[test]
    fn test_extract_source_dart_sdk_deemphasized() {
        let frame = serde_json::json!({
            "location": {
                "script": { "uri": "dart:core/list.dart" },
                "line": 100
            }
        });
        let source = extract_source(&frame).unwrap();
        assert_eq!(source.presentation_hint.as_deref(), Some("deemphasize"));
        // SDK sources have no local path in Phase 3.
        assert!(source.path.is_none());
        assert_eq!(source.name.as_deref(), Some("list.dart"));
    }

    #[test]
    fn test_extract_source_flutter_framework_deemphasized() {
        let frame = serde_json::json!({
            "location": {
                "script": { "uri": "package:flutter/src/widgets/framework.dart" },
                "line": 55
            }
        });
        let source = extract_source(&frame).unwrap();
        assert_eq!(source.presentation_hint.as_deref(), Some("deemphasize"));
        assert!(source.path.is_none());
    }

    #[test]
    fn test_extract_source_user_package_no_hint() {
        // package: URIs that are not flutter/ are user packages — no
        // de-emphasis but also no path yet (Phase 4 will resolve these).
        let frame = serde_json::json!({
            "location": {
                "script": { "uri": "package:my_app/src/home.dart" },
                "line": 10
            }
        });
        let source = extract_source(&frame).unwrap();
        // Not a flutter package — no deemphasize hint.
        assert!(source.presentation_hint.is_none());
        assert!(source.path.is_none());
    }

    #[test]
    fn test_extract_source_returns_none_when_no_location() {
        let frame = serde_json::json!({ "kind": "Regular" });
        assert!(extract_source(&frame).is_none());
    }

    #[test]
    fn test_extract_source_returns_none_when_no_script() {
        let frame = serde_json::json!({
            "location": { "line": 1 }
        });
        assert!(extract_source(&frame).is_none());
    }

    #[test]
    fn test_extract_source_returns_none_when_no_uri() {
        let frame = serde_json::json!({
            "location": {
                "script": {},
                "line": 1
            }
        });
        assert!(extract_source(&frame).is_none());
    }

    // ── extract_line_column ───────────────────────────────────────────────

    #[test]
    fn test_extract_line_column_both_present() {
        let frame = serde_json::json!({
            "location": { "line": 42, "column": 5 }
        });
        assert_eq!(extract_line_column(&frame), (Some(42), Some(5)));
    }

    #[test]
    fn test_extract_line_column_line_only() {
        let frame = serde_json::json!({
            "location": { "line": 10 }
        });
        assert_eq!(extract_line_column(&frame), (Some(10), None));
    }

    #[test]
    fn test_extract_line_column_no_location() {
        let frame = serde_json::json!({ "kind": "Regular" });
        assert_eq!(extract_line_column(&frame), (None, None));
    }

    #[test]
    fn test_extract_line_column_empty_location() {
        let frame = serde_json::json!({ "location": {} });
        assert_eq!(extract_line_column(&frame), (None, None));
    }

    // ── dart_uri_to_path ──────────────────────────────────────────────────

    #[test]
    fn test_dart_uri_to_path_file_uri() {
        assert_eq!(
            dart_uri_to_path("file:///home/user/app/lib/main.dart"),
            Some("/home/user/app/lib/main.dart".to_string())
        );
    }

    #[test]
    fn test_dart_uri_to_path_file_uri_strips_prefix_only() {
        // Ensure the path starts with / after converting file:/// URI.
        let result = dart_uri_to_path("file:///tmp/app.dart");
        assert_eq!(result, Some("/tmp/app.dart".to_string()));
    }

    #[test]
    fn test_dart_uri_to_path_dart_scheme_returns_none() {
        assert!(dart_uri_to_path("dart:core/list.dart").is_none());
        assert!(dart_uri_to_path("dart:async").is_none());
    }

    #[test]
    fn test_dart_uri_to_path_package_scheme_returns_none() {
        assert!(dart_uri_to_path("package:my_app/main.dart").is_none());
        assert!(dart_uri_to_path("package:flutter/widgets.dart").is_none());
    }

    #[test]
    fn test_dart_uri_to_path_unknown_scheme_returns_none() {
        assert!(dart_uri_to_path("http://example.com/file.dart").is_none());
        assert!(dart_uri_to_path("").is_none());
    }

    #[test]
    fn test_dart_uri_to_path_unix_absolute() {
        assert_eq!(
            dart_uri_to_path("file:///home/user/app/lib/main.dart"),
            Some("/home/user/app/lib/main.dart".to_string())
        );
    }

    #[test]
    fn test_dart_uri_to_path_windows_drive_letter() {
        // The url crate's to_file_path() is platform-specific.
        //
        // On Windows: file:///C:/Users/app/lib/main.dart → "C:\Users\app\lib\main.dart"
        //   (the leading slash before the drive letter is correctly stripped)
        //
        // On Unix: the url crate parses file:///C:/... as a valid file URI
        // with empty host and path /C:/... — to_file_path() succeeds and
        // returns /C:/Users/app/lib/main.dart. This is not meaningful as a
        // local path, but it does not panic and is an inherent limitation
        // of cross-platform path handling (documented in the function).
        //
        // In both cases the function must return Some (not panic/return None
        // for a syntactically valid file:/// URI).
        let result = dart_uri_to_path("file:///C:/Users/app/lib/main.dart");
        assert!(
            result.is_some(),
            "Expected Some for a well-formed file:/// URI, got None"
        );
        if cfg!(windows) {
            let path = result.unwrap();
            assert!(
                !path.starts_with("/C:"),
                "On Windows, path must not have leading / before drive letter, got: {path}"
            );
            assert!(
                path.starts_with("C:"),
                "On Windows, path must start with drive letter, got: {path}"
            );
        }
    }

    #[test]
    fn test_dart_uri_to_path_percent_encoded() {
        // Percent-encoded characters in the path should be decoded.
        // file:///home/my%20project/main.dart → /home/my project/main.dart
        assert_eq!(
            dart_uri_to_path("file:///home/my%20project/main.dart"),
            Some("/home/my project/main.dart".to_string())
        );
    }

    #[test]
    fn test_dart_uri_to_path_percent_encoded_special_chars() {
        // Parentheses and other characters that may be percent-encoded
        // by some tools or systems.
        assert_eq!(
            dart_uri_to_path("file:///home/user/my%28app%29/main.dart"),
            Some("/home/user/my(app)/main.dart".to_string())
        );
    }

    #[test]
    fn test_dart_uri_to_path_dart_scheme_core() {
        // dart: URIs for SDK sources should return None.
        assert!(dart_uri_to_path("dart:core/list.dart").is_none());
    }

    #[test]
    fn test_dart_uri_to_path_dart_scheme_async() {
        assert!(dart_uri_to_path("dart:async").is_none());
    }

    #[test]
    fn test_dart_uri_to_path_package_scheme_user() {
        // package: URIs should return None (resolved in Phase 4).
        assert!(dart_uri_to_path("package:my_app/main.dart").is_none());
    }

    #[test]
    fn test_dart_uri_to_path_package_scheme_flutter() {
        assert!(dart_uri_to_path("package:flutter/widgets.dart").is_none());
    }

    #[test]
    fn test_dart_uri_to_path_nested_path_preserved() {
        // Deep paths should be preserved verbatim.
        assert_eq!(
            dart_uri_to_path("file:///home/user/projects/my_app/lib/src/widgets/home.dart"),
            Some("/home/user/projects/my_app/lib/src/widgets/home.dart".to_string())
        );
    }

    #[test]
    fn test_dart_uri_to_path_two_slash_file_uri_returns_none() {
        // file:// with a hostname component (two slashes) is unusual but
        // valid per RFC 8089. The url crate parses it but to_file_path()
        // rejects non-empty hosts, so we return None gracefully.
        // Flutter's VM Service always produces three-slash file:/// URIs.
        let result = dart_uri_to_path("file://hostname/path/to/file.dart");
        assert!(
            result.is_none(),
            "file:// URIs with a non-empty host should return None"
        );
    }

    // ── FrameStore::lookup_by_index ───────────────────────────────────────

    #[test]
    fn test_frame_store_lookup_by_index_finds_correct_frame() {
        let mut store = FrameStore::new();
        store.allocate(FrameRef::new("isolates/1", 0));
        store.allocate(FrameRef::new("isolates/1", 1));
        store.allocate(FrameRef::new("isolates/1", 2));

        let found = store
            .lookup_by_index(1)
            .expect("Should find frame at index 1");
        assert_eq!(found.frame_index, 1);
        assert_eq!(found.isolate_id, "isolates/1");
    }

    #[test]
    fn test_frame_store_lookup_by_index_returns_none_when_not_found() {
        let mut store = FrameStore::new();
        store.allocate(FrameRef::new("isolates/1", 0));
        assert!(store.lookup_by_index(99).is_none());
    }

    #[test]
    fn test_frame_store_lookup_by_index_returns_none_after_reset() {
        let mut store = FrameStore::new();
        store.allocate(FrameRef::new("isolates/1", 0));
        store.reset();
        assert!(store.lookup_by_index(0).is_none());
    }

    #[test]
    fn test_frame_store_lookup_by_index_empty_store() {
        let store = FrameStore::new();
        assert!(store.lookup_by_index(0).is_none());
    }

    // ── FrameStore reset (from task spec) ─────────────────────────────────

    #[test]
    fn test_frame_store_reset_invalidates_all() {
        let mut store = FrameStore::new();
        let id = store.allocate(FrameRef {
            isolate_id: "i/1".into(),
            frame_index: 0,
        });
        assert!(store.lookup(id).is_some());
        store.reset();
        assert!(store.lookup(id).is_none());
    }
}
