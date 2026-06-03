## Task: Update Documentation for Toolchain Preflight & Install Wizard (Phase 1)

**Agent:** doc_maintainer

**Objective**: Update core project documentation to reflect the new `toolchain/` subsystem in
`fdemon-daemon` and the new `UiMode::InstallWizard` modal, and register the wizard in the
modal-precedence list.

**Depends on**: 01, 02, 03, 04, 05

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`:
  - Add the `toolchain/` subsystem under `fdemon-daemon`'s Module Reference (the
    `crates/fdemon-daemon/src/` tree and the module table): `toolchain/mod.rs` (`run_preflight`),
    `types.rs` (report types), `checks.rs` (structured component probes), `doctor.rs`
    (`flutter doctor -v` capture + marker parser). Note it is **read-only diagnostics** in Phase 1.
  - Add `UiMode::InstallWizard` and the `install_wizard/` feature module to `fdemon-app`'s
    structure/tables; add `handler/install_wizard/` (navigation + actions); add the
    `widgets/install_wizard/` panel to `fdemon-tui`.
  - Note the new `UpdateAction::RunToolchainPreflight` → `Message::ToolchainPreflightCompleted`
    async flow (preflight on a background task).
- `docs/CODE_STANDARDS.md`:
  - Add `InstallWizard` to the modal-precedence list in the Region Registry / modal-precedence
    section (the list currently naming `Startup`, `NewSessionDialog`, `ConfirmDialog`, `Settings`,
    `LinkHighlight`, `FlutterVersion`).

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules.
- All Phase 1 task files + the resulting source for change context.

### Change Context

1. **New subsystem** (`fdemon-daemon/src/toolchain/`): structured, read-only toolchain diagnosis
   (`run_preflight` → `ToolchainReport`) reusing `find_flutter_sdk` + `probe_flutter_version`, plus
   `flutter doctor -v` text capture/parse. No new dependencies; no install/network code in Phase 1.
2. **New UI mode** (`UiMode::InstallWizard`): a two-pane diagnostics modal (step list + detail +
   embedded doctor view) modeled on `UiMode::FlutterVersion`, opened at startup when no Flutter SDK
   resolves and via `I` from Normal mode.
3. **Modal precedence**: `InstallWizard` joins the mouse-suppression modal list.

### Acceptance Criteria

1. ARCHITECTURE.md accurately documents the `toolchain/` modules and the `InstallWizard` UI mode
   across the daemon/app/tui crate sections, consistent with existing table/tree formatting.
2. CODE_STANDARDS.md modal-precedence list includes `InstallWizard`.
3. No content-boundary violations (architecture content only in ARCHITECTURE.md; convention/list
   content in CODE_STANDARDS.md).
4. All required sections present per schemas.md; cross-references valid.

### Notes

- `docs/KEYBINDINGS.md` (wizard keys) is updated by task 03 (implementor-editable) — do **not**
  duplicate it here.
- `docs/CONFIGURATION.md` is **not** touched in Phase 1 — no `[toolchain]` config keys are added
  until Phase 2/3.
- Make targeted edits; do not rewrite whole documents. Follow content boundaries strictly.
