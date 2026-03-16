## Task: Version Manager Config Parsers

**Objective**: Implement config file parsers for all supported version managers (FVM, Puro, asdf, mise, proto, flutter_wrapper) and a parent-directory tree walk to find config files in monorepo layouts.

**Depends on**: 01-core-types

### Scope

- `crates/fdemon-daemon/src/flutter_sdk/version_managers.rs`: **NEW** — All version manager detection functions
- `crates/fdemon-daemon/src/flutter_sdk/mod.rs`: Add `mod version_managers` and re-exports

### Details

#### Module Structure

Create `version_managers.rs` with one public detection function per version manager. Each function:
- Takes `project_path: &Path` (the Flutter project root)
- Returns `Result<Option<PathBuf>>` — the resolved SDK root path, or `None` if the tool's config file is not found
- Does NOT validate the SDK (caller runs `validate_sdk_path()` on the result)
- Logs at `debug!` level: config file searched, found/not found, resolved path

#### Parent Directory Tree Walk

Config files may be at the project root or any ancestor directory (monorepo support). Implement a shared helper:

```rust
/// Walk from `start` upward to the filesystem root, looking for `filename`.
/// Returns the first matching path found, or None.
fn find_config_upward(start: &Path, filename: &str) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let candidate = current.join(filename);
        if candidate.exists() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}
```

This is used by FVM, Puro, asdf, mise, proto, and flutter_wrapper detectors.

#### Detection Functions

##### 1. FVM Modern (`.fvmrc`)

```rust
/// Detect Flutter SDK via FVM modern config (.fvmrc).
///
/// Parses `.fvmrc` (JSON) for the `flutter` field (version string).
/// Resolves SDK path: `$FVM_CACHE_PATH/versions/<version>/` or `~/fvm/versions/<version>/`.
pub fn detect_fvm_modern(project_path: &Path) -> Result<Option<PathBuf>>
```

- Walk upward for `.fvmrc`
- Parse as JSON: `{ "flutter": "3.19.0", ... }` — only read the `flutter` field
- Resolve FVM cache: check `FVM_CACHE_PATH` env var first, fall back to `dirs::home_dir()/fvm/versions/`
- Return `cache_path/<version>/`

`.fvmrc` format (JSON):
```json
{
  "flutter": "3.19.0",
  "flavors": {}
}
```

##### 2. FVM Legacy (`.fvm/fvm_config.json` + symlink)

```rust
/// Detect Flutter SDK via FVM legacy config (.fvm/fvm_config.json or .fvm/flutter_sdk symlink).
pub fn detect_fvm_legacy(project_path: &Path) -> Result<Option<PathBuf>>
```

- Walk upward for `.fvm` directory
- First try: `fs::canonicalize(.fvm/flutter_sdk)` — resolve the symlink to the real SDK path
- Fallback: parse `.fvm/fvm_config.json` for `flutterSdkVersion` field, then resolve via FVM cache

`.fvm/fvm_config.json` format (JSON):
```json
{
  "flutterSdkVersion": "3.19.0",
  "flavors": {}
}
```

##### 3. Puro (`.puro.json`)

```rust
/// Detect Flutter SDK via Puro config (.puro.json).
///
/// Parses `.puro.json` for the `env` field. SDK at `$PURO_ROOT/envs/<env>/flutter/`
/// or `~/.puro/envs/<env>/flutter/`.
pub fn detect_puro(project_path: &Path) -> Result<Option<PathBuf>>
```

- Walk upward for `.puro.json`
- Parse as JSON: `{ "env": "stable" }`
- Resolve: check `PURO_ROOT` env var, fall back to `dirs::home_dir()/.puro/`
- Return `puro_root/envs/<env>/flutter/`

##### 4. asdf (`.tool-versions`)

```rust
/// Detect Flutter SDK via asdf config (.tool-versions).
///
/// Parses `.tool-versions` (line format: `tool version`).
/// SDK at `~/.asdf/installs/flutter/<version>/`.
pub fn detect_asdf(project_path: &Path) -> Result<Option<PathBuf>>
```

- Walk upward for `.tool-versions`
- Parse as plain text: find line starting with `flutter `, extract version
- Also check `ASDF_DATA_DIR` env var, fall back to `dirs::home_dir()/.asdf/`
- Return `asdf_root/installs/flutter/<version>/`

`.tool-versions` format (plain text):
```
flutter 3.19.0
ruby 3.2.0
```

##### 5. mise (`.mise.toml`)

```rust
/// Detect Flutter SDK via mise config (.mise.toml).
///
/// Parses `.mise.toml` (TOML) `[tools]` section for `flutter` key.
/// SDK at `~/.local/share/mise/installs/flutter/<version>/`.
pub fn detect_mise(project_path: &Path) -> Result<Option<PathBuf>>
```

- Walk upward for `.mise.toml`
- Parse as TOML: read `[tools]` table, get `flutter` value (can be string `"3.19.0"` or array)
- Also check `MISE_DATA_DIR` env var, fall back to `dirs::data_local_dir()/mise/` (platform-aware)
- Return `mise_root/installs/flutter/<version>/`

`.mise.toml` format:
```toml
[tools]
flutter = "3.19.0"
node = "20"
```

##### 6. proto (`.prototools`)

```rust
/// Detect Flutter SDK via proto config (.prototools).
///
/// Parses `.prototools` (TOML) for `flutter` key.
/// SDK at `~/.proto/tools/flutter/<version>/`.
pub fn detect_proto(project_path: &Path) -> Result<Option<PathBuf>>
```

- Walk upward for `.prototools`
- Parse as TOML: top-level key `flutter` (can be `"3.19.0"` or an inline table with `version` field)
- Also check `PROTO_HOME` env var, fall back to `dirs::home_dir()/.proto/`
- Return `proto_root/tools/flutter/<version>/`

