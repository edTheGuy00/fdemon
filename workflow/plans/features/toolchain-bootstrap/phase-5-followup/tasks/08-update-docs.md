## Task: Update ARCHITECTURE & KEYBINDINGS for the Phase 5 followup behavioural fixes

**Severity:** — (documentation; routed to `doc_maintainer`)

**Objective**: Reflect the corrected behaviours from tasks 01–07 in the core docs,
without re-describing unchanged Phase 5 surface.

**Depends on**: 01, 02, 03, 04, 06 (the behavioural/data-flow changes)

**Estimated Time**: 1–1.5 hours

### Scope

**Files Modified (Write — `doc_maintainer` only):**
- `docs/ARCHITECTURE.md`
- `docs/KEYBINDINGS.md`

**Files Read (Dependencies):**
- This followup's task files 01–07 + their Completion Summaries.

### Details

Only update what actually changed in behaviour/data-flow:

1. **Handback (Task 01):** the wizard→device-discovery handback transitions to
   `UiMode::Startup` (not `Normal`) so `DevicesDiscovered` populates the new-session
   dialog. If ARCHITECTURE describes the handback flow, correct the mode transition.
2. **Abort/cancel lifecycle (Tasks 02, 03):** the `CancellationToken` is stored
   synchronously at `begin_step`; `WizardInstallTaskReady` carries `kind` + `run_seq`
   and only *upgrades* the stored handle's `join` field. A dedicated
   `StepExecStatus::Cancelled` state renders cancellation neutrally (distinct from a
   genuine `Failed`). Update any state-machine / message-flow description accordingly.
3. **Download safety (Task 04):** note the per-read idle guard (`read_timeout`), the
   RAII temp-dir cleanup that survives `JoinHandle::abort()`, and that `git_install`
   now honours the cancel token. Keep it to module-table / data-flow level.
4. **Doctor (Task 06):** `fdemon doctor` honours `[flutter] sdk_path`; note the
   `fdemon ./doctor` workaround for the bare-token `doctor` collision (F25) in the CLI
   usage section if present.
5. **KEYBINDINGS:** if Task 03 changed how `Esc`-cancel is surfaced (cancelled vs
   failed framing), add/adjust the clarifying note. Otherwise leave the existing
   Phase 5 `Esc` (cancel running step / else close) entry as-is.

### Acceptance Criteria

1. ARCHITECTURE.md reflects the handback `UiMode::Startup` transition, the
   synchronous-token + validated-ready abort lifecycle, the `StepExecStatus::Cancelled`
   state, and the download RAII/cancel notes — at the doc's existing altitude (no code
   dumps).
2. KEYBINDINGS.md `Esc` semantics are accurate for the post-Task-03 behaviour.
3. The `doc-standards` skill / `doc_maintainer` content-boundary rules are respected;
   no `CONFIGURATION.md` change (no new config) and no new keybindings invented.
4. `cargo fmt`/build are unaffected (docs-only).

### Notes

- Route via `doc_maintainer` (the only agent allowed to edit core docs).
- This mirrors Phase 5's own Task 08; keep the diff minimal and factual.
