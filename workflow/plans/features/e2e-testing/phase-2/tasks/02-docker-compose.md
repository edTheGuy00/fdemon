## Task: Create docker-compose.test.yml

**Objective**: Create Docker Compose configuration for orchestrating E2E test execution with proper volume mounts, environment variables, and service definitions.

**Depends on**: 01-dockerfile-test

### Scope

- `docker-compose.test.yml`: **NEW** - Test orchestration configuration

### Details

Create a docker-compose file that:
1. Defines the main test runner service
2. Configures volume mounts for source code and fixtures
3. Sets environment variables for test configuration
4. Provides easy commands for different test scenarios

#### Docker Compose Structure

```yaml
version: '3.8'

services:
  flutter-e2e-test:
    build:
      context: .
      dockerfile: Dockerfile.test
      cache_from:
        - fdemon-test:latest
    image: fdemon-test:latest

    # Mount source code and fixtures
    volumes:
      - .:/app:cached
      - cargo-cache:/root/.cargo/registry
      - cargo-git:/root/.cargo/git
      - target-cache:/app/target

    # Environment configuration
    environment:
      - FDEMON_TEST_TIMEOUT=${FDEMON_TEST_TIMEOUT:-60000}
      - RUST_BACKTRACE=1
      - RUST_LOG=fdemon=debug
      - CI=${CI:-false}
      - TERM=xterm-256color

    # Working directory
    working_dir: /app

    # Default command
    command: ["./tests/e2e/scripts/run_all_e2e.sh"]

    # Resource limits
    deploy:
      resources:
        limits:
          memory: 4G

  # Service for running specific test scripts
  flutter-e2e-startup:
    extends:
      service: flutter-e2e-test
    command: ["./tests/e2e/scripts/test_startup.sh"]

  flutter-e2e-hot-reload:
    extends:
      service: flutter-e2e-test
    command: ["./tests/e2e/scripts/test_hot_reload.sh"]

  # Interactive shell for debugging
  flutter-e2e-shell:
    extends:
      service: flutter-e2e-test
    command: ["/bin/bash"]
    stdin_open: true
    tty: true

volumes:
  cargo-cache:
  cargo-git:
  target-cache:
```

#### Key Considerations

1. **Volume Mounts**:
   - Source code mounted for live changes during development
   - Named volumes for cargo cache persistence
   - `:cached` flag for better performance on macOS

2. **Environment Variables**:
   - `FDEMON_TEST_TIMEOUT`: Configurable timeout for CI
   - `RUST_BACKTRACE`: Enable backtraces for debugging
   - `RUST_LOG`: Configure logging level
   - `CI`: Signal CI environment for conditional behavior

3. **Service Variants**:
   - Main service runs all tests
   - Specialized services for individual test scripts
   - Shell service for interactive debugging

4. **Resource Limits**:
   - 4GB memory limit to prevent runaway tests
   - No CPU limit (tests should run fast)

### Acceptance Criteria

1. `docker-compose -f docker-compose.test.yml build` completes successfully
2. `docker-compose -f docker-compose.test.yml run --rm flutter-e2e-test echo "test"` works
3. Source code changes are reflected in container without rebuild
4. Cargo cache persists between runs
5. All defined services can be started independently

### Testing

```bash
# Build services
docker-compose -f docker-compose.test.yml build

# Test volume mounts work
docker-compose -f docker-compose.test.yml run --rm flutter-e2e-test ls -la /app

# Test environment variables
docker-compose -f docker-compose.test.yml run --rm flutter-e2e-test env | grep FDEMON

# Test interactive shell
docker-compose -f docker-compose.test.yml run --rm flutter-e2e-shell

# Clean up
docker-compose -f docker-compose.test.yml down -v
```

### Notes

- Named volumes provide significant speedup for cargo builds
- The `extends` feature requires docker-compose v3.8+
- Consider adding healthcheck for production CI use
- May need to adjust memory limits based on test complexity

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `docker-compose.test.yml` | Created new Docker Compose configuration with main test service, specialized service variants (startup, hot-reload), interactive shell, and proper volume/environment configuration |

### Notable Decisions/Tradeoffs

1. **Version attribute**: Docker Compose warns that `version: '3.8'` is obsolete, but keeping it for backward compatibility with older docker-compose versions. Can be removed in future if minimum version requirement is established.
2. **Script paths**: Referenced test scripts that don't exist yet (`./tests/e2e/scripts/`) as they will be created in subsequent tasks (03-test-runner-scripts). Services are fully configured and ready.
3. **Volume strategy**: Used named volumes for cargo caches (registry, git, target) for persistence between runs, and bind mount for source code to reflect changes without rebuild.

### Testing Performed

- `docker-compose -f docker-compose.test.yml build` - Passed (built all 4 services)
- `docker-compose -f docker-compose.test.yml run --rm flutter-e2e-test echo "test"` - Passed (basic execution works)
- Volume mount verification: `ls -la /app` - Passed (source code visible in container)
- Environment variables: `env | grep FDEMON` - Passed (FDEMON_TEST_TIMEOUT=60000)
- Service variant: flutter-e2e-startup - Passed (extends works correctly)
- Service variant: flutter-e2e-hot-reload - Passed (extends works correctly)
- Named volumes created: cargo-cache, cargo-git, target-cache - Passed
- `docker-compose -f docker-compose.test.yml down -v` - Passed (cleanup successful)

### Risks/Limitations

1. **Script dependencies**: Services reference test scripts that don't exist yet. This is expected and documented - scripts will be created in task 03. Services will fail if run before scripts are created, but configuration is correct.
2. **Memory limits**: 4GB memory limit may need adjustment based on actual test complexity and resource usage patterns once tests are running.
3. **Docker Compose version warning**: The `version` field triggers a deprecation warning in newer docker-compose. Consider removing once minimum version requirements are established.
