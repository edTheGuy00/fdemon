## Task: Update Documentation for Inspector Isolate Resolution

**Agent:** doc_maintainer

**Objective**: Update `docs/ARCHITECTURE.md` to reflect the new Flutter UI isolate resolution flow and the readiness-poll model. Do NOT touch code.

**Depends on**: 07-tests-inspector-handlers

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` — "DevTools Subsystem" section (around line 839) and "VM Service Client" mention:
  - Document the `resolve_flutter_ui_isolate` behavior: enumerate isolates → match on `extensionRPCs` containing `ext.flutter.*` → cache → invalidate on hot restart.
  - Note that the readiness poll is now bounded (≤ 2 attempts × 250 ms by default) and is bypassed on `r` refresh.
  - Mention the `FetchTrigger` enum used to differentiate Initial vs Refresh vs AutoRehydrate fetches.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md`: Content boundary rules.
- All implementation tasks 01-07.

### Change Context

The Inspector fetch flow now resolves the Flutter UI isolate explicitly rather than picking the first non-system isolate. Readiness polling has been moved from a 20 s worst-case budget to a 2.5 s budget, with bypass on `r` refresh once a tree has been rendered. These changes are user-relevant insofar as the panel now loads in < 2 s and `r` is responsive after a failure.

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` "DevTools Subsystem" section accurately describes the new flow.
2. No content boundaries violated (no code in ARCHITECTURE.md, no architecture content leaked into CODE_STANDARDS.md / DEVELOPMENT.md).
3. Cross-references (if any) remain valid.
4. No changes to `CODE_STANDARDS.md` or `DEVELOPMENT.md` (no new conventions / commands introduced).

### Notes

- Follow content boundaries strictly per `~/.claude/skills/doc-standards/schemas.md`.
- Make targeted edits — do not rewrite the whole DevTools section.
- The `FetchTrigger` enum is an internal detail; mention it only as a sentence, not a code block.
