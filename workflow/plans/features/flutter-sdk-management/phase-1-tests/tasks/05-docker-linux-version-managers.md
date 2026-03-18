## Task: Tier 2 — Docker Linux Tests (Real Version Managers)

**Objective**: Create Docker images with real Flutter version manager installations and write tests that verify `fdemon` correctly detects the SDK in each environment when run headless.

**Depends on**: 04-docker-infrastructure

### Scope

- `tests/docker/fvm.Dockerfile`: FVM v3 + Flutter stable
- `tests/docker/asdf.Dockerfile`: asdf + flutter plugin
- `tests/docker/mise.Dockerfile`: mise + Flutter
- `tests/docker/proto.Dockerfile`: proto + flutter plugin
- `tests/docker/puro.Dockerfile`: Puro + default environment
- `tests/docker/manual.Dockerfile`: Manual git clone installation
- `tests/sdk_detection/tier2_linux.rs`: Docker-based integration tests

### Details

#### Dockerfile Specifications

##### FVM v3

```dockerfile
# tests/docker/fvm.Dockerfile
FROM rust:1.83-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl git unzip xz-utils tar \
    && rm -rf /var/lib/apt/lists/*

# Install FVM
RUN curl -fsSL https://fvm.app/install.sh | bash
ENV PATH="/root/fvm/bin:$PATH"

# Install Flutter via FVM (skip pub-get to save time)
RUN fvm install stable --skip-pub-get

# Create test project with .fvmrc
RUN mkdir -p /test-project && \
    printf 'name: test_project\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' > /test-project/pubspec.yaml && \
    echo '{"flutter":"stable"}' > /test-project/.fvmrc

COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon
WORKDIR /test-project
```

##### asdf

```dockerfile
# tests/docker/asdf.Dockerfile
FROM rust:1.83-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl git unzip xz-utils bash \
    && rm -rf /var/lib/apt/lists/*

# Install asdf
RUN git clone https://github.com/asdf-vm/asdf.git /root/.asdf --branch v0.14.1
ENV PATH="/root/.asdf/shims:/root/.asdf/bin:$PATH"

# Install flutter plugin and stable version
RUN asdf plugin add flutter https://github.com/asdf-community/asdf-flutter.git && \
    asdf install flutter latest && \
    asdf global flutter latest

# Create test project with .tool-versions
RUN mkdir -p /test-project && \
    printf 'name: test_project\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' > /test-project/pubspec.yaml && \
    asdf current flutter | awk '{print "flutter " $2}' > /test-project/.tool-versions

COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon
WORKDIR /test-project
```

##### mise

```dockerfile
# tests/docker/mise.Dockerfile
FROM rust:1.83-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl git unzip xz-utils bash \
    && rm -rf /var/lib/apt/lists/*

# Install mise
RUN curl https://mise.run | sh
ENV PATH="/root/.local/share/mise/shims:/root/.local/bin:$PATH"

# Install Flutter via mise
RUN mise install flutter@latest && mise use -g flutter@latest

# Create test project with .mise.toml
RUN mkdir -p /test-project && \
    printf 'name: test_project\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' > /test-project/pubspec.yaml && \
    cd /test-project && mise use flutter@latest

COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon
WORKDIR /test-project
```

##### proto

```dockerfile
# tests/docker/proto.Dockerfile
FROM rust:1.83-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl git unzip xz-utils bash \
    && rm -rf /var/lib/apt/lists/*

# Install proto
RUN curl -fsSL https://moonrepo.dev/install/proto.sh | bash
ENV PATH="/root/.proto/shims:/root/.proto/bin:$PATH"

# Install Flutter via proto (community plugin)
RUN proto plugin add flutter "github://nickclaw/proto-flutter-plugin" && \
    proto install flutter latest

# Create test project with .prototools
RUN mkdir -p /test-project && \
    printf 'name: test_project\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' > /test-project/pubspec.yaml && \
    cd /test-project && proto pin flutter latest

COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon
WORKDIR /test-project
```

##### Puro

```dockerfile
# tests/docker/puro.Dockerfile
FROM rust:1.83-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl git unzip xz-utils bash \
    && rm -rf /var/lib/apt/lists/*

# Install Puro
RUN curl -o- https://puro.dev/install.sh | PURO_ROOT="/root/.puro" bash
ENV PATH="/root/.puro/bin:/root/.puro/envs/default/bin:$PATH"

# Create a Puro environment with Flutter
RUN puro create default stable

# Create test project with .puro.json
RUN mkdir -p /test-project && \
    printf 'name: test_project\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' > /test-project/pubspec.yaml && \
    echo '{"env":"default"}' > /test-project/.puro.json

COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon
WORKDIR /test-project
```

##### Manual Install

