## Task: Update Documentation for Phase 5

**Agent:** doc_maintainer

**Objective**: Update core docs to reflect Phase 5's new CLI `doctor` subcommand,
download safety (disk/network preflight + abortable downloads), and the
wizard→device-discovery handback; document the `Esc`-cancel keybinding.

**Depends on**: 01, 02, 03, 04, 05, 06, 07

**Estimated Time**: 1.5-2 hours

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`:
  - New binary CLI surface: `fdemon doctor` subcommand + `src/doctor.rs` module
    (calls `toolchain::run_preflight`, prints a text report, exit 0/1).
  - `toolchain/download.rs` gains disk-space (`fs4`) + network (HEAD) **preflight**
    and **cancellation** (`CancellationToken`, `tokio::select!`, `.part` Drop guard);
    note the daemon's new `fs4`/`tokio_util` deps and the `Error::Cancelled` variant.
  - Install-wizard data flow: after a successful Flutter install, preflight
    completion (or manual close) **hands back to device discovery** once
    `flutter_executable()` resolves; note the SDK re-resolution step and the
    `install_task` (`JoinHandle` + token) held on `InstallWizardState`.
- `docs/KEYBINDINGS.md`:
  - `Esc` in the Install Wizard now **cancels a running install step** when one is in
    progress (and still closes the wizard when idle).

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md`: content boundary rules.
- Phase 5 task files 01–07 for change context.
- `docs/ARCHITECTURE.md` existing `toolchain/` and `UiMode::InstallWizard` sections
  (extend, don't rewrite).

### Change Context

1. **CLI surface** (task 05): first subcommand (`doctor`) on a previously flat-arg
   binary; new `src/doctor.rs`. `fdemon setup` is explicitly deferred.
2. **Download safety** (tasks 01, 02): disk/network preflight + abortable downloads;
   new daemon dependencies (`fs4`, `tokio_util`) and a `Cancelled` error.
3. **Wizard handback** (tasks 03, 04): the wizard now re-triggers device discovery
   and auto-closes once Flutter is live; new `install_task` handle + cancel message.
4. **Keybinding** (tasks 03, 06): `Esc`-cancel-while-running.

### Acceptance Criteria

1. ARCHITECTURE.md accurately reflects the CLI `doctor` subcommand, the download
   preflight/cancellation surface, the new daemon deps, and the handback data flow —
   as **targeted edits** to the existing `toolchain/` and wizard sections.
2. KEYBINDINGS.md documents the `Esc`-cancel-when-running behavior.
3. No content-boundary violations (architecture content only in ARCHITECTURE.md, key
   bindings only in KEYBINDINGS.md); no new config implies **no** CONFIGURATION.md
   change.
4. Cross-references valid; no whole-document rewrites.

### Notes

- Follow content boundaries strictly — see `~/.claude/skills/doc-standards/schemas.md`.
- Resumable downloads, fish `conf.d`, and `fdemon setup` are **deferred** — mention
  them only if ARCHITECTURE.md has a Future Enhancements / deferred section;
  otherwise omit.
- Do not document implementation line numbers — describe modules and data flow.

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/toolchain-bootstrap
