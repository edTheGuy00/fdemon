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
</content>
