# Phase 2: Docker E2E Infrastructure - Task Index

## Overview

Create Docker-based test environment with real Flutter daemon for comprehensive validation. This enables end-to-end testing of actual Flutter process interaction, file watching, and complete user workflows.

**Total Tasks:** 10
**Parent Plan:** [../PLAN.md](../PLAN.md)
**Prerequisite:** Phase 1 complete

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
| 1 | [01-dockerfile-test](tasks/01-dockerfile-test.md) | Not Started | - | `Dockerfile.test` |
| 2 | [02-docker-compose](tasks/02-docker-compose.md) | Not Started | 1 | `docker-compose.test.yml` |
| 3 | [03-simple-app-fixture](tasks/03-simple-app-fixture.md) | Not Started | - | `tests/fixtures/simple_app/` |
| 4 | [04-error-app-fixture](tasks/04-error-app-fixture.md) | Not Started | 3 | `tests/fixtures/error_app/` |
| 5 | [05-plugin-fixture](tasks/05-plugin-fixture.md) | Not Started | 3 | `tests/fixtures/plugin_with_example/` |
| 6 | [06-multi-module-fixture](tasks/06-multi-module-fixture.md) | Not Started | 3 | `tests/fixtures/multi_module/` |
| 7 | [07-test-startup-script](tasks/07-test-startup-script.md) | Not Started | 2, 3 | `tests/e2e/scripts/test_startup.sh` |
| 8 | [08-test-hot-reload-script](tasks/08-test-hot-reload-script.md) | Not Started | 7 | `tests/e2e/scripts/test_hot_reload.sh` |
| 9 | [09-run-all-e2e-script](tasks/09-run-all-e2e-script.md) | Not Started | 7, 8 | `tests/e2e/scripts/run_all_e2e.sh` |
| 10 | [10-ci-workflow](tasks/10-ci-workflow.md) | Not Started | 9 | `.github/workflows/e2e.yml` |

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

Phase 2 is complete when:

- [ ] Docker test image builds and runs reliably
- [ ] `Dockerfile.test` uses `ghcr.io/cirruslabs/flutter:stable` base
- [ ] Rust toolchain installed and working in container
- [ ] 4+ Flutter test fixtures created:
  - [ ] `simple_app` - Minimal runnable Flutter app
  - [ ] `error_app` - App with intentional compile errors
  - [ ] `plugin_with_example` - Plugin structure with example app
  - [ ] `multi_module` - Monorepo with multiple packages
- [ ] 3+ bash test scripts with proper error handling:
  - [ ] `test_startup.sh` - Verifies fdemon launches Flutter correctly
  - [ ] `test_hot_reload.sh` - Verifies hot reload workflow
  - [ ] `run_all_e2e.sh` - Orchestrates all tests
- [ ] Docker tests complete in <5 minutes
- [ ] CI workflow runs Docker tests on PR merge
- [ ] <5% flake rate across 20 consecutive runs
- [ ] Test logs uploaded as artifacts on failure

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

## References

- [Cirrus Labs Flutter Docker Images](https://github.com/cirruslabs/docker-images-flutter)
- [Docker BuildKit Caching](https://docs.docker.com/build/cache/)
- [GitHub Actions Docker](https://docs.github.com/en/actions/publishing-packages/publishing-docker-images)
