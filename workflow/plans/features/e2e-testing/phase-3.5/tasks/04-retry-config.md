## Task: Configure Test Retry Mechanism

**Objective**: Set up cargo-nextest with automatic retry for flaky PTY tests to improve CI reliability.

**Depends on**: 01-fix-spawn-default

### Scope

- `Cargo.toml`: Add dev dependency
- `.config/nextest.toml`: **NEW** - nextest configuration
- `.github/workflows/test.yml`: Update CI workflow (if exists)

### Details

#### 1. Add cargo-nextest Dependency (Optional)

nextest is installed as a cargo plugin, not a dependency:

```bash
# Install globally (CI step)
cargo install cargo-nextest --locked
```

#### 2. Create nextest Configuration

Create `.config/nextest.toml`:

```toml
# nextest configuration for flutter-demon
# See: https://nexte.st/book/configuration.html

[profile.default]
# Retry flaky tests up to 2 times
retries = 2

# Fail fast on non-flaky failures
fail-fast = true

# Test timeout (per test)
slow-timeout = { period = "30s", terminate-after = 2 }

[profile.ci]
# More aggressive settings for CI
retries = 3
fail-fast = false

# Longer timeout for CI environments
slow-timeout = { period = "60s", terminate-after = 2 }

# Mark tests as flaky if they fail then pass on retry
[profile.ci.junit]
store-success-output = true
store-failure-output = true
```

#### 3. Update CI Workflow

Add to GitHub Actions (if `.github/workflows/test.yml` exists):

```yaml
- name: Install nextest
  uses: taiki-e/install-action@nextest

- name: Run E2E tests with retry
  run: cargo nextest run --profile ci --test e2e
  env:
    CI: true
    RUST_BACKTRACE: 1
```

#### 4. Add Local Convenience Script

Create `scripts/test-e2e.sh`:

```bash
#!/bin/bash
# Run E2E tests with retry support

if command -v cargo-nextest &> /dev/null; then
    echo "Running with nextest (retry enabled)"
    cargo nextest run --test e2e "$@"
else
    echo "Running with cargo test (no retry)"
    echo "Install nextest for retry support: cargo install cargo-nextest"
    cargo test --test e2e "$@"
fi
```

### Acceptance Criteria

1. nextest configuration created
2. CI workflow uses nextest with retry
3. Flaky tests retry up to 3 times in CI
4. Documentation updated with nextest usage

### Testing

```bash
# Install nextest
cargo install cargo-nextest --locked

# Run with default profile
cargo nextest run --test e2e

# Run with CI profile
cargo nextest run --profile ci --test e2e

# Show test timing
cargo nextest run --test e2e --status-level all
```

### Notes

- nextest is not required for local dev, just recommended
- Standard `cargo test` still works
- Retry masks flakiness - investigate root cause if >5% retry rate

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `.config/nextest.toml` | NEW - Created nextest configuration with retry profiles (default: 2 retries, ci: 3 retries) |
| `scripts/test-e2e.sh` | NEW - Created convenience script that auto-detects nextest and falls back to cargo test |
| `.github/workflows/e2e.yml` | Updated mock-tests job to use nextest with ci profile for automatic retry |
| `docs/TESTING.md` | Added section on cargo-nextest usage, updated mock daemon tests section, updated CI workflows documentation |
| `docs/DEVELOPMENT.md` | Added nextest commands to test commands table with installation instructions |

### Notable Decisions/Tradeoffs

1. **CI Profile vs Default Profile**: Used separate profiles for CI (3 retries) vs local development (2 retries) to balance reliability with feedback speed. CI profile also stores success/failure output for debugging.

2. **e2e.yml vs test.yml**: Updated existing `.github/workflows/e2e.yml` instead of non-existent `test.yml` file. The mock-tests job now uses nextest with automatic retry.

3. **Convenience Script**: Created `scripts/test-e2e.sh` that gracefully falls back to cargo test if nextest is not installed, making it optional for local development while required in CI.

4. **Test Timeouts**: Configured 30s timeout for default profile and 60s for CI to handle slower CI environments.

### Testing Performed

- `cargo fmt` - Passed (no code files modified)
- `cargo check` - Passed
- `cargo clippy -- -D warnings` - Passed
- Verified `.config/nextest.toml` created with correct TOML syntax
- Verified `scripts/test-e2e.sh` is executable (755 permissions)
- Tested convenience script execution - correctly detects missing nextest and falls back to cargo test

### Risks/Limitations

1. **Retry Masks Flakiness**: While retry improves CI reliability, it can mask underlying test flakiness. The task notes recommend investigating if retry rate exceeds 5%.

2. **nextest Not Required Locally**: The implementation makes nextest optional for local development but required in CI. Developers need to manually install it with `cargo install cargo-nextest --locked`.

3. **JUnit Output Storage**: CI profile stores both success and failure output which increases storage requirements but aids debugging.
