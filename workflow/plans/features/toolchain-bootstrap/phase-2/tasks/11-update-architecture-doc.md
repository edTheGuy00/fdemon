## Task: Update Documentation for Phase 2 (ARCHITECTURE.md)

**Agent:** doc_maintainer

**Objective**: Update `docs/ARCHITECTURE.md` to reflect the Phase 2 toolchain
installer subsystem and the wizard step-execution flow.

**Depends on**: 02, 03, 04, 08, 09, 10

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`: document the new modules and the install data flow.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md`: content boundary rules.
- Phase 2 task files 02, 03, 04, 08, 09, 10 for change context.
- The implemented source for accurate file/type names.

### Change Context

1. **New daemon modules** under `crates/fdemon-daemon/src/toolchain/`:
   - `download.rs` — streaming download, SHA-256 verify, zip/tar.xz extraction.
   - `process_stream.rs` — child-process stdout/stderr line streaming.
   - `flutter_install.rs` — releases-manifest fetch, git-clone / archive install,
     `flutter precache`; `InstallEvent`, `FlutterInstallTarget`, `FlutterInstallOutcome`.
   - `path_config.rs` — shell-aware, idempotent, marker-fenced PATH writers;
     `PathConfigOutcome`.
   Update the `fdemon-daemon` module table and the `toolchain/` tree (which
   currently lists only `mod/types/checks/doctor` as "read-only diagnostics
   (Phase 1)") to note Phase 2 adds installation.

2. **New types in `toolchain/types.rs`**: `InstallMethod`, `HostArch`,
   `FlutterRelease`, `FlutterReleaseManifest`, `FlutterInstallTarget`,
   `DownloadProgress`, `FlutterInstallOutcome`.

3. **New `fdemon-daemon` dependencies**: `reqwest`, `zip`, `tar`, `lzma-rs`,
   `sha2` (note the layering rationale: all network/archive code stays inside
   `toolchain/`).

4. **App layer**: `UpdateAction::RunWizardStep`; `Message` variants
   `InstallWizardRunSelectedStep`, `WizardStepStarted/Log/DownloadProgress/
   Completed/Failed`; `InstallWizardState.execution` (`StepExecution`,
   `StepExecStatus`); `[toolchain]` `ToolchainSettings`. The completion flow:
   write `[flutter] sdk_path` → `PersistSettings` → re-run preflight → re-scan FVM.

5. **TUI**: new `widgets/install_wizard/progress.rs`; step-detail action hints.

6. **Data flow**: add a short subsection (or extend the Install Wizard notes)
   describing: `Enter` → `InstallWizardRunSelectedStep` → `RunWizardStep` action →
   `handle_action` spawns daemon install → `WizardStep*` messages stream back →
   completion persists `sdk_path` + re-runs preflight.

### Acceptance Criteria

1. ARCHITECTURE.md accurately lists the new modules, types, and dependencies.
2. The `toolchain/` description is updated from "read-only (Phase 1)" to include
   Phase 2 installation, without deleting the Phase 1 description.
3. No content-boundary violations (no build commands → that's DEVELOPMENT.md; no
   config key reference tables → that's CONFIGURATION.md, handled in task 12).
4. Cross-references remain valid; edits are targeted, not a rewrite.

### Notes

- Follow content boundaries strictly — see `~/.claude/skills/doc-standards/schemas.md`.
- Keep the existing "fdemon-tui consumes daemon display types via fdemon-app
  re-exports" note accurate (no new direct TUI→daemon dependency was added).

---

## Completion Summary

**Status:** Not Started
</content>
