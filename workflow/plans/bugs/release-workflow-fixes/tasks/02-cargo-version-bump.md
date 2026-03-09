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

**Status:** Not Started
