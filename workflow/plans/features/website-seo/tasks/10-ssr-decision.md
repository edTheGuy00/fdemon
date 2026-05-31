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
</content>
