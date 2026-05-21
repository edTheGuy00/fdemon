## Task: Add user-facing privacy disclosure for the GitHub version check

**Objective**: Document that `fdemon` issues one outbound HTTPS request to `api.github.com` on every TUI launch (when `version_check = true`, the default), what is transmitted, and how to opt out.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**

- `docs/CONFIGURATION.md`: Insert a "Privacy" subsection inside the existing `#### version_check` block (currently around line 275–289). The subsection should appear immediately after the opt-out example.

- `README.md`: Add a one-paragraph "Privacy" section near the bottom (after Installation / before License, wherever logically fits the current README structure). Cross-reference the longer disclosure in `docs/CONFIGURATION.md`.

**Files Read (Dependencies):**

- `docs/CONFIGURATION.md`: existing `version_check` subsection — match formatting.
- `README.md`: existing section structure — match heading depth and tone.

### Details

**CONFIGURATION.md — proposed copy** (drop in after the opt-out example block):

```markdown
##### Privacy

When `version_check` is enabled (default), fdemon issues a single HTTPS GET request to
`https://api.github.com/repos/edTheGuy00/fdemon/releases/latest` per launch (or once per
24-hour cache window, whichever is more recent). The request contains:

- A `User-Agent: fdemon/<version>` header
- An `Accept: application/vnd.github+json` header

No other data is transmitted. The source IP is visible to GitHub as with any HTTPS request.

Set `version_check = false` to disable this behavior entirely — no outbound request will be
made, and the on-disk cache will not be created.
```

(Heading depth `#####` because `version_check` is `####` per task 05a; if depth differs in
the actual file, match the existing style.)

**README.md — proposed copy** (one paragraph, plain prose):

```markdown
## Privacy

On startup, fdemon checks GitHub for newer releases by issuing a single HTTPS request to
`api.github.com`. No personal data is transmitted beyond a `User-Agent: fdemon/<version>`
header and the request itself. Disable this by setting `version_check = false` under
`[behavior]` in `.fdemon/config.toml`. See `docs/CONFIGURATION.md` for full details.
```

### Acceptance Criteria

1. `grep -n "Privacy" docs/CONFIGURATION.md` returns at least one match inside the `version_check` block.
2. `grep -n "Privacy" README.md` returns at least one match.
3. `grep -n "api.github.com" README.md` returns at least one match — the readme explicitly names the endpoint.
4. The CONFIGURATION.md privacy block names the cache TTL (mentions "24-hour" so it remains accurate after task 04 lands the cache).
5. Both disclosures name `version_check = false` as the opt-out toggle.

### Testing

This is a docs-only task. No code-level tests are added. Manual smoke:

- Render `docs/CONFIGURATION.md` and `README.md` (e.g., GitHub preview or `mdcat`) — verify headings nest correctly and the new content is not orphaned.

### Notes

- This task can land before task 04 even though it references the cache TTL — the cache copy is forward-looking; if task 04 changes the TTL, this doc edit needs an update.
- Tone: neutral disclosure, not apologetic. fdemon is a developer tool that does an entirely conventional update check.
- Do not list the request headers fdemon does NOT send — that's a security-theater style. Just state what IS sent.

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/version-check-banner-followup

### Files Modified

| File | Changes |
|------|---------|
| | |

### Notable Decisions/Tradeoffs

1. **<Decision>**: <Rationale>

### Testing Performed

- `grep -n Privacy docs/CONFIGURATION.md` — Pending
- `grep -n Privacy README.md` — Pending
- Visual review of rendered markdown — Pending

### Risks/Limitations

1. **<Risk>**: <Description>
