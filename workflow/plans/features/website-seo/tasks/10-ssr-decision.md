## Task: SSR/SSG deferral decision record (Tier 3)

**Objective**: Record, for the future, why a full Leptos SSR/SSG migration is deferred and
what would trigger revisiting it.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**
- `workflow/plans/features/website-seo/DECISION-ssr.md` (new).

**Files Read (Dependencies):**
- `workflow/plans/features/website-seo/PLAN.md`: research + references.

### Details

Capture:
- **Decision:** Stay CSR + build-time prerender (S07). Do not migrate to Leptos SSR/SSG now.
- **Rationale:**
  - Leptos SSG is buggy in 0.7/0.8 (issues #3226, #3822, #3871 — fallback-only output,
    broken static route context).
  - SSR requires a running Rust server (loses nginx-static simplicity), dual feature
    flags, two build targets, hydration testing.
  - Only ~11 mostly-static routes; the prerender already gives all crawlers full HTML, so
    SSR's marginal SEO benefit is small.
  - Niche audience discovers via GitHub/Reddit/HN; search is a secondary channel.
- **Revisit triggers:** site grows to many dynamic routes; search becomes a primary growth
  channel; Leptos SSG bugs are fixed and a one-command static export works.
- Link the research sources from `PLAN.md` References.

### Acceptance Criteria

1. `DECISION-ssr.md` exists with decision, rationale, and revisit triggers.

### Notes

- Documentation-only task; no website code changes.

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `workflow/plans/features/website-seo/DECISION-ssr.md` | Created new decision record |

### Notable Decisions/Tradeoffs

1. **Structure:** Organized the record as Decision / Context / Rationale (4 numbered points) / Chosen Alternative / Trigger Conditions / References — maps directly to the task's required sections and is easy to scan.
2. **Bug numbers:** All three Leptos SSG issue numbers (#3226, #3822, #3871) included with links and a short description of each so a future reader can check their status without searching.
3. **Deployment diagram:** Added a short ASCII flow under "Chosen Alternative" to make the CSR + prerender pipeline concrete and grounded in the actual repo structure (`trunk build` -> `dist/` -> prerender -> nginx).

### Testing Performed

- File created at correct path `workflow/plans/features/website-seo/DECISION-ssr.md` — verified.
- All four required content areas present: decision statement, rationale (4 sub-points), chosen alternative, revisit triggers.
- All research sources from `PLAN.md` References section linked in the decision record.

### Risks/Limitations

1. **Leptos bug status:** Issue numbers and summaries are based on the PLAN.md research findings; the actual open/closed state of those issues was not re-verified at time of writing — the record is accurate as a historical snapshot and instructs the reader to check current status.
</content>