```dockerfile
# tests/docker/manual.Dockerfile
FROM rust:1.83-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl git unzip xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Install Flutter manually via tarball (faster than git clone)
ARG FLUTTER_VERSION=3.22.0
RUN curl -fsSL "https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_${FLUTTER_VERSION}-stable.tar.xz" \
    | tar -xJ -C /opt/
ENV PATH="/opt/flutter/bin:$PATH"

# Create test project (no version manager config files)
RUN mkdir -p /test-project && \
    printf 'name: test_project\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' > /test-project/pubspec.yaml

COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon
WORKDIR /test-project
```

#### Test Structure

```rust
// tests/sdk_detection/tier2_linux.rs
use super::docker_helpers::*;
use super::assertions::*;

/// Helper: build a version manager Docker image (cached by Docker layers)
fn ensure_image(dockerfile: &str, tag: &str) {
    let root = project_root();
    docker_build(dockerfile, tag, &root)
        .unwrap_or_else(|e| panic!("Failed to build {}: {}", tag, e));
}

#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK)"]
fn test_fvm_detection_real_install() {
    if !docker_available() { return; }
    ensure_image("tests/docker/fvm.Dockerfile", "fdemon-test-fvm");

    let result = docker_run_headless("fdemon-test-fvm", &[], 60).unwrap();
    let events = parse_headless_events(&result.stdout);

    // fdemon should detect Flutter via FVM
    // Check that no "No Flutter SDK found" error was emitted
    assert!(!events.iter().any(|e| e.event == "error" && e.fatal == Some(true)),
        "fdemon failed to detect Flutter SDK in FVM environment.\nstdout: {}\nstderr: {}",
        result.stdout, result.stderr);
}

#[test]
#[ignore = "requires Docker + internet"]
fn test_asdf_detection_real_install() {
    if !docker_available() { return; }
    ensure_image("tests/docker/asdf.Dockerfile", "fdemon-test-asdf");
    let result = docker_run_headless("fdemon-test-asdf", &[], 60).unwrap();
    let events = parse_headless_events(&result.stdout);
    assert!(!events.iter().any(|e| e.event == "error" && e.fatal == Some(true)),
        "fdemon failed to detect Flutter SDK in asdf environment");
}

#[test]
#[ignore = "requires Docker + internet"]
fn test_mise_detection_real_install() { ... }

#[test]
#[ignore = "requires Docker + internet"]
fn test_proto_detection_real_install() { ... }

#[test]
#[ignore = "requires Docker + internet"]
fn test_puro_detection_real_install() { ... }

#[test]
#[ignore = "requires Docker + internet"]
fn test_manual_install_detection() { ... }

/// Verify that fdemon correctly identifies the SDK source in each environment.
/// This test uses `docker exec` to run fdemon and inspect debug logs.
#[test]
#[ignore = "requires Docker + internet"]
fn test_fvm_source_identified_in_logs() {
    if !docker_available() { return; }
    ensure_image("tests/docker/fvm.Dockerfile", "fdemon-test-fvm");

    // Run with RUST_LOG=debug to capture detection chain logs
    let result = docker_run_headless(
        "fdemon-test-fvm",
        &["-e", "RUST_LOG=fdemon_daemon::flutter_sdk=debug"],
        60,
    ).unwrap();

    // Check stderr (where tracing logs go) for FVM detection
    assert!(result.stderr.contains("FVM") || result.stderr.contains("fvm"),
        "Expected FVM detection in logs.\nstderr: {}", result.stderr);
}
```

#### What Each Test Verifies

| Image | Expected `SdkSource` | Key Verification |
|-------|----------------------|------------------|
| fvm | `Fvm { version: "stable" }` | `.fvmrc` parsed, `~/fvm/versions/stable/` resolved |
| asdf | `Asdf { version: "..." }` | `.tool-versions` parsed, `~/.asdf/installs/flutter/<ver>/` resolved |
| mise | `Mise { version: "..." }` | `.mise.toml` generated, `~/.local/share/mise/installs/flutter/<ver>/` resolved |
| proto | `Proto { version: "..." }` | `.prototools` present, `~/.proto/tools/flutter/<ver>/` resolved |
| puro | `Puro { env: "default" }` | `.puro.json` parsed, `~/.puro/envs/default/flutter/` resolved |
| manual | `SystemPath` | No config files, Flutter found via PATH |

### Acceptance Criteria

1. All 6 Dockerfiles build successfully
2. fdemon headless detects the SDK correctly in each container
3. No "No Flutter SDK found" errors in any version manager environment
4. Manual install correctly falls back to SystemPath detection
5. Debug logs confirm the expected detection strategy was used
6. All Docker tests pass with `cargo test -- --ignored`
7. Docker images are re-usable across runs (layer caching)

### Testing

