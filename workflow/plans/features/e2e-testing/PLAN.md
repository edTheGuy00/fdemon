# Plan: E2E Integration Testing Infrastructure

## TL;DR

Implement a multi-layered E2E testing strategy for Flutter Demon combining: (1) mock daemon integration tests for fast CI feedback, (2) Docker-based real Flutter daemon tests for comprehensive validation, and (3) PTY-based TUI interaction testing for full user workflow verification. This provides 80%+ handler coverage with <30s mock tests and <5min Docker tests.

---

## Background

### Current Testing Landscape

Flutter Demon currently has:

| Test Type | Status | Coverage |
|-----------|--------|----------|
| Unit tests | Implemented | Core modules (`handler`, `session`) |
| Widget tests | Implemented | `TestBackend` for UI components |
| Integration tests | Minimal | Only `discovery_integration.rs` |
| Daemon interaction | **Missing** | Critical functionality untested |
| E2E user workflows | **Missing** | No full user journey testing |

### Critical Gaps

1. **No Flutter daemon interaction testing** - The core value proposition (hot reload, app lifecycle) is untested
2. **No file watcher integration** - Auto-reload triggers not validated
3. **No multi-session testing** - Session manager edge cases unexplored
4. **No CI with real Flutter** - Environment-specific issues may slip through

### Why This Matters

- **Regression risk**: Handler has 90+ message variants; manual testing misses edge cases
- **CI confidence**: Merges without daemon testing may break critical workflows
- **Developer experience**: Contributors need fast feedback on daemon-related changes

---

## Affected Modules

### Existing (modifications needed)

- `src/daemon/process.rs` - Extract trait for mocking `FlutterProcess`
- `src/daemon/commands.rs` - Extract trait for mocking `CommandSender`
- `src/services/flutter_controller.rs` - Already trait-based, ready for mocking

### New Files

- `tests/e2e/mod.rs` - **NEW** Mock daemon infrastructure and test utilities
- `tests/e2e/mock_daemon.rs` - **NEW** `MockFlutterDaemon` implementation
- `tests/e2e/daemon_interaction.rs` - **NEW** Device discovery, session lifecycle tests
- `tests/e2e/hot_reload.rs` - **NEW** Hot reload workflow tests
- `tests/e2e/session_management.rs` - **NEW** Multi-session tests
- `tests/e2e/tui_interaction.rs` - **NEW** PTY-based full E2E tests
- `tests/fixtures/simple_app/` - **NEW** Minimal Flutter test app
- `tests/fixtures/error_app/` - **NEW** App with intentional errors
- `Dockerfile.test` - **NEW** Docker test environment
- `docker-compose.test.yml` - **NEW** Test orchestration
- `.github/workflows/e2e.yml` - **NEW** CI workflow for E2E tests

### Dependencies to Add

```toml
[dev-dependencies]
# Existing
tokio-test = "0.4"
tempfile = "3"

# New for E2E testing
mockall = "0.13"           # Async trait mocking
expectrl = "0.7"           # PTY interaction testing
insta = "1.34"             # Snapshot testing for UI
```

---

## Architecture

### Multi-Layered Testing Strategy

```
┌─────────────────────────────────────────────────────┐
│ Layer 3: Full Docker E2E Tests                      │
│ - Real Flutter daemon interaction                   │
│ - Actual file watching                              │
│ - Complete user workflows                           │
│ - Run on: Pre-merge, Nightly                        │
│ - Duration: ~5 minutes                              │
└─────────────────────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────┐
│ Layer 2: Mock Daemon Integration Tests              │
│ - Mock Flutter daemon JSON-RPC responses            │
│ - Test state transitions via handler                │
│ - Verify message handling paths                     │
│ - Run on: Every commit                              │
│ - Duration: <30 seconds                             │
└─────────────────────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────┐
│ Layer 1: Widget/Unit Tests (Current)                │
│ - TestBackend rendering                             │
│ - Individual component logic                        │
│ - Run on: Every commit                              │
│ - Duration: <10 seconds                             │
└─────────────────────────────────────────────────────┘
```

### Mock Daemon Architecture

```
┌──────────────────┐     ┌──────────────────┐
│   Test Case      │     │ MockFlutterDaemon│
│                  │────▶│                  │
│  - Arrange state │     │  - cmd_rx        │
│  - Send Message  │     │  - event_tx      │
│  - Assert result │     │  - scenarios     │
└──────────────────┘     └──────────────────┘
         │                        │
         │                        │
         ▼                        ▼
┌──────────────────┐     ┌──────────────────┐
│   AppState       │     │  DaemonHandle    │
│                  │◀────│                  │
│  - handler::     │     │  - cmd_tx        │
│    update()      │     │  - event_rx      │
└──────────────────┘     └──────────────────┘
```

---

## Development Phases

### Phase 1: Mock Daemon Foundation

**Goal**: Create mock daemon infrastructure enabling fast, deterministic integration tests without Flutter installation.

#### Steps

