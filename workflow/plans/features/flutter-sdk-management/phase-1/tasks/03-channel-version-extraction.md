## Task: Channel and Version Extraction

**Objective**: Implement channel detection (stable/beta/main) from a Flutter SDK's git state, and version string parsing from the VERSION file. This metadata is used by `FlutterSdk` to provide version and channel info for display in the UI and logging.

**Depends on**: 01-core-types

### Scope

- `crates/fdemon-daemon/src/flutter_sdk/channel.rs`: **NEW** — Channel detection and version helpers
- `crates/fdemon-daemon/src/flutter_sdk/mod.rs`: Add `mod channel` and re-exports

### Details

#### Channel Detection

Flutter SDKs are git repos. The current channel corresponds to the git branch:
- `stable` → stable channel
- `beta` → beta channel
- `master` or `main` → main/dev channel

```rust
//! # Flutter SDK Channel Detection
//!
//! Extracts channel and version information from a Flutter SDK installation
//! by reading its git state and VERSION file.

use std::path::Path;

/// Known Flutter release channels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlutterChannel {
    Stable,
    Beta,
    Main,
    /// Unknown branch name (e.g., custom fork or detached HEAD)
    Unknown(String),
}

/// Detect the Flutter channel from the SDK's git state.
///
/// Reads `.git/HEAD` to determine the current branch:
/// - `ref: refs/heads/stable` → Stable
/// - `ref: refs/heads/beta` → Beta
/// - `ref: refs/heads/master` or `main` → Main
/// - Detached HEAD (hash) → Unknown
///
/// Returns `None` if the SDK has no `.git` directory (e.g., archive install).
pub fn detect_channel(sdk_root: &Path) -> Option<FlutterChannel>
```

**Implementation approach — read `.git/HEAD` directly (no git CLI):**

1. Read `<sdk_root>/.git/HEAD` as a string
2. If it starts with `ref: refs/heads/`, extract the branch name
3. Map branch name → `FlutterChannel` variant
4. If it's a raw commit hash (detached HEAD), return `FlutterChannel::Unknown` with the short hash
5. If `.git/HEAD` doesn't exist, return `None`

This avoids any dependency on `git` being installed.

**Edge case — `.git` is a file (gitdir reference):**
In some setups (submodules, worktrees), `.git` is a file containing `gitdir: /path/to/actual/.git`. Handle this by:
1. If `.git` is a file, read its content
2. If it starts with `gitdir:`, follow the path
3. Read `HEAD` from the resolved git directory

#### Version String Parsing

The `read_version_file()` function is already defined in task 01. This task adds:

```rust
/// Parse a Flutter version string into its components.
///
/// Examples:
/// - "3.19.0" → (3, 19, 0, None)
/// - "3.22.0-beta.1" → (3, 22, 0, Some("beta.1"))
/// - "3.24.0-0.0.pre" → (3, 24, 0, Some("0.0.pre"))
pub struct FlutterVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub pre_release: Option<String>,
}

impl FlutterVersion {
    /// Parse a version string. Returns None for unparseable strings.
    pub fn parse(version_str: &str) -> Option<Self>
}

impl std::fmt::Display for FlutterVersion {
    // Renders as "3.19.0" or "3.22.0-beta.1"
}
```

#### Dart SDK Version

```rust
/// Read the Dart SDK version bundled with this Flutter SDK.
///
/// Reads `<sdk_root>/bin/cache/dart-sdk/version`.
pub fn read_dart_version(sdk_root: &Path) -> Option<String>
```

### Acceptance Criteria

