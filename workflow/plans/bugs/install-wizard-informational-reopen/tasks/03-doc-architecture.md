## Task: Update Documentation for origin-gated wizard handback

**Agent:** doc_maintainer

**Objective**: Update `docs/ARCHITECTURE.md` to reflect the new `WizardOrigin` enum and the
origin-gated handback in the Install Wizard subsystem.

**Depends on**: 01-core-origin-fix

**Estimated Time**: 0.5–1 hour

### Scope

**Files Modified (Write):**

- `docs/ARCHITECTURE.md` — update the `install_wizard` module descriptions to document
  `WizardOrigin` and the `Bootstrap`-only handback. Targeted edits to the existing entries:
  - The `install_wizard/types.rs` description — add `WizardOrigin { Bootstrap, UserInvoked }`.
  - The `install_wizard/state.rs` description — add the `origin` field, `is_bootstrap()`, and
    `all_components_ok()`.
  - The `handler/install_wizard/actions.rs` description — note that the post-install handback
    (`close_wizard_and_dispatch_discovery`) now only fires for a `Bootstrap`-origin wizard and is
    additionally skipped when a session is already running; a `UserInvoked` open is a read-only
    informational view that returns to `UiMode::Normal` on close.

**Files Read (Dependencies):**

- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules.
- `workflow/plans/bugs/install-wizard-informational-reopen/BUG.md` — change context.
- The task 01 implementation files for accurate symbol names.

### Change Context

1. **New type `WizardOrigin`** distinguishes a startup/bootstrap open from a user-invoked
   informational open of the Install Wizard.
2. **Handback is now origin-gated**: only a `Bootstrap` wizard auto-advances to device discovery
   after the toolchain becomes healthy; `UserInvoked` shows installed-component info and returns
   to Normal. This fixes the bug where pressing `I` on a healthy toolchain dropped the user into
   the new-session dialog.

### Acceptance Criteria

1. ARCHITECTURE.md accurately describes `WizardOrigin` and the origin-gated handback.
2. No content-boundary violations (architecture content only; keys belong in KEYBINDINGS.md,
   updated in task 01).
3. Edits are targeted — do not rewrite the document or unrelated sections.

### Notes

- Match the existing terse, parenthetical style of the `install_wizard` entries in the project
  structure tree (they already enumerate Phase 2/3/4 additions).
- `docs/KEYBINDINGS.md` is updated in task 01 (implementor-editable), not here.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Three targeted edits to install_wizard entries: types.rs description adds WizardOrigin { Bootstrap, UserInvoked }; state.rs description adds origin field, is_bootstrap(), all_components_ok(); actions.rs description documents origin-gated close_wizard_and_dispatch_discovery (Bootstrap-only, skipped when session running; UserInvoked returns to Normal). |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: NO (N/A)

### Notable Decisions/Tradeoffs

1. **Terse parenthetical style**: Matched the existing Phase 2/3/4 annotation style used throughout the install_wizard entries — one-line additions appended to existing file descriptions rather than new sections.
2. **No keybindings**: `I`-key behaviour is already documented in KEYBINDINGS.md (task 01); not duplicated here per task instructions.

### Testing Performed

- Verified symbol names against implemented source: `WizardOrigin`, `is_bootstrap()`, `all_components_ok()`, `close_wizard_and_dispatch_discovery`, `origin` field.
- Content boundary check: no build commands, no code samples, no keybindings — architecture module descriptions only.
