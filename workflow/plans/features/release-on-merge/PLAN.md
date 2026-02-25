# Trunk-Based Release Strategy

## Overview

Migrate from gitflow (`develop` + `master`) to **trunk-based development** with a single `main` branch. Releases are triggered on-demand via a manual "Release" button in GitHub Actions, with version computation and changelog generation handled automatically by git-cliff.

## Current State

| Component | Current Setup |
|-----------|---------------|
| **Branches** | `develop` (active work), `master` (release), default = `master` |
| **Release trigger** | Manual tag push → `release.yml` |
| **Website trigger** | Tag push OR `develop` push (paths: `website/**`) |
| **E2E trigger** | `main` branch (wrong — should be `master` or `develop`) |
| **Changelog** | git-cliff via `cliff.toml` (conventional commits) |
| **Version** | `workspace.package.version = "0.1.0"` in root `Cargo.toml` |
| **Tags** | `v0.1.0` only |

## Target State

| Component | New Setup |
|-----------|-----------|
| **Branches** | `main` only (trunk) |
| **Release trigger** | `workflow_dispatch` — manual "Release" button |
| **Website trigger** | Part of the release workflow |
| **E2E trigger** | `main` branch (now correct) |
| **Changelog** | Same git-cliff, generated per-release for GitHub Release body |
| **Version** | Tag is the source of truth, computed by `git cliff --bumped-version` |

## Flow

```
Contributors ──PR──→ main (trunk)
                       │
            (work continues, PRs merge freely)
                       │
            Maintainer clicks "Release" in GitHub Actions
                       │
                       ▼
              release.yml (workflow_dispatch)
                       │
         ┌─────────────┼─────────────────┐
         ▼             ▼                  ▼
    version job   (waits for version)  (waits for version)
    git cliff       build-macos          ...
    --bumped-version  build-linux
    → v0.2.0          build-windows
    create tag            │
    push tag              │
         │      ┌────────┘
         ▼      ▼
       release job
       git-cliff --latest → changelog
       Create GitHub Release
       Attach binaries + checksums
              │
              ▼
       publish-site job
       Build website Docker image
       Push to GHCR with version tag
```

**Key properties:**
- Single workflow — no cross-workflow triggers, no PAT required
- Version is computed automatically from conventional commits since last tag
- `feat:` → minor bump, `fix:` → patch bump, `BREAKING CHANGE` → major bump
- No commits to `main` by CI — only a lightweight tag
- Releases happen when you decide, not on every merge

## Changes Required

### 1. REWRITE: `.github/workflows/release.yml`

The existing tag-triggered workflow becomes a single self-contained `workflow_dispatch` workflow that handles: version computation → tagging → building → releasing → website deploy.

Key changes from the current `release.yml`:
- Trigger changes from `push: tags` to `workflow_dispatch`
- New `version` job at the start that computes and creates the tag
- Build jobs read version from `needs.version.outputs.version` instead of `GITHUB_REF_NAME`
- `softprops/action-gh-release` gets explicit `tag_name` instead of auto-detecting from ref
- Website publish becomes a final job (absorbed from `publish-site.yml`)
- Optional `version` input allows manual override

