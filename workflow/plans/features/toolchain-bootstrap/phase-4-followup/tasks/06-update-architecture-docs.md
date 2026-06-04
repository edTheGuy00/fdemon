## Task: Update ARCHITECTURE.md module table for Phase 4 (+ followup) additions (m10)

**Agent:** doc_maintainer

**Severity:** MINOR (m10)

**Objective**: Refresh the stale per-phase module-table entries in
`docs/ARCHITECTURE.md` so they reflect the Phase 4 additions (and any module-shape
change from the task-04 TEA refactor).

**Depends on**: 04-pure-guided-commands-tea, 05-test-quality-fixes
(run last so the docs capture the final module shape, especially if task 04 moves
detection into `ToolchainReport`)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md`: content-boundary rules.
- The final state of `install_wizard/state.rs`, `handler/install_wizard/navigation.rs`,
  `handler/install_wizard/actions.rs`, and (if task 04 landed the refactor)
  `toolchain/types.rs` / `toolchain/mod.rs`.

### Change Context

The review found these module-table lines stale after Phase 4:

1. **Line ~353 (`install_wizard/state.rs`)** stops at "Phase 3 adds GuidedCommand
   population for AndroidTools (JDK gate)". Phase 4 added: `selected_command_index`
   field, `select_next_command()` / `select_prev_command()`, index-aware
   `selected_guided_command()`, and `prerequisites_guided_commands()` (per-OS install
   command generation).

2. **Line ~357 (`handler/install_wizard/navigation.rs`)** says only "Navigation
   handlers (up/down, pane switch)" — Phase 4 added `handle_prev_command` /
   `handle_next_command` for guided-command index cycling.

3. **Line ~358 (`handler/install_wizard/actions.rs`)** stops at Phase 3 — Phase 4
   split the `Prerequisites` Enter arm out of `Doctor` to emit the guided "Run the
   listed command(s), then press r to re-check." message instead of the "later phase"
   stub.

4. **If task 04 landed the TEA refactor:** also note that `ToolchainReport`
   (`toolchain/types.rs`) now carries pre-computed environment detection
   (package-manager / winget availability) and that `prerequisites_guided_commands`
   is a pure function of the report (no app-land PATH probing). Update the relevant
   `toolchain/types.rs` / `toolchain/mod.rs` / `state.rs` lines accordingly.

### Acceptance Criteria

1. The `state.rs`, `navigation.rs`, and `actions.rs` module-table entries mention the
   Phase 4 additions, matching the established per-phase annotation style.
2. If task 04 moved detection into `ToolchainReport`, the daemon `toolchain` entries
   reflect that (and the `which`-in-`fdemon-app` mention, if any, is corrected).
3. Targeted edits only — no rewrite; no content-boundary violations; only
   `ARCHITECTURE.md` is edited.

### Notes

- Per the Phase 4 plan, **no** new module/layer/data-flow was introduced (and no
  `CONFIGURATION.md` change is warranted) — this is purely keeping the existing
  module table honest. `docs/KEYBINDINGS.md` was already updated in Phase 4 task 06.
- Follow content boundaries strictly (`doc_maintainer` owns ARCHITECTURE.md).
