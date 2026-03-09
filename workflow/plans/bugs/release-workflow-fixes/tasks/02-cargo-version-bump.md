## Task: Automate Cargo.toml Version Bump in Release Workflow

**Objective**: Update the release workflow to automatically bump the `[workspace.package] version` in `Cargo.toml` and commit it before creating the git tag, so that `fdemon --version` reports the correct release version.

**Depends on**: None

### Scope

- `.github/workflows/release.yml`: Add version bump, lockfile update, and commit steps to the `version` job

### Details

**Problem:** The `version` job computes a version and creates a tag, but never updates `Cargo.toml`. The binary is built with `version = "0.1.0"` hardcoded in `[workspace.package]`. All sub-crates use `version.workspace = true`, so only the root `Cargo.toml` needs changing.

**Changes to `.github/workflows/release.yml`:**

Insert these steps in the `version` job, **after** "Compute next version" and **before** "Create and push tag":

```yaml
- name: Install Rust toolchain
  uses: dtolnay/rust-toolchain@stable

- name: Update Cargo.toml version
  run: |
    VERSION="${{ steps.compute.outputs.version }}"
    # Update [workspace.package] version (first version = "..." in file)
    sed -i 's/^version = ".*"/version = "'"$VERSION"'"/' Cargo.toml
    # Verify
    grep -q "^version = \"$VERSION\"" Cargo.toml || { echo "::error::Failed to update version"; exit 1; }
    echo "Updated Cargo.toml to version $VERSION"

- name: Update Cargo.lock
  run: cargo generate-lockfile

- name: Commit version bump
  run: |
    git config user.name "github-actions[bot]"
    git config user.email "github-actions[bot]@users.noreply.github.com"
    git add Cargo.toml Cargo.lock
    git commit -m "chore(release): bump version to ${{ steps.compute.outputs.version }}"
```

**Modify** the existing "Create and push tag" step to push both the commit and tag:

```yaml
- name: Create and push tag
  run: |
    git tag "${{ steps.compute.outputs.tag }}"
    git push origin HEAD:main --follow-tags
```

**Important detail about `sed`:** The root `Cargo.toml` has two version-related lines:
- Line 7: `version = "0.1.0"` (under `[workspace.package]`) — this is the target
- Line 67: `version.workspace = true` (under `[package]`) — this must NOT be changed

The `sed` pattern `^version = ".*"` only matches `version = "X.Y.Z"` (with quotes), not `version.workspace = true`, so it's safe.

### Acceptance Criteria

1. After a release workflow run, `Cargo.toml` on `main` has the new version
2. `Cargo.lock` is updated to reflect the new version
3. The version bump commit uses message `chore(release): bump version to X.Y.Z`
4. The git tag points to the version bump commit (not the previous commit)
5. Both the commit and tag are pushed to `main`
6. The commit does NOT appear in the changelog (already handled by `skip = true` for `chore(release)` in `cliff.toml`)
7. Build jobs checkout the updated code and produce binaries with the correct version

### Testing

- Verify locally by simulating the sed command:
  ```bash
  # Dry run — check what sed would change
  sed -n 's/^version = ".*"/version = "0.3.0"/p' Cargo.toml
  # Should output: version = "0.3.0"
  ```
- After first release with this change, verify: `fdemon --version` outputs the correct version

### Notes

- The `git config` for user name/email is duplicated from the existing tag step — could consolidate, but keeping it explicit per step is clearer for CI
- `cargo generate-lockfile` is preferred over `cargo update --workspace` as it's lighter and doesn't pull new dependency versions
- If branch protection on `main` blocks the push, the workflow may need a PAT or GitHub App token instead of `GITHUB_TOKEN`. By default, `GITHUB_TOKEN` can push to the default branch from Actions
- The build jobs already use `actions/checkout@v4` which will fetch the tag ref, so they'll get the updated `Cargo.toml` automatically

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `.github/workflows/release.yml` | Added 4 new steps to the `version` job between "Compute next version" and "Create and push tag": Install Rust toolchain, Update Cargo.toml version (sed), Update Cargo.lock (cargo generate-lockfile), Commit version bump. Modified "Create and push tag" to use `git push origin HEAD:main --follow-tags` instead of pushing only the tag ref. |

### Notable Decisions/Tradeoffs

1. **`cargo generate-lockfile` over `cargo update`**: Used as specified in the task — lighter weight and does not pull new transitive dependency versions, only regenerates the lockfile to reflect the new workspace version.
2. **sed pattern safety**: The pattern `^version = ".*"` only matches the `version = "0.1.0"` line under `[workspace.package]` (line 7 of Cargo.toml). The `version.workspace = true` line at line 67 starts with `version.` (dot), not `version =` with a quoted value, so it is unaffected.
3. **`git config` in Commit step only**: The `git config` user name/email is set once in "Commit version bump" rather than in "Create and push tag". The tag step no longer needs it since `git tag` inherits the already-configured identity.
4. **`--follow-tags` push**: Pushing with `HEAD:main --follow-tags` ensures the version bump commit lands on `main` before the tag ref is pushed, so the tag points to the version bump commit (not a detached state).

### Testing Performed

- `sed -n 's/^version = ".*"/version = "0.3.0"/p' Cargo.toml` dry-run — Passed (output: `version = "0.3.0"`, exactly one match, `version.workspace = true` not matched)
- Reviewed final workflow YAML for correct step ordering and indentation — Passed

### Risks/Limitations

1. **Branch protection on `main`**: If the repository has branch protection rules that require PRs, the `GITHUB_TOKEN`-based push via `git push origin HEAD:main` will fail with a 403. In that case a PAT or GitHub App token with bypass permissions would be needed. This is noted in the task but not addressed here, as it requires repository admin configuration outside the workflow file.
2. **Checkout ref**: The `actions/checkout@v4` in the `version` job uses the default ref (the triggering branch/SHA). If the workflow is dispatched from a branch other than `main`, the push to `HEAD:main` could overwrite unrelated changes. This is an existing workflow design concern, not introduced by this change.
