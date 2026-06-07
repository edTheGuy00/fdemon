# Bug: Windows re-check can't see a newly-installed PATH tool (git/JDK) until fdemon restarts

## Status

🔬 **Investigated — root cause confirmed.** Awaiting approval of the fix before the
task breakdown is dispatched.

Found while testing the install wizard on a real Windows 11 VM
(`tests/docker/windows/`). Relates to
[`workflow/plans/features/toolchain-bootstrap/PLAN.md`](../../features/toolchain-bootstrap/PLAN.md)
Phase 4 (OS prerequisites — guided install + re-check).

## Symptom

On Windows, the wizard's Prerequisites step shows `git` missing and offers a guided
install (e.g. `winget install --id Git.Git -e`). The user runs it, git installs
successfully, then presses **`r`** to re-check — but fdemon **still reports git not
found**. A pre-existing terminal also can't find it (`'git' is not recognized`).
Only restarting fdemon (or opening a brand-new terminal) makes git visible.

## Root Cause (CONFIRMED)

fdemon resolves toolchain binaries against **its own process `PATH`, captured at
startup**:

- `check_git()` → `Command::new("git")` (`crates/fdemon-daemon/src/toolchain/checks/mod.rs:127-129`)
- prerequisites → `which::which("git")` / `which::which(tool)`
  (`crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs:213,685`)

Both use the process environment block. On **Windows**, a `winget install` (or the
Git installer) writes the new bin dir into the **registry** PATH (HKLM Machine
and/or HKCU User) and broadcasts `WM_SETTINGCHANGE` — but **a running process does
not re-read the registry**; it keeps the environment block it inherited at launch
(confirmed: [winget-cli#2815](https://github.com/microsoft/winget-cli/issues/2815)
— "the running terminal process retains its old environment block").

The re-check path does **not** refresh PATH:
`InstallWizardRerunPreflight` → `handle_rerun_preflight` →
`UpdateAction::RunToolchainPreflight` → `fdemon_daemon::toolchain::run_preflight(...)`
(`crates/fdemon-app/src/actions/mod.rs:800-807`) all execute **inside the same
fdemon process** with the stale PATH. fdemon currently only ever **writes** PATH +
broadcasts `WM_SETTINGCHANGE` (`path_config.rs` `BROADCAST_WM_SETTINGCHANGE_SCRIPT`);
it never **re-reads** the registry PATH into its own process.

**Why Windows-only:** Linux/macOS guided installs (`apt`/`dnf`/`pacman`/`brew`)
drop binaries into directories already on `PATH`, so `which::which` finds them on
the next re-check with no env change. Windows installers add **new** PATH
directories, which a live process can't see. (The same staleness affects the JDK
check after a guided JDK install, and any other Windows guided prerequisite.)

## Evidence Map

| Finding | Location |
|---|---|
| git probed via process PATH (`Command::new`) | `crates/fdemon-daemon/src/toolchain/checks/mod.rs:127-129` |
| prereqs probed via `which::which` (process PATH) | `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs:213,249,685-686` |
| re-check runs `run_preflight` in-process, no PATH refresh | `crates/fdemon-app/src/actions/mod.rs:800-807`; `handler/update.rs:3246` |
| fdemon only WRITES PATH + broadcasts WM_SETTINGCHANGE (never re-reads) | `crates/fdemon-daemon/src/toolchain/path_config.rs:1037-1067` |
| no registry PATH re-read anywhere in toolchain | (grep: only `std::env::var("PATH")` reads the frozen block) |

## Proposed Fix

**Windows-only: refresh the process `PATH` from the registry at the start of every
preflight**, so a re-check (`r`) picks up tools installed since fdemon launched.

