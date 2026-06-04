## Task: Consolidate the Android SDK-root resolver into a single daemon helper (M2)

**Objective**: Eliminate the two divergent `resolve_android_sdk_root` implementations
(install-time in `fdemon-app`, check-time in `fdemon-daemon`) by extracting one shared
daemon helper that both consume, so the installer and the post-install check can never
drift apart on env-var order or platform-default paths.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 2-3 hours

### Background (verified)

- `crates/fdemon-app/src/actions/mod.rs:1567` — private `fn resolve_android_sdk_root(sdk_root: Option<&Path>) -> PathBuf`: **always** returns a `PathBuf` (no `is_dir()` guard — it is an install target to be created). Order: caller → `$ANDROID_HOME` → `$ANDROID_SDK_ROOT` → platform default. Has an extra last-resort fallback (`PathBuf::from("Android/Sdk")`) when `dirs::home_dir()` is `None`.
- `crates/fdemon-daemon/src/toolchain/checks/android.rs:32` — `pub fn android_sdk_root() -> Option<AndroidSdkRoot>`: returns `Some` **only if `is_dir()`**; no caller-override param; wraps in the `pub(super)` `AndroidSdkRoot` newtype. `platform_default_android_sdk` is at ~line 66.
- Platform defaults currently **agree** in both (Linux `~/Android/Sdk`, macOS `~/Library/Android/sdk`, Windows `%LOCALAPPDATA%\Android\Sdk`) — but only by accident; there is no shared source of truth. **This task makes the agreement structural, not coincidental.**
- The daemon `AndroidSdkRoot` newtype does **not** need to be exported — a `PathBuf`-returning helper is sufficient.

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/checks/android.rs`: add `pub(super) fn resolve_android_sdk_root_path(override_path: Option<&Path>) -> PathBuf` containing the **unconditional** resolution logic (env order + platform default + the last-resort `Android/Sdk` fallback adopted from the app version for install robustness). Rewrite `android_sdk_root()` to call this helper then apply the `is_dir()` filter and wrap in `AndroidSdkRoot`.
- `crates/fdemon-daemon/src/toolchain/checks/mod.rs`: `pub use android::resolve_android_sdk_root_path;`.
- `crates/fdemon-daemon/src/toolchain/mod.rs`: `pub use checks::resolve_android_sdk_root_path;` (next to existing Phase 3 re-exports).
- `crates/fdemon-daemon/src/lib.rs`: add `resolve_android_sdk_root_path` to the `pub use toolchain::{...}` re-export list.
- `crates/fdemon-app/src/actions/mod.rs`: **delete** the private `fn resolve_android_sdk_root` (lines ~1553–1608) and replace its one call site (~line 960, the AndroidTools executor arm) with `fdemon_daemon::resolve_android_sdk_root_path(params.sdk_root.as_deref())` (or the equivalent path via the `use fdemon_daemon::toolchain` import already present).

### Details

The shared helper is the **unconditional** resolver (it must return a path even when
nothing exists yet, because the installer creates it). The check-time `android_sdk_root()`
becomes a thin wrapper: `resolve_android_sdk_root_path(None)` → `is_dir()` filter →
`AndroidSdkRoot`. `run_preflight` (`toolchain/mod.rs`) continues calling
`checks::android_sdk_root()` unchanged — no behavior change for preflight.

Adopt the app version's last-resort `PathBuf::from("Android/Sdk")` fallback into the
shared helper so the install-time robustness is preserved (the daemon version previously
returned `None` for the `home_dir() == None` case on non-Windows/macOS).

### Acceptance Criteria

1. Exactly one resolution function (`resolve_android_sdk_root_path`) holds the env-var
   order and platform-default paths; both the installer call site and `android_sdk_root()`
   derive from it.
2. The private `fdemon-app` `resolve_android_sdk_root` is deleted; its call site uses the
   daemon export.
3. `android_sdk_root()` behavior is unchanged for callers (still `Option<AndroidSdkRoot>`,
   still `is_dir()`-gated); `AndroidSdkRoot` stays `pub(super)`.
4. A new unit test asserts the install-time resolver and the check-time resolver agree on
   identical inputs — e.g. with `$ANDROID_HOME` set to a tempdir, both resolve to it; with
   no env vars, both resolve to the same platform default string.
5. `cargo check --workspace --all-targets`, `cargo test --workspace`,
   `cargo clippy --workspace --all-targets -- -D warnings` pass.

### Coordination

- This task and task 02 both write `crates/fdemon-app/src/actions/mod.rs`. Task 02 depends
  on this one and runs after it on the same branch. The two edits are far apart (this task:
  call site ~960 and deletion ~1553–1608; task 02: ~968 and ~1069–1101), but they share the
  file, so they must not run concurrently.
- This task is the **only** writer of `toolchain/mod.rs` and `lib.rs` re-exports in this phase.

### Notes

- Do not export `AndroidSdkRoot`. Keep the public surface to the `PathBuf` helper.
- `dunce`/`dirs` usage should match what the existing daemon resolver already uses — do not
  introduce a new path-canonicalization dependency.

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
