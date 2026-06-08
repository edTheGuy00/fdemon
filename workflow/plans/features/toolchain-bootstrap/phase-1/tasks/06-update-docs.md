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

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Added `toolchain/` subsystem table entries and project tree under `fdemon-daemon`; added `install_wizard/` module and `handler/install_wizard/` to `fdemon-app` section and project tree; added `widgets/install_wizard/` panel to `fdemon-tui` section and project tree; added `InstallWizard` to `is_modal_ui_mode()` modal list; added `RunToolchainPreflight` `UpdateAction` variant and `ToolchainPreflightCompleted` `Message` variant to Key Types; updated startup sequence to note InstallWizard open path; updated `fdemon-daemon` and `fdemon-app` public API surface sections. |
| `docs/CODE_STANDARDS.md` | Added `InstallWizard` to the modal-precedence list in the Region Registry Pattern step 3 guidance. |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: YES/NO/N/A — N/A (no violations found)

### Notable Decisions/Tradeoffs

1. **Startup sequence step renumbering**: Inserted the InstallWizard/SDK-resolution check as step 9 and renumbered the subsequent steps (10-12). This accurately reflects the conditional branch in the startup path.
2. **Public API surface for toolchain types**: Added `run_preflight` and the report types to the `fdemon-daemon` public API section as they are the primary surface consumed by `fdemon-app`'s `RunToolchainPreflight` action; the internal check/doctor implementation details remain `pub(crate)`.
3. **handler/install_wizard/ placement in project tree**: Represented as a sibling entry to `install_wizard/` under `fdemon-app/src/` in the tree, consistent with how `handler/devtools/` is shown relative to `handler/` elsewhere.
