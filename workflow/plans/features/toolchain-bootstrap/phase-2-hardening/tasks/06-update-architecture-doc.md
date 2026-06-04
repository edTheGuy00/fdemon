# Task 06 — Update ARCHITECTURE.md for Phase 2 hardening

**Agent:** doc_maintainer
**Status:** Not Started
**Depends On:** 01, 02, 03, 04, 05
**Estimated Hours:** 1h
**Module:** `docs/ARCHITECTURE.md`

## Context

The Phase 2 hardening tasks add one new `Message` variant and establish a few security
patterns in the toolchain installer. `docs/ARCHITECTURE.md` must reflect these. This is a
`doc_maintainer` task because ARCHITECTURE.md has enforced content boundaries (no build
commands, no config-key value tables, no code-style rules — those live in
DEVELOPMENT.md / CONFIGURATION.md / CODE_STANDARDS.md).

Read `~/.claude/skills/doc-standards/schemas.md` for content-boundary rules before editing.

## Changes to Document

After tasks 01–05 land, update ARCHITECTURE.md to reflect:

1. **New `Message` variant** — add `WizardStepPhase { kind, label }` to the install-wizard
   Message inventory (alongside `WizardStepStarted/Log/Progress/DownloadProgress/
   Completed/Failed`), noting it routes to `set_step_phase` to drive the live phase row.
2. **Toolchain installer hardening** — in the `fdemon-daemon/toolchain/` module
   descriptions, note that:
   - `download.rs` extraction is traversal-safe (zip-slip / tar path + symlink guards) and
     `.tar.xz` decode is streaming; downloads use timeouts + bounded retry + `.part` files.
   - `flutter_install.rs` validates the `channel` before `git clone` (option-terminated),
     honors the configured channel on the archive path, reclaims incomplete `final_dir`s,
     and serializes installs with an advisory lockfile under the install root.
   - `path_config.rs` passes the Windows PATH value to PowerShell out-of-band (env var,
     not interpolated) and validates/quotes `bin_dir` before writing rc files.
3. **Log tail** — note `StepExecution::log_tail` is a bounded `VecDeque` and streamed
   lines are ANSI-sanitized before rendering (if this rises to architectural relevance;
   otherwise omit).

Keep edits targeted — update the relevant module-reference rows / subsystem notes and the
Message inventory; do not rewrite sections. Match the existing doc style.

## Also Correct (pre-existing carryover)

While editing, fix the known pre-existing inaccuracy noted in the Phase 2 review and
TASKS.md: `handler/install_wizard/mod.rs` is described as "Navigation (up/down, pane
switch)" but is actually a re-export shim — the navigation logic lives in
`handler/install_wizard/navigation.rs`, which is missing from the directory tree. Correct
the label and add the `navigation.rs` row.

## Acceptance Criteria

- [ ] ARCHITECTURE.md lists `WizardStepPhase` in the install-wizard Message inventory.
- [ ] Toolchain installer module descriptions reflect the hardening (traversal-safe
      extraction, streaming xz, channel validation, install lock, out-of-band PowerShell,
      bin_dir validation) accurately and within content boundaries.
- [ ] The `handler/install_wizard/` tree entry is corrected (`mod.rs` re-export shim;
      `navigation.rs` present).
- [ ] No content-boundary violations (no build commands, config value tables, or
      code-style rules added).
- [ ] Descriptions match the actual code on the branch after tasks 01–05 merge.

## Notes

- This is the only sequential task; it runs after all implementation tasks merge so the
  doc matches the final code.
- Commit only the documentation change.
