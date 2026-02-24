## Task: Create GitHub Actions Release Workflow

**Objective**: Create a GitHub Actions workflow that automatically builds cross-platform release binaries when a version tag is pushed, and publishes them as a GitHub Release with SHA256 checksums.

**Depends on**: 03-cross-config

**Estimated Time**: 4-5 hours

### Scope

- `.github/workflows/release.yml` (**NEW**): Multi-platform release workflow

### Details

#### Trigger

The workflow triggers on tags matching semver: `v[0-9]+.[0-9]+.[0-9]+` (e.g., `v0.1.0`, `v1.2.3`).

#### Job structure

```
trigger (tag push)
    │
    ├── build-macos (matrix: x86_64 + aarch64)
    ├── build-linux (matrix: x86_64 native + aarch64 cross)
    └── build-windows (x86_64)
          │
          └── release (needs: build-macos, build-linux, build-windows)
                └── Create GitHub Release with all artifacts + checksums
```

#### Build matrix

| Job | Target | Runner | Build Command |
|-----|--------|--------|---------------|
| `build-macos` | `x86_64-apple-darwin` | `macos-13` | `cargo build --release --target x86_64-apple-darwin` |
| `build-macos` | `aarch64-apple-darwin` | `macos-latest` | `cargo build --release --target aarch64-apple-darwin` |
| `build-linux` | `x86_64-unknown-linux-gnu` | `ubuntu-latest` | `cargo build --release --target x86_64-unknown-linux-gnu` |
| `build-linux` | `aarch64-unknown-linux-gnu` | `ubuntu-latest` | `cross build --release --target aarch64-unknown-linux-gnu` |
| `build-windows` | `x86_64-pc-windows-msvc` | `windows-latest` | `cargo build --release --target x86_64-pc-windows-msvc` |

**Important**: The macOS matrix maps target → runner (`x86_64` → `macos-13` Intel, `aarch64` → `macos-latest` M1). The Linux aarch64 entry uses `cross` instead of `cargo`.

#### Artifact naming convention

```
fdemon-v{VERSION}-{TARGET}.tar.gz       (macOS, Linux)
fdemon-v{VERSION}-{TARGET}.zip          (Windows)
```

Example for v0.1.0:
- `fdemon-v0.1.0-x86_64-apple-darwin.tar.gz`
- `fdemon-v0.1.0-aarch64-apple-darwin.tar.gz`
- `fdemon-v0.1.0-x86_64-unknown-linux-gnu.tar.gz`
- `fdemon-v0.1.0-aarch64-unknown-linux-gnu.tar.gz`
- `fdemon-v0.1.0-x86_64-pc-windows-msvc.zip`

#### Packaging

Each build job:
1. Builds `--release` for the target
2. Strips the binary (`strip` on macOS/Linux — skip for cross, not available)
3. Creates archive:
   - **macOS/Linux**: `tar czf fdemon-v{VERSION}-{TARGET}.tar.gz fdemon`
   - **Windows**: `Compress-Archive -Path fdemon.exe -DestinationPath fdemon-v{VERSION}-{TARGET}.zip`
4. Uploads artifact via `actions/upload-artifact@v4`

#### Release job

After all build jobs complete:
1. Downloads all artifacts via `actions/download-artifact@v4`
2. Generates SHA256 checksums:
   ```bash
   sha256sum fdemon-v*.tar.gz fdemon-v*.zip > checksums-sha256.txt
   ```
3. Creates GitHub Release via `softprops/action-gh-release@v2` with:
   - All archive files as release assets
   - `checksums-sha256.txt` as a release asset
   - Auto-generated release notes (GitHub's built-in)
   - `draft: false`, `prerelease: false`

#### Workflow structure (pseudocode)

```yaml
name: Release
on:
  push:
    tags: ['v[0-9]+.[0-9]+.[0-9]+']

permissions:
  contents: write  # For creating releases

env:
  CARGO_TERM_COLOR: always

jobs:
  build-macos:
    strategy:
      matrix:
        include:
          - target: x86_64-apple-darwin
            runner: macos-13
          - target: aarch64-apple-darwin
            runner: macos-latest
    runs-on: ${{ matrix.runner }}
    steps:
      - checkout
      - install rust (stable) + add target
      - cargo cache (Swatinem/rust-cache@v2)
      - cargo build --release --target ${{ matrix.target }}
      - strip binary
      - package as .tar.gz
      - upload-artifact

  build-linux:
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            use_cross: false
          - target: aarch64-unknown-linux-gnu
            use_cross: true
    runs-on: ubuntu-latest
    steps:
      - checkout
      - install rust (stable) + add target
      - cargo cache
      - if use_cross: install cross
      - build (cargo or cross depending on use_cross)
      - strip binary (skip for cross/aarch64)
      - package as .tar.gz
      - upload-artifact

  build-windows:
    runs-on: windows-latest
    steps:
      - checkout
      - install rust (stable) + add target
      - cargo cache
      - cargo build --release --target x86_64-pc-windows-msvc
      - package as .zip (PowerShell Compress-Archive)
      - upload-artifact

  release:
    needs: [build-macos, build-linux, build-windows]
    runs-on: ubuntu-latest
    steps:
      - checkout (for release notes context)
      - download all artifacts
      - generate SHA256 checksums
      - create GitHub Release (softprops/action-gh-release@v2)
```

#### Version extraction

Extract version from the git tag in each job:
```yaml
- name: Get version
  id: version
  run: echo "VERSION=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"
```

This strips the `v` prefix: tag `v0.1.0` → version `0.1.0`.

### Acceptance Criteria

1. `.github/workflows/release.yml` exists and is valid YAML
2. Workflow triggers only on semver tags (`v*.*.*`)
3. Build jobs cover all 5 targets in the target matrix
4. macOS jobs use correct runners (Intel for x86_64, M1 for aarch64)
5. Linux aarch64 job uses `cross` instead of `cargo`
6. Artifact names follow `fdemon-v{VERSION}-{TARGET}.{ext}` convention
7. Release job waits for all build jobs (`needs: [...]`)
8. Release job generates `checksums-sha256.txt`
9. Release is created via `softprops/action-gh-release@v2` with all assets
10. `permissions: contents: write` is set for release creation

### Testing

Validate YAML syntax:
```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"
```

Full validation requires pushing a tag — defer to manual testing after merge:
```bash
git tag v0.1.0 && git push origin v0.1.0
```

### Notes

- The existing `e2e.yml` workflow is unrelated and should not be modified
- `softprops/action-gh-release@v2` is the de facto standard for GitHub Release creation in Actions
- `Swatinem/rust-cache@v2` caches `~/.cargo` and `target/` for faster builds — use `shared-key` per target to avoid cache collisions
- `cross` is installed via `cargo install cross --git https://github.com/cross-rs/cross` (the crates.io version may lag behind)
- The binary name is `fdemon` (not `flutter-demon`) — located at `target/{target}/release/fdemon` (or `fdemon.exe` on Windows)
- For macOS, `strip` works natively. For Linux x86_64, use `strip`. For Linux aarch64 (cross-compiled), skip strip — the cross container's strip may not be in PATH
- Windows binary does not need stripping (MSVC linker handles it)
