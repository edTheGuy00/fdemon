# Task 01 — Refresh the Windows process PATH from the registry on every preflight

**Agent:** implementor
**Severity:** 🟠 MAJOR (Windows re-check is broken after a guided install)
**Depends On:** —
**Crate(s):** `fdemon-daemon`, `fdemon-app`

## Problem

On Windows, fdemon detects toolchain binaries against its **frozen process PATH**
(`Command::new("git")` at `checks/mod.rs:127`; `which::which(...)` at
`prerequisites.rs:213,685`). After a guided `winget`/installer adds a tool to the
**registry** PATH, a running fdemon can't see it — so pressing `r` to re-check still
reports it missing until fdemon restarts. `run_preflight` never re-reads the
registry PATH. See `../BUG.md` for the full analysis.

## Goal

Windows-only: at the **start of every `run_preflight`**, refresh the process `PATH`
from the registry (expanded Machine + User `Path`, as a new shell would see it) so a
re-check picks up newly-installed tools **without restarting fdemon**. No-op on
non-Windows. Also clarify the Prerequisites guided wording.

## Acceptance Criteria

- [ ] A `#[cfg(windows)]` helper reads the **expanded** Machine + User `Path` via the
      existing out-of-band PowerShell pattern
      (`[Environment]::GetEnvironmentVariable('Path','Machine')` +
      `'User'`, joined with `;`) and updates the process var via
      `std::env::set_var("PATH", merged)`.
- [ ] `run_preflight` calls the refresh **once, up front**, before fanning out the
      component probes, under `#[cfg(windows)]`. Non-Windows builds compile to a
      no-op (no subprocess, no behavior change).
- [ ] After the refresh, `which::which("git")` / `Command::new("git")` resolve a
      git that was installed *after* fdemon launched (verified E2E in the Windows VM —
      see below).
- [ ] The initial preflight is a near-no-op (process PATH already matches the
      registry at launch — no spurious changes).
- [ ] Prerequisites guided text clarifies: after installing, **press `r` to
      re-check** (now works in-process); your own already-open terminals still need a
      new window. No "restart fdemon" implication.
- [ ] No new runtime dependency (PowerShell read, not the `winreg` crate).

## Recommended Approach

- Add `refresh_process_path_from_registry()` in
  `crates/fdemon-daemon/src/toolchain/path_config.rs` (it already owns the Windows
  PowerShell env scripts and the out-of-band conventions), gated `#[cfg(windows)]`,
  with a non-Windows no-op sibling (or `#[cfg(not(windows))]` empty fn) so the call
  site is unconditional.