```yaml
name: Release

on:
  workflow_dispatch:
    inputs:
      version:
        description: 'Version override (e.g. 0.3.0). Leave empty for auto-bump from conventional commits.'
        required: false
        type: string

permissions:
  contents: write
  packages: write

env:
  CARGO_TERM_COLOR: always

jobs:
  # ── Step 1: Compute version and create tag ──────────────────────────
  version:
    name: Compute Version
    runs-on: ubuntu-latest
    outputs:
      version: ${{ steps.compute.outputs.version }}
      tag: ${{ steps.compute.outputs.tag }}

    steps:
      - name: Checkout with full history
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install git-cliff
        uses: taiki-e/install-action@v2
        with:
          tool: git-cliff

      - name: Compute next version
        id: compute
        run: |
          if [ -n "${{ inputs.version }}" ]; then
            VERSION="${{ inputs.version }}"
            TAG="v${VERSION}"
            echo "Using manual version: $TAG"
          else
            TAG=$(git cliff --bumped-version 2>/dev/null || true)
            if [ -z "$TAG" ]; then
              echo "::error::No releasable conventional commits found since last tag."
              exit 1
            fi
            LAST=$(git describe --tags --abbrev=0 2>/dev/null || echo "none")
            if [ "$TAG" = "$LAST" ]; then
              echo "::error::No version bump needed ($TAG already tagged)."
              exit 1
            fi
            VERSION="${TAG#v}"
            echo "Auto-computed version: $TAG (was $LAST)"
          fi

          echo "version=$VERSION" >> "$GITHUB_OUTPUT"
          echo "tag=$TAG" >> "$GITHUB_OUTPUT"

      - name: Create and push tag
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git tag "${{ steps.compute.outputs.tag }}"
          git push origin "${{ steps.compute.outputs.tag }}"

  # ── Step 2: Build binaries ──────────────────────────────────────────
  build-macos:
    name: Build macOS (${{ matrix.target }})
    needs: [version]
    runs-on: ${{ matrix.runner }}
    strategy:
      matrix:
        include:
          - target: x86_64-apple-darwin
            runner: macos-latest
          - target: aarch64-apple-darwin
            runner: macos-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2
        with:
          shared-key: ${{ matrix.target }}

      - name: Build
        run: cargo build --release --target ${{ matrix.target }}

      - name: Strip binary
        run: strip target/${{ matrix.target }}/release/fdemon

      - name: Package
        run: |
          VERSION="${{ needs.version.outputs.version }}"
          ARCHIVE="fdemon-v${VERSION}-${{ matrix.target }}.tar.gz"
          cp target/${{ matrix.target }}/release/fdemon ./fdemon
          tar czf "$ARCHIVE" fdemon
          echo "ARCHIVE=$ARCHIVE" >> "$GITHUB_ENV"

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: fdemon-v${{ needs.version.outputs.version }}-${{ matrix.target }}
          path: ${{ env.ARCHIVE }}
          retention-days: 1

  build-linux:
    name: Build Linux (${{ matrix.target }})
    needs: [version]
    runs-on: ubuntu-latest
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            use_cross: false
          - target: aarch64-unknown-linux-gnu
            use_cross: true

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2
        with:
          shared-key: ${{ matrix.target }}

      - name: Install cross
        if: matrix.use_cross
        run: cargo install cross --git https://github.com/cross-rs/cross

      - name: Build (cargo)
        if: ${{ !matrix.use_cross }}
        run: cargo build --release --target ${{ matrix.target }}

      - name: Build (cross)
        if: matrix.use_cross
        run: cross build --release --target ${{ matrix.target }}

      - name: Strip binary (x86_64 only)
        if: ${{ !matrix.use_cross }}
        run: strip target/${{ matrix.target }}/release/fdemon

      - name: Package
        run: |
          VERSION="${{ needs.version.outputs.version }}"
          ARCHIVE="fdemon-v${VERSION}-${{ matrix.target }}.tar.gz"
          cp target/${{ matrix.target }}/release/fdemon ./fdemon
          tar czf "$ARCHIVE" fdemon
          echo "ARCHIVE=$ARCHIVE" >> "$GITHUB_ENV"

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: fdemon-v${{ needs.version.outputs.version }}-${{ matrix.target }}
          path: ${{ env.ARCHIVE }}
          retention-days: 1

  build-windows:
    name: Build Windows (x86_64)
    needs: [version]
    runs-on: windows-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-pc-windows-msvc

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2
        with:
          shared-key: x86_64-pc-windows-msvc

      - name: Build
        run: cargo build --release --target x86_64-pc-windows-msvc

      - name: Package
        shell: pwsh
        run: |
          $version = "${{ needs.version.outputs.version }}"
          $archive = "fdemon-v${version}-x86_64-pc-windows-msvc.zip"
          Copy-Item "target\x86_64-pc-windows-msvc\release\fdemon.exe" -Destination "fdemon.exe"
          Compress-Archive -Path fdemon.exe -DestinationPath $archive
          "ARCHIVE=$archive" | Out-File -FilePath $env:GITHUB_ENV -Append

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: fdemon-v${{ needs.version.outputs.version }}-x86_64-pc-windows-msvc
          path: ${{ env.ARCHIVE }}
          retention-days: 1

  # ── Step 3: Create GitHub Release ───────────────────────────────────
  release:
    name: Create GitHub Release
    needs: [version, build-macos, build-linux, build-windows]
    runs-on: ubuntu-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Generate release changelog
        uses: orhun/git-cliff-action@v4
        id: changelog
        with:
          config: cliff.toml
          args: --latest --strip header
        env:
          OUTPUT: CHANGES.md

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts/
          merge-multiple: true

      - name: Generate SHA256 checksums
        working-directory: artifacts/
        run: sha256sum fdemon-v*.tar.gz fdemon-v*.zip > checksums-sha256.txt

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: ${{ needs.version.outputs.tag }}
          draft: false
          prerelease: false
          body_path: CHANGES.md
          files: |
            artifacts/fdemon-v*.tar.gz
            artifacts/fdemon-v*.zip
            artifacts/checksums-sha256.txt

  # ── Step 4: Deploy Website ──────────────────────────────────────────
  publish-site:
    name: Publish Website
    needs: [version, release]
    runs-on: ubuntu-latest

    permissions:
      contents: read
      packages: write

    env:
      REGISTRY: ghcr.io
      IMAGE_NAME: edtheguy00/flutter-demon-site

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to GitHub Container Registry
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Build and push
        uses: docker/build-push-action@v6
        with:
          context: ./website
          file: ./website/Dockerfile
          push: true
          tags: |
            ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:${{ needs.version.outputs.version }}
            ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:latest
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

### 2. MODIFY: `.github/workflows/publish-site.yml`

Keep as a standalone workflow for **ad-hoc website deploys** (without a full release). Simplify triggers — remove tag push and develop branch triggers since releases now handle website deploy.

```yaml
# Before:
on:
  push:
    tags:
      - 'v[0-9]+.[0-9]+.[0-9]+'
    branches:
      - develop
    paths:
      - 'website/**'
  workflow_dispatch:

