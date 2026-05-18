## Task: Update documentation for Phase 3 follow-up

**Agent:** doc_maintainer

**Objective**: Update `docs/ARCHITECTURE.md` to reflect the post-followup state: `compute_details_context` honestly performs a single DFS pass, and `DiagnosticsNode::object_id` is now sanitized at the serde boundary like its sibling fields.

**Depends on**: 01-core-depth-and-fuse, 02-handler-clamp-and-tests, 03-tui-render-assert, 04-state-cleanup-dead-code

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — Content boundary rules
- Tasks 01–04 completion summaries — change context
- Current `docs/ARCHITECTURE.md` Inspector Details Tab Visibility section (~lines 978–991) — what to update
- Current `docs/ARCHITECTURE.md` DiagnosticsNode / fdemon-core types entries (~line 1748) — what to update

### Change Context

1. **Task 01 fused the two DFS walks** (M2). `compute_details_context` now performs a single depth-first traversal that captures both the matching node and its parent in one pass. The Phase 3 architecture doc at `docs/ARCHITECTURE.md:986` already says "performs a single depth-first walk" — that claim becomes accurate post-task-01. **No edit may be needed** if the existing text is already correct after the implementation catches up; verify and either leave as-is or refine the wording.

2. **Task 01 also added depth bounding** to the new walker(s) (M1) — they now respect `MAX_TREE_WALK_DEPTH` like every other walker in `widget_tree.rs`. If the architecture doc mentions depth bounding policy generally, ensure the new walker is included or covered by the existing general statement.

3. **Task 01 sanitized `DiagnosticsNode::object_id`** (s3). The current `fdemon-core` types entry for `DiagnosticsNode` may list which fields are ANSI-sanitized; if so, add `object_id` to that list. If the doc only mentions the general policy "all `Option<String>` fields on `DiagnosticsNode` are sanitized at deserialization," then no per-field edit is needed.

4. **Tasks 02, 03, 04 are pure code hygiene** — handler clamp symmetry, renderer dev-assertion, dead-code removal. These do NOT introduce new architectural patterns, modules, or layer changes. **No `ARCHITECTURE.md` edit is expected from these three tasks** unless the existing doc inaccurately describes the renderer or the cycle-tab behavior in a way that conflicts with the post-followup code. Verify by re-reading the relevant sections and only edit if there's drift.

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` Inspector Details Tab Visibility section accurately describes `compute_details_context` as a single-walk function (or remains accurate if it already did).
2. If the DiagnosticsNode types entry enumerates sanitized fields, `object_id` is included; otherwise the general policy statement covers it.
3. No content boundary violations introduced (architecture content only — no implementation prose, no Rust syntax > 3 lines, no task-level commentary).
4. No content from `CODE_STANDARDS.md` or `DEVELOPMENT.md` duplicated into `ARCHITECTURE.md`.
5. Diff is the minimal set of additions/changes — no unrelated sections refactored.
6. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass (verification that the docs-only change does not break the build).

### Notes

- Follow content boundaries strictly — see `~/.claude/skills/doc-standards/schemas.md`.
- Make targeted edits; do not rewrite entire sections.
- **No `KEYBINDINGS.md` edit expected** — Phase 3 follow-up introduces no key changes.
- **No `CODE_STANDARDS.md` edit expected** — no new coding patterns or conventions established. (The `debug_assert!` usage in task 03 is consistent with existing project patterns; no new standard to document.)
- **No `DEVELOPMENT.md` edit expected** — no new build/test commands.
- If after reviewing the current `ARCHITECTURE.md` you determine that **no edit is needed** (because the existing text already accurately describes the post-followup state), document that finding in the Completion Summary and skip writing to the file. A no-op outcome is valid and preferable to making churn-only edits.

---

## Completion Summary

**Status:** Pending
**Branch:** _to be filled_

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | _to be filled_ |

### Notable Decisions/Tradeoffs

1. _to be filled_

### Testing Performed

- _to be filled_

### Risks/Limitations

1. _to be filled_
