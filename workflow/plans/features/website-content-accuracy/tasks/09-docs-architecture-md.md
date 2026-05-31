## Task: Update Documentation — docs/ARCHITECTURE.md (verify 5-crate workspace)

**Agent:** doc_maintainer

**Objective**: Verify the canonical architecture doc reflects the current 5-crate
workspace and fix only real drift.

**Depends on**: None

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`: correct any drifted crate/layer/data-flow description.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md`: content boundary rules.
- `Cargo.toml`, `crates/` tree, `CLAUDE.md`: source of truth.

### Change Context

This is the source the website's stale Architecture page (T06) should have matched. Fix
only real drift.

Checklist:
1. Crate list/layering matches: `fdemon-core`, `fdemon-daemon`, `fdemon-app`,
   `fdemon-tui`, `fdemon-dap`, `flutter-demon` binary.
2. `fdemon-dap` is present and its layer/dependencies are described.
3. No phantom "Common"/"Services" *crates* (services is a module in `fdemon-app`).
4. Data-flow / `update()` signature `(AppState, Option<UpdateAction>)` is accurate.
5. Per-session DevTools state and native-log capture modules described (per `CLAUDE.md`).

### Acceptance Criteria

1. Crate structure and layering match `Cargo.toml` + `crates/`.
2. No content boundary violations; `doc-validate` passes for `docs/ARCHITECTURE.md`.
3. PR notes list already-correct vs. fixed items.

### Notes

- Make targeted edits, do not rewrite the whole document.
- Follow content boundaries strictly — see `~/.claude/skills/doc-standards/schemas.md`.
</content>
