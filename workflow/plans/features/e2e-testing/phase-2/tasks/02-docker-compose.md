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

**Status:** Not Started