`.prototools` format:
```toml
flutter = "3.19.0"
node = "20.0.0"
```

##### 7. flutter_wrapper (`flutterw` + `.flutter/`)

```rust
/// Detect Flutter SDK via flutter_wrapper (flutterw script + .flutter/ directory).
///
/// Checks for `flutterw` script at project root and `.flutter/` directory.
/// SDK at `<project_root>/.flutter/`.
pub fn detect_flutter_wrapper(project_path: &Path) -> Result<Option<PathBuf>>
```

- Check `project_path/flutterw` exists (no tree walk — flutter_wrapper is always at project root)
- Check `project_path/.flutter/` is a directory
- Return `project_path/.flutter/`

#### Shared FVM Cache Helper

```rust
/// Resolves the FVM cache directory path.
/// Priority: $FVM_CACHE_PATH > ~/fvm/versions/
fn resolve_fvm_cache() -> Option<PathBuf>
```

Used by both `detect_fvm_modern()` and `detect_fvm_legacy()`.

### Acceptance Criteria

1. Each of the 7 detection functions compiles and is publicly exported
2. `find_config_upward()` walks parent directories correctly, stopping at filesystem root
3. FVM modern: correctly parses `.fvmrc` JSON, resolves via `FVM_CACHE_PATH` env var with fallback
4. FVM legacy: resolves `.fvm/flutter_sdk` symlink via `canonicalize()`, falls back to config JSON
5. Puro: parses `.puro.json`, resolves via `PURO_ROOT` env var with fallback
6. asdf: parses `.tool-versions` line format, extracts flutter version
7. mise: parses `.mise.toml` TOML `[tools]` section
8. proto: parses `.prototools` TOML top-level key
9. flutter_wrapper: checks `flutterw` + `.flutter/` at project root only
10. All functions return `Ok(None)` when config file not found (not `Err`)
11. All functions log at `debug!` level
12. Each parser has tests with `tempfile::TempDir`

### Testing

Use `tempfile::TempDir` to create mock project structures. Each parser needs:
- **Happy path**: config file present with valid content → returns correct SDK path
- **Not found**: no config file → returns `Ok(None)`
- **Invalid content**: malformed JSON/TOML → returns `Ok(None)` (graceful degradation, log warning)
- **Parent directory**: config in ancestor dir → tree walk finds it

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_find_config_upward_in_parent() {
        let tmp = TempDir::new().unwrap();
        let parent = tmp.path();
        let child = parent.join("packages/my_app");
        fs::create_dir_all(&child).unwrap();
        fs::write(parent.join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();

        let found = find_config_upward(&child, ".fvmrc");
        assert_eq!(found, Some(parent.join(".fvmrc")));
    }

    #[test]
    fn test_find_config_upward_not_found() {
        let tmp = TempDir::new().unwrap();
        let found = find_config_upward(tmp.path(), ".nonexistent");
        assert!(found.is_none());
    }

    #[test]
    fn test_detect_fvm_modern_valid() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();

        // Create mock FVM cache
        let cache = project.join("fvm_cache/versions/3.19.0");
        fs::create_dir_all(&cache).unwrap();

        // Test with FVM_CACHE_PATH pointing to our mock cache
        std::env::set_var("FVM_CACHE_PATH", project.join("fvm_cache/versions"));
        let result = detect_fvm_modern(project).unwrap();
        assert_eq!(result, Some(cache));
        std::env::remove_var("FVM_CACHE_PATH");
    }

    #[test]
    fn test_detect_asdf_parses_tool_versions() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join(".tool-versions"), "flutter 3.19.0\nruby 3.2.0\n").unwrap();

        let result = detect_asdf(project).unwrap();
        // Result will be Some(~/.asdf/installs/flutter/3.19.0/) — existence not checked here
        assert!(result.is_some());
    }

    #[test]
    fn test_detect_mise_parses_toml() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join(".mise.toml"), "[tools]\nflutter = \"3.19.0\"\n").unwrap();

        let result = detect_mise(project).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_detect_puro_not_found() {
        let tmp = TempDir::new().unwrap();
        let result = detect_puro(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_flutter_wrapper_both_present() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join("flutterw"), "#!/bin/sh").unwrap();
        fs::create_dir(project.join(".flutter")).unwrap();

        let result = detect_flutter_wrapper(project).unwrap();
        assert_eq!(result, Some(project.join(".flutter")));
    }

    #[test]
    fn test_detect_flutter_wrapper_missing_flutterw() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::create_dir(project.join(".flutter")).unwrap();
        // No flutterw script

        let result = detect_flutter_wrapper(project).unwrap();
        assert!(result.is_none());
    }
}
```

### Notes

- **Graceful degradation**: If a config file exists but is malformed, log a `warn!` and return `Ok(None)` — do not fail the entire detection chain. Move on to the next strategy.
- **Env var fallbacks**: Each tool has a primary env var (`FVM_CACHE_PATH`, `PURO_ROOT`, `ASDF_DATA_DIR`, `MISE_DATA_DIR`, `PROTO_HOME`) and a default home directory fallback. Check env var first.
- **No CLI invocations**: All parsing is file-based. We never run `fvm`, `puro`, `asdf`, etc.
- **Tests that set env vars**: Be careful with `std::env::set_var` in tests — it's not thread-safe. Consider using a helper that passes env values as parameters, or use `#[serial]` if needed.
- **`.tool-versions` edge cases**: Lines may have comments (`#`), multiple versions (`flutter 3.19.0 3.16.0`), or use `ref:` prefixes for git refs.

---

## Completion Summary

**Status:** Not Started
