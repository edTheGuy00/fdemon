## Task: Update Documentation for Phase 3 (Android tools + JDK + guided commands)

**Agent:** doc_maintainer

**Objective**: Update `docs/ARCHITECTURE.md` to reflect the Phase 3 additions: the
new `toolchain/android_install.rs` and `toolchain/jdk.rs` modules, the
`process_stream.rs` stdin-feeding addition, the `path_config.rs` generalized
`ANDROID_HOME` writer, the new install types, the `GuidedCommand` wizard model, and
the now-executable Android Tools wizard step.

**Depends on**: 02, 03, 06, 07, 08

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`: update the `fdemon-daemon` `toolchain/` module table and
  the `fdemon-app` install-wizard rows to document Phase 3 behavior.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md`: content boundary rules.
- task files 02, 03, 06, 07, 08 for change context.
- the implemented sources under `crates/fdemon-daemon/src/toolchain/` and
  `crates/fdemon-app/src/{install_wizard,handler/install_wizard,actions}/`.

### Change Context

1. **New daemon modules:** `toolchain/android_install.rs`
   (`install_android_tools` — cmdline-tools download, `cmdline-tools/latest`
   relocation, `sdkmanager` package install, non-interactive license acceptance,
   streamed via `InstallEvent`) and `toolchain/jdk.rs` (`resolve_jdk_home`,
   `configure_flutter_jdk_dir`).
2. **`process_stream.rs`:** add `run_streaming_with_input` (feeds child stdin while
   streaming merged stdout/stderr — used to answer `sdkmanager --licenses`).
3. **`path_config.rs`:** new `add_android_env` — generalized, distinct-fence,
   idempotent writer for `ANDROID_HOME` + cmdline-tools/platform-tools PATH entries
   (POSIX rc files + Windows user-registry via PowerShell).
4. **`types.rs`:** `AndroidInstallTarget`, `AndroidInstallOutcome`,
   `cmdline_tools_url`, `sdkmanager_packages`, `DEFAULT_CMDLINE_TOOLS_BUILD`.
5. **Wizard:** `GuidedCommand` model (first guided-command surface); the Android
   Tools step is now executable and **gated** on a present JDK 17 (privileged JDK
   install is guided, not auto-run); the PATH Configuration step now also writes
   `ANDROID_HOME`; `c` copies the selected step's guided command. Completion
   persists `[toolchain] android_sdk_root` and re-runs preflight.

### Acceptance Criteria

1. The `toolchain/` module table in ARCHITECTURE.md lists `android_install.rs` and
   `jdk.rs` with accurate one-line descriptions, and the `process_stream.rs` /
   `path_config.rs` / `types.rs` rows reflect the Phase 3 additions.
2. The install-wizard rows (app + tui) document the `GuidedCommand` model, the
   executable+gated Android Tools step, the extended PATH/env step, and the `c` key.
3. No content-boundary violations (architecture-only content in ARCHITECTURE.md;
   config/keybinding specifics go to CONFIGURATION.md/KEYBINDINGS.md via task 10).
4. Edits are targeted (no wholesale rewrite); cross-references remain valid.

### Notes

- Follow content boundaries strictly — see `~/.claude/skills/doc-standards/schemas.md`.
- Also fix the pre-existing Phase 1 inaccuracy noted in Phase 2's orchestration
  notes if still present: `handler/install_wizard/mod.rs` is a re-export shim (not
  "Navigation…"), and `navigation.rs` should appear in the tree — correct these
  while editing nearby rows.
- Keep descriptions to the architectural "what/where", not implementation detail.

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
