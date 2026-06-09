## Task: Document `fdemon doctor` WebBrowser non-gating in ARCHITECTURE.md

**Objective**: Update the `docs/ARCHITECTURE.md` `fdemon doctor` entry to reflect Task 01: `WebBrowser` is a
**non-gating** component — it is printed in the doctor listing but never fails the exit code, mirroring the
wizard's non-blocking `Missing → Partial` treatment of the Web leaf.

**Depends on**: Task 01 (the doctor non-gating behaviour must be implemented first).

**Agent:** doc_maintainer

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` — the `fdemon doctor` subcommand entry.

**Files Read (Dependencies):**
- `src/doctor.rs` (post-Task-01) — the implemented gating rules.
- `~/.claude/skills/doc-standards/schemas.md` — content-boundary rules.

### Details

The `fdemon doctor` entry in `docs/ARCHITECTURE.md` (the subcommands table, ~line 741) currently documents
exit-code gating as: core components always gate; **Android** components gate only when an Android SDK is
present. After Task 01, add the **WebBrowser** rule:

- `WebBrowser` is **non-gating** — a missing/absent web browser is printed in the component listing but does
  **not** contribute to a failing exit code, so a browser-less host (CI container, headless server) with a
  healthy Flutter + Android toolchain exits `0`.
- Frame it as consistent with the Install Wizard's non-blocking Web semantics (the wizard caps a missing
  browser to `Partial`; the doctor consumer exempts it from gating) — both consumers treat Web as optional.

Keep the edit within the existing entry's style (one concise addition to the gating description); do not
duplicate the full gating algorithm or config-key details into ARCHITECTURE.md.

### Acceptance Criteria

1. The `fdemon doctor` ARCHITECTURE.md entry documents WebBrowser as non-gating (printed, never fails exit).
2. The note is consistent with the wizard's non-blocking Web wording already present in the
   `install_wizard/state.rs` entry.
3. Stays within doc content boundaries (structural/behavioural description, no algorithm dump); `doc-validate`
   passes.

### Notes

- Source `//!` module doc in `src/doctor.rs` is updated by Task 01 (implementor-editable). This task only
  touches the managed `docs/ARCHITECTURE.md`.
- No `docs/CONFIGURATION.md` change needed (this is exit-code behaviour, not a config key).
