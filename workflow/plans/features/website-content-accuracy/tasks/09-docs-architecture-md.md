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

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | No changes required — already correct |

### Verification Results

| Checklist Item | Status | Notes |
|---|---|---|
| 1. Crate list/layering | PASS | All 6 entries present: `fdemon-core`, `fdemon-daemon`, `fdemon-app`, `fdemon-tui`, `fdemon-dap`, `flutter-demon` binary |
| 2. `fdemon-dap` present with layer/deps | PASS | Described in Layered Architecture table (line 174), Project Structure (lines 417–441), Module Reference (lines 659–685), API Surface (lines 2198–2213) |
| 3. No phantom "Common"/"Services" crates | PASS | "Services" in overview diagram labelled `(controllers)` is clearly a module within `fdemon-app`, not a crate |
| 4. `update()` signature accuracy | PASS | Lines 2114–2133 describe `UpdateResult` accurately (`message`, `action`, `extra_actions`); matches actual `pub fn update(state: &mut AppState, message: Message) -> UpdateResult` |
| 5. Per-session DevTools state + native-log capture | PASS | DevTools state covered at lines 899–935; native-log capture covered at lines 1794–1988 |

### Content Boundary Compliance

- All updates within correct document boundaries: N/A (no edits made)
- Cross-contamination detected and fixed: N/A

### Notable Decisions/Tradeoffs

1. **No edits made**: All 5 checklist items were verified as already correct. The document was previously updated (visible in recent commits to `docs/ARCHITECTURE.md`). Making unnecessary changes would introduce noise.
</content>
