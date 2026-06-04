## Task: Add macOS (Rosetta/CocoaPods) and Windows (git/winget) prerequisite probes

**Objective**: Replace the coarse macOS single-gate and the Windows git-proxy stub
with the additional read-only probes Phase 4 needs, and expose a stable
missing-key contract so the app-land guided-command helper (task 03) can trim macOS
commands to exactly the missing items.

**Depends on**: 01-linux-prereq-detection (same file — sequential)

**Estimated Time**: 4-6 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs`: extend
  `check_macos_prerequisites` and `check_windows_prerequisites`; add the
  canonical missing-key constants + `parse_missing_prereq_keys` helper.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs`: `ComponentCheck`, `ComponentStatus`,
  `HostPlatform`.

### Details

**macOS** (`check_macos_prerequisites`):
- Keep `xcode-select -p` as the Command Line Tools gate.
- Add **CocoaPods** presence via `pod --version`.
- Add **Rosetta** presence **only on Apple Silicon** via `pgrep oahd` (a returned
  PID means installed). Gate the probe on `std::env::consts::ARCH == "aarch64"` so
  x86_64 Macs never report Rosetta missing.
- Detection cannot distinguish full Xcode from CLT-only via `xcode-select -p`
  alone; leave the CLT-level gate and note the limitation in `detail` rather than
  adding `xcodebuild -version` probing this phase.

**Windows** (`check_windows_prerequisites`):
- Keep `git` via `which::which` (proxy is acceptable).
- Add a **winget availability** probe via `which::which("winget")` so task 03 can
  choose between `winget install Git.Git` and the git-scm.com download URL.
- Do **not** gate on PowerShell (assumed present on Win10 1903+).
- Visual Studio "Desktop development with C++" (`vswhere.exe`) detection is **out
  of scope** — mention it as a manual note in `detail` only.

**All probes** go through `tokio::process::Command` with `PROBE_TIMEOUT` and
`Stdio::null()`, matching the existing macOS `xcode-select` block: a `NotFound` IO
error → `Missing`, timeout/other → `Unknown`.

**Missing-key contract (the cross-crate seam for task 03):**
Keep one aggregate `ComponentCheck` per OS (no new `ComponentKind` variants, no new
struct fields). To let task 03 emit *only* the missing macOS commands without
brittle prose parsing, define canonical key constants and a parser **in the daemon**
(single source of truth), and include the keys in the human-readable `detail`:

```rust
pub const PREREQ_KEY_XCODE_CLT: &str = "xcode-clt";
pub const PREREQ_KEY_COCOAPODS: &str = "cocoapods";
pub const PREREQ_KEY_ROSETTA: &str = "rosetta";
pub const PREREQ_KEY_GIT: &str = "git";          // Windows
// ... (Linux uses the package-manager command directly; keys optional there)

/// Parse the stable, comma-joined missing-item keys out of a Prerequisites
/// `ComponentCheck.detail`. Single source of truth for the detail format.
pub fn parse_missing_prereq_keys(detail: &str) -> Vec<&str> { /* ... */ }
```

Re-export the constants + `parse_missing_prereq_keys` from `toolchain::mod` so
`fdemon-app` can import them. Document the `detail` format precisely (e.g.
`"missing: xcode-clt, cocoapods, rosetta"`).

### Acceptance Criteria

1. macOS reports CLT (`xcode-select -p`), CocoaPods (`pod --version`), and — on
   `aarch64` only — Rosetta (`pgrep oahd`); x86_64 never reports Rosetta missing.
2. Windows reports git presence and winget availability; PowerShell is not gated;
   VS C++ is not probed (note-only).
3. All new probes use `PROBE_TIMEOUT` + `Stdio::null()` and map `NotFound`→`Missing`,
   timeout/other→`Unknown` (matching the existing macOS arm).
4. `parse_missing_prereq_keys(detail)` round-trips the keys that detection writes;
   the constants + parser are re-exported and usable from `fdemon-app`.
5. Status is `Ok` only when every applicable item is present.

### Testing

```rust
#[cfg(test)]
mod tests {
    // - ARCH-gated Rosetta: aarch64 includes the probe; x86_64 omits it
    // - winget-present vs absent branch
    // - parse_missing_prereq_keys round-trips the detail format (incl. empty)
    // - status mapping: NotFound -> Missing, timeout -> Unknown
}
```

Where `which`/process outcomes are not mockable, test the pure status-mapping and
`parse_missing_prereq_keys` helpers directly.

### Notes

- Same-file sequential with task 01 — append to the shared `#[cfg(test)] mod tests`.
- `pgrep oahd` is the community-standard Rosetta check (Flutter does not document a
  programmatic one); treat a missing `pgrep` as `Unknown`, not `Missing`.
- Keep the contract minimal: Linux does not strictly need per-item keys (it emits
  the full package list, per the resolved scope decision); macOS/Windows do.
