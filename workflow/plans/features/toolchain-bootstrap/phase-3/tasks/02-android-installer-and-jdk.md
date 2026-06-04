## Task: Android installer (cmdline-tools + sdkmanager + licenses) + JDK guidance

**Objective**: Implement the managed Android toolchain install in the daemon:
download the command-line tools zip, relocate them to `cmdline-tools/latest/`, run
`sdkmanager` to install the required packages, and accept the SDK licenses
non-interactively — all streamed back via `InstallEvent`. Also add JDK guided-install
command generation and the `flutter config --jdk-dir` helper.

**Depends on**: 01

**Agent:** implementor

**Estimated Time**: 7-9 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/android_install.rs`: **NEW** —
  `install_android_tools(target, on_event) -> Result<AndroidInstallOutcome>`.
- `crates/fdemon-daemon/src/toolchain/jdk.rs`: **NEW** — `resolve_jdk_home()` and
  `configure_flutter_jdk_dir()`. (The per-OS JDK *guided-install command string* is
  a display concern and lives in app-land — task 05 — not here.)
- `crates/fdemon-daemon/src/toolchain/process_stream.rs`: add
  `run_streaming_with_input` (feed bytes to child stdin while streaming output).
- `crates/fdemon-daemon/src/toolchain/mod.rs`: `mod android_install; mod jdk;` +
  re-exports.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/download.rs`: `download_to_file`,
  `extract_zip` (wrap sync extract in `spawn_blocking`).
- `crates/fdemon-daemon/src/toolchain/checks/android.rs`: `sdkmanager_bin_name()`,
  `android_sdk_root()` (default-root layout), the `cmdline-tools/latest/bin` path.
- `crates/fdemon-daemon/src/toolchain/types.rs`: `AndroidInstallTarget`,
  `AndroidInstallOutcome`, `cmdline_tools_url`, `sdkmanager_packages`, `InstallEvent`.
- `crates/fdemon-daemon/src/toolchain/flutter_install.rs`: atomic temp-dir →
  rename pattern, `InstallEvent` emission style, lockfile pattern (optional).

### Details

**`install_android_tools` flow** (async, streaming via `on_event: FnMut(InstallEvent)`):

1. `on_event(InstallEvent::Phase("Downloading command-line tools"))`. Resolve URL
   via `cmdline_tools_url(target.platform, &target.cmdline_tools_build)`; error
   clearly if `None`. `download_to_file(url, tmp_zip, |p| on_event(InstallEvent::Download(p)))`.
2. `Phase("Extracting")`. `spawn_blocking(extract_zip(tmp_zip, tmp_extract))`. The
   zip extracts to `tmp_extract/cmdline-tools/`.
3. `Phase("Relocating to cmdline-tools/latest")`. **Mandatory relocation:** move
   `tmp_extract/cmdline-tools` → `<sdk_root>/cmdline-tools/latest` (create parent
   `<sdk_root>/cmdline-tools/` first; if `latest/` already exists, replace it
   atomically). This is what `check_android_cmdline_tools` looks for.
4. `Phase("Accepting licenses")`. Run `<sdk_root>/cmdline-tools/latest/bin/<sdkmanager_bin_name()>
   --licenses` (set `cwd`/env so it finds the SDK root; pass `--sdk_root=<root>`),
   feeding `"y\n"` repeatedly via `run_streaming_with_input`, streaming each output
   line as `InstallEvent::Log`. If `target.jdk_path` is set, export
   `JAVA_HOME`/`PATH` for the child so `sdkmanager` finds the JDK.
5. `Phase("Installing packages")`. Run `sdkmanager <sdkmanager_packages(api_level)...>`
   via `run_streaming` (or `_with_input` answering license prompts), streaming output.
6. Return `AndroidInstallOutcome { sdk_root, packages_installed }`.

```rust
pub async fn install_android_tools<F>(
    target: &AndroidInstallTarget,
    mut on_event: F,
) -> Result<AndroidInstallOutcome>
where
    F: FnMut(InstallEvent) + Send,
{ /* ... */ }
```

**`run_streaming_with_input`** in `process_stream.rs` — same merged-line streaming
as `run_streaming`, but spawns the child with `Stdio::piped()` stdin, writes
`input` bytes, then drops stdin (EOF). Used to answer `sdkmanager --licenses`
prompts with a stream of `y\n`.

