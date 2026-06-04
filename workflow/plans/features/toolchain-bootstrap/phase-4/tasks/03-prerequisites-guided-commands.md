## Task: Populate Prerequisites guided commands per-OS in build_steps

**Objective**: Replace the hardcoded `guided_commands: Vec::new()` on the
`Prerequisites` `WizardStep` (`state.rs:357`) with per-OS guided install commands
derived from the refined detection, mirroring the existing `jdk_guided_command`
pattern, and relax the Enter stub so the step reads as guided (not "later phase").

**Depends on**: 01-linux-prereq-detection, 02-macos-windows-prereq-detection

**Estimated Time**: 4-6 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/state.rs`: add
  `prerequisites_guided_commands(...)`; wire it into `build_steps`.
- `crates/fdemon-app/src/handler/install_wizard/actions.rs`: split `Prerequisites`
  out of the `Doctor` arm and change its status message.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/types.rs`: `GuidedCommand { label, command, note }`.
- `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs`: `LinuxPackageManager`
  + `detect_linux_package_manager`, the `PREREQ_KEY_*` constants, and
  `parse_missing_prereq_keys` (task 01/02 contract).

### Details

Add a helper alongside `jdk_guided_command` (`state.rs:238-253`):

```rust
fn prerequisites_guided_commands(
    platform: HostPlatform,
    components: &[ComponentCheck],
) -> Vec<GuidedCommand>
```

- **Return empty** when the `Prerequisites`/`Git` checks are all
  `ComponentStatus::Ok` (nothing to show — trims to zero when satisfied).
- **Linux** — emit **one** combined `GuidedCommand` chosen by
  `detect_linux_package_manager()`; emit the **full canonical package list** (the
  resolved scope decision — apt/dnf skip already-installed packages):
  - apt: `sudo apt-get install -y curl git unzip xz-utils zip libglu1-mesa clang cmake ninja-build pkg-config libgtk-3-dev libstdc++-12-dev`
  - dnf: `sudo dnf install -y curl git unzip xz zip mesa-libGLU clang cmake ninja-build pkgconf gtk3-devel`
  - pacman: `sudo pacman -S --needed curl git unzip xz zip glu clang cmake ninja pkgconf gtk3`
  - zypper: `sudo zypper in curl git unzip xz zip Mesa-libGLU1 clang gcc cmake ninja pkg-config gtk3-devel`
  - Unknown: a single command whose value is the Flutter Linux setup docs URL, in
    the `note`, with a clear `label`.
  - Use `note` for an alternate-manager hint, exactly like `jdk_guided_command`.
- **macOS** — emit up to three `GuidedCommand`s, **only** for the items
  `parse_missing_prereq_keys` reports missing:
  - `PREREQ_KEY_XCODE_CLT` → `xcode-select --install` (note: opens a GUI dialog)
  - `PREREQ_KEY_COCOAPODS` → `brew install cocoapods` (note: `or: sudo gem install cocoapods`)
  - `PREREQ_KEY_ROSETTA` → `sudo softwareupdate --install-rosetta --agree-to-license`
  - Order most-likely-missing first (CLT, then CocoaPods, then Rosetta). These are
    individually copyable via task 04's per-command navigation.
- **Windows** — emit `winget install Git.Git` when git is missing **and** winget is
  present; otherwise a command/note pointing at `https://git-scm.com/downloads/win`.

Wire it in `build_steps` (`state.rs:285-358`): add a `prereq_guided` block parallel
to the existing `android_guided` block (`state.rs:345-349`), computed from the
already-collected `prerequisites` vec + `report.platform`, and replace
`guided_commands: Vec::new()` at `state.rs:357` with `guided_commands: prereq_guided`.
**No changes** to `WizardStep`, `GuidedCommand`, or `InstallWizardState` structs.

In `handler/install_wizard/actions.rs:211-215`, split `Prerequisites` out of the
`Doctor` arm: keep Enter **non-executable** (no `RunWizardStep` dispatch — the
`actions/mod.rs:1103` guard must stay unreached), but change the `status_message`
to a guided message, e.g. `"Run the listed command(s), then press r to re-check."`.
Leave `Doctor` on its existing message.

### Acceptance Criteria

1. `prerequisites_guided_commands` returns `[]` when all prerequisites are `Ok`.
2. Linux returns exactly one command matching the detected package manager (full
   canonical list); Unknown falls back to the docs URL.
