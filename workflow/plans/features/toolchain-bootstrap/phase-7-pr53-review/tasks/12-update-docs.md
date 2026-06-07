## Task: Update ARCHITECTURE.md to reflect Phase 7 review-remediation behaviour (docs)

**Severity:** — (documentation)

**Agent:** doc_maintainer

**Objective**: Reflect the behavioural and contract changes from tasks 01–11 in the
core docs, so the documented invariants match the hardened code.

**Depends on**: 01, 02, 03, 04, 05, 06, 08, 09 (and 07/10/11 if they alter
documented behaviour)

**Estimated Time**: 1–1.5 hours

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`
- (only if changed) `docs/DEVELOPMENT.md` — if task 07 promotes `tempfile` to a
  runtime dependency
- (only if changed) `docs/CONFIGURATION.md` — if task 09 adds a `--require-android`
  flag or doctor exit-code semantics worth documenting

**Files Read (Dependencies):**
- The completion summaries of tasks 01–11

### Details / Required Updates

Update the toolchain/install-wizard sections of `docs/ARCHITECTURE.md` to record:

1. **Install-task lifecycle (task 01):** `WizardStepStarted` now carries `run_seq`
   and is discarded when stale; the cross-kind `begin_step` fallback no longer
   silently drops a live cancellation token. Update any description of the
   step-lifecycle/abort-handle handoff.
2. **Windows PATH persistence (task 02):** PATH is written via the raw
   `HKCU:\Environment` value, preserving `REG_EXPAND_SZ` and literal `%VAR%` tokens.
3. **Download pipeline (tasks 03/04):** HTTPS-only with a bounded, no-downgrade
   redirect policy; Android cmdline-tools integrity behaviour; `extract_tar_xz`
   fail-closed on traversal; cancellation honoured through verify/extract and the
   temp-dir guard disarmed only after a successful rename (RAII contract corrected).
4. **Android/JDK install (task 05):** OS-correct child PATH separator; tightened
   JAVA_HOME heuristic; atomic (backup-restore) cmdline-tools relocation.
5. **Wizard re-check (task 06):** `apply_report` resets the per-run `execution`
   display state.
6. **rc-file writes (task 07):** atomic write preserves original permissions and
   uses a unique temp file.
7. **Prerequisite probes (task 08):** pkgconf-aware GTK/GLU probing; Rosetta
   install-based detection.
8. **`fdemon doctor` (task 09):** exit-code semantics (Android optional / CI-gate
   behaviour), and that doctor-incompatible top-level flags are rejected.

Keep edits within `doc_maintainer` content boundaries (system design, modules,
data flow, invariants) — do not duplicate task-file detail.

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` accurately describes the post-Phase-7 invariants for the
   eight areas above; no stale claims remain (e.g. the old temp-dir "removed on any
   failure" wording matches the corrected disarm-after-rename behaviour).
2. The doc-standards audit passes (`/doc-validate` or `doc-standards` skill) with no
   structural/content-boundary violations.
3. Only docs are modified by this task.

### Notes

- Runs last (Wave 3), after the implementation tasks land, so the descriptions match
  the merged behaviour. Routed to `doc_maintainer` (the only agent permitted to edit
  the core docs).

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Updated toolchain/install-wizard sections with post-Phase-7 invariants for all 8 areas (tasks 01-11) |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: NO (N/A — no violations found)

### Notable Decisions/Tradeoffs

1. **task 10 (strip_ansi / version.json) scope**: The strip_ansi fix and VERSION fallback were documented under `flutter_sdk/diagnostics.rs` and `flutter_sdk/locator.rs`/`channel.rs` respectively, as architectural invariants of those modules — within ARCHITECTURE.md scope.
2. **task 11 (VM-service-unavailable hint)**: Documented as a guard invariant in the Session Launch Lifecycle section (the `Running` phase row) rather than as a separate subsection, since it describes a single conditional in the phase-transition logic.
3. **DEVELOPMENT.md not modified**: `tempfile` promotion (task 07) is noted in ARCHITECTURE.md's `fdemon-daemon` Dependencies line. DEVELOPMENT.md does not enumerate per-crate runtime dependency lists at that granularity, so no change was needed there.
4. **CONFIGURATION.md not modified**: task 09 did not add any new CLI flags or configuration keys — it corrected exit-code semantics and rejected incompatible flag combinations. No CONFIGURATION.md entry was warranted.
