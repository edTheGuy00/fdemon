## Task: Delete `?1003 DECSET` from `docs/ARCHITECTURE.md`

**Agent:** doc_maintainer

**Objective:** Remove the factually-incorrect `(?1003 DECSET)` parenthetical from the `SetMouseCapture(bool)` description in `docs/ARCHITECTURE.md`. The parent fix's entire purpose was to drop `?1003`; the doc currently teaches the opposite. Five of six review agents flagged this independently. Task 10 of the parent plan explicitly forbade naming `?1003` in this doc.

**Depends on:** None

**Estimated time:** 15 minutes

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`: line 1666 (search for "SetMouseCapture(bool)" — the line containing "(?1003 DECSET)").

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md`: content boundary rules (terminal-protocol detail does not belong in ARCHITECTURE.md).
- `workflow/plans/bugs/log-text-selection-broken/tasks/10-architecture-doc.md`: original task spec, which says: "Do NOT document the `?1003` DECSET decision in ARCHITECTURE.md — it is a terminal-protocol detail, not an architectural concern."

### Change Context

The parent fix (`workflow/plans/bugs/log-text-selection-broken/`) removed `?1003` from the mouse DECSET sequence — that is the root-cause fix. `docs/ARCHITECTURE.md:1666` currently reads:

> `SetMouseCapture(bool)` — Instruct the TUI runner to enable or disable terminal mouse capture (`?1003` DECSET).

This is wrong on two levels: (1) `?1003` is no longer used; the actual modes are `?1000`/`?1002`/`?1006`. (2) Naming any DECSET mode in ARCHITECTURE.md violates the doc-boundary rule (terminal-protocol details belong in `terminal.rs` comments and `BUG.md`, not in the architecture doc).

### Required Edit

Change the line to remove the parenthetical entirely. Suggested replacement:

> `SetMouseCapture(bool)` — Instruct the TUI runner to enable or disable terminal mouse capture. The runner performs the synchronous terminal write outside the TEA pipeline.

The second sentence is optional but clarifies the architectural role (which IS in scope for this doc).

### Acceptance Criteria

1. `grep -n '1003' docs/ARCHITECTURE.md` returns no matches.
2. `grep -n 'DECSET' docs/ARCHITECTURE.md` returns no matches (terminal-protocol detail also out of scope; if there are pre-existing matches not introduced by the parent fix, leave them alone but verify).
3. The `SetMouseCapture(bool)` description still exists and conveys its architectural role (TEA → runner side-effect).
4. No other content in ARCHITECTURE.md is changed.

### Notes

- Follow content boundaries strictly per `~/.claude/skills/doc-standards/schemas.md`.
- The `WriteClipboard { text }` description on the next line (1667) is fine as-is — leave it alone.
- This is a single-token deletion plus optional clarifying sentence. No refactor.

---

## Completion Summary

**Status:** Done
**Branch:** plan/log-text-selection-fix

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Removed `(?1003` DECSET)` parenthetical from `SetMouseCapture(bool)` description; added "outside the TEA pipeline" clarifying phrase |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: YES (terminal-protocol detail `?1003 DECSET` removed from ARCHITECTURE.md)

### Notable Decisions/Tradeoffs

1. **Clarifying phrase retained**: Added "outside the TEA pipeline" to preserve architectural meaning (the runner acts synchronously, bypassing TEA) while keeping the terminal-protocol detail out of scope.
