//! # Frame and Variable Reference Stores
//!
//! Provides [`FrameStore`], [`VariableStore`], and [`SourceReferenceStore`] for
//! allocating and looking up DAP frame IDs, variable references, and source
//! reference handles.
//!
//! ## Lifecycle
//!
//! Both [`FrameStore`] and [`VariableStore`] are valid only while the debuggee
//! is stopped. When the debuggee resumes, both stores must be reset via
//! [`FrameStore::reset`] and [`VariableStore::reset`] (or equivalently via
//! [`DapAdapter::on_resume`]).
//!
//! [`SourceReferenceStore`] persists across stop/resume transitions but is
//! invalidated on hot restart via [`SourceReferenceStore::clear`].
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
//! - [`extract_source_with_store`] — extract a [`DapSource`], assigning
//!   source references for SDK/package sources that cannot be resolved locally
//! - [`extract_line_column`] — extract line and column from a frame's location
//! - [`dart_uri_to_path`] — convert a Dart URI to a filesystem path
//! - [`resolve_package_uri`] — try to resolve a `package:` URI to a local path

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::protocol::types::DapSource;

// ─────────────────────────────────────────────────────────────────────────────
// SourceReferenceStore
// ─────────────────────────────────────────────────────────────────────────────

/// Entry stored in [`SourceReferenceStore`] for each allocated reference.
struct SourceRefEntry {
    /// The Dart VM isolate ID that loaded this script.
    isolate_id: String,
    /// The VM Service script object ID (used in `getObject` calls).
    script_id: String,
    /// The original URI of the script (e.g. `"dart:core/string.dart"`).
    uri: String,
}

/// Maps DAP `sourceReference` IDs to the script information needed to fetch
/// source text via `getObject`.
///
/// ## Lifecycle
///
/// Source references are allocated on demand during `stackTrace` responses for
/// any source URI that cannot be mapped to a local file (e.g., `dart:` SDK
/// URIs and unresolvable `package:` URIs). Unlike frame IDs and variable
/// references, source references persist across stop/resume transitions — the
/// IDE may re-request source text at any time. They are only invalidated on
/// hot restart (which creates a new isolate with new script IDs) via
/// [`SourceReferenceStore::clear`].
///
/// ## ID stability
///
/// The same `(isolate_id, script_id)` pair always returns the same reference
/// ID within a session so that IDEs can cache content without duplicate
/// lookups.
pub struct SourceReferenceStore {
    next_id: i64,
    /// reference_id → script information
    references: HashMap<i64, SourceRefEntry>,
}

impl SourceReferenceStore {
    /// Create a new empty store. The first allocated ID will be 1.
    pub fn new() -> Self {
        Self {
            next_id: 1,
            references: HashMap::new(),
        }
    }

    /// Return the existing reference ID for `(isolate_id, script_id)`, or
    /// allocate and store a new one.
    ///
    /// ID stability: the same `(isolate_id, script_id)` always produces the
    /// same reference ID within a debug session.
    pub fn get_or_create(&mut self, isolate_id: &str, script_id: &str, uri: &str) -> i64 {
        // Check for an existing reference for this script.
        for (&id, entry) in &self.references {
            if entry.script_id == script_id && entry.isolate_id == isolate_id {
                return id;
            }
        }
        // Allocate a new reference.
        let id = self.next_id;
        self.next_id += 1;
        self.references.insert(
            id,
            SourceRefEntry {
                isolate_id: isolate_id.to_string(),
                script_id: script_id.to_string(),
                uri: uri.to_string(),
            },
        );
        id
    }

    /// Look up the script information for a given reference ID.
    ///
    /// Returns `None` if the reference was never allocated or has been cleared.
    pub fn get(&self, reference_id: i64) -> Option<SourceRefInfo> {
        self.references.get(&reference_id).map(|e| SourceRefInfo {
            isolate_id: e.isolate_id.clone(),
            script_id: e.script_id.clone(),
            uri: e.uri.clone(),
        })
    }

    /// Invalidate all source references.
    ///
    /// Must be called on hot restart — a new isolate is created with new
    /// script IDs, so old references are no longer valid.
    ///
    /// Note: `next_id` is **not** reset, because DAP clients may have cached
    /// old reference IDs. Reusing them would cause stale cache hits.
    pub fn clear(&mut self) {
        self.references.clear();
    }

