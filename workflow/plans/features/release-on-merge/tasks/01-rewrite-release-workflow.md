## Task: Rewrite release.yml to workflow_dispatch

**Objective**: Replace the tag-triggered release workflow with a self-contained `workflow_dispatch` workflow that handles version computation, tagging, building, releasing, and website deployment in a single pipeline.

**Depends on**: None

**Wave**: 1 (parallel)

### Scope

- `.github/workflows/release.yml`: **Full rewrite**

### Details

The current `release.yml` triggers on `push: tags: v[0-9]+.[0-9]+.[0-9]+`. It needs to be rewritten as a `workflow_dispatch` workflow with an optional `version` input override.

**Key structural changes:**

1. **Trigger**: `push: tags` → `workflow_dispatch` with optional `version` string input
2. **Permissions**: Add `packages: write` (for website GHCR push)
3. **New `version` job** at the start:
   - Checkout with `fetch-depth: 0`
   - Install git-cliff via `taiki-e/install-action@v2`
   - Compute next version: manual override or `git cliff --bumped-version`
   - Error if no releasable commits or version already tagged
   - Create and push tag via `GITHUB_TOKEN`
   - Outputs: `version` (e.g. `0.2.0`) and `tag` (e.g. `v0.2.0`)
4. **Build jobs** (`build-macos`, `build-linux`, `build-windows`):
   - Add `needs: [version]`
   - Replace all `${GITHUB_REF_NAME#v}` version extraction with `${{ needs.version.outputs.version }}`
   - Remove per-job `Get version` steps (no longer needed)
5. **Release job**:
   - Add `tag_name: ${{ needs.version.outputs.tag }}` to `softprops/action-gh-release@v2`
   - Keep `git-cliff --latest --strip header` for changelog body
6. **New `publish-site` job** (absorbed from `publish-site.yml`):
   - `needs: [version, release]`
   - Docker login, build, and push to GHCR
   - Explicit tags: `registry/image:version` and `registry/image:latest`
   - No `docker/metadata-action` (tags are known from version output)

**The complete YAML is provided in the PLAN.md** — the implementor should use it as the source, copying the full workflow from the "REWRITE: `.github/workflows/release.yml`" section.

### Acceptance Criteria

1. `release.yml` triggers only on `workflow_dispatch` (no `push: tags` trigger)
2. Has optional `version` input with description
3. `version` job computes version via git-cliff or manual override
4. `version` job creates and pushes tag
5. Build jobs use `needs.version.outputs.version` for artifact naming
6. Release job passes explicit `tag_name` to `action-gh-release`
7. `publish-site` job builds and pushes website Docker image to GHCR
8. Top-level permissions include both `contents: write` and `packages: write`

### Testing

- This is a GitHub Actions workflow — cannot be tested locally
- Verify YAML syntax: `yamllint .github/workflows/release.yml` or similar
- Verify structure matches PLAN.md specification exactly

### Notes

- The `publish-site` job uses explicit Docker tags (`version` + `latest`) instead of `docker/metadata-action` because `workflow_dispatch` doesn't provide tag refs for semver extraction
- `GITHUB_TOKEN` suffices for tag push since no other workflow needs to trigger from it
- The `release` job's `fetch-depth: 0` is required for `git-cliff --latest` to access full history

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `.github/workflows/release.yml` | Full rewrite: replaced tag-triggered workflow with `workflow_dispatch` workflow including `version`, `build-macos`, `build-linux`, `build-windows`, `release`, and `publish-site` jobs |

### Notable Decisions/Tradeoffs

1. **Exact copy from PLAN.md**: The YAML was copied verbatim from the "REWRITE" section in PLAN.md (lines 84-379), ensuring the implementation matches the specification exactly without interpretation drift.
2. **`publish-site` job absorbed**: The website deploy job is now the final step in the unified release workflow rather than a separate `publish-site.yml` file. The separate `publish-site.yml` still exists as an ad-hoc manual deploy option.

### Testing Performed

- YAML syntax validation via `ruby -c` / `YAML.load_file` - Passed
- All 8 acceptance criteria verified via grep checks - Passed
- No Rust compilation involved (GitHub Actions YAML only)

### Risks/Limitations

1. **GitHub Actions only**: The workflow cannot be tested locally — it will be validated on first dispatch run in CI.
2. **`GITHUB_TOKEN` tag push**: The `version` job pushes a tag using `GITHUB_TOKEN`. This works for `workflow_dispatch` but will not re-trigger other tag-based workflows (by design — no cross-workflow triggers required).
