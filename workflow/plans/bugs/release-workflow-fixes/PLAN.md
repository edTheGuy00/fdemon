# Bug Fix: Release Workflow — Version Bump & Changelog Completeness

## Problem Statement

Two issues with `.github/workflows/release.yml`:

1. **Cargo.toml version never gets bumped.** The release workflow creates a git tag (e.g. `v0.3.0`) but the `[workspace.package] version` in `Cargo.toml` stays at `"0.1.0"`. This means `fdemon --version` always reports `0.1.0` regardless of the actual release.

2. **Changelog misses most commits between tags.** The `v0.2.0` release changelog only captured 5 entries out of 8 commits — and the 3 missing ones (`Feat/auto changelog website (#7)`, `Feat/responsive session dialog (#5)`, `Feat/session resilience (#3)`) were dropped because they use **non-conventional commit message format** (e.g. `Feat/...` instead of `feat: ...`). The `cliff.toml` config has `filter_unconventional = true`, which silently drops these.

## Root Cause Analysis

### Issue 1: Cargo.toml version not bumped

The `version` job in `release.yml` computes the version, creates a tag, and pushes it — but never updates `Cargo.toml`. The build jobs then checkout the code and build it with the old `version = "0.1.0"`.

**Current flow:**
```
compute version → create tag → push tag → build → release
```

**Required flow:**
```
compute version → update Cargo.toml → commit → create tag → push both → build → release
```

### Issue 2: Changelog missing commits

**Root cause:** `filter_unconventional = true` in `cliff.toml` (line 27). This setting causes `git-cliff` to silently skip any commit whose message doesn't match conventional commit format (`type: description` or `type(scope): description`).

Between `v0.1.0` and `v0.2.0`, these commits were **dropped** because they don't follow conventional format:
- `Feat/auto changelog website (#7)` — should be `feat: auto changelog website (#7)`
- `Feat/responsive session dialog (#5)` — should be `feat: responsive session dialog (#5)`
- `Feat/session resilience (#3)` — should be `feat: session resilience (#3)`

Two options exist:
- **Option A (recommended):** Set `filter_unconventional = false` and add a catch-all commit parser to group unconventional commits under "Other Changes". This ensures nothing is silently dropped.
- **Option B:** Enforce conventional commits via a CI check or git hook. This prevents the problem going forward but doesn't fix historical gaps.

**Recommendation:** Do both — catch-all parser for safety + a note in contributing docs about conventional commits.

## Affected Files

| File | Change |
|------|--------|
| `.github/workflows/release.yml` | Add Cargo.toml version bump + commit before tagging |
| `cliff.toml` | Set `filter_unconventional = false`, add catch-all parser |
| `Cargo.lock` | Auto-updated when Cargo.toml version changes (by cargo) |

## Plan

### Task 1: Automate Cargo.toml Version Bump in Release Workflow

**File:** `.github/workflows/release.yml`

In the `version` job, after computing the version and before creating the tag, add steps to:

1. Run `sed` to update `version = "..."` under `[workspace.package]` in `Cargo.toml` to the computed version.
2. Run `cargo check --workspace` to regenerate `Cargo.lock` with the new version (or `cargo update --workspace`).
3. `git add Cargo.toml Cargo.lock`
4. `git commit -m "chore(release): bump version to $VERSION"`
5. Then create the tag on the new commit (existing step).
6. `git push origin HEAD:main --follow-tags` (push both the commit and tag together).

**Important considerations:**
- The commit message `chore(release)` is already in the `skip = true` list in `cliff.toml`, so it won't pollute the changelog.
- The workflow needs `contents: write` permission (already present).
- The checkout step needs `ref: main` (or the default branch) and a token with push permissions.

**Sketch of new steps:**
```yaml
- name: Update Cargo.toml version
  run: |
    VERSION="${{ steps.compute.outputs.version }}"
    sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
    # Verify the change
    grep "^version = \"$VERSION\"" Cargo.toml

- name: Update Cargo.lock
  run: cargo update --workspace

- name: Commit version bump
  run: |
    git config user.name "github-actions[bot]"
    git config user.email "github-actions[bot]@users.noreply.github.com"
    git add Cargo.toml Cargo.lock
    git commit -m "chore(release): bump version to ${{ steps.compute.outputs.version }}"

- name: Create and push tag
  run: |
    git tag "${{ steps.compute.outputs.tag }}"
    git push origin HEAD:main --follow-tags
```

**Note:** The `sed` command targets the **first** `version = "..."` line in `Cargo.toml`, which is under `[workspace.package]` (line 7). The root `[package]` section uses `version.workspace = true` (line 67), so it won't be affected. However, to be safe, we should use a more targeted sed pattern that only changes the version within the `[workspace.package]` section.

### Task 2: Fix Changelog to Capture All Commits

**File:** `cliff.toml`

Changes:
1. Set `filter_unconventional = false` (line 27) — stops silently dropping non-conventional commits.
2. Add a catch-all commit parser at the **end** of the `commit_parsers` list:
   ```toml
   { message = ".*", group = "Other Changes" }
   ```
   This ensures any commit that doesn't match a specific conventional type still appears in the changelog.

**Result:** For `v0.3.0`, all commits since `v0.2.0` will appear — conventional ones in their proper groups, unconventional ones under "Other Changes".

### Task 3: Regenerate CHANGELOG.md (One-Time Fix)

After the above changes, regenerate the full `CHANGELOG.md` so historical entries are correct:

```bash
git cliff -o CHANGELOG.md
```

This regenerates the full changelog from all tags. The v0.2.0 section will now include the previously-dropped commits.

## Verification

1. **Version bump:** After running the release workflow, check that `Cargo.toml` has the new version and `fdemon --version` reports it.
2. **Changelog:** Run `git cliff --latest` after the cliff.toml changes and confirm all commits since the last tag appear.
3. **Skip pattern:** Confirm that `chore(release): bump version to X.Y.Z` commits do NOT appear in the changelog (already configured).

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| `sed` modifies wrong version line | Target only `[workspace.package]` section; verify with `grep` |
| Push to main fails (branch protection) | Workflow uses `GITHUB_TOKEN` which bypasses branch protection for GitHub Actions by default; if custom rules exist, may need a PAT or app token |
| `cargo update` fails in CI | Use `cargo generate-lockfile` as fallback; or just run `cargo check` |
| Catch-all parser creates noisy changelogs | Can refine later to skip specific patterns (e.g. `WIP`, `index on`) |
