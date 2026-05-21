## Task: Update CONFIGURATION.md

**Objective**: Remove the obsolete "Behavior change (post-v0.5.0)" callout that referenced the old migration banner, and document the new `[behavior] version_check` opt-out key.

**Depends on**: 02-config-key, 03-banner-refactor

**Agent:** implementor

**Estimated Time**: 0.5-1 hour

### Scope

**Files Modified (Write):**
- `docs/CONFIGURATION.md`: 
  - Delete the migration callout at line 272 (the `> **Behavior change (post-v0.5.0):** Cache-driven auto-launch is now opt-in …` blockquote).
  - In the `[behavior]` section, add a row/subsection documenting `version_check`:
    - Default: `true`
    - Description: On startup, fdemon queries the GitHub releases API to see if a newer version is available; if so, a one-line banner appears above the New Session Dialog. Set to `false` to disable the check entirely (no outbound HTTP).
  - Keep all other `[behavior]` documentation (`confirm_quit`, `auto_launch`) intact.

**Files Read (Dependencies):**
- `docs/CONFIGURATION.md` itself — match the existing formatting/heading style for behavior-table fields.

### Details

The current callout to delete (line 272):

```markdown
> **Behavior change (post-v0.5.0):** Cache-driven auto-launch is now opt-in via `[behavior] auto_launch = true`. If you were relying on `settings.local.toml` to silently auto-launch on each run (the behavior introduced by commit `c5879fa`), add `auto_launch = true` to `[behavior]` in your `config.toml`. This change does **not** affect users who use per-config `auto_start = true` — that path is unchanged.
```

**Why delete instead of rewrite**: The change is no longer "post-v0.5.0" — we're now ~5 versions past that point. The `[behavior] auto_launch` key itself is still documented elsewhere in CONFIGURATION.md; readers who care about the opt-in can find it there. The historical callout is noise.

**New version_check entry** — match the existing style for `auto_launch`. Approximate copy:

```markdown
### `version_check`

- **Type:** boolean
- **Default:** `true`

On startup, fdemon queries the GitHub releases API for the latest `fdemon` release and, if a newer version is available, displays a one-line banner above the New Session Dialog: `⬆ New version available: v<latest> (current v<current>)`.

Set to `false` to disable the check entirely:

\`\`\`toml
[behavior]
version_check = false
\`\`\`

When disabled, no outbound HTTP requests are made on startup. Network failures during the check are silent — no banner appears and no error is logged at user-visible levels.
```

Adapt heading depth (`###` vs `####` etc.) and styling to match whatever convention CONFIGURATION.md already uses for sibling `[behavior]` keys.

### Acceptance Criteria

1. `grep -n "Cache-driven" docs/CONFIGURATION.md` returns no matches.
2. `grep -n "version_check" docs/CONFIGURATION.md` returns at least one match in the `[behavior]` section.
3. The document still renders cleanly (no orphaned headings, no broken cross-references).
4. The `[behavior] auto_launch` documentation is preserved.

### Notes

- This is a pure documentation edit — no code changes. Implementor allowed (not doc_maintainer territory) because `docs/CONFIGURATION.md` is not in the core-docs list.
- ARCHITECTURE.md updates are handled separately in task 05b by `doc_maintainer`.

---

## Completion Summary

**Status:** Done
**Branch:** feat/version-check-banner

### Files Modified

| File | Changes |
|------|---------|
| `docs/CONFIGURATION.md` | Removed stale "Behavior change (post-v0.5.0)" blockquote; added `version_check` to property table, example code block, and `#### version_check` subsection under Behavior Settings |

### Notable Decisions/Tradeoffs

1. **Heading depth `####` not `###`**: The existing `[behavior]` keys (`confirm_quit`, `auto_launch`) are documented only via the property table with no individual sub-headings. Using `####` keeps `version_check` as a subsection of `### Behavior Settings` rather than a peer section alongside `### Watcher Settings`, which better reflects the document's existing style.

2. **Table row added in addition to subsection**: The subsection provides narrative context while the table row gives the same quick-reference format as the other two keys — consistent with how readers scan the reference.

### Testing Performed

- `grep -n "Cache-driven" docs/CONFIGURATION.md` — Empty (PASS)
- `grep -n "version_check" docs/CONFIGURATION.md` — 5 matches in [behavior] section (PASS)
- `grep -c "auto_launch" docs/CONFIGURATION.md` — 9 occurrences preserved (PASS)
- Visual review of surrounding headings and document flow — Clean, no orphaned headings

### Risks/Limitations

1. **No code changes**: This is a pure documentation edit; no compilation or test verification needed.
