## Task: Fix Changelog Config to Capture All Commits

**Objective**: Update `cliff.toml` so that non-conventional commits are no longer silently dropped from the changelog. All commits between tags should appear.

**Depends on**: None

### Scope

- `cliff.toml`: Change `filter_unconventional` and add catch-all commit parser

### Details

**Problem:** `filter_unconventional = true` (line 27) causes `git-cliff` to silently drop any commit that doesn't match conventional commit format (`type: description`). Between v0.1.0 and v0.2.0, three commits were dropped:
- `Feat/auto changelog website (#7)`
- `Feat/responsive session dialog (#5)`
- `Feat/session resilience (#3)`

**Changes to `cliff.toml`:**

1. **Line 27:** Change `filter_unconventional = true` to `filter_unconventional = false`

2. **Add catch-all parser** at the **end** of the `commit_parsers` array (after the `revert` entry):
   ```toml
   { message = "^revert", group = "Reverted" },
   { message = ".*", group = "Other Changes" },   # <-- add this
   ```

3. **Optional but recommended:** Add skip patterns for noisy commits that shouldn't appear in changelogs:
   ```toml
   { message = "^WIP", skip = true },
   { message = "^index on", skip = true },
   { message = "^Merge branch", skip = true },
   ```
   Place these **before** the catch-all `.*` parser so they take priority.

### Acceptance Criteria

1. `git cliff --latest` includes non-conventional commits under "Other Changes"
2. Conventional commits still appear in their proper groups (Features, Bug Fixes, etc.)
3. `chore(release)` and `chore(deps)` commits are still skipped
4. WIP/merge commits do not appear in the changelog

### Testing

Run these commands to verify:

```bash
# Should show non-conventional commits under "Other Changes"
git cliff --latest

# Full changelog should include all historical commits
git cliff

# Verify the v0.2.0 section now includes Feat/... commits
git cliff v0.1.0..v0.2.0
```

### Notes

- The catch-all parser **must** be the last entry in `commit_parsers` — git-cliff uses first-match semantics
- This change is backwards-compatible; existing conventional commits are unaffected
- WIP and stash commits (e.g. `index on feat/dap-server: ...`) exist in the history and should be skipped

---

## Completion Summary

**Status:** Not Started