3. macOS returns one command per **missing** item only, ordered CLT → CocoaPods →
   Rosetta, driven by `parse_missing_prereq_keys`.
4. Windows returns `winget install Git.Git` when git missing + winget present, else
   the download-URL fallback.
5. `build_steps` populates the `Prerequisites` step's `guided_commands` from the
   helper; the `AndroidTools` guided command is unchanged.
6. Pressing Enter on `Prerequisites` does not dispatch `RunWizardStep`; the status
   message reads as guided (not "Available in a later phase").

### Testing

```rust
#[cfg(test)]
mod tests {
    // - empty when all prereqs Ok
    // - per-HostPlatform / per-LinuxPackageManager command strings
    // - macOS emits only the missing keys, in order; Rosetta only when key present
    // - Windows winget-present vs fallback URL
    // - build_steps wires Prerequisites.guided_commands correctly
    // - actions.rs: Enter on Prerequisites returns no RunWizardStep + guided message
}
```

Update the existing `actions.rs` test asserting "Available in a later phase" for
`Prerequisites` (it now expects the guided message); the `Doctor` assertion stays.

### Notes

- Command strings for non-apt managers are community-sourced (only apt is officially
  Flutter-documented); use the `note` field for alternate-manager hints to mitigate
  a wrong package name on uncommon distros.
- Keep all command strings in app-land here (display concern), consistent with
  `jdk_guided_command` — the daemon stays detection-only.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/Cargo.toml` | Added `which.workspace = true` for Windows winget detection |
| `crates/fdemon-app/src/install_wizard/state.rs` | Added `prerequisites_guided_commands()` helper; wired `prereq_guided` into `build_steps()`; added imports for `detect_linux_package_manager`, `parse_missing_prereq_keys`, `LinuxPackageManager`, `PREREQ_KEY_*`; added 17 new unit tests |
| `crates/fdemon-app/src/handler/install_wizard/actions.rs` | Split `Prerequisites` from `Doctor` arm; changed Prerequisites `status_message` to guided message; added 2 new tests (`test_prerequisites_enter_returns_guided_message_not_later_phase`, `test_doctor_enter_still_returns_later_phase_message`) |
| `Cargo.lock` | Updated lock file for new `which` dependency in `fdemon-app` |

### Notable Decisions/Tradeoffs

1. **Windows winget detection at command-generation time**: The `prerequisites_guided_commands` function calls `which::which("winget")` synchronously when building the Windows guided command. This is consistent with how `detect_linux_package_manager()` works in the daemon — pure PATH inspection, never invokes the tool.

2. **Early-out logic for all-Ok**: The helper returns empty for two distinct cases: (a) when all prereq/git components are Ok, and (b) when there are no prereq/git components at all (e.g., the empty slice passed from `report_with_jdk` in existing tests). This preserves backward compatibility with all existing tests.

3. **Linux Unknown PM fallback**: Returns the Flutter Linux setup docs URL in the `command` field (not the `note`) with a clear label, consistent with the task spec. The `note` is `None` in this case since there's no "alternate" manager to suggest.

4. **macOS missing key ordering**: CLT → CocoaPods → Rosetta is the fixed ordering; items only appear when their key is present in `parse_missing_prereq_keys(detail)`. Rosetta only shows on systems where it was actually reported missing (it's excluded via `NotApplicable` on x86_64 at probe time).

5. **Test for `test_non_android_steps_have_no_guided_commands`**: This test still passes unchanged because `report_with_jdk` produces a report with only a `ComponentKind::Jdk` entry; the prerequisites slice is empty, so `prerequisites_guided_commands` returns `Vec::new()` via the `!has_prereq_or_git` early-out.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed (0 warnings)
- `cargo test --workspace` — Passed (6,823+ tests total, 0 failed)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed (0 warnings)

### Risks/Limitations

1. **Linux command strings for non-apt managers are community-sourced**: As noted in the task, only apt is officially Flutter-documented. The `note` field is used for an alternate-manager hint to mitigate wrong package names on uncommon distros — consistent with the task spec.

2. **Windows winget path detection**: The winget check runs at `build_steps()` time (when the preflight report is applied), not at probe time. On a Windows machine that gains/loses winget between preflight runs, the guided command may be stale until `r` re-check is pressed. This is acceptable for the intended use case.