# After:
on:
  workflow_dispatch:
```

This keeps the manual trigger for deploying website changes independently of a release.

### 3. MODIFY: `.github/workflows/e2e.yml`

Update branch references from `main` to match the new trunk branch name. Since we're renaming `develop` → `main`, the current `main` references happen to become correct, but they were wrong before (referencing a non-existent branch).

```yaml
# Already says [main] — will now be correct after branch rename
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
```

No change needed here — the existing `main` references will align once `develop` is renamed.

### 4. MODIFY: `cliff.toml`

Add `[bump]` section for `git cliff --bumped-version` to work correctly:

```toml
[bump]
initial_tag = "v0.1.0"
```

### 5. DELETE: `CHANGELOG.md`

The in-repo CHANGELOG becomes unnecessary. The changelog lives on the GitHub Releases page, generated fresh per-release by `git-cliff --latest`. Removes a file that would always be stale.

Alternatively, keep it but accept it's informational only. Contributors can run `git cliff` locally to preview.

### 6. MODIFY: `install.sh`

Update branch references from `master`/`main` to `main`:

```bash
# Line 6 — already references master, will need updating
curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/main/install.sh | bash
```

(The URLs currently reference `master` — they'll need to point to `main` after rename.)

## Branch Migration

### Step-by-step

1. **Verify `master` has no unique commits** not in `develop`:
   ```bash
   git log master --not develop --oneline
   ```
   If empty, safe to proceed. If not, merge master into develop first.

2. **Rename `develop` → `main` on GitHub**:
   - Repo Settings → Default branch → Rename `develop` to `main`
   - GitHub automatically: updates the default branch, retargets open PRs, sets up redirects for old URLs
   - Contributors' existing clones: `git fetch && git branch -m develop main && git branch -u origin/main`

3. **Delete `master` branch**:
   - Verify all tags are preserved (tags are commit-level, not branch-level)
   - Delete via GitHub UI or `git push origin --delete master`

4. **Protect `main` branch**:
   - Require PR before merging (prevents direct push)
   - Require status checks (cargo test, clippy, fmt)
   - Allow maintainers to bypass for emergencies

5. **Add the `RELEASE_TOKEN`** (still needed for tag push within workflow_dispatch):
   - Actually — since `release.yml` uses `workflow_dispatch` and pushes a tag within the same workflow, `GITHUB_TOKEN` suffices. No PAT needed because no cross-workflow trigger is required.

### What happens to existing clones

Contributors run:
```bash
git fetch origin
git branch -m develop main
git branch -u origin/main main
git remote set-head origin -a
```

GitHub shows a banner on the old branch name with instructions.

## Summary of All Changes

| Change | Type | File |
|--------|------|------|
| Unified release workflow | **Rewrite** | `.github/workflows/release.yml` |
| Website: remove auto-triggers, keep manual | **Edit** | `.github/workflows/publish-site.yml` |
| E2E: already references `main` (now correct) | **No change** | `.github/workflows/e2e.yml` |
| Add `[bump]` section | **Edit** | `cliff.toml` |
| Update raw URL references | **Edit** | `install.sh` |
| Remove stale changelog | **Delete** (optional) | `CHANGELOG.md` |
| Rename `develop` → `main` | **GitHub Settings** | Manual |
| Delete `master` branch | **GitHub Settings** | Manual |
| Protect `main` branch | **GitHub Settings** | Manual |

## Key Advantages Over Gitflow Approach

| Aspect | Gitflow (previous plan) | Trunk-Based (this plan) |
|--------|------------------------|------------------------|
| Branches | `develop` + `master` | `main` only |
| Release trigger | Merge develop → master | Manual "Release" button |
| PAT required | Yes (cross-workflow tag push) | **No** (single workflow) |
| Backmerge needed | Potentially (if CI commits to master) | **Never** (no CI commits) |
| Contributor experience | "Target `develop`, not `master`" | "Just target `main`" |
| When to release | Every merge to master | **You choose** |
| Workflow files changed | 3 modified + 1 new | 2 modified + 0 new |
| Website deploy | Separate workflow triggered by tag | Job within release workflow |
