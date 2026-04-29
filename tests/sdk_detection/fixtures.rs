//! # Test Fixtures
//!
//! Provides [`MockSdkBuilder`] for constructing valid mock Flutter SDK directory
//! structures and per-version-manager layout creator functions that produce the
//! exact filesystem shapes that each `detect_*` function expects to find.
//!
//! All layout creators return the SDK root path that was created so callers can
//! compare it against [`FlutterSdk::root`] in assertions.

use std::fs;
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────────────────
// MockSdkBuilder
// ─────────────────────────────────────────────────────────────────────────────

/// Builds a valid mock Flutter SDK directory structure inside an existing directory.
///
/// The minimum valid SDK (required by [`validate_sdk_path`]) contains:
/// - `bin/flutter`  (executable; chmod 755 on Unix)
/// - `VERSION`      (version string)
///
/// Optional additions via builder methods:
/// - `bin/cache/dart-sdk/` — Dart SDK cache directory
/// - `.git/HEAD`            — git HEAD file encoding the channel
/// - `bin/flutter.bat`      — Windows batch file (for Windows-path tests)
///
/// # Example
///
/// ```rust,no_run
/// use tempfile::TempDir;
///
/// let tmp = TempDir::new().unwrap();
/// let sdk_root = MockSdkBuilder::new(tmp.path(), "3.22.0")
///     .with_dart_sdk()
///     .with_channel("stable")
///     .build();
/// ```
pub struct MockSdkBuilder {
    root: PathBuf,
    version: String,
    channel: Option<String>,
    include_dart_sdk: bool,
    create_bat_file: bool,
}

impl MockSdkBuilder {
    /// Create a new builder that will place the mock SDK at `root`.
    ///
    /// `root` need not exist yet; `build()` will create it along with all
    /// required sub-directories.
    pub fn new(root: &Path, version: &str) -> Self {
        Self {
            root: root.to_path_buf(),
            version: version.to_string(),
            channel: None,
            include_dart_sdk: false,
            create_bat_file: false,
        }
    }

    /// Write a `.git/HEAD` file encoding `ref: refs/heads/<channel>` so that
    /// [`detect_channel`] can read the channel name from git state.
    pub fn with_channel(mut self, channel: &str) -> Self {
        self.channel = Some(channel.to_string());
        self
    }

    /// Create the `bin/cache/dart-sdk/` directory (simulates a populated SDK
    /// cache that exists after `flutter run` or `flutter doctor`).
    pub fn with_dart_sdk(mut self) -> Self {
        self.include_dart_sdk = true;
        self
    }

    /// Create a `bin/flutter.bat` file alongside `bin/flutter`.
    ///
    /// Useful for tests that exercise Windows-path code paths without running
    /// on Windows.
    pub fn with_bat_file(mut self) -> Self {
        self.create_bat_file = true;
        self
    }

