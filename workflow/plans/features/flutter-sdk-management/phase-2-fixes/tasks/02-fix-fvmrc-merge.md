## Task: Implement JSON Merge for .fvmrc Writes

**Objective**: Fix `switch_flutter_version()` to read-merge-write `.fvmrc` instead of overwriting the entire file, preserving existing FVM configuration fields.

**Depends on**: None

**Severity**: CRITICAL — silently destroys user configuration on every version switch

### Scope

- `crates/fdemon-app/src/actions/mod.rs`: Rewrite the `.fvmrc` write logic in `switch_flutter_version()`

### Details

#### The Bug

**File:** `crates/fdemon-app/src/actions/mod.rs`, lines 835-840

```rust
// Current broken code
let fvmrc_path = project_path.join(".fvmrc");
let fvmrc_content = format!(r#"{{"flutter": "{}"}}"#, version);
std::fs::write(&fvmrc_path, &fvmrc_content).map_err(|e| {
    fdemon_core::Error::config(format!("Failed to write {}: {e}", fvmrc_path.display()))
})?;
```

This writes `{"flutter": "<version>"}` as the entire file content, destroying any existing fields. FVM v3's `.fvmrc` supports additional fields:
- `flavors` — build flavor configurations
- `runPubGetOnSdkChanges` — auto-run pub get on SDK change
- `updateVscodeSettings` — auto-update VS Code settings
- `updateGitIgnore` — auto-update .gitignore
- `privilegedAccess` — allow privileged operations

The PLAN.md explicitly states: "Read additional fields but don't modify them."

#### The Fix

Replace the `format!` + `write` with a read-parse-merge-write pattern:

```rust
let fvmrc_path = project_path.join(".fvmrc");

// Read and parse existing file, or start with empty object
let mut json: serde_json::Value = std::fs::read_to_string(&fvmrc_path)
    .ok()
    .and_then(|s| serde_json::from_str(&s).ok())
    .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));

// If existing file was not a JSON object (corrupted), reset to empty object
if !json.is_object() {
    json = serde_json::Value::Object(serde_json::Map::new());
}

// Set only the flutter field; all other fields are preserved
json["flutter"] = serde_json::Value::String(version.to_string());

let fvmrc_content = serde_json::to_string_pretty(&json)
    .map_err(|e| fdemon_core::Error::config(format!("Failed to serialize .fvmrc: {e}")))?;

std::fs::write(&fvmrc_path, fvmrc_content).map_err(|e| {
    fdemon_core::Error::config(format!("Failed to write {}: {e}", fvmrc_path.display()))
})?;
```

**Why `serde_json`?** It is already a workspace dependency of `fdemon-app` (`Cargo.toml:15: serde_json.workspace = true`). The daemon's `detect_fvm_modern` already reads `.fvmrc` as `serde_json::Value` (`version_managers.rs:89`), so this mirrors the existing pattern.

**Why `to_string_pretty`?** FVM itself writes pretty-printed JSON. Using `to_string_pretty` keeps the file human-readable and consistent with FVM's own output.

#### Edge Cases

| Scenario | Behavior |
|----------|----------|
| `.fvmrc` does not exist | Creates new file with `{"flutter": "version"}` |
| `.fvmrc` exists with extra fields | Preserves all fields, updates only `"flutter"` |
| `.fvmrc` is not valid JSON | Resets to new object with just `"flutter"` field (existing corrupted content is lost) |
| `.fvmrc` is valid JSON but not an object (e.g., `"string"` or `[array]`) | Resets to new object |
| `.fvmrc` read fails (permissions) | Falls back to creating new file with just `"flutter"` |

### Acceptance Criteria

1. `switch_flutter_version()` reads existing `.fvmrc` before writing
2. Only the `"flutter"` field is modified; all other fields are preserved
3. If `.fvmrc` does not exist, a new file is created with `{"flutter": "<version>"}`
4. If `.fvmrc` is corrupted/non-JSON, it falls back to a clean write (not a crash)
5. The written JSON is pretty-printed
6. `cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

### Testing

Add tests in the existing test module near `switch_flutter_version`:

```rust
#[test]
fn test_switch_version_preserves_fvmrc_fields() {
    let dir = tempfile::tempdir().unwrap();
    let fvmrc = dir.path().join(".fvmrc");

    // Write initial .fvmrc with extra fields
    std::fs::write(&fvmrc, r#"{"flutter": "3.19.0", "flavors": {"dev": "3.19.0"}, "runPubGetOnSdkChanges": true}"#).unwrap();

    switch_flutter_version("3.22.0", dir.path(), None).unwrap();

    let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&fvmrc).unwrap()).unwrap();
    assert_eq!(content["flutter"], "3.22.0");
    assert_eq!(content["flavors"]["dev"], "3.19.0");       // preserved
    assert_eq!(content["runPubGetOnSdkChanges"], true);     // preserved
}

#[test]
fn test_switch_version_creates_fvmrc_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    let fvmrc = dir.path().join(".fvmrc");
    assert!(!fvmrc.exists());

    switch_flutter_version("3.22.0", dir.path(), None).unwrap();

    let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&fvmrc).unwrap()).unwrap();
    assert_eq!(content["flutter"], "3.22.0");
}

#[test]
fn test_switch_version_handles_corrupted_fvmrc() {
    let dir = tempfile::tempdir().unwrap();
    let fvmrc = dir.path().join(".fvmrc");
    std::fs::write(&fvmrc, "not json at all").unwrap();

    switch_flutter_version("3.22.0", dir.path(), None).unwrap();

    let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&fvmrc).unwrap()).unwrap();
    assert_eq!(content["flutter"], "3.22.0");
}
```

Note: These tests will need to mock or bypass the `find_flutter_sdk` call at the end of `switch_flutter_version`. If the function is not currently testable in isolation, extract the `.fvmrc` write logic into a helper function (`write_fvmrc_version(path, version)`) that can be tested independently.

### Notes

- `serde_json` is already available — no new dependency needed.
- The doc comment on `switch_flutter_version` says "minimal FVM-compatible JSON" — update it to describe the merge behavior.
- This also fixes the related minor issue #14 from the review (`.fvmrc` JSON written with `format!` instead of `serde_json`).