    /// Return the number of currently allocated references.
    pub fn len(&self) -> usize {
        self.references.len()
    }

    /// Return `true` if no references are currently allocated.
    pub fn is_empty(&self) -> bool {
        self.references.is_empty()
    }
}

impl Default for SourceReferenceStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolved source reference information returned by [`SourceReferenceStore::get`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRefInfo {
    /// The Dart VM isolate ID that loaded this script.
    pub isolate_id: String,
    /// The VM Service script object ID.
    pub script_id: String,
    /// The original URI of the script.
    pub uri: String,
}

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
    /// A lazy getter evaluation on a specific object.
    ///
    /// Created when `evaluateGettersInDebugViews` is `false`. Expanding this
    /// reference evaluates the getter on demand via `backend.evaluate`.
    GetterEval {
        /// The isolate the parent object belongs to.
        isolate_id: String,
        /// The VM Service object ID of the parent instance.
        instance_id: String,
        /// The getter method name to evaluate (e.g., `"name"`, `"age"`).
        getter_name: String,
    },
}

/// The kind of scope a [`VariableRef::Scope`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// Local variables visible in the current frame.
    Locals,
    /// Module-level (global) variables.
    Globals,
    /// The current exception when paused at an exception.
    ///
    /// This scope appears only when the isolate is paused at a
    /// `PauseException` event. It contains a single variable (the exception
    /// object) that can be expanded to inspect its fields.
    Exceptions,
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
/// - Other URIs → no path
///
/// Returns `None` when the frame has no `location.script.uri` field.
///
/// For source references (SDK/unresolvable package URIs), use
/// [`extract_source_with_store`] instead.
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

/// Extract a [`DapSource`] from a VM Service frame, assigning source references
/// for SDK and unresolvable package sources.
///
/// ## Source strategy by URI type
///
/// | URI prefix              | Strategy                                              |
/// |-------------------------|-------------------------------------------------------|
/// | `file://`               | Local path via [`dart_uri_to_path`]; `sourceReference: 0` |
/// | `dart:`                 | `sourceReference > 0`; no `path`; `"deemphasize"` hint |
/// | `org-dartlang-sdk:`     | `sourceReference > 0`; no `path`; `"deemphasize"` hint |
/// | `package:`              | Try resolving via `.dart_tool/package_config.json`. If found: use `path`. If not: `sourceReference > 0`. |
///
/// `isolate_id` is required to allocate a unique source reference per
/// `(isolate, script)` pair. `project_root` is used to locate
/// `.dart_tool/package_config.json` for package resolution.
///
/// Returns `None` when the frame has no `location.script.uri` field.
pub fn extract_source_with_store(
    frame: &serde_json::Value,
    store: &mut SourceReferenceStore,
    isolate_id: &str,
    project_root: Option<&Path>,
) -> Option<DapSource> {
    let location = frame.get("location")?;
    let script = location.get("script")?;
    let uri = script.get("uri")?.as_str()?;
    // script_id is the VM object ID used for getObject (the script's `id` field).
    let script_id = script.get("id").and_then(|v| v.as_str()).unwrap_or(uri);

    let name = uri.rsplit('/').next().unwrap_or(uri).to_string();

    if uri.starts_with("file://") {
        // User code — map to local filesystem path, no source reference.
        let path = dart_uri_to_path(uri);
        return Some(DapSource {
            name: Some(name),
            path,
            source_reference: None,
            presentation_hint: None,
        });
    }

    if uri.starts_with("package:") {
        // Try to resolve to a local path via package_config.json.
        let resolved = project_root.and_then(|root| resolve_package_uri(uri, root));
        if let Some(local_path) = resolved {
            // Found locally — use path so the IDE opens an editable file.
            let hint = if uri.starts_with("package:flutter/") {
                Some("deemphasize".to_string())
            } else {
                None
            };
            return Some(DapSource {
                name: Some(name),
                path: Some(local_path.to_string_lossy().into_owned()),
                source_reference: None,
                presentation_hint: hint,
            });
        }
        // Not resolvable — fall through to assign a source reference.
        let source_ref = store.get_or_create(isolate_id, script_id, uri);
        let hint = if uri.starts_with("package:flutter/") {
            Some("deemphasize".to_string())
        } else {
            None
        };
        return Some(DapSource {
            name: Some(name),
            path: None,
            source_reference: Some(source_ref),
            presentation_hint: hint,
        });
    }

    // dart: and org-dartlang-sdk: URIs — SDK sources, always fetch via VM Service.
    if uri.starts_with("dart:") || uri.starts_with("org-dartlang-sdk:") {
        let source_ref = store.get_or_create(isolate_id, script_id, uri);
        return Some(DapSource {
            name: Some(name),
            path: None,
            source_reference: Some(source_ref),
            presentation_hint: Some("deemphasize".to_string()),
        });
    }

    // Unknown URI scheme — return without path or source reference.
    Some(DapSource {
        name: Some(name),
        path: None,
        source_reference: None,
        presentation_hint: None,
    })
}