1. `detect_channel()` correctly identifies `stable`, `beta`, `master`/`main` from `.git/HEAD`
2. `detect_channel()` returns `None` for SDKs without a `.git` directory
3. `detect_channel()` handles detached HEAD (returns `Unknown` with short hash)
4. `detect_channel()` follows `gitdir:` references in `.git` file
5. `FlutterVersion::parse()` parses standard Flutter version strings
6. `FlutterVersion::parse()` handles pre-release suffixes
7. `FlutterVersion::parse()` returns `None` for invalid strings
8. `read_dart_version()` reads the Dart SDK version file
9. `FlutterChannel` and `FlutterVersion` implement `Display`

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn create_git_dir(root: &Path, head_content: &str) {
        let git_dir = root.join(".git");
        fs::create_dir_all(&git_dir).unwrap();
        fs::write(git_dir.join("HEAD"), head_content).unwrap();
    }

    #[test]
    fn test_detect_channel_stable() {
        let tmp = TempDir::new().unwrap();
        create_git_dir(tmp.path(), "ref: refs/heads/stable\n");
        assert_eq!(detect_channel(tmp.path()), Some(FlutterChannel::Stable));
    }

    #[test]
    fn test_detect_channel_beta() {
        let tmp = TempDir::new().unwrap();
        create_git_dir(tmp.path(), "ref: refs/heads/beta\n");
        assert_eq!(detect_channel(tmp.path()), Some(FlutterChannel::Beta));
    }

    #[test]
    fn test_detect_channel_master() {
        let tmp = TempDir::new().unwrap();
        create_git_dir(tmp.path(), "ref: refs/heads/master\n");
        assert_eq!(detect_channel(tmp.path()), Some(FlutterChannel::Main));
    }

    #[test]
    fn test_detect_channel_main() {
        let tmp = TempDir::new().unwrap();
        create_git_dir(tmp.path(), "ref: refs/heads/main\n");
        assert_eq!(detect_channel(tmp.path()), Some(FlutterChannel::Main));
    }

    #[test]
    fn test_detect_channel_detached_head() {
        let tmp = TempDir::new().unwrap();
        create_git_dir(tmp.path(), "abc123def456789\n");
        let channel = detect_channel(tmp.path());
        assert!(matches!(channel, Some(FlutterChannel::Unknown(_))));
    }

    #[test]
    fn test_detect_channel_no_git_dir() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(detect_channel(tmp.path()), None);
    }

    #[test]
    fn test_detect_channel_gitdir_reference() {
        let tmp = TempDir::new().unwrap();
        let actual_git = tmp.path().join("actual_git");
        fs::create_dir_all(&actual_git).unwrap();
        fs::write(actual_git.join("HEAD"), "ref: refs/heads/stable\n").unwrap();

        // .git is a file pointing to actual_git
        fs::write(tmp.path().join(".git"),
            format!("gitdir: {}", actual_git.display())).unwrap();

        assert_eq!(detect_channel(tmp.path()), Some(FlutterChannel::Stable));
    }

    #[test]
    fn test_flutter_version_parse_stable() {
        let v = FlutterVersion::parse("3.19.0").unwrap();
        assert_eq!((v.major, v.minor, v.patch), (3, 19, 0));
        assert_eq!(v.pre_release, None);
    }

    #[test]
    fn test_flutter_version_parse_pre_release() {
        let v = FlutterVersion::parse("3.22.0-beta.1").unwrap();
        assert_eq!((v.major, v.minor, v.patch), (3, 22, 0));
        assert_eq!(v.pre_release.as_deref(), Some("beta.1"));
    }

    #[test]
    fn test_flutter_version_parse_invalid() {
        assert!(FlutterVersion::parse("not-a-version").is_none());
        assert!(FlutterVersion::parse("").is_none());
    }

    #[test]
    fn test_flutter_version_display() {
        let v = FlutterVersion::parse("3.19.0").unwrap();
        assert_eq!(v.to_string(), "3.19.0");

        let v = FlutterVersion::parse("3.22.0-beta.1").unwrap();
        assert_eq!(v.to_string(), "3.22.0-beta.1");
    }

    #[test]
    fn test_read_dart_version() {
        let tmp = TempDir::new().unwrap();
        let dart_dir = tmp.path().join("bin/cache/dart-sdk");
        fs::create_dir_all(&dart_dir).unwrap();
        fs::write(dart_dir.join("version"), "3.3.0\n").unwrap();

        let version = read_dart_version(tmp.path());
        assert_eq!(version.as_deref(), Some("3.3.0"));
    }
}
```

### Notes

- **No git dependency**: Read `.git/HEAD` directly. This is stable across git versions and avoids requiring git to be installed.
- **gitdir references**: Flutter SDKs managed by version managers may have `.git` as a file (not a directory). Handle this gracefully.
- **Detached HEAD**: When Flutter is checked out to a specific tag (e.g., FVM does `git checkout 3.19.0`), HEAD contains a raw commit hash. This is expected for version-managed SDKs.
- **Channel is optional**: `FlutterSdk.channel` is `Option<String>` in the types — archive-based installations won't have git state.
- **FlutterVersion is informational**: It's used for display and comparison, not for critical logic. If parsing fails, the raw version string from the VERSION file is still available in `FlutterSdk.version`.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/error.rs` | Added `FlutterSdkInvalid { path, reason }` variant, `flutter_sdk_invalid()` constructor, and `FlutterSdkInvalid` to `is_fatal()` |
| `crates/fdemon-daemon/Cargo.toml` | Added `toml.workspace = true` and `dirs.workspace = true` dependencies |
| `crates/fdemon-daemon/src/flutter_sdk/types.rs` | NEW — `SdkSource`, `FlutterExecutable`, `FlutterSdk`, `validate_sdk_path()`, `read_version_file()`, `Display for SdkSource` |
| `crates/fdemon-daemon/src/flutter_sdk/channel.rs` | NEW — `FlutterChannel`, `detect_channel()`, `FlutterVersion`, `read_dart_version()` |
| `crates/fdemon-daemon/src/flutter_sdk/mod.rs` | NEW — Module root with re-exports from `types` and `channel` |
| `crates/fdemon-daemon/src/lib.rs` | Added `pub mod flutter_sdk`, re-exports for all flutter_sdk public items, updated doc comment |

