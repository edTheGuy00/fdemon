## Task: Landing copy + heading hierarchy (Tier 2)

**Objective**: Make the home page rank for real developer queries and read as substantive
prose, not just a feature grid.

**Depends on**: 05-per-route-meta (home.rs meta); website-content-accuracy plan T01
(data.rs)

**Agent:** implementor

**Estimated Time**: 2-4 hours

### Scope

**Files Modified (Write):**
- `website/src/pages/home.rs`: headings, copy, internal links, alt text.
- `website/src/data.rs`: ensure `features()` reflects multi-launch (coordinate with T01).

**Files Read (Dependencies):**
- `website/src/pages/docs/native_logs.rs`: "Boot your whole stack" framing (from T03).

### Details

- Headings: exactly one `<h1>` (tool name + category, e.g. "Flutter Demon — A Rust TUI for
  Flutter Development"), then `<h2>` sections phrased as developers search: "Real-Time Log
  Viewer", "Hot Reload Without an IDE", "Multi-Device Flutter Sessions", "Built-in
  DevTools".
- Above-the-fold copy: first paragraph states category + differentiator and naturally
  includes primary keywords: *flutter tui*, *flutter terminal ui*, *flutter hot reload
  cli*, *flutter development without IDE*.
- 3–4 genuine paragraphs answering "what is this" and "why use it". Work in the **second
  differentiator** — custom sources as a process orchestrator (fdemon can boot your
  backend, health-check it, then launch Flutter, all in one log view). See the Native Logs
  "Boot your whole stack" section (content plan T03) for accurate framing.
- Internal links with descriptive anchors (not "read more"): e.g.
  `<a href="/docs/installation">Install fdemon with Cargo</a>`, link to Configuration and
  Changelog.
- `features()` (data.rs): ensure the multi-device launch story is reflected; coordinate
  with content plan T01 which also edits `features()` — land T01 first.
- Keep semantic HTML; ensure the hero logo `alt` text is descriptive.

### Acceptance Criteria

1. Home page has a single `<h1>` and keyword-relevant `<h2>` sections.
2. At least 3 paragraphs of descriptive prose with primary keywords used naturally.
3. Internal links use descriptive anchor text.
4. `cd website && trunk build` compiles; layout intact.

### Notes

- Runs after S05 (home.rs) and content T01 (data.rs) to avoid merge conflicts.
</content>
