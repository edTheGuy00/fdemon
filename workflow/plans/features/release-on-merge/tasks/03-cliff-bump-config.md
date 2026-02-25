## Task: Add bump configuration to cliff.toml

**Objective**: Add a `[bump]` section to `cliff.toml` so that `git cliff --bumped-version` can correctly compute the next semantic version from conventional commits.

**Depends on**: None

**Wave**: 1 (parallel)

### Scope

- `cliff.toml`: **Edit** (append section)

### Details

The `release.yml` workflow uses `git cliff --bumped-version` to auto-compute the next version. For this to work correctly, `cliff.toml` needs a `[bump]` section.

**Append to the end of `cliff.toml`** (after the existing `[git]` section which ends at line 46):

```toml

[bump]
initial_tag = "v0.1.0"
```

This tells git-cliff:
- `initial_tag` — the base version to use if no tags exist in the repo
- Default bump rules apply: `feat:` → minor bump, `fix:` → patch bump, `BREAKING CHANGE` / `!` → major bump

### Current file structure (for reference)

```
cliff.toml
├── [changelog]     (lines 1-23)  — header, body template, footer
└── [git]           (lines 25-46) — conventional commits, parsers, tag pattern
                    ← ADD [bump] here
```

### Acceptance Criteria

1. `cliff.toml` has a `[bump]` section
2. `initial_tag = "v0.1.0"` is set
3. Existing `[changelog]` and `[git]` sections are unchanged

### Testing

- Run `git cliff --bumped-version` locally to verify it computes a version
- Expected: something like `v0.2.0` (minor bump since there are `feat:` commits since `v0.1.0`)

### Notes

- The `[bump]` section has additional optional fields (like `features_always_bump_minor`, `breaking_always_bump_major`) but the defaults match conventional semver rules, which is what we want
- This is a 3-line addition — smallest task in the wave

---

## Completion Summary

**Status:** Not Started
