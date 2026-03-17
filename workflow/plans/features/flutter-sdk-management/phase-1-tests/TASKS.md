# Phase 1 Tests: Comprehensive SDK Detection Test Suite - Task Index

## Overview

Create a two-tier test suite for the Flutter SDK detection system (Phase 1). **Tier 1** uses tempdir-based integration tests for fast, CI-friendly coverage of all 11 detection strategies. **Tier 2** uses Docker containers with real version manager installations and headless binary smoke tests for high-fidelity verification.

**Total Tasks:** 7

## Task Dependency Graph

```
┌─────────────────────────────┐     ┌─────────────────────────────┐
│  01-shared-test-infra       │     │  04-docker-infrastructure   │
│  (fixtures, helpers, utils) │     │  (Dockerfiles, helpers)     │
└──────────┬──────────────────┘     └──────────┬──────────────────┘
           │                                   │
    ┌──────┼──────────┐                 ┌──────┼──────────┐
    │      │          │                 │      │          │
    ▼      ▼          ▼                 ▼      ▼          ▼
┌──────┐ ┌──────┐ ┌──────┐      ┌──────────┐ ┌──────────────┐
│  02  │ │  03  │ │      │      │    05    │ │      06      │
│tempdir│ │edge  │ │      │      │  docker  │ │   docker     │
│integ.│ │cases │ │      │      │  linux   │ │   windows    │
└──┬───┘ └──┬───┘ │      │      └────┬─────┘ └──────┬───────┘
   │        │     │      │           │               │
   └────┬───┘     │      │           └───────┬───────┘
        │         │      │                   │
        └─────────┼──────┘───────────────────┘
                  ▼
         ┌────────────────┐
         │       07       │
         │ binary headless│
         │  smoke tests   │
         └────────────────┘
```

### Parallelism Waves

| Wave | Tasks | Can Run In Parallel |
|------|-------|-------------------|
| 1 | 01, 04 | Yes |
| 2 | 02, 03, 05, 06 | Yes (02, 03 depend on 01; 05, 06 depend on 04) |
| 3 | 07 | No (depends on 01 + 04, benefits from 02-06 being done) |

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-shared-test-infrastructure](tasks/01-shared-test-infrastructure.md) | Done | - | `tests/sdk_detection/mod.rs`, `tests/sdk_detection/fixtures.rs`, `tests/sdk_detection/assertions.rs` |
| 2 | [02-tempdir-integration-tests](tasks/02-tempdir-integration-tests.md) | Done | 01 | `tests/sdk_detection/tier1_detection_chain.rs` |
| 3 | [03-edge-case-stress-tests](tasks/03-edge-case-stress-tests.md) | Done | 01 | `tests/sdk_detection/tier1_edge_cases.rs` |
| 4 | [04-docker-infrastructure](tasks/04-docker-infrastructure.md) | Done | - | `tests/docker/`, `tests/sdk_detection/docker_helpers.rs` |
| 5 | [05-docker-linux-version-managers](tasks/05-docker-linux-version-managers.md) | Done | 04 | `tests/docker/*.Dockerfile`, `tests/sdk_detection/tier2_linux.rs` |
| 6 | [06-docker-windows-wine](tasks/06-docker-windows-wine.md) | Done | 04 | `tests/docker/windows-wine.Dockerfile`, `tests/sdk_detection/tier2_windows.rs` |
| 7 | [07-binary-headless-smoke-tests](tasks/07-binary-headless-smoke-tests.md) | Done | 01, 04 | `tests/sdk_detection/tier2_headless.rs` |

## Architecture

### Test Layout