```bash
# Build and test all Docker environments
cargo test --test sdk_detection tier2_linux -- --ignored --nocapture

# Test a specific version manager
cargo test --test sdk_detection test_fvm_detection -- --ignored --nocapture
```

### Notes

- **First run is slow** — Docker builds compile fdemon (~5min) and download Flutter SDK (~2-3GB per image). Subsequent runs use cached layers.
- **Internet required** — version manager install scripts and Flutter SDK downloads need network access. Tests should be clearly documented as requiring internet.
- **proto's Flutter plugin** — the plugin may be community-maintained and could break. If `proto install flutter` fails, check if the plugin source has changed.
- **Puro install script** — Puro's installer may require `PURO_ROOT` to be set explicitly in Docker (some installers auto-detect `$HOME` differently in containers).
- **Timeout of 60 seconds** — fdemon headless with `--headless` should start quickly since it only does SDK detection + device discovery. If Flutter SDK triggers first-run setup (`flutter precache`), it may take longer. Consider running `flutter precache` in the Dockerfile to avoid this.
- **Flutter precache in Dockerfiles** — add `RUN flutter precache --no-android --no-ios --no-web --no-linux --no-macos --no-windows --no-fuchsia` after installation to pre-cache the Dart SDK. This prevents first-run delays during tests. Or use `--universal` if available.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `tests/docker/fvm.Dockerfile` | New — FVM v3 + Flutter stable multi-stage image with `flutter precache` |
| `tests/docker/asdf.Dockerfile` | New — asdf v0.14.1 + asdf-flutter plugin multi-stage image |
| `tests/docker/mise.Dockerfile` | New — mise + Flutter latest multi-stage image with `.mise.toml` |
| `tests/docker/proto.Dockerfile` | New — proto + community Flutter plugin multi-stage image |
| `tests/docker/puro.Dockerfile` | New — Puro + "default" environment multi-stage image with explicit `PURO_ROOT` |
| `tests/docker/manual.Dockerfile` | New — Flutter tarball extraction to `/opt/flutter` via `ARG FLUTTER_VERSION` |
| `tests/sdk_detection/tier2_linux.rs` | New — 7 `#[ignore]` tests (1 per version manager + FVM log verification) |

### Notable Decisions/Tradeoffs

1. **120-second test timeouts**: Increased from the 60s noted in the task to 120s across all tests. Even with `flutter precache` in the Dockerfiles, the headless run may still do some first-run initialisation. 120s provides headroom without being excessively long.

2. **`flutter precache` in every Dockerfile**: Added `flutter precache --no-android --no-ios ...` after each SDK installation (with `|| true` to handle edge cases). This eliminates first-run Dart SDK download delays during the actual test run, keeping container execution well inside the timeout.

3. **Explicit `PURO_ROOT` in puro.Dockerfile**: Set `PURO_ROOT=/root/.puro` both in the install command and as a persistent `ENV` so fdemon can locate Puro environments at runtime. The task notes flagged this as a potential issue.

4. **`ARG FLUTTER_VERSION=3.22.0` in manual.Dockerfile**: Pinned to a known stable release with a valid tarball URL. The build arg allows callers to override without touching the Dockerfile.

5. **`assert_no_fatal_error` helper**: Extracted the fatal-error check into a shared private function to avoid repeating the assertion string across all 6 basic detection tests.

6. **7 tests instead of 6**: Added `test_fvm_source_identified_in_logs` (a source-verification test using `RUST_LOG` debug output) as the task sketch included it and it validates a distinct acceptance criterion (criterion 5).

7. **Pre-existing warnings not addressed**: `cargo clippy --workspace -- -D warnings` was clean. The warnings visible in `cargo test` output come from pre-existing code in `docker_helpers.rs` (unused `mut`, unused field `exit_code`, unused `docker_exec`) and `fixtures.rs` (unused `with_bat_file`) — none introduced by this task.

### Testing Performed

- `cargo fmt --all` — Passed (fmt reformatted `ensure_image` closure in tier2_linux.rs)
- `cargo check --workspace` — Passed (clean)
- `cargo clippy --workspace -- -D warnings` — Passed (clean)
- `cargo test --test sdk_detection` — Passed (60 passed; 0 failed; 8 ignored)

### Risks/Limitations

1. **proto community plugin stability**: The `proto install flutter` step uses a community-maintained plugin (`nickclaw/proto-flutter-plugin`). If that repository is removed or its URL changes, `proto.Dockerfile` will fail to build. The Dockerfile and task notes document this risk.

2. **Version manager installer URLs**: FVM, mise, and Puro use `curl | bash` installer scripts from their respective CDNs. These URLs are correct as of 2026-03 but may change in future releases.

3. **Docker tests not executed locally**: The Docker tests themselves require Docker + internet and are `#[ignore]`. Compilation and lint gates pass; functional correctness of the container builds will be validated only when run explicitly with `--ignored`.