    /// Materialise the SDK directory tree and return the root path.
    ///
    /// On Unix the `bin/flutter` file is made executable (mode 0o755).
    pub fn build(self) -> PathBuf {
        // bin/ directory is always required
        fs::create_dir_all(self.root.join("bin")).unwrap();

        // bin/flutter — the primary executable
        let flutter_bin = self.root.join("bin").join("flutter");
        fs::write(&flutter_bin, "#!/bin/sh\n# mock flutter binary\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&flutter_bin, fs::Permissions::from_mode(0o755)).unwrap();
        }

        // VERSION file
        fs::write(self.root.join("VERSION"), format!("{}\n", self.version)).unwrap();

        // Optional: bin/cache/dart-sdk/
        if self.include_dart_sdk {
            fs::create_dir_all(self.root.join("bin").join("cache").join("dart-sdk")).unwrap();
        }

        // Optional: .git/HEAD
        if let Some(channel) = &self.channel {
            fs::create_dir_all(self.root.join(".git")).unwrap();
            fs::write(
                self.root.join(".git").join("HEAD"),
                format!("ref: refs/heads/{channel}\n"),
            )
            .unwrap();
        }

        // bin/flutter.bat is required by validate_sdk_path on Windows; opt-in on
        // other platforms via .with_bat_file() for tests exercising Windows-path
        // code paths from a Unix host.
        let need_bat = cfg!(target_os = "windows") || self.create_bat_file;
        if need_bat {
            fs::write(
                self.root.join("bin").join("flutter.bat"),
                "@echo off\nrem mock flutter.bat\n",
            )
            .unwrap();
        }

        self.root
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EnvGuard
// ─────────────────────────────────────────────────────────────────────────────

/// RAII guard that sets an environment variable and restores the previous value
/// (or removes it) when dropped.
///
/// Because [`std::env::set_var`] is process-global, tests that manipulate
/// environment variables **must** be annotated with `#[serial]` from the
/// `serial_test` crate to prevent data races.
///
/// # Example
///
/// ```rust,no_run
/// use serial_test::serial;
///
/// #[test]
/// #[serial]
/// fn my_test() {
///     let _guard = EnvGuard::set("FVM_CACHE_PATH", "/tmp/test-fvm");
///     // env var is set here
/// }
/// // env var is restored/removed here when _guard drops
/// ```
pub struct EnvGuard {
    key: String,
    original: Option<String>,
}

impl EnvGuard {
    /// Set `key` to `value`, saving the current value for restoration on drop.
    ///
    /// If `key` was not previously set, it will be removed (not set to empty)
    /// on drop.
    pub fn set(key: &str, value: &str) -> Self {
        let original = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self {
            key: key.to_string(),
            original,
        }
    }

    /// Remove `key`, saving the current value for restoration on drop.
    ///
    /// If `key` was not set, drop is a no-op.
    pub fn remove(key: &str) -> Self {
        let original = std::env::var(key).ok();
        std::env::remove_var(key);
        Self {
            key: key.to_string(),
            original,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.original {
            Some(prev) => std::env::set_var(&self.key, prev),
            None => std::env::remove_var(&self.key),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Flutter Project Helper
// ─────────────────────────────────────────────────────────────────────────────

/// Create a minimal Flutter project directory containing only a `pubspec.yaml`.
///
/// This is sufficient for the SDK detection functions, which only need the
/// project directory to exist when walking upward for config files.
pub fn create_flutter_project(project_dir: &Path, name: &str) {
    fs::create_dir_all(project_dir).unwrap();
    fs::write(
        project_dir.join("pubspec.yaml"),
        format!(
            "name: {name}\n\
             dependencies:\n\
             \x20 flutter:\n\
             \x20   sdk: flutter\n"
        ),
    )
    .unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// FVM Modern Layout (.fvmrc)
// ─────────────────────────────────────────────────────────────────────────────

/// Creates an FVM v3 (modern) layout in `project_dir` and builds a mock SDK
/// under `fvm_cache_dir/<version>/`.
///
/// Files created:
/// - `<project_dir>/.fvmrc`  — `{"flutter": "<version>"}`
/// - `<fvm_cache_dir>/<version>/bin/flutter`  (mock SDK)
/// - `<fvm_cache_dir>/<version>/VERSION`
///
/// The `fvm_cache_dir` parameter stands in for `~/fvm/versions/`.  In tests
/// point `FVM_CACHE_PATH` at this directory so `detect_fvm_modern()` resolves
/// the path correctly.
///
/// Returns the SDK root: `<fvm_cache_dir>/<version>/`.
pub fn create_fvm_layout(project_dir: &Path, fvm_cache_dir: &Path, version: &str) -> PathBuf {
    // Write .fvmrc
    fs::write(
        project_dir.join(".fvmrc"),
        format!(r#"{{"flutter":"{version}"}}"#),
    )
    .unwrap();

    // Build mock SDK at <fvm_cache_dir>/<version>/
    let sdk_root = fvm_cache_dir.join(version);
    MockSdkBuilder::new(&sdk_root, version).build()
}

// ─────────────────────────────────────────────────────────────────────────────
// FVM Legacy Layout (.fvm/fvm_config.json + symlink)
// ─────────────────────────────────────────────────────────────────────────────

/// Creates an FVM v2 (legacy) layout in `project_dir`.
///
/// Files created:
/// - `<project_dir>/.fvm/fvm_config.json`  — `{"flutterSdkVersion": "<version>"}`
/// - `<project_dir>/.fvm/flutter_sdk`       — symlink → `<fvm_cache_dir>/<version>/`
/// - `<fvm_cache_dir>/<version>/bin/flutter`  (mock SDK)
/// - `<fvm_cache_dir>/<version>/VERSION`
///
/// The `fvm_cache_dir` parameter stands in for `~/fvm/versions/`.  In tests
/// point `FVM_CACHE_PATH` at this directory.
///
/// Returns the canonical SDK root: `<fvm_cache_dir>/<version>/`.
pub fn create_fvm_legacy_layout(
    project_dir: &Path,
    fvm_cache_dir: &Path,
    version: &str,
) -> PathBuf {
    let fvm_dir = project_dir.join(".fvm");
    fs::create_dir_all(&fvm_dir).unwrap();

    // fvm_config.json
    fs::write(
        fvm_dir.join("fvm_config.json"),
        format!(r#"{{"flutterSdkVersion":"{version}"}}"#),
    )
    .unwrap();

    // Build mock SDK at <fvm_cache_dir>/<version>/
    let sdk_root = fvm_cache_dir.join(version);
    MockSdkBuilder::new(&sdk_root, version).build();

    // Create .fvm/flutter_sdk symlink → <sdk_root>
    let symlink_path = fvm_dir.join("flutter_sdk");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&sdk_root, &symlink_path).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&sdk_root, &symlink_path).unwrap();

    sdk_root
}

// ─────────────────────────────────────────────────────────────────────────────
// Puro Layout (.puro.json)
// ─────────────────────────────────────────────────────────────────────────────

/// Creates a Puro layout in `project_dir`.
///
/// Files created:
/// - `<project_dir>/.puro.json`                         — `{"env": "<env_name>"}`
/// - `<puro_root>/envs/<env_name>/flutter/bin/flutter`  (mock SDK)
/// - `<puro_root>/envs/<env_name>/flutter/VERSION`
///
/// The `puro_root` parameter stands in for `~/.puro/`.  In tests point
/// `PURO_ROOT` at this directory.
///
/// Returns the SDK root: `<puro_root>/envs/<env_name>/flutter/`.
pub fn create_puro_layout(project_dir: &Path, puro_root: &Path, env_name: &str) -> PathBuf {
    // Write .puro.json
    fs::write(
        project_dir.join(".puro.json"),
        format!(r#"{{"env":"{env_name}"}}"#),
    )
    .unwrap();

    // Build mock SDK at <puro_root>/envs/<env_name>/flutter/
    let sdk_root = puro_root.join("envs").join(env_name).join("flutter");
    MockSdkBuilder::new(&sdk_root, "3.22.0").build()
}

// ─────────────────────────────────────────────────────────────────────────────
// asdf Layout (.tool-versions)
// ─────────────────────────────────────────────────────────────────────────────

/// Creates an asdf layout in `project_dir`.
///
/// Files created:
/// - `<project_dir>/.tool-versions`                               — `flutter <version>`
/// - `<asdf_data_dir>/installs/flutter/<version>/bin/flutter`     (mock SDK)
/// - `<asdf_data_dir>/installs/flutter/<version>/VERSION`
///
/// The `asdf_data_dir` parameter stands in for `~/.asdf/`.  In tests point
/// `ASDF_DATA_DIR` at this directory.
///
/// Returns the SDK root: `<asdf_data_dir>/installs/flutter/<version>/`.
pub fn create_asdf_layout(project_dir: &Path, asdf_data_dir: &Path, version: &str) -> PathBuf {
    // Write .tool-versions
    fs::write(
        project_dir.join(".tool-versions"),
        format!("flutter {version}\n"),
    )
    .unwrap();

    // Build mock SDK at <asdf_data_dir>/installs/flutter/<version>/
    let sdk_root = asdf_data_dir.join("installs").join("flutter").join(version);
    MockSdkBuilder::new(&sdk_root, version).build()
}

// ─────────────────────────────────────────────────────────────────────────────
// mise Layout (.mise.toml)
// ─────────────────────────────────────────────────────────────────────────────

/// Creates a mise layout in `project_dir`.
///
/// Files created:
/// - `<project_dir>/.mise.toml`                                        — `[tools]\nflutter = "<version>"`
/// - `<mise_data_dir>/installs/flutter/<version>/bin/flutter`          (mock SDK)
/// - `<mise_data_dir>/installs/flutter/<version>/VERSION`
///
/// The `mise_data_dir` parameter stands in for `~/.local/share/mise/`.
/// In tests point `MISE_DATA_DIR` at this directory.
///
/// Returns the SDK root: `<mise_data_dir>/installs/flutter/<version>/`.
pub fn create_mise_layout(project_dir: &Path, mise_data_dir: &Path, version: &str) -> PathBuf {
    // Write .mise.toml
    fs::write(
        project_dir.join(".mise.toml"),
        format!("[tools]\nflutter = \"{version}\"\n"),
    )
    .unwrap();

    // Build mock SDK at <mise_data_dir>/installs/flutter/<version>/
    let sdk_root = mise_data_dir.join("installs").join("flutter").join(version);
    MockSdkBuilder::new(&sdk_root, version).build()
}

// ─────────────────────────────────────────────────────────────────────────────
// proto Layout (.prototools)
// ─────────────────────────────────────────────────────────────────────────────

/// Creates a proto layout in `project_dir`.
///
/// Files created:
/// - `<project_dir>/.prototools`                                     — `flutter = "<version>"`
/// - `<proto_home>/tools/flutter/<version>/bin/flutter`              (mock SDK)
/// - `<proto_home>/tools/flutter/<version>/VERSION`
///
/// The `proto_home` parameter stands in for `~/.proto/`.  In tests point
/// `PROTO_HOME` at this directory.
///
/// Returns the SDK root: `<proto_home>/tools/flutter/<version>/`.
pub fn create_proto_layout(project_dir: &Path, proto_home: &Path, version: &str) -> PathBuf {
    // Write .prototools
    fs::write(
        project_dir.join(".prototools"),
        format!("flutter = \"{version}\"\n"),
    )
    .unwrap();

    // Build mock SDK at <proto_home>/tools/flutter/<version>/
    let sdk_root = proto_home.join("tools").join("flutter").join(version);
    MockSdkBuilder::new(&sdk_root, version).build()
}

// ─────────────────────────────────────────────────────────────────────────────
// flutter_wrapper Layout (flutterw + .flutter/)
// ─────────────────────────────────────────────────────────────────────────────

/// Creates a flutter_wrapper layout at `project_dir`.
///
/// Files created:
/// - `<project_dir>/flutterw`         — shell script stub
/// - `<project_dir>/.flutter/`        — mock SDK root directory
/// - `<project_dir>/.flutter/bin/flutter`
/// - `<project_dir>/.flutter/VERSION`
///
/// Returns the SDK root: `<project_dir>/.flutter/`.
pub fn create_flutter_wrapper_layout(project_dir: &Path) -> PathBuf {
    // Create flutterw script
    let flutterw_path = project_dir.join("flutterw");
    fs::write(
        &flutterw_path,
        "#!/bin/sh\n# flutter_wrapper stub\nexec .flutter/bin/flutter \"$@\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&flutterw_path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    // Build mock SDK at <project_dir>/.flutter/
    let sdk_root = project_dir.join(".flutter");
    MockSdkBuilder::new(&sdk_root, "3.22.0").build()
}
