# Phase 2: Docker E2E Infrastructure - Task Index

## Overview

Create Docker-based test environment with real Flutter daemon for comprehensive validation. This enables end-to-end testing of actual Flutter process interaction, file watching, and complete user workflows.

**Total Tasks:** 10 (All Done) + 9 Follow-up Tasks
**Parent Plan:** [../PLAN.md](../PLAN.md)
**Prerequisite:** Phase 1 complete
**Follow-up Tasks:** [FOLLOWUP.md](FOLLOWUP.md)

## Status Summary

**Initial Tasks:** All 10 tasks completed. Docker image builds, scripts run, CI workflow created.

**Discovered Limitations:**
1. **No Flutter devices in Docker**: Flutter requires a connected device (emulator/simulator) to run an app. Docker containers don't have devices attached.
2. **TUI output not parseable**: fdemon's TUI renders ANSI escape codes, not plaintext events that scripts can grep/parse.

**Follow-up tasks** have been created to address these limitations. See [FOLLOWUP.md](FOLLOWUP.md) for details.

## Task Dependency Graph

```
┌─────────────────────────────┐
│  01-dockerfile-test         │
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐     ┌─────────────────────────────┐
│  02-docker-compose          │     │  03-simple-app-fixture      │
└─────────────┬───────────────┘     └─────────────┬───────────────┘
              │                                   │
              │     ┌─────────────────────────────┼─────────────────┐
              │     │                             │                 │
              │     ▼                             ▼                 ▼
              │  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
              │  │ 04-error-app     │  │ 05-plugin-       │  │ 06-multi-module  │
              │  │    fixture       │  │    fixture       │  │    fixture       │
              │  └────────┬─────────┘  └────────┬─────────┘  └────────┬─────────┘
              │           │                     │                     │
              └───────────┴──────────┬──────────┴─────────────────────┘
                                     │
                                     ▼
                          ┌─────────────────────────────┐
                          │  07-test-startup-script     │
                          └─────────────┬───────────────┘
                                        │
                                        ▼
                          ┌─────────────────────────────┐
                          │  08-test-hot-reload-script  │
                          └─────────────┬───────────────┘
                                        │
                                        ▼
                          ┌─────────────────────────────┐
                          │  09-run-all-e2e-script      │
                          └─────────────┬───────────────┘
                                        │
                                        ▼
                          ┌─────────────────────────────┐
                          │  10-ci-workflow             │
                          └─────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-dockerfile-test](tasks/01-dockerfile-test.md) | Done | - | `Dockerfile.test` |
| 2 | [02-docker-compose](tasks/02-docker-compose.md) | Done | 1 | `docker-compose.test.yml` |
| 3 | [03-simple-app-fixture](tasks/03-simple-app-fixture.md) | Done | - | `tests/fixtures/simple_app/` |
| 4 | [04-error-app-fixture](tasks/04-error-app-fixture.md) | Done | 3 | `tests/fixtures/error_app/` |
| 5 | [05-plugin-fixture](tasks/05-plugin-fixture.md) | Done | 3 | `tests/fixtures/plugin_with_example/` |
| 6 | [06-multi-module-fixture](tasks/06-multi-module-fixture.md) | Done | 3 | `tests/fixtures/multi_module/` |
| 7 | [07-test-startup-script](tasks/07-test-startup-script.md) | Done | 2, 3 | `tests/e2e/scripts/test_startup.sh` |
| 8 | [08-test-hot-reload-script](tasks/08-test-hot-reload-script.md) | Done | 7 | `tests/e2e/scripts/test_hot_reload.sh` |
| 9 | [09-run-all-e2e-script](tasks/09-run-all-e2e-script.md) | Done | 7, 8 | `tests/e2e/scripts/run_all_e2e.sh` |
| 10 | [10-ci-workflow](tasks/10-ci-workflow.md) | Done | 9 | `.github/workflows/e2e.yml` |

## Parallel Execution Opportunities

**Wave 1 (Foundation):**
- Task 01: Create Dockerfile.test

**Wave 2 (Parallel - Infrastructure + First Fixture):**
- Task 02: Create docker-compose.test.yml
- Task 03: Create simple_app fixture

**Wave 3 (Parallel - Additional Fixtures):**
- Task 04: Create error_app fixture
- Task 05: Create plugin_with_example fixture
- Task 06: Create multi_module fixture

**Wave 4 (Sequential - Scripts):**
- Task 07: Create test_startup.sh
- Task 08: Create test_hot_reload.sh

**Wave 5 (Orchestration):**
- Task 09: Create run_all_e2e.sh

**Wave 6 (CI Integration):**
- Task 10: Create GitHub Actions workflow

## Success Criteria

### Initial Tasks (Completed)

- [x] Docker test image builds and runs reliably
- [x] `Dockerfile.test` uses `ghcr.io/cirruslabs/flutter:stable` base
- [x] Rust toolchain installed and working in container
- [x] 4+ Flutter test fixtures created:
  - [x] `simple_app` - Minimal runnable Flutter app
  - [x] `error_app` - App with intentional compile errors
  - [x] `plugin_with_example` - Plugin structure with example app
  - [x] `multi_module` - Monorepo with multiple packages
- [x] 3+ bash test scripts with proper error handling:
  - [x] `test_startup.sh` - Tests fdemon startup flow
  - [x] `test_hot_reload.sh` - Tests hot reload workflow
  - [x] `run_all_e2e.sh` - Orchestrates all tests
- [x] CI workflow created at `.github/workflows/e2e.yml`

### Remaining Criteria (Requires Follow-up Tasks)

- [ ] Docker tests verify actual fdemon behavior (blocked: TUI not parseable)
- [ ] Tests run Flutter app on device (blocked: no device in Docker)
- [ ] Docker tests complete in <5 minutes
- [ ] <5% flake rate across 20 consecutive runs

**See [FOLLOWUP.md](FOLLOWUP.md) for tasks to address these blockers.**

## Test Execution

```bash
# Build test Docker image
docker build -f Dockerfile.test -t fdemon-test .