```
tests/
├── sdk_detection/
│   ├── mod.rs                      # Module root, shared imports
│   ├── fixtures.rs                 # MockSdkBuilder, version manager layout creators
│   ├── assertions.rs               # NDJSON parsing, SDK detection result assertions
│   ├── docker_helpers.rs           # Docker build/run/cleanup helpers
│   ├── tier1_detection_chain.rs    # Tier 1: full chain integration tests
│   ├── tier1_edge_cases.rs         # Tier 1: broken configs, symlinks, permissions
│   ├── tier2_linux.rs              # Tier 2: Docker tests with real version managers
│   ├── tier2_windows.rs            # Tier 2: Wine-based Windows .bat detection
│   └── tier2_headless.rs           # Tier 2: full binary headless smoke tests
├── docker/
│   ├── base.Dockerfile             # Shared Rust builder + minimal runtime
│   ├── fvm.Dockerfile              # FVM v3 + Flutter stable
│   ├── asdf.Dockerfile             # asdf + flutter plugin + version
│   ├── mise.Dockerfile             # mise + flutter
│   ├── proto.Dockerfile            # proto + flutter plugin
│   ├── puro.Dockerfile             # Puro + default env
│   ├── manual.Dockerfile           # Manual git clone install
│   └── windows-wine.Dockerfile     # Wine64 + cross-compiled fdemon.exe
└── sdk_detection.rs                # Integration test entry point (declares mod sdk_detection)
```

### Gating Strategy

- **Tier 1 tests**: Run on every `cargo test` — fast tempdir tests with no external deps
- **Tier 2 tests**: Gated behind `#[ignore]` — run via `cargo test -- --ignored` or `DOCKER_TESTS=1 cargo test`
- Docker images are built once and cached locally via Docker layer caching

### Binary Testing Approach

Docker Tier 2 tests compile fdemon inside the Docker image (multi-stage build) and run it in **headless mode**. Headless mode outputs clean NDJSON to stdout, making output parsing straightforward:

```json
{"event":"sdk_resolved","source":"fvm","version":"3.22.0","path":"/root/fvm/versions/stable"}
```

Or on failure:
```json
{"event":"error","message":"No Flutter SDK found","fatal":true}
```

Tests use `std::process::Command` to invoke `docker run` and parse the NDJSON stdout.

## Success Criteria

Phase 1 Tests are complete when:

- [ ] Shared fixture builders can create mock layouts for all 11 detection strategies
- [ ] Tier 1: `find_flutter_sdk()` tested end-to-end for every strategy via tempdirs
- [ ] Tier 1: Priority ordering verified (explicit > FLUTTER_ROOT > FVM > Puro > asdf > mise > proto > wrapper > PATH)
- [ ] Tier 1: Multi-manager conflict resolution tested
- [ ] Tier 1: Monorepo parent-directory walk tested
- [ ] Tier 1: Edge cases covered (broken symlinks, corrupted configs, missing components, empty files)
- [ ] Tier 2: Docker images built for FVM, asdf, mise, proto, Puro, manual install
- [ ] Tier 2: Real version manager installations produce correct `SdkSource` variant
- [ ] Tier 2: Windows `.bat` detection verified via Wine Docker image
- [ ] Tier 2: Headless binary smoke tests verify full startup → SDK detection → NDJSON output
- [ ] All Tier 1 tests pass on `cargo test`
- [ ] All Tier 2 tests pass on `cargo test -- --ignored` (with Docker running)
- [ ] Existing tests unaffected — no regressions
- [ ] `cargo fmt --all && cargo check --workspace && cargo clippy --workspace -- -D warnings` passes

## Notes

- **No `assert_cmd` dependency needed.** Docker binary tests use `std::process::Command` to invoke `docker run`, which captures stdout/stderr. Headless mode outputs clean NDJSON (no ANSI), so parsing is trivial.
- **`serial_test` is available** at the binary crate level for integration tests that mutate env vars.
- **Docker images are multi-stage builds**: Rust builder stage compiles fdemon, runtime stage installs version manager + Flutter. This avoids cross-compilation complexity.
- **Snap is tested via Tier 1 tempdir only** — snapd requires systemd, which is incompatible with standard Docker containers.
- **Windows Docker uses Wine** — macOS Docker Desktop only runs Linux containers. Wine provides basic Windows binary execution in a Linux container. This tests `.bat` detection and `FlutterExecutable::WindowsBatch` variant creation, but is not a substitute for real Windows CI.
- **Headless output format**: fdemon headless mode emits `{"event":"...","message":"..."}` NDJSON lines to stdout. Tests parse these to verify SDK detection results.