1. Add a `#[cfg(windows)]` helper (e.g. `refresh_process_path_from_registry()` in
   `toolchain/path_config.rs`, or a small new `toolchain/env_refresh.rs`) that reads
   the **expanded** Machine + User `Path` — the effective PATH a new shell would
   get — and sets it on the process:
   - Simplest, dependency-free, consistent with existing code: a PowerShell read
     using the out-of-band pattern already in `path_config.rs`:
     ```powershell
     $m = [Environment]::GetEnvironmentVariable('Path','Machine')
     $u = [Environment]::GetEnvironmentVariable('Path','User')
     "$m;$u"
     ```
     (`GetEnvironmentVariable` expands `REG_EXPAND_SZ`, so the result is ready for
     `which`.) Then `std::env::set_var("PATH", merged)`.
   - Alternative: the `winreg` crate (Windows-only dep) for a fast, no-subprocess
     read, expanding `REG_EXPAND_SZ` values via `ExpandEnvironmentStringsW`. Either
     is acceptable; prefer the PowerShell read to avoid a new dependency and reuse
     the existing env-script conventions.
2. Call it at the **top of `run_preflight`** (the single chokepoint for both the
   initial run and every re-check) under `#[cfg(windows)]`. On the initial run it is
   a near-no-op (process PATH already matches the registry); on a re-check after a
   guided install it makes the new tool discoverable. No-op on non-Windows.
3. **Guided-message clarity (small, optional):** keep/adjust the Prerequisites
   guided text so it tells the user to **press `r` to re-check** after installing
   (which now works), and notes that *their own* already-open terminals still need a
   new window. Do not promise that fdemon must be restarted — it no longer must.

### Caveats / risks

- `std::env::set_var("PATH", …)` is process-global and (Rust 2024) `unsafe` due to
  potential data races with concurrent `getenv`. Do the refresh **once, up front**
  in `run_preflight` before fanning out the probes, and document the caveat. This
  matches how other tools (e.g. shells, installers' "refreshenv") handle it.
- Reading Machine PATH does not require elevation (read-only). User PATH likewise.
- Must remain **Windows-only** (`#[cfg(windows)]`); Linux/macOS behaviour unchanged.
- Verify it doesn't disturb Flutter resolution (fdemon resolves Flutter via the
  persisted `[flutter] sdk_path`, not PATH, so refreshing PATH only *adds*
  discoverability) — no regression expected.

## Verification

- Unit: a Windows-gated test that the merge/refresh helper produces a `;`-joined
  Machine+User PATH and updates the process var (mock the read where possible).
- **End-to-end on the real Windows VM** (`tests/docker/windows/`): in the guest,
  start fdemon with git absent → Prerequisites shows git missing → `winget install
  Git.Git` → press `r` → **git now shows present without restarting fdemon**. This is
  the authoritative check; rebuild `fdemon.exe` via the `windows-wine` builder and
  re-stage it.

## Affected Modules

| Crate | File | Change |
|---|---|---|
| `fdemon-daemon` | `toolchain/path_config.rs` (or new `toolchain/env_refresh.rs`) | `#[cfg(windows)]` `refresh_process_path_from_registry()` |
| `fdemon-daemon` | `toolchain/mod.rs` | call the refresh at the top of `run_preflight` (`#[cfg(windows)]`) |
| `fdemon-app` | `install_wizard/state.rs` (guided text) | optional: clarify "press r to re-check" wording |
| docs | `docs/ARCHITECTURE.md` | note the Windows preflight PATH-refresh (→ `doc_maintainer`) |

## Success Criteria

- [ ] On Windows, after a guided prerequisite install (git/JDK), pressing `r`
      re-detects the tool **without restarting fdemon**.
- [ ] Non-Windows behaviour is unchanged (no refresh, no new subprocess).
- [ ] The refresh is a near-no-op on the initial preflight (no false changes).
- [ ] `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
      all pass; verified E2E in the Windows VM.

## Open Questions (for approval)

1. **Read mechanism:** PowerShell read (no new dep, recommended) vs. the `winreg`
   crate (faster, Windows-only dep)?
2. **Scope of the guided-message tweak:** include the wording clarification now, or
   keep this bug strictly to the PATH-refresh fix?
