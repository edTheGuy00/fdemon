## Task: Create Cross.toml for Linux ARM Cross-Compilation

**Objective**: Create a `Cross.toml` configuration file at the workspace root to pin the Docker image used for `aarch64-unknown-linux-gnu` cross-compilation and configure environment variable passthrough.

**Depends on**: None

**Estimated Time**: 0.5 hours

### Scope

- `Cross.toml` (**NEW**): Cross-compilation configuration at workspace root

### Details

[`cross`](https://github.com/cross-rs/cross) is the standard Rust cross-compilation tool that uses Docker containers with pre-built sysroots. It's only needed for the `aarch64-unknown-linux-gnu` target in the release workflow — all other targets use native runners.

#### File content

Create `Cross.toml` at the workspace root (`/Users/ed/Dev/zabin/flutter-demon/Cross.toml`):

```toml
# Cross-compilation configuration for aarch64 Linux builds
# Used by the release workflow (.github/workflows/release.yml)
# See: https://github.com/cross-rs/cross

[target.aarch64-unknown-linux-gnu]
image = "ghcr.io/cross-rs/aarch64-unknown-linux-gnu:0.2.5"

[build.env]
passthrough = ["RUST_BACKTRACE", "CARGO_TERM_COLOR"]
```

#### Why pin the image version

Without pinning, `cross` uses `:latest` which can break builds when upstream images change. Pinning to `0.2.5` ensures reproducible builds. The version corresponds to `cross` v0.2.5 which supports Rust stable and handles all of this project's dependencies (tokio, crossterm, serde, etc.).

#### Why passthrough env vars

- `RUST_BACKTRACE`: Enables backtraces in CI for debugging build failures
- `CARGO_TERM_COLOR`: Preserves colored output in CI logs

### Acceptance Criteria

1. `Cross.toml` exists at the workspace root
2. `aarch64-unknown-linux-gnu` target image is pinned to a specific version
3. `RUST_BACKTRACE` and `CARGO_TERM_COLOR` are configured as passthrough env vars
4. File is valid TOML (parseable)

### Testing

No automated tests — this is a configuration file used only by the GitHub Actions workflow. Validate:

```bash
# Check TOML syntax
cat Cross.toml | python3 -c "import sys, tomllib; tomllib.load(sys.stdin.buffer); print('Valid TOML')"
```

### Notes

- `cross` is installed in the release workflow via `cargo install cross --git https://github.com/cross-rs/cross` — no local install needed
- This file is only used by the `build-linux` job's `aarch64-unknown-linux-gnu` matrix entry
- macOS targets cannot use `cross` (Docker can't target macOS)
- Windows uses `msvc` toolchain (not `gnu`), so `cross` is not applicable
