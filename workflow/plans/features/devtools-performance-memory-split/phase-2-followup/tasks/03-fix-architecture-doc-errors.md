## Task: Fix M3 + m2 — ARCHITECTURE.md doc errors

**Objective:** Correct two factual errors in the Phase 2 additions to `docs/ARCHITECTURE.md`: the wrong field name on the `OverBudget` hint variant (M3), and the stale `PerfSection` variant name (m2). Both are single-line edits in the same doc section.

**Depends on:** — (Wave 1)

**Agent:** doc_maintainer

**Estimated Time:** 0.5 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` — two targeted line edits in the "Performance Panel Interactivity" section.

**Files Read (Dependencies):**
- `crates/fdemon-core/src/frame_hints.rs:70-74` — to verify the actual `OverBudget` variant field names.
- `crates/fdemon-app/src/session/performance.rs` (lines 18-24) — to verify the actual `PerfSection` variant names.

### Background

Phase 2's T07 (the doc-maintainer task) added a "Performance Panel Interactivity" section to ARCHITECTURE.md that introduced the `frame_hints` module, the dual-pane layout, the `PerfDetailsTab` enum, and the new state fields. Two factual errors slipped through:

- **M3 — `docs/ARCHITECTURE.md:1091`** — The `OverBudget` row in the `FrameHint` variants table is documented as `OverBudget { budget_ms, actual_ms }`. The source defines `OverBudget { excess_ms: f64, budget_ms: f64 }`. `actual_ms` (= total frame elapsed time) and `excess_ms` (= elapsed - budget) carry different semantics. A Phase 3 implementor reading the doc to write additional hint message strings would construct the wrong payload.
- **m2 — `docs/ARCHITECTURE.md:1044`** — The `PerfSection` description says "has two variants — `FrameChart` and `DetailsTab`". The actual variant name is `Details` (per `session/performance.rs:18-24`). The Phase 2 TASKS.md notes explicitly rejected the `DetailsTab` rename ("PerfSection rename rejected. Code uses `PerfSection::Details`"), so the doc is stale.

### Details

#### 1. M3 — Fix `OverBudget` variant signature (line 1091)

Locate the row in the `FrameHint` variants table that reads `OverBudget { budget_ms, actual_ms }`. Replace `actual_ms` with `excess_ms` and reorder so it matches the source declaration order:

**Before:**
```
| `OverBudget { budget_ms, actual_ms }` | ... |
```

**After:**
```
| `OverBudget { excess_ms, budget_ms }` | ... |
```

Also update any prose in the same section that mentions `actual_ms` in the context of `OverBudget` (search the doc for `actual_ms` — there should be one occurrence to fix). Ensure the description-column text accurately reflects that `excess_ms` is the *overage above budget*, not the full elapsed time.

#### 2. m2 — Fix `PerfSection` variant name (line 1044)

Locate the sentence in the "Performance Panel Interactivity" section that reads:

> `PerfSection` has two variants — `FrameChart` and `DetailsTab`.

Replace `DetailsTab` with `Details`. The corrected sentence reads:

> `PerfSection` has two variants — `FrameChart` and `Details`.

Audit the surrounding paragraph for any other use of `DetailsTab` that refers to the `PerfSection` variant (versus the unrelated `state::DetailsTab` Inspector type — that one keeps its name). If found, also replace.

### Content-Boundary Reminders

This task touches `docs/ARCHITECTURE.md` which is `doc_maintainer`-managed. Confirm against [`~/.claude/skills/doc-standards/schemas.md`](file:///Users/ed/.claude/skills/doc-standards/schemas.md):

- **Allowed in ARCHITECTURE.md:** Module / type / variant names, layer-boundary descriptions, data-flow diagrams, render-hint Cell field declarations.
- **NOT allowed:** Build commands, key-binding documentation, coding conventions, error-handling patterns.

This task's edits are pure name/signature corrections — fully within content boundaries.

### Acceptance Criteria

1. `docs/ARCHITECTURE.md:1091` (or wherever the `OverBudget` variant row lives in the table) uses `OverBudget { excess_ms, budget_ms }`.
2. `docs/ARCHITECTURE.md:1044` (or wherever the `PerfSection` variant list appears) says `Details`, not `DetailsTab`.
3. No other content changes — no rephrasing, no section reorganisation, no diagram changes.
4. `git diff` on this task shows only the two corrections (plus any prose references to `actual_ms` in the same section that were also fixed).

### Testing

- N/A — doc-only change.
- Manually `grep -n 'actual_ms\|DetailsTab' docs/ARCHITECTURE.md` after editing to verify no stale references remain in the Phase 2 section. (The `state::DetailsTab` Inspector type can legitimately appear elsewhere; only the `PerfSection`-context occurrence is wrong.)

### Risk

- Very low. Two targeted line edits in a doc file with no compile-time impact.

### Out of Scope

- Do NOT touch any other doc files. T04 handles `REVIEW_FOCUS.md`.
- Do NOT rewrite or expand the Performance Panel Interactivity section. Only the two specific errors.
- Do NOT modify ASCII diagrams. The variant-name fix is sentence-level only.
- Do NOT rename the `OverBudget` enum field in source code — that field is correctly named in source. Only the doc is wrong.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Fixed `PerfSection` variant name (`DetailsTab` -> `Details`) at line 1044; fixed `OverBudget` field names (`budget_ms, actual_ms` -> `excess_ms, budget_ms`) and clarified description at line 1091 |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: N/A

### Notable Decisions/Tradeoffs

1. **`actual_ms` description updated**: The description column was updated to clarify that `excess_ms` is the overage above budget (not the full elapsed time), which is a necessary clarification given the semantics differ from the stale `actual_ms` name.
2. **Inspector `DetailsTab` left untouched**: The `DetailsTab` on line 914 refers to the `state::DetailsTab` Inspector type, which is unrelated to `PerfSection`. It is correct and was not modified.
