//! # Flutter SDK Channel Detection
//!
//! Extracts channel and version information from a Flutter SDK installation
//! by reading its git state and VERSION file.

use std::path::Path;

/// Known Flutter release channels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlutterChannel {
    /// The stable release channel
    Stable,
    /// The beta release channel
    Beta,
    /// The main/master/dev development channel
    Main,
    /// Unknown branch name (e.g., custom fork, detached HEAD, or non-standard branch)
    Unknown(String),
}

impl std::fmt::Display for FlutterChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stable => write!(f, "stable"),
            Self::Beta => write!(f, "beta"),
            Self::Main => write!(f, "main"),
            Self::Unknown(s) => write!(f, "{s}"),
        }
    }
}

/// Detect the Flutter channel from the SDK's git state.
///
/// Reads `.git/HEAD` to determine the current branch:
/// - `ref: refs/heads/stable` → [`FlutterChannel::Stable`]
/// - `ref: refs/heads/beta` → [`FlutterChannel::Beta`]
/// - `ref: refs/heads/master` or `ref: refs/heads/main` → [`FlutterChannel::Main`]
/// - Detached HEAD (raw commit hash) → [`FlutterChannel::Unknown`] with short hash
/// - Any other branch name → [`FlutterChannel::Unknown`] with the branch name
///
/// Returns `None` if the SDK has no `.git` directory (e.g., archive install).
///
/// This function reads git state directly without invoking the `git` CLI.
pub fn detect_channel(sdk_root: &Path) -> Option<FlutterChannel> {
    let git_path = sdk_root.join(".git");

    // Resolve the actual git directory — handles both .git directories
    // and .git files (gitdir references used by submodules and worktrees).
    let git_dir = resolve_git_dir(&git_path)?;
    let head_path = git_dir.join("HEAD");

    let content = std::fs::read_to_string(&head_path).ok()?;
    let content = content.trim();

    // Symbolic ref: "ref: refs/heads/<branch>"
    if let Some(branch_ref) = content.strip_prefix("ref: refs/heads/") {
        let branch = branch_ref.trim();
        return Some(branch_to_channel(branch));
    }

    // Detached HEAD: raw commit hash (40 hex chars, or abbreviated)
    if !content.is_empty() {
        // Use first 7 characters as short hash for display
        let short = &content[..content.len().min(7)];
        return Some(FlutterChannel::Unknown(short.to_string()));
    }

    None
}

/// Map a git branch name to a [`FlutterChannel`] variant.
fn branch_to_channel(branch: &str) -> FlutterChannel {
    match branch {
        "stable" => FlutterChannel::Stable,
        "beta" => FlutterChannel::Beta,
        "master" | "main" => FlutterChannel::Main,
        other => FlutterChannel::Unknown(other.to_string()),
    }
}

/// Resolve the actual git directory from a `.git` path.
///
/// In standard repos, `.git` is a directory and is returned as-is.
/// In worktrees and submodules, `.git` is a file containing
/// `gitdir: /path/to/actual/.git`. This function follows the reference.
///
/// Returns `None` if:
/// - The `.git` path does not exist
/// - `.git` is a file but cannot be read or parsed as a gitdir reference
fn resolve_git_dir(git_path: &Path) -> Option<std::path::PathBuf> {
    if git_path.is_dir() {
        return Some(git_path.to_path_buf());
    }

    if git_path.is_file() {
        let content = std::fs::read_to_string(git_path).ok()?;
        let content = content.trim();
        if let Some(gitdir_path) = content.strip_prefix("gitdir:") {
            let resolved = gitdir_path.trim();
            let dir = std::path::Path::new(resolved);

            // Resolve relative paths relative to the file's parent directory
            if dir.is_absolute() {
                return Some(dir.to_path_buf());
            } else {
                let parent = git_path.parent()?;
                return Some(parent.join(dir));
            }
        }
    }

    None
}