- Read script (out-of-band, no interpolation needed — it's a constant):
  ```powershell
  $m = [Environment]::GetEnvironmentVariable('Path','Machine')
  $u = [Environment]::GetEnvironmentVariable('Path','User')
  Write-Output "$m;$u"
  ```
  Trim, drop empty halves, `set_var("PATH", ...)`. `GetEnvironmentVariable` already
  expands `REG_EXPAND_SZ`, so the value is ready for `which`.
- Call it at the very top of `run_preflight` in
  `crates/fdemon-daemon/src/toolchain/mod.rs`.
- Update the Prerequisites guided message in
  `crates/fdemon-app/src/install_wizard/state.rs` (the `prerequisites_guided_commands`
  / guided-text path) per the wording criterion.

## Files Modified (Write)

- `crates/fdemon-daemon/src/toolchain/path_config.rs`
- `crates/fdemon-daemon/src/toolchain/mod.rs`
- `crates/fdemon-app/src/install_wizard/state.rs`

## Files Read (Dependencies)

- `crates/fdemon-daemon/src/toolchain/checks/mod.rs`, `checks/prerequisites.rs`
  (confirm probes use process PATH — read only)
- `crates/fdemon-app/src/actions/mod.rs` (preflight dispatch — read only)

## Testing

- Windows-gated unit test for the merge logic (Machine + User → `;`-joined, empties
  dropped). The actual registry read / `set_var` is hard to unit-test
  deterministically — keep the *pure merge* logic in a testable helper and cover it;
  gate the subprocess read behind it.
- Non-Windows: assert the call site compiles to a no-op (the function is empty under
  `#[cfg(not(windows))]`).
- `cargo fmt`, `cargo check`, `cargo test`, `cargo clippy -D warnings` all green.
- **E2E (authoritative), in `tests/docker/windows/`:** rebuild `fdemon.exe`
  (`docker build --target builder -f tests/docker/windows-wine.Dockerfile .` →
  `docker cp` the exe into `tests/docker/windows/oem/`), boot the VM, run fdemon with
  git absent, `winget install Git.Git`, press `r` → git flips to present with no
  fdemon restart.

## Notes

- `std::env::set_var("PATH", …)` is process-global and `unsafe` in Rust 2024 — do it
  once, up front in `run_preflight`, before spawning probe tasks; document the
  caveat in a comment.
- Keep everything `#[cfg(windows)]`; Linux/macOS must be untouched (their guided
  installs land in already-on-PATH dirs, so no refresh is needed).

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/path_config.rs` | Added `READ_EXPANDED_MACHINE_USER_PATH_SCRIPT` constant, `merge_machine_user_path()` pure helper, and `refresh_process_path_from_registry()` function (Windows-only, no-op on non-Windows). Added 9 unit tests covering merge logic and script shape. |
| `crates/fdemon-daemon/src/toolchain/mod.rs` | Added `#[cfg(target_os = "windows")]` call to `path_config::refresh_process_path_from_registry()` at the top of `run_preflight`, before `HostPlatform::detect()`. Added `merge_machine_user_path` and `refresh_process_path_from_registry` to public re-exports. |
| `crates/fdemon-app/src/install_wizard/state.rs` | Updated `prerequisites_guided_commands` Windows arm to add a `note` to both the winget and URL-fallback `GuidedCommand`s, clarifying that pressing `r` re-checks in-process (no fdemon restart needed) and that own open terminals still need a new window. Updated the corresponding test that previously asserted `note.is_none()` for the winget arm. |

### Notable Decisions/Tradeoffs

1. **PowerShell read, not winreg crate**: Task specified the PowerShell approach (`[Environment]::GetEnvironmentVariable('Path','Machine/User')`) to avoid a new dependency, consistent with the existing out-of-band PowerShell conventions in `path_config.rs`. `GetEnvironmentVariable` expands `REG_EXPAND_SZ`, so the returned value is already usable as a process PATH.

2. **`#[cfg(target_os = "windows")]` call site in mod.rs**: Used a conditional call at the `run_preflight` level (rather than making the function itself a no-op internally) to make the Windows-only behavior explicit at the call site. The exported function still has the no-op `#[cfg(not(target_os = "windows"))]` body for callers that want to call it unconditionally.

3. **Process-global caveat documented**: The `set_var` call is documented in both the function doc comment and the call site comment, explaining it must run before `tokio::join!` fans out the probe tasks to avoid concurrent env reads.

4. **Non-Windows compile verification**: The Linux CI host builds and tests all non-Windows paths cleanly. The Windows registry read is verified later in the real Windows VM (E2E, as specified in the task).

5. **GuidedCommand note for winget arm**: Previously the winget arm had `note: None`. Changed to `Some(...)` with the "press r to re-check" and "own terminals need a new window" wording. Updated the existing test assertion from `is_none()` to `is_some()` + content check.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (6,148+ tests, 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **E2E verification on Windows required**: The actual registry read and `set_var` path are Windows-only and cannot be tested on this Linux host. The E2E test (boot Windows VM, start fdemon with git absent, `winget install Git.Git`, press `r`, see git flip to present) is the authoritative check. The pure merge logic and script shape are unit-tested on Linux.

2. **`set_var` thread-safety**: As documented, `set_var` is process-global. The up-front call before `tokio::join!` avoids concurrent access, but if `run_preflight` is ever called concurrently by multiple callers, a data race would be possible. Current codebase only calls `run_preflight` sequentially (initial open + `r` re-checks are serialized by the TEA update loop).

### Doc Updates Needed

- `docs/ARCHITECTURE.md`: Note the Windows preflight PATH-refresh in the toolchain subsystem description (Windows-only `refresh_process_path_from_registry` call in `run_preflight`).
