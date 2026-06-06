## Task: Update ARCHITECTURE.md for the Phase 6 install-wizard fixes

**Agent:** doc_maintainer

**Objective:** Reflect the Phase 6 behavioural changes in the core architecture doc.
This is a docs-only task routed to `doc_maintainer` (the only agent permitted to
edit `docs/ARCHITECTURE.md`).

**Depends on:** 01, 02, 03 (documents their combined, merged behaviour)

**Estimated Time:** 1–1.5 hours

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`

### Details

Update the existing toolchain / install-wizard entries to describe:

1. **Detail-pane wrapping & layout (Task 01).** In the `fdemon-tui`
   `install_wizard/` widget description, note that `step_detail.rs` /
   `doctor_view.rs` now wrap long content lines (`Paragraph::wrap`, per-item height
   advance) instead of clipping into 1-row rects, and that the panel uses ~85%
   height / 28% left pane / 2-row header (`MIN_RENDER_HEIGHT == 12`). Note that the
   guided-command windowing (`command_block_height` / `guided_section_full_height`)
   accounts for wrapped row counts.

2. **Per-manager JDK command (Task 02).** In the `install_wizard/state.rs` entry,
   note `jdk_guided_command` now takes `&ToolchainReport` and dispatches the JDK
   package on `report.linux_package_manager` (pacman/dnf/yum/zypper/apt), matching
   `prerequisites_guided_commands`.

3. **Filtered Linux prerequisites + new probes (Task 02).** In the
   `toolchain/checks/prerequisites.rs` entry, note that `check_linux_prerequisites`
   now also probes GLU (`pkg-config --exists glu`) and libstdc++ (compiler-presence
   heuristic), and that `prerequisites_guided_commands` (Linux) filters the install
   command to only-missing packages via `parse_missing_prereq_keys`, mapping each
   key to the distro-specific package name.

4. **Android PATH + fallback (Task 03).** In the `toolchain/path_config.rs` entry,
   note `add_android_env` now also writes `$ANDROID_HOME/emulator` (in addition to
   `cmdline-tools/latest/bin` + `platform-tools`), and that the PathConfig executor
   falls back to `resolve_android_sdk_root_path(None)` (filtered by `is_dir()`) when
   `settings.toolchain.android_sdk_root` is unset, so an out-of-band Android SDK
   still gets `ANDROID_HOME` written.

### Acceptance Criteria

1. The four behavioural changes above are reflected accurately in the relevant
   existing ARCHITECTURE.md entries (no new top-level sections needed — extend the
   existing module-reference rows/notes).
2. No content-boundary violations (no build/run commands, no how-to-use prose —
   those belong in DEVELOPMENT/CONFIGURATION, which are out of scope here).
3. Wording matches the merged implementation (verify against the final code, not
   this task file, in case task details shifted during implementation).

### Notes

- `CONFIGURATION.md` and `KEYBINDINGS.md` are intentionally **not** touched — Phase 6
  adds no config keys and no keybindings.
- Keep the edits surgical; these are amendments to existing entries.
