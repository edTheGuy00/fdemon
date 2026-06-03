## Task: Managed Flutter SDK install (manifest → git clone / archive → precache)

**Objective**: Implement `flutter_install.rs`: fetch the Flutter releases manifest,
resolve the stable archive + sha for the host OS/arch, install via `git clone`
(default) or archive download+verify+extract (fallback), run `flutter precache`,
and report progress/log through callbacks. Installs atomically into the configured
install root.

**Depends on**: 02

**Agent:** implementor

**Estimated Time**: 5-7 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/flutter_install.rs` — **NEW**
- `crates/fdemon-daemon/src/toolchain/mod.rs` — add `mod flutter_install;` and
  re-export `install_flutter`, `fetch_release_manifest`, `resolve_install_dir`,
  and relevant types.

**Files Read (Dependencies):**
- `toolchain/download.rs`, `toolchain/process_stream.rs` (task 02)
- `toolchain/types.rs` (task 01: manifest types, `InstallMethod`, `HostArch`,
  `FlutterInstallTarget`, `FlutterInstallOutcome`)
- `crates/fdemon-daemon/src/flutter_sdk/cache_scanner.rs` — `resolve_fvm_cache_path()`
  for the default install root (`~/fvm/versions`).
- `crates/fdemon-daemon/src/flutter_sdk/locator.rs` / `mod.rs` — to construct the
  resolved `FlutterExecutable`/SDK path after install.

### Details

Callback-driven public API (UI-agnostic; task 08 bridges to messages):

```rust
/// Events emitted during install, forwarded to the caller's sink.
pub enum InstallEvent {
    Log(String),                 // a streamed line from git/flutter
    Download(DownloadProgress),  // archive download progress
    Phase(&'static str),         // "Cloning", "Downloading", "Extracting", "Precaching"
}

/// Determine where to install. Honors an explicit override, else
/// `resolve_fvm_cache_path()`, else `~/fvm/versions` (created if missing).
pub fn resolve_install_dir(explicit_root: Option<&Path>) -> Result<PathBuf>;

/// Fetch + parse the releases manifest for the host OS.
/// URL: https://storage.googleapis.com/flutter_infra_release/releases/releases_<os>.json
pub async fn fetch_release_manifest(platform: HostPlatform) -> Result<FlutterReleaseManifest>;

/// Install a managed Flutter SDK. Picks git clone unless `target.method` forces
/// archive or `git` is unavailable. Returns the resolved SDK path + version.
pub async fn install_flutter<F: FnMut(InstallEvent) + Send>(
    target: &FlutterInstallTarget,
    mut on_event: F,
) -> Result<FlutterInstallOutcome>;
```

**Install algorithm:**

1. Compute `final_dir = target.install_root.join(&target.version_dir_name)`.
   If it already exists and contains `bin/flutter`, short-circuit to
   "already installed" (return outcome; let preflight confirm).
2. Create a sibling temp dir under `install_root` (e.g. `.fdemon-install-tmp-<n>`;
   no `Math.random`/clock — derive a unique name from a counter or pid via
   `std::process::id()`).
3. **git path** (default, when `git` resolves on PATH and method != Archive):
   `git clone -b <channel> --depth 1 https://github.com/flutter/flutter.git <tmp>`
   streamed via `process_stream::run_streaming`, emitting `InstallEvent::Log`.
4. **archive path** (method == Archive, or git missing):
   - `fetch_release_manifest(platform)` → `resolve_stable(HostArch::detect())`.
   - Build archive URL = `base_url` + `/` + `release.archive`.
   - `download_to_file` into `<tmp>/archive.<ext>` (emit `Download` progress).
   - `verify_sha256` against `release.sha256` (run under `spawn_blocking`).
   - `extract_archive` into `<tmp>` (run under `spawn_blocking`). Flutter archives
     extract a top-level `flutter/` dir — normalize so `final_dir` ends up being
     the SDK root containing `bin/`.
5. Atomically `std::fs::rename(extracted_sdk_root, final_dir)`. On any failure,
   remove the temp dir and propagate the error.
6. Run `flutter precache` from `final_dir/bin` via `run_streaming` (emit Log +
   `Phase("Precaching")`). A precache failure is **non-fatal** — log it as a
   warning event but still return success (the SDK is usable; precache can be
   retried). Document this choice.
7. Best-effort version label: read `final_dir/version` file or run
   `flutter --version --machine` (reuse `version_probe` if convenient). Fall back
   to the channel name.

**OS handling:** `releases_<os>.json` uses `linux`/`macos`/`windows`. Archive
extension is `.tar.xz` on Linux and `.zip` on macOS/Windows.

### Acceptance Criteria

1. `resolve_install_dir(None)` returns `~/fvm/versions` (or `$FVM_CACHE_PATH`),
   creating it if absent; an explicit override is honored.
2. `fetch_release_manifest` parses a real-shaped manifest fixture into
   `FlutterReleaseManifest` with `base_url`, releases, and the stable hash.
3. `install_flutter` selects git clone by default and the archive path when git is
   absent or `method == Archive`; both produce a `final_dir` containing `bin/flutter`.
4. Archive installs verify SHA-256 before extraction and fail clearly on mismatch.
5. Temp dirs are cleaned up on failure; `final_dir` is only created via atomic rename.
6. `flutter precache` failure does not fail the overall install.
7. Manifest parsing + install-dir resolution + URL construction are unit-tested.
   No clippy warnings.

### Testing

```rust
#[test]
fn test_fetch_manifest_parses_fixture() {
    // parse a checked-in trimmed releases_linux.json fixture via the private serde shape
}

#[test]
fn test_archive_url_construction() { /* base_url + archive → expected URL */ }

#[test]
fn test_resolve_install_dir_default_and_override() {
    // override path honored; default falls back under HOME/fvm/versions (use a temp HOME)
}
```

Network-dependent paths (`download_to_file`, `git clone`, `precache`) are
integration-level — keep unit tests on the pure resolution/parsing logic and use
`wiremock` for the manifest fetch where practical. Gate any test that shells out
to `git`/`flutter` behind availability checks so CI stays green on bare runners.

### Notes

- `--depth 1` keeps the clone small; do not fetch full history.
- Some Flutter `doctor` warnings appear on archive installs (no git metadata) —
  acceptable per PLAN.md; git clone is the default precisely to avoid this.
- Do not write `[flutter] sdk_path` here — that is the app layer's job (task 09).
- Keep `InstallEvent` daemon-side and UI-agnostic; the app maps it to messages.

---

## Completion Summary

**Status:** Not Started
</content>
