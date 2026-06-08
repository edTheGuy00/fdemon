## Task: Executor JDK fallback (M1) + PathConfig summary-string fix (M4)

**Objective**: In the `RunWizardStep` executor, (M1) fall back to `resolve_jdk_home()`
when no `[toolchain] jdk_path` is configured so `sdkmanager` reliably gets `JAVA_HOME`,
and (M4) rebuild the PathConfig completion summary so it reads cleanly when both Flutter
and Android env writes occur. Both edits are in the same file (`actions/mod.rs`) and are
grouped to keep that file a single-task write in this wave.

**Depends on**: 01 (shared write file `crates/fdemon-app/src/actions/mod.rs`)

**Agent:** implementor

**Estimated Time**: 1-2 hours

### Background (verified)

**M1** — `crates/fdemon-app/src/handler/install_wizard/actions.rs:136-141` builds
`AndroidStepParams { jdk_path: ts.jdk_path.clone(), .. }`; `ts.jdk_path` defaults to `None`.
The executor (`crates/fdemon-app/src/actions/mod.rs:968`) passes `params.jdk_path` into
`AndroidInstallTarget.jdk_path`. In `android_install.rs:190-212`, `JAVA_HOME` + the JDK
`bin/` PATH prepend are only set when `jdk_path` is `Some`. So unless the user sets
`[toolchain] jdk_path`, `sdkmanager` inherits an ambient env that may lack `JAVA_HOME` even
though the JDK gate passed at preflight. `resolve_jdk_home()` (`toolchain/jdk.rs:30`)
implements exactly the right fallback (JAVA_HOME → `which java`) but is never called.

**The fix must be in the executor (inside `tokio::spawn`), NOT the handler.**
`resolve_jdk_home` calls `std::env::var`, `is_dir`, `which::which`, `fs::canonicalize` —
all I/O — which must not run in the pure TEA handler. Sampling the env at actual install
time (not dispatch time) is also more correct.

**M4** — `crates/fdemon-app/src/actions/mod.rs:1069-1101` (the
`Ok(Ok((flutter_outcome, android_outcome)))` arm) has two defects: (a) the android summary
format strings (~lines 1082, 1087) bake in a **trailing space**; (b) the joiner
`format!(", {}and ", android_summary)` (~line 1099) creates a comma-splice + double space,
producing: *"Added Flutter to PATH in X, Added ANDROID_HOME to Y and Restart your
terminal…"*. No existing test catches this (all PathConfig tests use `..` wildcards on the
message variant).

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/mod.rs`:
  - **M1:** add `resolve_jdk_home` to the `use fdemon_daemon::toolchain::{...}` import
    (~line 830-834); change the `AndroidInstallTarget` construction (~line 968) from
    `jdk_path: params.jdk_path` to `jdk_path: params.jdk_path.or_else(resolve_jdk_home)`.
  - **M4:** rebuild the summary in the PathConfig arm (~1069-1101): collect the non-empty
    clauses (Flutter, optional Android) into a `Vec<String>`, join with `". "`, then append
    `". Restart your terminal for changes to take effect."`. Drop the trailing spaces from
    the android summary format strings.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/jdk.rs` (`resolve_jdk_home` signature/semantics).
- `crates/fdemon-daemon/src/toolchain/android_install.rs` (confirm `JAVA_HOME`/PATH wiring).
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` (confirm handler stays unchanged).

### Acceptance Criteria

1. With `params.jdk_path == None` and a JDK discoverable via `JAVA_HOME`, the
   `AndroidInstallTarget` carries `jdk_path == Some(<JAVA_HOME>)`. The handler is **not**
   modified.
2. (M1) A test proves the fallback. Recommended: extract a tiny pure helper
   `resolve_effective_jdk_path(config_jdk: Option<PathBuf>) -> Option<PathBuf>` =
   `config_jdk.or_else(resolve_jdk_home)` and unit-test `resolve_effective_jdk_path(None)`
   returns `Some(tempdir)` when `JAVA_HOME` points at a valid JDK dir. If the test mutates
   `JAVA_HOME`, gate it with `#[serial]` (matching the existing `jdk.rs` test convention).
3. (M4) The PathConfig summary reads cleanly for **Flutter-only** and **Flutter+Android**
   outcomes (no comma-splice, no double space, single trailing period before "Restart").
   A new test asserts the combined string.
4. `cargo fmt`/`check`/`test`/`clippy -D warnings` pass workspace-wide.

### Notes

- Keep the M1 and M4 edits independent within the file; they touch different arms of the
  same `match`.
- Do not change `AndroidStepParams` (field shape stays `Option<PathBuf>`).

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/mod.rs` | M1: removed `resolve_jdk_home`/`PathConfigOutcome` from the inner `use` block (no longer referenced directly in executor); extracted `resolve_effective_jdk_path` free function; wired it into `AndroidInstallTarget` construction. M4: extracted `build_pathconfig_summary` free function; replaced 20-line inline summary logic with a single call; fixed trailing spaces and comma-splice. Added 5 unit tests (3 for M1, 2 for M4). |

### Notable Decisions/Tradeoffs

1. **Extract helpers rather than inline**: Both `resolve_effective_jdk_path` and `build_pathconfig_summary` are small pure wrappers extracted into module-level `pub(crate)` functions. This makes them directly unit-testable without going through the async executor machinery, which mirrors the existing test pattern in this file.

2. **`#[serial]` for JAVA_HOME tests**: Tests that set `JAVA_HOME` are gated with `#[serial_test::serial]` to avoid races with other tests in the suite that also touch env vars (consistent with `jdk.rs` convention).

3. **M4 separator is `. ` (period-space)**: The clauses are logical sentences, so joining with `. ` and appending a final `.` produces the cleanest prose. A `Vec<String>` + `join` approach is both readable and easy to extend if more clauses are added later.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed (0 warnings)
- `cargo test --workspace` - Passed (6,557 total tests, 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed
- `cargo test -p fdemon-app -- resolve_effective_jdk pathconfig_summary` - 5/5 new tests passed

### Risks/Limitations

1. **`resolve_jdk_home` is synchronous I/O in async context**: The call is inside a `tokio::spawn` task but is not explicitly wrapped in `spawn_blocking`. The function only calls `std::env::var`, `Path::is_dir`, `which::which`, and `std::fs::canonicalize` — lightweight blocking calls. This matches how the existing `HostPlatform::detect()` and `HostShell::detect()` calls are handled in the same spawn block and is acceptable for this usage pattern.
