## Task: Update Documentation for Phase 2 Follow-up Changes

**Agent:** doc_maintainer

**Objective**: Update `docs/ARCHITECTURE.md` to reflect the Phase 2 follow-up changes: expanded `DiagnosticsNode` field sanitization, the unified stale-guard key for properties + layout handlers, the new total-budget timeout semantics for `spawn_fetch_inspector_properties`, and the title-bar-based main-axis label presentation in the Flex Explorer.

**Depends on**: 01-flex-explorer-visual-fix, 02-handler-stale-guard-unification, 03-actions-inspector-hardening, 04-core-diagnostics-name-sanitize

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules
- `workflow/plans/features/devtools-inspector-parity/phase-2-followup/tasks/01-flex-explorer-visual-fix.md` — completion summary
- `workflow/plans/features/devtools-inspector-parity/phase-2-followup/tasks/02-handler-stale-guard-unification.md` — completion summary
- `workflow/plans/features/devtools-inspector-parity/phase-2-followup/tasks/03-actions-inspector-hardening.md` — completion summary
- `workflow/plans/features/devtools-inspector-parity/phase-2-followup/tasks/04-core-diagnostics-name-sanitize.md` — completion summary
- Current `docs/ARCHITECTURE.md` DevTools Subsystem section (already updated by Phase 2 task 10)

### Change Context

Summarize what implementation changes require doc updates:

1. **Expanded ANSI sanitization on `DiagnosticsNode`** (from task 04): Five additional string fields now strip ANSI at deserialize time. The existing ARCHITECTURE.md "DevTools Subsystem" section mentions sanitization in passing — add an explicit list of sanitized `DiagnosticsNode` fields so future contributors know which fields are pre-sanitized vs which still need careful rendering.

2. **Unified stale-guard key** (from task 02): Both properties and layout fetch handlers now stale-check on `state.devtools.inspector.details_node_id` (previously: properties used `pending_*_node_id`, layout used `selected_value_id()`). This is a TEA-pattern-relevant change worth one sentence in the "Inspector Properties Fetch" / "Inspector Widget Tree Fetch" sections.

3. **Total-budget timeout semantics** (from task 03): The `PROPERTIES_FETCH_TIMEOUT` is now a true total wall-clock budget, not per-RPC. Update the existing "Inspector Properties Fetch (Two-Stage Pipeline)" subsection (added by Phase 2 task 10) to clarify this.

4. **Flex Explorer title bar carries axis labels** (from task 01): The block title now shows both main-axis and cross-axis labels; the side strip carries only arrows. If ARCHITECTURE.md has any prose describing the Flex Explorer's visual layout, update it. (Likely no update needed if the doc only describes data flow, not visual layout.)

### Acceptance Criteria

1. The `DiagnosticsNode` sanitization coverage is enumerated in ARCHITECTURE.md (one new bullet listing the sanitized fields: `description`, `property_type`, `name`, `level`, `node_type`, `style`, `value_id`).
2. The stale-guard key change is documented in the "Inspector Properties Fetch" subsection — single sentence noting `state.devtools.inspector.details_node_id` as the unified comparison key.
3. The total-budget timeout description in the "Inspector Properties Fetch (Two-Stage Pipeline)" subsection accurately reflects the new outer-timeout wrapper.
4. No content boundary violations (architecture content only in ARCHITECTURE.md; no build commands, no code style content).
5. All required sections per `~/.claude/skills/doc-standards/schemas.md` remain valid.
6. Edits are targeted/surgical (no rewrites of existing prose unrelated to these four changes).

### Notes

- Follow content boundaries strictly — see `~/.claude/skills/doc-standards/schemas.md`.
- Make targeted edits, do not rewrite entire sections.
- If the existing "Inspector Properties Fetch (Two-Stage Pipeline)" subsection currently implies per-RPC timeouts (it should not, since Phase 2 task 10 was written before the change), correct it.
- Do NOT add cleanup/follow-up plan content to ARCHITECTURE.md — those belong in `workflow/plans/`.
- Do NOT remove or rename existing sections.
- The visual layout of the Flex Explorer title is an implementation detail that probably does not need ARCHITECTURE.md mention. Skip it unless the existing doc already describes the visual structure.

---

## Completion Summary

**Status:** Not Started
