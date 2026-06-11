## Task: Update Documentation for the Platforms submenu (Phase 2)

**Agent:** doc_maintainer

**Objective**: Update `docs/ARCHITECTURE.md` so it reflects the new install-wizard Platforms submenu —
the renamed/added `WizardStepKind` variants, the expandable parent/leaf model, the `platforms_expanded`
state, and the `build_steps(report, expanded)` projection.

**Depends on**: 01-enum-datamodel-rename, 02-expand-collapse-nav, 03-tui-indent-caret-height

**Estimated Time**: ~1 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` — the install-wizard subsystem description.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules.
- `crates/fdemon-app/src/install_wizard/{types.rs,state.rs}` — final `WizardStepKind` variants,
  `WizardStep.indent`, `InstallWizardState.platforms_expanded`, `build_steps` signature.
- `crates/fdemon-app/src/handler/install_wizard/navigation.rs` — `handle_toggle_expand`, Esc tiering.
- The Phase 2 task files (this directory) for change context.

### Change Context

Phase 2 changed the install wizard from a flat 5-step list to a flat list with an **expandable Platforms
submenu**:

1. **`WizardStepKind`**: `AndroidTools` renamed to `PlatformAndroid`; added `Platforms` (non-executable
   parent) + `PlatformIos`, `PlatformMacos`, `PlatformWeb`, `PlatformWindows` leaves (+ `is_platform_leaf()`).
   In Phase 2 only `PlatformAndroid` is functional; the rest are host-gated placeholders.
2. **State model**: `WizardStep` gained `indent: u8`; `InstallWizardState` gained `platforms_expanded: bool`.
   `build_steps(report, expanded)` projects the visible list (collapsed = parent only; expanded = parent +
   host-applicable leaves, gated by `report.platform`). The parent's status rolls up its leaves.
3. **Interaction**: `Enter` on the parent toggles expansion (`InstallWizardToggleExpand` →
   `handle_toggle_expand`); `Esc` collapses an expanded submenu before closing (with `selected_index`
   clamping).
4. **TUI**: leaf rows are indented; the parent shows a `▸`/`▾` caret; the step-list height is dynamic; the
   footer hints expand/collapse on the parent.

### Acceptance Criteria

1. The install-wizard description in `docs/ARCHITECTURE.md` (the `install_wizard/` and
   `widgets/install_wizard/` entries and any prose) accurately reflects the Platforms submenu, the new
   `WizardStepKind` variants, `platforms_expanded`/`indent`, and the `build_steps(report, expanded)` model.
2. No content-boundary violations (architecture/structure only — no how-to or config detail that belongs
   in CONFIGURATION.md/KEYBINDINGS.md).
3. Targeted edits only — do not rewrite unrelated sections.
4. Cross-references remain valid.

### Notes

- Follow `~/.claude/skills/doc-standards/schemas.md` strictly.
- Note the Phase-2 scope clearly: only Android is functional; iOS/macOS/Web/Windows are placeholders whose
  detection + guided commands arrive in Phases 3–5. Don't document detection that doesn't exist yet.
- `docs/KEYBINDINGS.md` (expand/collapse keys) and the website docs are handled outside this task
  (implementor-editable; website deferred to the platform-content phases).

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Updated install-wizard subsystem description for Phase 2 Platforms submenu: new WizardStepKind variants (Platforms parent + 5 platform leaves + is_platform_leaf()), WizardStep.indent: u8, InstallWizardState.platforms_expanded: bool, build_steps(report, expanded) two-mode projection, rollup_step_statuses for parent status, handle_toggle_expand + Esc collapse tier in navigation.rs, InstallWizardToggleExpand message, TUI indent/caret/dynamic-height changes in step_list.rs; renamed all AndroidTools references to PlatformAndroid throughout. |

### Notable Decisions/Tradeoffs

1. **Phase 2 scope boundary**: Documented that only PlatformAndroid is functional in Phase 2; the four other platform leaves (PlatformIos/PlatformMacos/PlatformWeb/PlatformWindows) are explicitly noted as host-gated placeholders (StepStatus::Pending) whose detection arrives in Phases 3–5. This is stated in both the project-structure tree and the module reference table.
2. **Targeted edits only**: All seven edit locations are in install-wizard-specific entries; no unrelated sections were touched. Cross-references from Data Flow, Key Types, and Message Categories sections were all updated to use the new variant names.

### Testing Performed

Verified all edits against the live source: WizardStepKind variants from types.rs, WizardStep.indent and InstallWizardState.platforms_expanded from state.rs, build_steps signature and rollup_step_statuses from state.rs, handle_toggle_expand and Esc collapse tier from navigation.rs, InstallWizardToggleExpand from message.rs. No AndroidTools references remain in ARCHITECTURE.md.

### Risks/Limitations

None. The documented types and signatures exactly match the code as read.