/// Parse a Flutter version string into its components.
///
/// # Examples
///
/// ```text
/// "3.19.0"           → major=3, minor=19, patch=0, pre_release=None
/// "3.22.0-beta.1"    → major=3, minor=22, patch=0, pre_release=Some("beta.1")
/// "3.24.0-0.0.pre"   → major=3, minor=24, patch=0, pre_release=Some("0.0.pre")
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlutterVersion {
    /// Major version component
    pub major: u32,
    /// Minor version component
    pub minor: u32,
    /// Patch version component
    pub patch: u32,
    /// Pre-release suffix (e.g., `"beta.1"`, `"0.0.pre"`)
    pub pre_release: Option<String>,
}

impl FlutterVersion {
    /// Parse a Flutter version string.
    ///
    /// Returns `None` for unparseable strings (empty, non-numeric components,
    /// or missing required version parts).
    pub fn parse(version_str: &str) -> Option<Self> {
        if version_str.is_empty() {
            return None;
        }

        // Split on first '-' to separate version from pre-release
        let (version_part, pre_release) = match version_str.split_once('-') {
            Some((v, pre)) => (v, Some(pre.to_string())),
            None => (version_str, None),
        };

        let parts: Vec<&str> = version_part.split('.').collect();
        if parts.len() < 3 {
            return None;
        }

        let major = parts[0].parse::<u32>().ok()?;
        let minor = parts[1].parse::<u32>().ok()?;
        let patch = parts[2].parse::<u32>().ok()?;

        Some(Self {
            major,
            minor,
            patch,
            pre_release,
        })
    }
}

impl std::fmt::Display for FlutterVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.pre_release {
            write!(f, "-{pre}")?;
        }
        Ok(())
    }
}