### Notable Decisions/Tradeoffs

1. **Also implemented Task 01 (core types)**: The `flutter_sdk/` module did not exist in the worktree, so `types.rs` and `mod.rs` were created as part of this task to satisfy the dependency. The task 03 channel module is the primary deliverable.
2. **gitdir relative path support**: The `resolve_git_dir()` helper resolves relative `gitdir:` references against the parent directory of the `.git` file, matching git's own behavior.
3. **Short hash for detached HEAD**: When HEAD is a commit hash, the display uses the first 7 characters (standard git short hash convention) rather than the full 40-char hash.
4. **`read_dart_version()` returns None for empty content**: An empty or whitespace-only version file returns `None` rather than an empty string, preventing downstream consumers from treating an empty string as a valid version.

### Testing Performed

- `cargo check -p fdemon-daemon` - Passed
- `cargo test -p fdemon-daemon -- channel` - Passed (27 tests)
- `cargo test -p fdemon-daemon -- flutter_sdk` - Passed (33 tests: 20 channel + 13 types)
- `cargo test -p fdemon-daemon` - Passed (613 tests, 0 failures)
- `cargo clippy -p fdemon-daemon -- -D warnings` - Passed (no warnings)
- `cargo check --workspace` - Passed (all crates compile)
- `cargo fmt --all` - Applied (no formatting issues)

### Risks/Limitations

1. **Task 01 completion summary not updated**: The task 01 file (`01-core-types.md`) still shows "Not Started" since this agent was only assigned task 03. The task 01 implementation was done here as a prerequisite.
2. **No `dirs` or `toml` usage in channel.rs**: These were added to `Cargo.toml` per task 01 spec (needed by the future locator/version-manager tasks). They are not used in the channel module itself — clippy `unused` warnings are suppressed because the deps are used in tests indirectly via tempfile.