1. **Extract Daemon Traits**
   - Create `FlutterDaemonTrait` in `src/daemon/traits.rs`
   - Trait methods: `spawn()`, `command_sender()`, `wait()`
   - Implement for existing `FlutterProcess`
   - Use `#[cfg_attr(test, mockall::automock)]` for test mocking

2. **Implement MockFlutterDaemon**
   - Create `tests/e2e/mock_daemon.rs`
   - Simulate JSON-RPC protocol (`daemon.connected`, `app.start`, `app.log`)
   - Support recorded response fixtures from `tests/fixtures/daemon_responses/`
   - Implement scenario builders for common test patterns

3. **Create Test Utilities**
   - Create `tests/e2e/mod.rs` with test helpers
   - Helper functions: `test_device()`, `test_session()`, `test_app_state()`
   - Macros for common assertions
   - Async test setup/teardown

4. **Implement Core Integration Tests**
   - Device discovery flow (`test_device_discovery_flow`)
   - Session lifecycle (`test_session_lifecycle`)
   - Hot reload trigger (`test_hot_reload_triggers_all_sessions`)
   - Error handling (`test_daemon_disconnect_recovery`)

5. **Add CI Integration**
   - Add mock tests to existing GitHub Actions workflow
   - Target: Run on every commit in <30 seconds

**Milestone**: Developers can run `cargo test --test e2e` for fast daemon interaction testing without Flutter installed.

---

### Phase 2: Docker E2E Infrastructure

**Goal**: Create Docker-based test environment with real Flutter daemon for comprehensive validation.

#### Steps

1. **Create Docker Test Environment**
   - Create `Dockerfile.test` using `ghcr.io/cirruslabs/flutter:stable`
   - Install Rust toolchain in image
   - Configure headless Flutter environment
   - Optimize layer caching for fast rebuilds

2. **Create Flutter Test Fixtures**
   - `tests/fixtures/simple_app/` - Minimal runnable Flutter app
   - `tests/fixtures/plugin_with_example/` - Plugin structure
   - `tests/fixtures/error_app/` - App with intentional compile errors
   - `tests/fixtures/multi_module/` - Monorepo structure

3. **Implement Docker Test Scripts**
   - `tests/e2e/scripts/run_all_e2e.sh` - Main test runner
   - `tests/e2e/scripts/test_hot_reload.sh` - Hot reload verification
   - `tests/e2e/scripts/test_startup.sh` - Startup flow verification
   - Error handling and cleanup in all scripts

4. **Create docker-compose Configuration**
   - `docker-compose.test.yml` with service definitions
   - Volume mounts for fdemon binary and fixtures
   - Environment variables for test configuration

5. **Add CI Workflow**
   - Create `.github/workflows/e2e.yml`
   - Run on PR merge and nightly
   - Docker BuildKit caching for speed
   - Artifact upload for test logs

**Milestone**: CI runs real Flutter daemon tests on every PR with <5 minute execution time.

---

### Phase 3: PTY-Based TUI Testing

**Goal**: Enable full end-to-end testing of keyboard input and terminal output using PTY interaction.

#### Steps

1. **Add expectrl Integration**
   - Add `expectrl = "0.7"` to dev-dependencies
   - Create `tests/e2e/tui_interaction.rs`
   - Implement PTY spawn helpers

2. **Implement TUI Interaction Tests**
   - Startup flow (`test_startup_shows_header`)
   - Device selector navigation (`test_device_selector_keyboard_navigation`)
   - Hot reload via keypress (`test_r_key_triggers_reload`)
   - Session switching (`test_number_keys_switch_sessions`)
   - Quit confirmation (`test_q_key_shows_confirm_dialog`)

3. **Add Snapshot Testing**
   - Add `insta = "1.34"` for snapshot testing
   - Create golden files for key UI states
   - Visual regression detection in CI

4. **Implement Complex Workflow Tests**
   - Full session lifecycle (create → run → reload → stop → remove)
   - Multi-session scenarios (parallel reloads, session ordering)
   - Error recovery (daemon crash → reconnect → resume)

**Milestone**: Full user workflows are automatically tested, catching UI regressions and keyboard handling bugs.

---

### Phase 4: Advanced Testing (Future)

**Goal**: Expand test coverage with performance testing, chaos testing, and multi-platform validation.

#### Steps

1. **Performance Benchmarking**
   - Add criterion benchmarks for critical paths
   - Track reload time, log throughput, memory usage
   - CI performance regression detection

2. **Multi-Flutter-Version Testing**
   - CI matrix with Flutter stable, beta
   - Detect API compatibility issues early

3. **Chaos/Fuzz Testing**
   - Random daemon disconnection
   - Malformed JSON-RPC responses
   - Property-based testing with proptest

4. **Platform Matrix**
   - Linux (primary), macOS, Windows (WSL)
   - PTY behavior differences

**Milestone**: Comprehensive test coverage with performance baselines and multi-version compatibility.

---

## Edge Cases & Risks

### Technical Risks

