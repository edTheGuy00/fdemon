## Task: Create GitHub Actions Workflow to Publish Website Image to GHCR

**Objective**: Create a GitHub Actions workflow that builds the website Docker image (Leptos WASM app served by nginx) and pushes it to the GitHub Container Registry (ghcr.io) for deployment to the fdemon.dev server.

**Depends on**: None

### Scope

- `.github/workflows/publish-site.yml` — **NEW** GitHub Actions workflow

### Details

The website already has a production-ready `Dockerfile` at `website/Dockerfile` (multi-stage: `rust:slim` builder with Trunk → `nginx:alpine` server) and an `nginx.conf` for SPA routing. This task creates the CI/CD workflow to build and push the image.

#### Workflow Design

**Trigger**: The workflow should trigger on:
1. **Release tags** (`v[0-9]+.[0-9]+.[0-9]+`) — automatically publish when a new version is tagged
2. **Manual dispatch** (`workflow_dispatch`) — allow manual builds for testing or hotfixes
3. Optionally: pushes to `develop` branch that modify `website/` files — for previewing changes (tag as `develop` or `latest-dev`)

**Image naming**: `ghcr.io/edtheguy00/flutter-demon-site`

**Image tags**:
- On version tags: `v1.2.3`, `1.2.3`, `1.2`, `latest`
- On develop pushes: `develop`
- Always: short SHA (`sha-abc1234`)

#### Workflow File

```yaml
name: Publish Website

on:
  push:
    tags:
      - 'v[0-9]+.[0-9]+.[0-9]+'
    branches:
      - develop
    paths:
      - 'website/**'
  workflow_dispatch:

env:
  REGISTRY: ghcr.io
  IMAGE_NAME: edtheguy00/flutter-demon-site

jobs:
  build-and-push:
    name: Build & Push Website Image
    runs-on: ubuntu-latest

    permissions:
      contents: read
      packages: write

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

      - name: Extract Docker metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}
          tags: |
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=ref,event=branch
            type=sha

      - name: Build and push
        uses: docker/build-push-action@v6
        with:
          context: ./website
          file: ./website/Dockerfile
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

#### Key Design Decisions

**1. Build context is `./website`**
The `docker build` context is set to `./website` and the Dockerfile is at `./website/Dockerfile`. This means the Dockerfile's `COPY . .` copies only the website directory contents — not the entire repository. This is correct because the existing Dockerfile expects to be built from the website directory.

**2. GHCR authentication uses `GITHUB_TOKEN`**
No Personal Access Token (PAT) needed. The `GITHUB_TOKEN` has `packages:write` permission when the workflow declares `permissions.packages: write`. The image is scoped to the repository automatically.

**3. Multi-tag strategy with `docker/metadata-action`**
- `type=semver` generates version tags from the git tag (e.g., `v1.0.0` → `1.0.0` + `1.0`)
- `type=ref,event=branch` tags branch pushes (e.g., `develop`)
- `type=sha` always adds a short-SHA tag for traceability
- The `latest` tag is automatically added by `docker/metadata-action` on semver tags

**4. BuildKit cache via GitHub Actions cache**
`cache-from: type=gha` and `cache-to: type=gha,mode=max` use the GitHub Actions cache backend for Docker layer caching. This avoids the complexity of managing `/tmp/.buildx-cache` and provides cross-run caching automatically.

**5. Branch push filtering with `paths`**
Branch pushes only trigger when files under `website/` change. This avoids rebuilding the website image on unrelated code changes. Tag pushes always trigger (no path filter).

#### Post-Deployment (Documentation)

The user deploys to fdemon.dev from their own server. Add a comment in the workflow file documenting how to pull and run:

```yaml
# To deploy on your server:
#   docker pull ghcr.io/edtheguy00/flutter-demon-site:latest
#   docker run -d -p 80:80 ghcr.io/edtheguy00/flutter-demon-site:latest
```

### Acceptance Criteria

1. `.github/workflows/publish-site.yml` exists and is valid YAML
2. The workflow triggers on version tags and manual dispatch
3. The workflow authenticates with GHCR using `GITHUB_TOKEN`
4. The Docker image is built from `website/Dockerfile` with context `./website`
5. Images are tagged with semver, branch name, and SHA
6. BuildKit caching is configured for faster rebuilds
7. The workflow has correct permissions (`contents: read`, `packages: write`)
8. A deployment pull command is documented in the workflow comments

### Testing

- Validate YAML syntax (e.g., `yq` or online YAML linter)
- Verify the workflow file references the correct Dockerfile path and context
- Test locally with `docker build -t fdemon-site ./website` to confirm the Dockerfile still works
- After merge, trigger a manual dispatch to verify the full pipeline

### Notes

- The website Dockerfile uses `rust:slim` with nightly toolchain — first build in CI will be slow (~10-15 min) but subsequent builds benefit from BuildKit layer caching
- Images pushed to GHCR are **private by default**. After the first push, the package visibility must be changed to "public" in GitHub repo settings (Settings → Packages → flutter-demon-site → Package settings → Change visibility)
- The existing `release.yml` workflow runs on the same `v*` tags. Both workflows will trigger simultaneously, which is fine — they are independent jobs
- The `REGISTRY` and `IMAGE_NAME` env vars match the pattern `ghcr.io/<owner>/<image-name>`. Adjust `edtheguy00` if the GitHub username differs
