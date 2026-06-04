## Task: Re-export `LinuxPackageManager` through `fdemon-app::install_wizard` (F2)

**Severity:** MINOR

**Objective**: Close the layer-pattern gap where `fdemon-tui` test fixtures reach into
`fdemon_daemon::toolchain::LinuxPackageManager` directly, bypassing the established
re-export gateway used for every other toolchain display type.

**Depends on**: 02-scroll-window-selected-command (re-touches `step_detail.rs`'s test
module — sequence after to avoid file conflict)

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/mod.rs` (add the re-export)
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` (test module: 2 sites)
- `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` (test module: 1 site)

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain` — `LinuxPackageManager` definition.

### Details

The presentation layer is meant to consume daemon toolchain *display* types only via
`fdemon-app::install_wizard` re-exports, never reaching into `fdemon_daemon::toolchain`
directly (`docs/ARCHITECTURE.md` "Note on daemon display types"; `fdemon-daemon` is in
`fdemon-tui`'s `[dev-dependencies]` only). The re-export block already carries an explicit
intent comment (`install_wizard/mod.rs:20-23`, the `n6` note): *"extend the gateway to
include all types needed by install-wizard TUI tests, so no module in fdemon-tui needs to
import directly from fdemon_daemon::toolchain."*

`LinuxPackageManager` — added to `ToolchainReport` by the first-round followup-04 — was
missed. Three `#[cfg(test)]` fixtures now reference it directly:
- `crates/fdemon-tui/src/widgets/install_wizard/mod.rs:370`
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs:646`
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs:661`

each as `linux_package_manager: Some(fdemon_daemon::toolchain::LinuxPackageManager::Unknown)`.

**Fix:**
1. Add `LinuxPackageManager` to the re-export block in
   `crates/fdemon-app/src/install_wizard/mod.rs:24-27`:
   ```rust
   pub use fdemon_daemon::toolchain::{
       ComponentCheck, ComponentKind, ComponentStatus, DoctorLine, DoctorMarker, HostPlatform,
       HostShell, LinuxPackageManager, ToolchainReport,
   };
   ```
   (keep alphabetical ordering).
2. Update the three TUI test sites to use the re-exported path — i.e.
   `LinuxPackageManager::Unknown` via the crate's existing
   `use fdemon_app::install_wizard::*;` (or an explicit
   `fdemon_app::install_wizard::LinuxPackageManager`), matching how the sibling display
   types are already referenced in those test modules.
3. Confirm no other `fdemon_daemon::` reference to this type remains in `fdemon-tui`
   (`grep -rn "fdemon_daemon::toolchain::LinuxPackageManager" crates/fdemon-tui/` returns
   nothing).

The ARCHITECTURE.md "four toolchain display types" note correction is handled separately
by task 07 (doc_maintainer), which depends on this task.

### Acceptance Criteria

1. `LinuxPackageManager` is re-exported via `fdemon-app::install_wizard`.
2. No `fdemon_daemon::` path for `LinuxPackageManager` (or any toolchain type) remains in
   `fdemon-tui` production or test source.
3. The three TUI test fixtures compile against the re-exported path; `cargo test -p
   fdemon-tui` is green.
4. No production behavior change (re-export + test-fixture import path only).

### Testing

Run `cargo test --workspace`; the install-wizard TUI tests still pass against the
re-exported type. No new tests required (import-path/visibility change only).

### Notes

- Touches `step_detail.rs` only in its `#[cfg(test)]` module — but it is the same file as
  tasks 01/02, hence the sequential dependency.
- Test-fixture + re-export change; no logic change.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/install_wizard/mod.rs` | Added `LinuxPackageManager` to `pub use fdemon_daemon::toolchain::{...}` re-export block (alphabetical order between `HostShell` and `ToolchainReport`) |
| `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | Added `LinuxPackageManager` to the test module `use fdemon_app::install_wizard::{...}` import; replaced 2 `fdemon_daemon::toolchain::LinuxPackageManager::Unknown` references with `LinuxPackageManager::Unknown` |
| `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` | Added `LinuxPackageManager` to the test module `use fdemon_app::install_wizard::{...}` import; replaced 1 `fdemon_daemon::toolchain::LinuxPackageManager::Unknown` reference with `LinuxPackageManager::Unknown` |

### Notable Decisions/Tradeoffs

1. **Alphabetical ordering**: `LinuxPackageManager` is inserted between `HostShell` and `ToolchainReport` in the re-export list, matching the convention already in place for all other exported types in that block.
2. **grep-based location**: Line numbers in the task description had shifted due to tasks 01/02 landing earlier; used `grep -rn` to locate the exact sites as instructed.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (6,878 tests across all crates, 0 failed)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed
- `grep -rn "fdemon_daemon::toolchain::LinuxPackageManager" crates/fdemon-tui/` - Returns nothing (confirmed)

### Risks/Limitations

1. **None**: Pure import-path and re-export change with no logic alteration. All three acceptance criteria met.