# Run all E2E tests via docker-compose
docker-compose -f docker-compose.test.yml run --rm flutter-e2e-test

# Run specific test script
docker-compose -f docker-compose.test.yml run --rm flutter-e2e-test ./tests/e2e/scripts/test_startup.sh

# Run interactively for debugging
docker-compose -f docker-compose.test.yml run --rm flutter-e2e-test bash
```

## Notes

- Tests require Docker to be installed and running
- Flutter test fixtures should be minimal to reduce image size and test time
- Scripts should have comprehensive error handling and cleanup
- Use BuildKit caching in CI for faster builds
- All scripts should support `FDEMON_TEST_TIMEOUT` environment variable for CI tuning
- Consider Android emulator headless mode for device testing (stretch goal)

## Follow-up Tasks

The following tasks were created to address limitations discovered during Phase 2:

### Wave 1: Enable Basic Docker Testing ✅
| Task | Priority | Status | Description |
|------|----------|--------|-------------|
| [F1-linux-desktop-support](tasks/F1-linux-desktop-support.md) | High | Done | Add Xvfb for Flutter Linux desktop |
| [F2-update-fixtures-linux](tasks/F2-update-fixtures-linux.md) | High | Done | Add Linux platform to fixtures |
| [F3-update-test-scripts](tasks/F3-update-test-scripts.md) | High | Done | Modify scripts for Linux target |

### Wave 2: Enable Parseable Output (Critical) ✅
| Task | Priority | Status | Description |
|------|----------|--------|-------------|
| [F4-fdemon-headless-mode](tasks/F4-fdemon-headless-mode.md) | Critical | Done | Add `--headless` JSON output mode |
| [F5-headless-test-scripts](tasks/F5-headless-test-scripts.md) | High | Done | Update scripts to use headless mode |

### Wave 3: Real Emulator Testing (Deferred)
> Deferred - Focus on Wave 1 & 2 first. F6 kept for reference.

| Task | Priority | Description |
|------|----------|-------------|
| [F6-github-android-emulator](tasks/F6-github-android-emulator.md) | Deferred | GitHub Actions with KVM emulator (reference) |
| F7-github-ios-simulator | Deferred | GitHub Actions with iOS simulator |
| F8-avd-snapshot-caching | Deferred | Cache AVD for faster startup |

### Wave 4: Documentation ✅
| Task | Priority | Status | Description |
|------|----------|--------|-------------|
| [F9-document-testing-strategy](tasks/F9-document-testing-strategy.md) | Medium | Done | Document E2E testing pyramid |

**Full details:** [FOLLOWUP.md](FOLLOWUP.md)

## References

- [Cirrus Labs Flutter Docker Images](https://github.com/cirruslabs/docker-images-flutter)
- [Docker BuildKit Caching](https://docs.docker.com/build/cache/)
- [GitHub Actions Docker](https://docs.github.com/en/actions/publishing-packages/publishing-docker-images)
- [Flutter Linux Desktop](https://docs.flutter.dev/platform-integration/linux/building)
- [reactivecircus/android-emulator-runner](https://github.com/ReactiveCircus/android-emulator-runner)
