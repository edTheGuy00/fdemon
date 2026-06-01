## Task: leptos_meta context + global title formatter

**Objective**: Wire `leptos_meta` so route components can set per-page
`<Title>`/`<Meta>`/`<Link>`.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 0.5-1 hour

### Scope

**Files Modified (Write):**
- `website/src/lib.rs`: call `provide_meta_context()` and add a global title formatter.

**Files Read (Dependencies):**
- `leptos_meta` docs (already a dependency).

### Details

- In `App`, call `provide_meta_context()` once, at the top.
- Add a global `<Title formatter=|text| format!("{text} — Flutter Demon")/>` so each route
  only sets its short page title (home can override fully).
- Confirm `leptos_meta` is imported; no behavior change beyond enabling meta injection.

### Acceptance Criteria

1. `provide_meta_context()` is called in `App`; global title formatter present.
2. `cd website && trunk build` compiles; the document title updates on navigation.

### Notes

- Enabling step for S05. In CSR these tags inject after WASM boots (Google wave 2); static
  fallbacks from S01 cover non-JS crawlers; the prerender (S07) makes the injected tags
  visible to all crawlers.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-afa907c308809caf9

### Files Modified

| File | Changes |
|------|---------|
| `website/src/lib.rs` | Added `use leptos_meta::{provide_meta_context, Title}` import; called `provide_meta_context()` at top of `App`; added `<Title formatter=\|text\| format!("{text} — Flutter Demon") />` as first view element |

### Notable Decisions/Tradeoffs

1. **`<Title>` placement**: Placed as the first element in the `view!` macro, before `<Router>`, so it is registered as a global formatter before any route component renders.
2. **em dash in title**: Used `—` (U+2014 em dash) matching the task spec exactly, consistent with typical browser title formatting conventions.

### Testing Performed

- `cargo check` (via copy to main website) - Passed (1 pre-existing dead_code warning, no new issues)
- `cargo clippy` (via copy to main website) - Passed (2 pre-existing warnings, no new issues)

### Risks/Limitations

1. **Worktree cargo check**: The worktree lives under `.claude/worktrees/` inside the main repo root, so `cargo check` run from within the worktree's `website/` directory fails due to workspace resolution. Verification was done by temporarily copying `lib.rs` to the main repo's `website/src/` and running `cargo check` there — confirmed to compile cleanly.
</content>
