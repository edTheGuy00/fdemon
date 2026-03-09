## Task: Regenerate CHANGELOG.md

**Objective**: Regenerate the full `CHANGELOG.md` using the updated `cliff.toml` config so that historical changelog entries include previously-dropped non-conventional commits.

**Depends on**: 01-fix-changelog-config, 02-cargo-version-bump

### Scope

- `CHANGELOG.md`: Full regeneration from git history

### Details

After `cliff.toml` is updated (Task 1), the full changelog needs to be regenerated so the v0.2.0 section (and any other historical sections) include all commits — not just conventional ones.

**Command:**
```bash
git cliff -o CHANGELOG.md
```

This regenerates the entire file from all tags in the git history. The header from `cliff.toml` is applied automatically.

**Expected changes to v0.2.0 section:**
The following commits should now appear (previously dropped):
- `Feat/auto changelog website (#7)` → under "Other Changes"
- `Feat/responsive session dialog (#5)` → under "Other Changes"
- `Feat/session resilience (#3)` → under "Other Changes"

### Acceptance Criteria

1. `CHANGELOG.md` is regenerated with all tags represented
2. The v0.2.0 section includes the 3 previously-dropped commits
3. The v0.1.0 section is unchanged (all commits there were already conventional)
4. No `WIP`, `index on`, or `Merge branch` entries appear (handled by skip rules from Task 1)
5. The unreleased section shows commits since v0.2.0

### Testing

```bash
# Verify the previously-dropped commits now appear
grep -i "auto changelog website" CHANGELOG.md
grep -i "responsive session dialog" CHANGELOG.md
grep -i "session resilience" CHANGELOG.md

# Verify WIP commits are NOT present
grep -c "^WIP" CHANGELOG.md  # should be 0
```

### Notes

- This is a one-time regeneration; future releases will generate correct changelogs automatically
- The regenerated file will differ from the current one significantly since many entries were previously missing
- Review the output before committing to ensure no unexpected entries appear

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `CHANGELOG.md` | Full regeneration from git history using `git cliff -o CHANGELOG.md` |

### Notable Decisions/Tradeoffs

1. **Merge commit bodies are included verbatim**: The "Other Changes" entries for the 3 PR merge commits include the full squash commit body text (all sub-commit messages). This is expected given `split_commits = false` in cliff.toml — git-cliff treats the entire merge commit as a single entry. The PR title appears as the bullet and the body follows.

2. **WIP/index on/Merge branch skip rules work correctly**: All 3 skip patterns from Task 01 (cliff.toml) function as expected — zero matching lines appear in the output.

### Testing Performed

- `grep -i "auto changelog website" CHANGELOG.md` - Passed (found in v0.2.0 Other Changes)
- `grep -i "responsive session dialog" CHANGELOG.md` - Passed (found in v0.2.0 Other Changes)
- `grep -i "session resilience" CHANGELOG.md` - Passed (found in v0.2.0 Other Changes)
- `grep -c "^WIP" CHANGELOG.md` - Passed (count = 0)
- `grep -c "^index on" CHANGELOG.md` - Passed (count = 0)
- `grep -c "^Merge branch" CHANGELOG.md` - Passed (count = 0)
- Version sections present: [Unreleased], [0.2.0], [0.1.0] - All confirmed

### Risks/Limitations

1. **Verbose Other Changes entries**: Merge commit bodies are long (include all squash messages). This is cosmetically verbose but technically correct — the information is there and all 3 previously-dropped commits are now represented.