- **Risk:** Tight coupling in `FlutterProcess` makes trait extraction difficult
- **Mitigation:** Start with thin wrapper trait, refactor incrementally

- **Risk:** Mock daemon doesn't match real protocol behavior
- **Mitigation:** Record real daemon responses as fixtures, validate mock against recordings

- **Risk:** Docker tests are flaky due to timing
- **Mitigation:** Add retry logic, increase timeouts, use deterministic waits

- **Risk:** PTY behavior differs across platforms
- **Mitigation:** Focus on Linux CI, document platform-specific issues

### Maintenance Risks

- **Risk:** Test fixtures become outdated with Flutter updates
- **Mitigation:** Schedule fixture updates with Flutter releases, minimal fixtures

- **Risk:** Test maintenance burden slows development
- **Mitigation:** Clear test patterns, shared helpers, documentation

---

## Test Execution Strategy

### Local Development

```bash
# Quick feedback loop (current)
cargo test                    # Unit + widget tests (<10s)

# New: Mock integration tests
cargo test --test e2e         # Mock daemon tests (<30s)

# New: Full Docker E2E (requires Docker)
docker-compose -f docker-compose.test.yml run --rm flutter-e2e-test
```

### CI Pipeline

| Trigger | Tests Run | Duration |
|---------|-----------|----------|
| Every commit | Unit + Widget + Mock integration | ~2 minutes |
| Pull request | + Docker E2E smoke tests | ~8 minutes |
| Nightly | + Full E2E suite, multi-Flutter-version | ~20 minutes |

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `MockFlutterDaemon` simulates core JSON-RPC protocol
- [ ] 10+ integration tests covering device discovery, session lifecycle, hot reload
- [ ] Tests run in <30 seconds without Flutter installed
- [ ] 80% code coverage of `handler::update()` paths
- [ ] CI runs mock tests on every commit

### Phase 2 Complete When:
- [ ] Docker test image builds and runs reliably
- [ ] 5+ Flutter test fixtures created
- [ ] 5+ bash test scripts for key workflows
- [ ] Docker tests run in <5 minutes
- [ ] CI runs Docker tests on PR merge
- [ ] <5% flake rate across 20 consecutive runs

### Phase 3 Complete When:
- [ ] PTY-based tests verify keyboard input handling
- [ ] Snapshot tests catch UI regressions
- [ ] Full user workflows (startup → reload → quit) tested
- [ ] All critical paths have E2E coverage

### Phase 4 Complete When:
- [ ] Performance benchmarks established
- [ ] Multi-Flutter-version CI matrix active
- [ ] Test documentation complete for contributors

---

## File Structure

```
tests/
├── e2e/
│   ├── mod.rs                    # Test utilities, MockFlutterDaemon export
│   ├── mock_daemon.rs            # MockFlutterDaemon implementation
│   ├── daemon_interaction.rs     # Device discovery, daemon connection
│   ├── hot_reload.rs             # Hot reload workflows
│   ├── session_management.rs     # Session lifecycle tests
│   ├── tui_interaction.rs        # PTY-based TUI tests
│   ├── error_handling.rs         # Error recovery scenarios
│   └── scripts/
│       ├── run_all_e2e.sh        # Docker test runner
│       ├── test_hot_reload.sh    # Hot reload verification
│       └── test_startup.sh       # Startup flow verification
├── fixtures/
│   ├── daemon_responses/         # Recorded JSON-RPC responses
│   │   ├── device_list.json
│   │   ├── app_started.json
│   │   └── hot_reload_sequence.json
│   ├── simple_app/               # Minimal Flutter app
│   ├── plugin_with_example/      # Plugin structure
│   ├── error_app/                # App with errors
│   └── multi_module/             # Monorepo structure
└── discovery_integration.rs      # Existing integration test

# Docker files at project root
Dockerfile.test
docker-compose.test.yml

# CI workflow
.github/workflows/e2e.yml
```

---

## Quality Metrics

| Metric | Target |
|--------|--------|
| Unit test coverage | >80% for core logic |
| Mock integration coverage | >80% for handler paths |
| E2E workflow coverage | All critical user workflows |
| Mock test execution | <30 seconds |
| Docker test execution | <5 minutes |
| CI feedback time | <10 minutes for PR checks |
| Flake rate | <5% |
| Test-to-code ratio | ~1:2 |

---

## References

- [Recommendation Document](../../research/e2e_test_planning/recommendation.md)
- [Flutter Daemon Protocol](https://github.com/flutter/flutter/blob/master/packages/flutter_tools/doc/daemon.md)
- [Ratatui Testing Guide](https://docs.rs/ratatui/latest/ratatui/backend/struct.TestBackend.html)
- [expectrl Documentation](https://docs.rs/expectrl/latest/expectrl/)
- [mockall Documentation](https://docs.rs/mockall/latest/mockall/)
- [Cirrus Labs Flutter Docker Images](https://github.com/cirruslabs/docker-images-flutter)
- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache)

---

**Document Version:** 1.0
**Created:** 2025-01-07
**Status:** Draft - Awaiting Approval
