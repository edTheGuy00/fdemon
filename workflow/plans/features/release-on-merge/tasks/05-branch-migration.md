## Task: Branch migration and cleanup (manual runbook)

**Objective**: Rename `develop` → `main`, delete all stale branches, and configure GitHub repository settings for trunk-based development.

**Depends on**: Tasks 01, 02, 03, 04 (all Wave 1 tasks must be committed and pushed first)

**Wave**: 2 (sequential)

### Scope

- GitHub repository settings (manual)
- Local and remote git branches (scripted)

### Pre-Flight Check

Before starting, verify all Wave 1 changes are committed and pushed to `develop`:

```bash
git status                    # Clean working tree
git log --oneline -5          # Recent commits include Wave 1 changes
git push origin develop       # Ensure remote is up to date
```

Verify `master` has no unique commits:

```bash
git log master --not develop --oneline
# Should output nothing. If not empty, merge master into develop first.
```

### Step 1: Rename develop → main on GitHub

**Option A: GitHub UI (recommended)**

1. Go to: **Repository → Settings → General → Default branch**
2. Click the edit (pencil) icon next to the default branch
3. Rename `develop` to `main`
4. Confirm the rename

GitHub automatically:
- Updates the default branch
- Retargets all open PRs
- Sets up URL redirects from `develop` to `main`

**Option B: Git CLI**

```bash
# Rename locally
git branch -m develop main

# Push the new name
git push origin main

# Set default branch on GitHub (requires gh CLI)
gh repo edit --default-branch main

# Delete the old remote branch
git push origin --delete develop
```

### Step 2: Delete master branch

```bash
# Delete remote
git push origin --delete master

# Delete local
git branch -D master
```

### Step 3: Delete all stale feature branches

All of these are fully merged into `develop` (verified in conversation). Delete them all.

**Remote cleanup:**

```bash
git push origin --delete \
  local_develop \
  feat/devtools \
  feat/redesign \
  feat/workspace-restructure \
  feat/website \
  feat/udpate-device-selector \
  feat/e2e-testing \
  feat/launch-settings \
  feat/settings \
  feat/refactor-logview \
  feat/hyperlinks \
  fix/major-refactoring
```

**Local cleanup:**

```bash
git branch -D \
  local_develop \
  feat/devtools \
  feat/redesign \
  feat/workspace-restructure \
  feat/website \
  feat/udpate-device-selector \
  feat/e2e-testing \
  feat/launch-settings \
  feat/settings \
  feat/refactor-logview \
  feat/hyperlinks \
  fix/major-refactoring
```

**Prune stale remote-tracking refs:**

```bash
git remote prune origin
```

### Step 4: Update local tracking

```bash
git branch -u origin/main main
git remote set-head origin -a
```

### Step 5: Configure branch protection for main

Go to: **Repository → Settings → Branches → Add branch protection rule**

- **Branch name pattern**: `main`
- [x] Require a pull request before merging
- [x] Require approvals (1)
- [ ] Require status checks to pass (optional — add later when CI is set up on PRs)
- [x] Do not allow bypassing the above settings (optional, can allow for emergencies)

### Step 6: Verify final state

```bash
# Should show only main
git branch -a

# Should show origin/HEAD → origin/main
git remote show origin | head -5

# Tags should be intact
git tag --list 'v*'

# Verify the release workflow is accessible
gh workflow list
```

### Acceptance Criteria

1. `main` is the only branch (local and remote)
2. `main` is the default branch on GitHub
3. All 14 stale branches deleted (local + remote)
4. Tags (`v0.1.0`) are preserved
5. Branch protection is configured on `main`
6. `origin/HEAD` points to `main`

### Notes

- **Tags are commit-level, not branch-level** — deleting branches does not affect tags
- **GitHub redirects** — old `develop` and `master` URLs will redirect to `main` temporarily
- **Contributors' existing clones** — GitHub shows a banner with instructions when they visit the repo:
  ```bash
  git fetch origin
  git branch -m develop main
  git branch -u origin/main main
  git remote set-head origin -a
  ```

---

## Completion Summary

**Status:** Not Started
