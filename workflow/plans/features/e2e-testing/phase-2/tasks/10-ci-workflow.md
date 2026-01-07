## Task: Create GitHub Actions CI Workflow

**Objective**: Create a GitHub Actions workflow that runs the Docker E2E tests on PR merges and nightly, with proper caching and artifact upload.

**Depends on**: 09-run-all-e2e-script

### Scope

- `.github/workflows/e2e.yml`: **NEW** - CI workflow for E2E tests

### Details

Create a GitHub Actions workflow that:
1. Triggers on PR merge to main and nightly schedule
2. Builds the Docker test image with caching
3. Runs the E2E test suite
4. Uploads logs as artifacts on failure
5. Reports status clearly

#### Workflow Structure

```yaml
name: E2E Tests

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
    types: [closed]
  schedule:
    # Run nightly at 2 AM UTC
    - cron: '0 2 * * *'
  workflow_dispatch:
    inputs:
      timeout:
        description: 'Test timeout in seconds'
        required: false
        default: '300'

env:
  FDEMON_TEST_TIMEOUT: ${{ github.event.inputs.timeout || '300' }}
  DOCKER_BUILDKIT: 1

jobs:
  e2e-tests:
    name: Docker E2E Tests
    runs-on: ubuntu-latest
    # Only run on merged PRs, not all closed PRs
    if: |
      github.event_name == 'push' ||
      github.event_name == 'schedule' ||
      github.event_name == 'workflow_dispatch' ||
      (github.event_name == 'pull_request' && github.event.pull_request.merged == true)

    timeout-minutes: 30

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Cache Docker layers
        uses: actions/cache@v4
        with:
          path: /tmp/.buildx-cache
          key: ${{ runner.os }}-buildx-${{ hashFiles('Dockerfile.test', 'Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-buildx-

      - name: Build test image
        uses: docker/build-push-action@v5
        with:
          context: .
          file: ./Dockerfile.test
          push: false
          load: true
          tags: fdemon-test:latest
          cache-from: type=local,src=/tmp/.buildx-cache
          cache-to: type=local,dest=/tmp/.buildx-cache-new,mode=max

      # Prevents cache from growing indefinitely
      - name: Move cache
        run: |
          rm -rf /tmp/.buildx-cache
          mv /tmp/.buildx-cache-new /tmp/.buildx-cache

      - name: Run E2E tests
        id: e2e
        run: |
          mkdir -p test-logs
          docker-compose -f docker-compose.test.yml run \
            --rm \
            -e FDEMON_LOG_DIR=/app/test-logs \
            -e FDEMON_TEST_TIMEOUT=${{ env.FDEMON_TEST_TIMEOUT }} \
            flutter-e2e-test
        continue-on-error: true

      - name: Upload test logs
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: e2e-test-logs-${{ github.run_number }}
          path: test-logs/
          retention-days: 7

      - name: Check test result
        if: steps.e2e.outcome == 'failure'
        run: |
          echo "::error::E2E tests failed. Check the test-logs artifact for details."
          exit 1

      - name: Report success
        if: steps.e2e.outcome == 'success'
        run: echo "::notice::All E2E tests passed!"

  # Optional: Run mock tests in parallel for fast feedback
  mock-tests:
    name: Mock Daemon Tests
    runs-on: ubuntu-latest
    if: github.event_name != 'schedule'  # Skip on nightly (run full only)

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Run mock tests
        run: cargo test --test e2e
```

#### Key Considerations

1. **Triggers**:
   - `push` to main: Run on direct pushes
   - `pull_request` closed + merged: Run on PR merge
   - `schedule`: Nightly runs for comprehensive testing
   - `workflow_dispatch`: Manual runs with custom timeout

2. **Docker Caching**:
   - Use BuildKit for better caching
   - Cache Docker layers between runs
   - Cache rotation to prevent bloat

3. **Artifact Upload**:
   - Always upload logs (even on success)
   - 7-day retention for debugging
   - Numbered by run for easy identification

4. **Parallel Jobs**:
   - Mock tests run alongside Docker tests
   - Provides fast feedback on basic functionality
   - Full Docker tests are slower but comprehensive

5. **Timeout Management**:
   - Job-level 30-minute timeout
   - Configurable test timeout via input
   - Default 5 minutes (300s)

### Acceptance Criteria

1. Workflow triggers on PR merge to main
2. Workflow triggers on nightly schedule
3. Docker image builds with caching
4. E2E tests execute successfully
5. Logs uploaded as artifacts on failure
6. Clear success/failure reporting
7. Manual trigger works with custom timeout

### Testing

```bash
# Test workflow locally with act (optional)
act -j e2e-tests --container-architecture linux/amd64

# Verify workflow syntax
gh workflow view e2e.yml

# Manual trigger
gh workflow run e2e.yml --field timeout=600
```

### CI Integration Notes

1. **Secrets**: No secrets required for basic E2E tests
2. **Permissions**: Default permissions sufficient
3. **Concurrency**: Consider adding concurrency limits for expensive tests
4. **Status Checks**: Add as required check after initial validation

#### Optional Enhancements

```yaml
# Add concurrency control
concurrency:
  group: e2e-${{ github.ref }}
  cancel-in-progress: true

# Add Slack notification on failure
- name: Notify Slack
  if: failure() && github.event_name == 'schedule'
  uses: 8398a7/action-slack@v3
  with:
    status: failure
    fields: repo,message,commit,author
```

### Notes

- First run will be slow (no cache)
- Subsequent runs should be <10 minutes
- Consider matrix strategy for multi-Flutter-version testing (Phase 4)
- May need self-hosted runner for ARM testing

---

## Completion Summary

**Status:** Not Started