/// Try to resolve a `package:` URI to a local filesystem path using
/// `.dart_tool/package_config.json`.
///
/// Returns `None` if:
/// - `uri` does not start with `"package:"`
/// - `package_config.json` cannot be read or parsed
/// - The named package is not in the config
///
/// # Package config format
///
/// The `package_config.json` file in `.dart_tool/` contains a `packages` array
/// where each entry has:
/// - `name` — the package name
/// - `rootUri` — a URI pointing to the package root
/// - `packageUri` — the relative lib path within the root (defaults to `"lib/"`)
///
/// ```json
/// {
///   "packages": [
///     { "name": "my_pkg", "rootUri": "file:///home/user/my_pkg", "packageUri": "lib/" }
///   ]
/// }
/// ```
pub fn resolve_package_uri(uri: &str, project_root: &Path) -> Option<PathBuf> {
    if !uri.starts_with("package:") {
        return None;
    }

    let config_path = project_root.join(".dart_tool/package_config.json");
    let config_text = std::fs::read_to_string(&config_path).ok()?;
    let config: serde_json::Value = serde_json::from_str(&config_text).ok()?;

    let package_name = uri.strip_prefix("package:")?.split('/').next()?;
    let rest = uri.strip_prefix(&format!("package:{}/", package_name))?;

    let packages = config["packages"].as_array()?;
    for pkg in packages {
        if pkg["name"].as_str() != Some(package_name) {
            continue;
        }
        let root_uri = pkg["rootUri"].as_str()?;
        let package_uri = pkg
            .get("packageUri")
            .and_then(|v| v.as_str())
            .unwrap_or("lib/");
        let root: PathBuf = if root_uri.starts_with("file://") {
            // Absolute file URI — use the url crate for correct decoding.
            url::Url::parse(root_uri)
                .ok()
                .and_then(|u| u.to_file_path().ok())?
        } else {
            // Relative URI — resolve relative to the config directory.
            config_path.parent()?.join(root_uri)
        };
        return Some(root.join(package_uri).join(rest));
    }
    None
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

    // ── SourceReferenceStore ──────────────────────────────────────────────

    #[test]
    fn test_source_reference_store_starts_empty() {
        let store = SourceReferenceStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_source_reference_store_get_or_create_returns_same_id_for_same_script() {
        let mut store = SourceReferenceStore::new();
        let id1 = store.get_or_create("isolate/1", "script/1", "dart:core/string.dart");
        let id2 = store.get_or_create("isolate/1", "script/1", "dart:core/string.dart");
        assert_eq!(
            id1, id2,
            "Same (isolate, script) must produce the same reference ID"
        );
    }

    #[test]
    fn test_source_reference_store_get_or_create_different_scripts_get_different_ids() {
        let mut store = SourceReferenceStore::new();
        let id1 = store.get_or_create("isolate/1", "script/1", "dart:core/string.dart");
        let id2 = store.get_or_create("isolate/1", "script/2", "dart:core/list.dart");
        assert_ne!(
            id1, id2,
            "Different scripts must get different reference IDs"
        );
    }

    #[test]
    fn test_source_reference_store_get_returns_correct_info() {
        let mut store = SourceReferenceStore::new();
        let id = store.get_or_create("isolate/1", "script/42", "dart:core/string.dart");
        let info = store.get(id).expect("Should find the allocated reference");
        assert_eq!(info.isolate_id, "isolate/1");
        assert_eq!(info.script_id, "script/42");
        assert_eq!(info.uri, "dart:core/string.dart");
    }

    #[test]
    fn test_source_reference_store_get_unknown_returns_none() {
        let store = SourceReferenceStore::new();
        assert!(store.get(999).is_none());
    }

    #[test]
    fn test_source_reference_store_clear_removes_all_entries() {
        let mut store = SourceReferenceStore::new();
        let id1 = store.get_or_create("isolate/1", "script/1", "dart:core/string.dart");
        let id2 = store.get_or_create("isolate/1", "script/2", "dart:core/list.dart");
        assert_eq!(store.len(), 2);

        store.clear();

        assert!(store.is_empty());
        assert!(store.get(id1).is_none(), "id1 should be None after clear");
        assert!(store.get(id2).is_none(), "id2 should be None after clear");
    }

    #[test]
    fn test_source_reference_store_clear_preserves_next_id() {
        // After clear, next allocated ID must be higher than the last one
        // so that stale client caches don't get false hits.
        let mut store = SourceReferenceStore::new();
        let id1 = store.get_or_create("isolate/1", "script/1", "dart:core/string.dart");
        store.clear();
        let id2 = store.get_or_create("isolate/1", "script/1", "dart:core/string.dart");
        // After clear, IDs continue from where they left off (not reset to 1).
        assert!(
            id2 > id1,
            "IDs must not be reused after clear (got id1={id1}, id2={id2})"
        );
    }

    // ── extract_source_with_store — file:// user code ─────────────────────

    #[test]
    fn test_extract_source_with_store_file_uri_gets_path_no_source_reference() {
        let mut store = SourceReferenceStore::new();
        let frame = serde_json::json!({
            "location": {
                "script": { "uri": "file:///home/user/app/lib/main.dart", "id": "scripts/1" },
                "line": 42,
                "column": 5
            }
        });
        let source = extract_source_with_store(&frame, &mut store, "isolate/1", None).unwrap();
        assert_eq!(source.path.as_deref(), Some("/home/user/app/lib/main.dart"));
        assert!(
            source.source_reference.is_none(),
            "User code must not have a source reference"
        );
        assert!(
            source.presentation_hint.is_none(),
            "User code must not be deemphasized"
        );
        assert!(
            store.is_empty(),
            "Store must not be modified for file:// URIs"
        );
    }

    // ── extract_source_with_store — dart: SDK sources ─────────────────────

    #[test]
    fn test_extract_source_with_store_dart_sdk_uri_gets_source_reference() {
        let mut store = SourceReferenceStore::new();
        let frame = serde_json::json!({
            "location": {
                "script": { "uri": "dart:core/string.dart", "id": "scripts/sdk/1" },
                "line": 100
            }
        });
        let source = extract_source_with_store(&frame, &mut store, "isolate/1", None).unwrap();
        assert!(source.path.is_none(), "SDK sources must have no path");
        let src_ref = source
            .source_reference
            .expect("SDK sources must have a source reference");
        assert!(
            src_ref > 0,
            "Source reference must be positive, got {src_ref}"
        );
        assert_eq!(source.presentation_hint.as_deref(), Some("deemphasize"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_extract_source_with_store_org_dartlang_sdk_uri_gets_source_reference() {
        let mut store = SourceReferenceStore::new();
        let frame = serde_json::json!({
            "location": {
                "script": { "uri": "org-dartlang-sdk:///sdk/lib/core/string.dart", "id": "scripts/sdk/2" },
                "line": 10
            }
        });
        let source = extract_source_with_store(&frame, &mut store, "isolate/1", None).unwrap();
        assert!(source.path.is_none());
        assert!(source.source_reference.unwrap_or(0) > 0);
        assert_eq!(source.presentation_hint.as_deref(), Some("deemphasize"));
    }

    // ── extract_source_with_store — same script returns same reference ─────

    #[test]
    fn test_extract_source_with_store_same_script_returns_same_source_reference() {
        let mut store = SourceReferenceStore::new();
        let frame = serde_json::json!({
            "location": {
                "script": { "uri": "dart:core/list.dart", "id": "scripts/sdk/list" },
                "line": 50
            }
        });
        let source1 = extract_source_with_store(&frame, &mut store, "isolate/1", None).unwrap();
        let source2 = extract_source_with_store(&frame, &mut store, "isolate/1", None).unwrap();
        assert_eq!(
            source1.source_reference, source2.source_reference,
            "Same script must yield the same source reference ID"
        );
        assert_eq!(store.len(), 1, "Only one entry should be stored");
    }

    // ── extract_source_with_store — package: unresolvable ─────────────────

    #[test]
    fn test_extract_source_with_store_unresolvable_package_gets_source_reference() {
        let mut store = SourceReferenceStore::new();
        let frame = serde_json::json!({
            "location": {
                "script": { "uri": "package:my_lib/src/util.dart", "id": "scripts/pkg/1" },
                "line": 20
            }
        });
        // No project_root → package resolution fails → source reference assigned.
        let source = extract_source_with_store(&frame, &mut store, "isolate/1", None).unwrap();
        assert!(source.path.is_none());
        assert!(source.source_reference.unwrap_or(0) > 0);
    }

    // ── resolve_package_uri ───────────────────────────────────────────────

    #[test]
    fn test_resolve_package_uri_returns_none_for_non_package_uri() {
        let tmp = tempdir_for_test();
        assert!(resolve_package_uri("dart:core/list.dart", &tmp).is_none());
        assert!(resolve_package_uri("file:///path/main.dart", &tmp).is_none());
    }

    #[test]
    fn test_resolve_package_uri_returns_none_when_no_config_file() {
        let tmp = tempdir_for_test();
        // No .dart_tool/package_config.json present.
        assert!(resolve_package_uri("package:my_pkg/main.dart", &tmp).is_none());
    }

    #[test]
    fn test_resolve_package_uri_resolves_local_package() {
        let tmp = tempdir_for_test();

        // Create the package root directory.
        std::fs::create_dir_all(tmp.join(".dart_tool")).unwrap();
        std::fs::create_dir_all(tmp.join("packages/my_pkg/lib")).unwrap();

        // Write a package_config.json with a relative rootUri.
        let config = serde_json::json!({
            "configVersion": 2,
            "packages": [
                {
                    "name": "my_pkg",
                    "rootUri": "../packages/my_pkg",
                    "packageUri": "lib/"
                }
            ]
        });
        std::fs::write(
            tmp.join(".dart_tool/package_config.json"),
            serde_json::to_string(&config).unwrap(),
        )
        .unwrap();

        let resolved = resolve_package_uri("package:my_pkg/main.dart", &tmp);
        assert!(
            resolved.is_some(),
            "Should resolve package:my_pkg/main.dart"
        );
        let path = resolved.unwrap();
        assert!(
            path.to_string_lossy().ends_with("main.dart"),
            "Resolved path should end with main.dart, got: {:?}",
            path
        );
    }

    #[test]
    fn test_resolve_package_uri_returns_none_for_unknown_package() {
        let tmp = tempdir_for_test();
        std::fs::create_dir_all(tmp.join(".dart_tool")).unwrap();
        let config = serde_json::json!({
            "configVersion": 2,
            "packages": [
                { "name": "other_pkg", "rootUri": "../other_pkg", "packageUri": "lib/" }
            ]
        });
        std::fs::write(
            tmp.join(".dart_tool/package_config.json"),
            serde_json::to_string(&config).unwrap(),
        )
        .unwrap();

        assert!(resolve_package_uri("package:my_pkg/main.dart", &tmp).is_none());
    }

    /// Create a temporary directory for file-based tests.
    ///
    /// Returns a [`PathBuf`] pointing to a unique temp directory. The directory
    /// is created via [`std::env::temp_dir`] with a unique suffix derived from
    /// the test thread ID for isolation.
    fn tempdir_for_test() -> PathBuf {
        let mut tmp = std::env::temp_dir();
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        tmp.push(format!("fdemon_stack_test_{}", ts));
        std::fs::create_dir_all(&tmp).unwrap();
        tmp
    }

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
