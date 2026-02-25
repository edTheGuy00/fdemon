## Task: Simplify publish-site.yml to manual-only trigger

**Objective**: Remove automatic triggers (tag push, develop branch) from `publish-site.yml`, keeping only `workflow_dispatch` for ad-hoc website deploys independent of a release.

**Depends on**: None

**Wave**: 1 (parallel)

### Scope

- `.github/workflows/publish-site.yml`: **Edit** (trigger section only)

### Details

The release workflow (Task 01) now includes website deployment as a final job. The standalone `publish-site.yml` should only be used for ad-hoc website deploys (e.g., fixing a typo on the site without cutting a release).

**Change the `on:` section:**

```yaml
# Before (lines 13-21):
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

Remove lines 14-20 (the `push:` block with `tags:`, `branches:`, and `paths:`). Keep only `workflow_dispatch:`.

**Also update the file header comment** (lines 1-9) to reflect the new purpose:

```yaml
# Ad-hoc website deploy (independent of release).
# The main release workflow handles website deployment automatically.
#
# To deploy manually:
#   GitHub → Actions → Publish Website → Run workflow
#
# To deploy on your server:
#   docker pull ghcr.io/edtheguy00/flutter-demon-site:latest
#   docker run -d -p 80:80 ghcr.io/edtheguy00/flutter-demon-site:latest
```

The rest of the workflow (jobs, steps, Docker build) remains unchanged.

### Acceptance Criteria

1. `publish-site.yml` triggers only on `workflow_dispatch`
2. No `push:` trigger remains (no tags, no branches, no paths)
3. Header comment updated to explain ad-hoc purpose
4. Job definition, Docker build, and GHCR push are unchanged

### Testing

- YAML syntax validation
- Verify no `push:` key exists in the `on:` section

### Notes

- The `docker/metadata-action` with `type=ref,event=branch` and `type=sha` will still generate reasonable tags for manual dispatches (branch name + SHA)
- The `type=semver` patterns won't match on manual dispatch (no tag ref), so Docker tags will be `main` and `sha-xxxxx` — acceptable for ad-hoc deploys

---

## Completion Summary

**Status:** Not Started