```rust
pub async fn run_streaming_with_input<F>(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    env: &[(&str, &str)],   // e.g. JAVA_HOME for the JDK
    stdin_data: &[u8],
    on_line: F,
) -> Result<ExitStatus>
where F: FnMut(String) + Send;
```

**`jdk.rs`** — JDK home resolution + flutter jdk-dir config (no guided-command
strings here — those live in app-land, task 05):

```rust
/// Best-effort resolution of the JDK home (JAVA_HOME, else `which java` → parent's parent).
pub fn resolve_jdk_home() -> Option<PathBuf> { /* ... */ }

/// `flutter config --jdk-dir=<dir>` so Flutter uses the right JDK.
pub async fn configure_flutter_jdk_dir(flutter: &Path, jdk_dir: &Path) -> Result<()> { /* run_streaming */ }
```

### Acceptance Criteria

1. `install_android_tools` downloads the cmdline-tools zip, extracts it, and
   **relocates** the extracted `cmdline-tools` dir to `<sdk_root>/cmdline-tools/latest`
   (verified against a fixture zip), then runs `sdkmanager` for the packages from
   `sdkmanager_packages(api_level)`, accepting licenses non-interactively.
2. Every phase emits an `InstallEvent::Phase`, download bytes emit
   `InstallEvent::Download`, and all child-process lines emit `InstallEvent::Log`.
3. A failed download (404 on a bad build number) returns `Err` with a message that
   names the URL; a non-zero `sdkmanager` exit returns `Err` (do not silently
   succeed). Temp dirs are cleaned up on failure.
4. `run_streaming_with_input` feeds stdin and streams merged stdout/stderr lines;
   non-zero exit is returned (not errored), matching `run_streaming` semantics.
5. `resolve_jdk_home` honors `JAVA_HOME` then falls back to `which java`;
   `configure_flutter_jdk_dir` runs `flutter config --jdk-dir=<dir>` and streams
   its output.
6. New symbols re-exported from `toolchain/mod.rs`. `cargo check -p fdemon-daemon`,
   `cargo clippy -p fdemon-daemon -- -D warnings`, and `cargo test -p fdemon-daemon`
   pass.

### Testing

- **Relocation:** unit-test the `cmdline-tools` → `cmdline-tools/latest` move with a
  small synthetic directory tree in a `tempdir()` (do not require network). Extract
  a tiny fixture zip if convenient, otherwise test the relocation helper directly.
- **URL/package wiring:** assert `install_android_tools` resolves the URL via the
  task-01 builder (inject build number; assert the request target without
  performing the download, e.g. by factoring URL resolution into a testable helper).
- **`run_streaming_with_input`:** run a trivial cross-platform command that echoes
  stdin (e.g. on Unix `cat`; gate Windows variant) and assert the streamed lines.
- **`resolve_jdk_home`:** set `JAVA_HOME` to a tempdir and assert it is returned.
- Network-dependent full install is **not** unit-tested (mirrors Phase 2's
  `install_flutter`); test the decomposed helpers (URL resolution, relocation,
  package list, stdin streaming) instead.

```rust
#[tokio::test]
async fn test_relocate_cmdline_tools_to_latest() { /* tempdir tree → assert latest/bin exists */ }

#[test]
fn test_resolve_jdk_home_honors_java_home() { /* set JAVA_HOME → assert returned */ }
```

### Notes

- **No SHA verification for cmdline-tools:** Google publishes no easily-fetched
  per-build sha256 (unlike the Flutter archive manifest). Rely on HTTPS/TLS; if a
  `[toolchain] cmdline_tools_sha256` override is ever desired, that is a future
  enhancement — do not add it now.
- **License acceptance is idempotent:** re-running `--licenses` on an
  already-licensed SDK is harmless; the step can be retried with `Enter`.
- **`spawn_blocking` for sync extract:** `extract_zip` is synchronous — wrap it,
  do not block the async executor. `download_to_file` is already async.
- **Atomic relocation:** extract to a temp dir under `sdk_root`, then rename into
  `cmdline-tools/latest`; clean temp on any failure (mirror `flutter_install.rs`).
- **`mod.rs` chain:** tasks 01→02→03 share `toolchain/mod.rs`; this task adds two
  `mod` lines and re-exports only — do not touch task 01's or task 03's regions.
- Keep `configure_flutter_jdk_dir` best-effort/non-fatal at the call site (task 06
  decides whether to surface its failure).

---

## Completion Summary

**Status:**
**Branch:**

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
