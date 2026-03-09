# Task 07: Full build verification

**Depends on:** Tasks 01-06
**Wave:** 3

## What to do

1. Run full workspace build:
   ```bash
   cargo build --workspace
   ```

2. Run full test suite:
   ```bash
   cargo test --workspace
   ```

3. Run clippy:
   ```bash
   cargo clippy --workspace
   ```

4. Run formatter check:
   ```bash
   cargo fmt --all -- --check
   ```

All must pass with zero warnings/errors.
