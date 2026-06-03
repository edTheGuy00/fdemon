## Task: Daemon install dependencies + shared install types

**Objective**: Add the download/extract/checksum crates to `fdemon-daemon` and
introduce the shared types Phase 2's installer modules need (release manifest,
install method/target, progress, outcomes).

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `Cargo.toml` (workspace root): add `zip`, `tar`, `lzma-rs`, `sha2`, and a
  `futures-util` (already present) note to `[workspace.dependencies]`. `reqwest`
  is already a workspace dep (rustls-tls, json) — reuse as-is.
- `crates/fdemon-daemon/Cargo.toml`: add `reqwest`, `zip`, `tar`, `lzma-rs`,
  `sha2`, `futures-util` (for `StreamExt` on the byte stream) under
  `[dependencies]`.
- `crates/fdemon-daemon/src/toolchain/types.rs`: add the new install-related
  types described below.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/version_check.rs`: existing reqwest usage (rustls,
  client construction) as a reference for how reqwest is configured in-repo.

### Details

Add to `[workspace.dependencies]` (pin conservative versions; verify latest on crates.io):

```toml
zip = { version = "2", default-features = false, features = ["deflate"] }
tar = "0.4"
lzma-rs = "0.3"          # pure-Rust xz decode for .tar.xz (no liblzma C dep)
sha2 = "0.10"
```

Add to `crates/fdemon-daemon/Cargo.toml` `[dependencies]`:

```toml
reqwest.workspace = true
futures-util.workspace = true   # already a workspace dep
zip.workspace = true
tar.workspace = true
lzma-rs.workspace = true
sha2.workspace = true
```

New types in `toolchain/types.rs` (all `Debug + Clone`):

```rust
/// How a managed Flutter SDK is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMethod {
    /// `git clone -b <channel> --depth 1` — keeps `flutter upgrade` working.
    GitClone,
    /// Download + verify + extract the release archive (no git required).
    Archive,
}

/// Host CPU architecture, used to select the right release archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostArch { X64, Arm64, Unknown }

impl HostArch {
    pub fn detect() -> Self { /* cfg!(target_arch=...) → X64 / Arm64 / Unknown */ }
}

/// A single entry from the Flutter releases manifest.
#[derive(Debug, Clone)]
pub struct FlutterRelease {
    pub version: String,        // e.g. "3.24.0"
    pub channel: String,        // "stable" | "beta" | ...
    pub archive: String,        // relative path under base_url
    pub sha256: String,
    pub dart_sdk_arch: Option<String>, // "x64" | "arm64" (when present)
}

/// The parsed releases manifest (`releases_<os>.json`).
#[derive(Debug, Clone)]
pub struct FlutterReleaseManifest {
    pub base_url: String,
    pub current_stable_hash: Option<String>,
    pub releases: Vec<FlutterRelease>,
}

impl FlutterReleaseManifest {
    /// Resolve the stable release matching the given arch (falls back to the
    /// `current_release.stable` hash when no arch match is found).
    pub fn resolve_stable(&self, arch: HostArch) -> Option<&FlutterRelease> { ... }
}

/// Resolved parameters for installing the Flutter SDK.
#[derive(Debug, Clone)]
pub struct FlutterInstallTarget {
    pub method: InstallMethod,
    pub channel: String,          // "stable"
    pub install_root: PathBuf,    // e.g. ~/fvm/versions
    pub version_dir_name: String, // e.g. "stable" or the resolved version
}

/// Progress emitted during a download.
#[derive(Debug, Clone, Copy)]
pub struct DownloadProgress { pub received: u64, pub total: Option<u64> }

/// Final outcome of a managed Flutter install.
#[derive(Debug, Clone)]
pub struct FlutterInstallOutcome {
    pub sdk_path: PathBuf,   // resolved SDK root (the dir containing bin/flutter)
    pub version: String,     // best-effort version label
    pub method: InstallMethod,
}
```

Keep `serde` derive off these unless the manifest parser needs it — the manifest
JSON should be deserialized via a private serde shape in `flutter_install.rs`
(task 03) and mapped into `FlutterReleaseManifest` to keep the public types
serde-agnostic. (Decide in task 03; this task only needs the owned types.)

Do **not** add module declarations for `download`/`flutter_install`/`path_config`
here — those land with their respective tasks to keep `mod.rs` churn ordered.

### Acceptance Criteria

1. `cargo check -p fdemon-daemon` succeeds with the new dependencies.
2. New types compile and are `pub` from `toolchain/types.rs`; `HostArch::detect()`
   and `FlutterReleaseManifest::resolve_stable()` have unit tests.
3. `cargo tree -p fdemon-daemon | rg liblzma` returns nothing (pure-Rust xz).
4. No new clippy warnings.

### Testing

```rust
#[test]
fn test_host_arch_detect_matches_cfg() { /* mirror HostPlatform test */ }

#[test]
fn test_resolve_stable_prefers_arch_match() {
    // manifest with two stable releases (x64, arm64) → resolve_stable(Arm64) picks arm64
}

#[test]
fn test_resolve_stable_falls_back_when_no_arch() {
    // single stable release without dart_sdk_arch → still resolved
}
```

### Notes

- `zip` v2 default features pull `bzip2`/`zstd`/time — disable defaults and enable
  only `deflate` to keep build time and C deps down (cmdline-tools/Flutter zips are deflate).
- Verify exact crate versions resolve under MSRV 1.77.2; if `zip` v2 requires a
  newer toolchain, pin a 1.x line that builds on 1.77.2 and note it.
- `HostArch` is distinct from `HostPlatform` (OS). The manifest URL uses the OS;
  the archive selection uses the arch.

---

## Completion Summary

**Status:** Not Started
</content>