/// Read the Dart SDK version bundled with this Flutter SDK.
///
/// Reads `<sdk_root>/bin/cache/dart-sdk/version`.
///
/// Returns `None` if the file is absent or unreadable (e.g., the Dart SDK
/// cache has not been populated yet).
pub fn read_dart_version(sdk_root: &Path) -> Option<String> {
    let version_path = sdk_root.join("bin/cache/dart-sdk/version");
    let content = std::fs::read_to_string(&version_path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_git_dir(root: &Path, head_content: &str) {
        let git_dir = root.join(".git");
        fs::create_dir_all(&git_dir).unwrap();
        fs::write(git_dir.join("HEAD"), head_content).unwrap();
    }

    // ─────────────────────────────────────────────────────────────
    // detect_channel tests
    // ─────────────────────────────────────────────────────────────

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
    fn test_detect_channel_custom_branch() {
        let tmp = TempDir::new().unwrap();
        create_git_dir(tmp.path(), "ref: refs/heads/my-feature\n");
        let channel = detect_channel(tmp.path());
        assert_eq!(
            channel,
            Some(FlutterChannel::Unknown("my-feature".to_string()))
        );
    }

    #[test]
    fn test_detect_channel_detached_head() {
        let tmp = TempDir::new().unwrap();
        create_git_dir(tmp.path(), "abc123def456789\n");
        let channel = detect_channel(tmp.path());
        assert!(matches!(channel, Some(FlutterChannel::Unknown(_))));
        // Short hash should be at most 7 chars
        if let Some(FlutterChannel::Unknown(hash)) = channel {
            assert!(hash.len() <= 7);
        }
    }

    #[test]
    fn test_detect_channel_no_git_dir() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(detect_channel(tmp.path()), None);
    }

    #[test]
    fn test_detect_channel_gitdir_reference_absolute() {
        let tmp = TempDir::new().unwrap();
        let actual_git = tmp.path().join("actual_git");
        fs::create_dir_all(&actual_git).unwrap();
        fs::write(actual_git.join("HEAD"), "ref: refs/heads/stable\n").unwrap();

        // .git is a file pointing to actual_git with absolute path
        fs::write(
            tmp.path().join(".git"),
            format!("gitdir: {}", actual_git.display()),
        )
        .unwrap();

        assert_eq!(detect_channel(tmp.path()), Some(FlutterChannel::Stable));
    }

    #[test]
    fn test_detect_channel_gitdir_reference_relative() {
        let tmp = TempDir::new().unwrap();
        let actual_git = tmp.path().join("actual_git");
        fs::create_dir_all(&actual_git).unwrap();
        fs::write(actual_git.join("HEAD"), "ref: refs/heads/beta\n").unwrap();

        // .git is a file pointing to actual_git with relative path
        fs::write(tmp.path().join(".git"), "gitdir: actual_git").unwrap();

        assert_eq!(detect_channel(tmp.path()), Some(FlutterChannel::Beta));
    }

    // ─────────────────────────────────────────────────────────────
    // FlutterVersion tests
    // ─────────────────────────────────────────────────────────────

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
    fn test_flutter_version_parse_dev_pre_release() {
        let v = FlutterVersion::parse("3.24.0-0.0.pre").unwrap();
        assert_eq!((v.major, v.minor, v.patch), (3, 24, 0));
        assert_eq!(v.pre_release.as_deref(), Some("0.0.pre"));
    }

    #[test]
    fn test_flutter_version_parse_large_version() {
        let v = FlutterVersion::parse("10.5.123").unwrap();
        assert_eq!((v.major, v.minor, v.patch), (10, 5, 123));
        assert_eq!(v.pre_release, None);
    }

    #[test]
    fn test_flutter_version_parse_invalid_not_a_version() {
        assert!(FlutterVersion::parse("not-a-version").is_none());
    }

    #[test]
    fn test_flutter_version_parse_invalid_empty() {
        assert!(FlutterVersion::parse("").is_none());
    }

    #[test]
    fn test_flutter_version_parse_invalid_partial() {
        assert!(FlutterVersion::parse("3.19").is_none());
    }

    #[test]
    fn test_flutter_version_parse_invalid_alpha() {
        assert!(FlutterVersion::parse("3.x.0").is_none());
    }

    #[test]
    fn test_flutter_version_display_stable() {
        let v = FlutterVersion::parse("3.19.0").unwrap();
        assert_eq!(v.to_string(), "3.19.0");
    }

    #[test]
    fn test_flutter_version_display_pre_release() {
        let v = FlutterVersion::parse("3.22.0-beta.1").unwrap();
        assert_eq!(v.to_string(), "3.22.0-beta.1");
    }

    #[test]
    fn test_flutter_version_display_dev() {
        let v = FlutterVersion::parse("3.24.0-0.0.pre").unwrap();
        assert_eq!(v.to_string(), "3.24.0-0.0.pre");
    }

    // ─────────────────────────────────────────────────────────────
    // FlutterChannel Display tests
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_flutter_channel_display() {
        assert_eq!(FlutterChannel::Stable.to_string(), "stable");
        assert_eq!(FlutterChannel::Beta.to_string(), "beta");
        assert_eq!(FlutterChannel::Main.to_string(), "main");
        assert_eq!(
            FlutterChannel::Unknown("custom".to_string()).to_string(),
            "custom"
        );
    }

    // ─────────────────────────────────────────────────────────────
    // read_dart_version tests
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_read_dart_version() {
        let tmp = TempDir::new().unwrap();
        let dart_dir = tmp.path().join("bin/cache/dart-sdk");
        fs::create_dir_all(&dart_dir).unwrap();
        fs::write(dart_dir.join("version"), "3.3.0\n").unwrap();

        let version = read_dart_version(tmp.path());
        assert_eq!(version.as_deref(), Some("3.3.0"));
    }

    #[test]
    fn test_read_dart_version_missing() {
        let tmp = TempDir::new().unwrap();
        let version = read_dart_version(tmp.path());
        assert_eq!(version, None);
    }

    #[test]
    fn test_read_dart_version_with_trailing_whitespace() {
        let tmp = TempDir::new().unwrap();
        let dart_dir = tmp.path().join("bin/cache/dart-sdk");
        fs::create_dir_all(&dart_dir).unwrap();
        fs::write(dart_dir.join("version"), "  3.3.0  \n").unwrap();

        let version = read_dart_version(tmp.path());
        assert_eq!(version.as_deref(), Some("3.3.0"));
    }
}
