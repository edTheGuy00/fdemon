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

**Status:** Done
**Branch:** worktree-agent-a63daa7fca1bb5e5b

### Files Modified

| File | Changes |
|------|---------|
| `Cargo.toml` (workspace root) | Added `zip = { version = "2", default-features = false, features = ["deflate"] }`, `tar = { version = "0.4", default-features = false }`, `lzma-rs = { version = "0.3", features = ["stream"] }`, `sha2 = "0.10"` to `[workspace.dependencies]`; added `stream` feature to `reqwest` |
| `crates/fdemon-daemon/Cargo.toml` | Added `reqwest`, `zip`, `tar`, `lzma-rs`, `sha2` as workspace dependencies |
| `crates/fdemon-daemon/src/toolchain/types.rs` | Added `InstallMethod`, `HostArch` (with `detect()` + `as_manifest_str()`), `FlutterRelease`, `FlutterReleaseManifest` (with `resolve_stable()`), `FlutterInstallTarget`, `DownloadProgress`, `FlutterInstallOutcome`; 9 new unit tests |
| `crates/fdemon-daemon/src/toolchain/mod.rs` | Extended `pub use` re-exports to include all 7 new Phase 2 install types |

### Notable Decisions/Tradeoffs

1. **zip 2.x not zip "2" latest**: The crate `zip` has a pre-release `9.0.0-pre2` that requires Rust 1.88 (above our 1.77.2 MSRV). Cargo resolved `zip = "2"` to `2.4.2` (MSRV 1.73.0), which is compatible. Noted in the workspace TOML comment.

2. **`tar` with `default-features = false`**: The `default` feature pulls in `xattr` (Linux-only optional dep). Disabling it keeps the dep cross-platform and lighter.

3. **`reqwest` stream feature added**: The workspace reqwest declaration already had `rustls-tls` and `json`. Added `stream` feature here so task 02 (download) can use `bytes_stream()` / `StreamExt` without requiring a separate dependency change.

4. **`lzma-rs` with `stream` feature**: The `stream` feature enables async/streaming XZ decode via `XzStreamDecoder`, which task 02 will need for `.tar.xz` extraction. Without it, only full-buffer decode is available.

5. **`serde` omitted from new types**: Per task spec, new types are serde-agnostic. The manifest JSON deserializer will live in `flutter_install.rs` (task 03) with a private serde shape that maps into `FlutterReleaseManifest`.

6. **`resolve_stable` two-pass approach**: First pass prefers exact arch match; second pass falls back to any stable entry (no-arch field). This handles both modern multi-arch manifests and older single-arch entries.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check -p fdemon-daemon` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-daemon --lib toolchain::types` - Passed (14 tests)
- `cargo test --workspace` - Passed (all suites green)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (no warnings)
- `cargo tree -p fdemon-daemon | grep liblzma` - Returns nothing (pure-Rust XZ confirmed)

### Risks/Limitations

1. **zip version constraint**: We pin `"2"` which resolves to 2.4.x today. If the project later needs zip 3.x features it will require a separate bump. The pre-release v8/v9 series has a breaking API change and requires Rust 1.88+.

2. **reqwest `stream` feature**: Adding `stream` to the workspace reqwest declaration may slightly increase compile time for crates that were previously only using `json`. Minimal impact expected.

