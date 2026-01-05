## Task: Document Test Organization in ARCHITECTURE.md

**Objective**: Add a "Testing" section to `docs/ARCHITECTURE.md` that documents the project's test organization strategy, providing clear guidance for contributors on where to place unit tests vs integration tests.

**Depends on**: None (can be done in parallel with other tasks)

### Scope

- `docs/ARCHITECTURE.md`: Add new "Testing" section

### Implementation Details

1. **Add a "Testing" section** after the "Error Handling" section in ARCHITECTURE.md:

   ```markdown
   ### Testing Strategy

   Flutter Demon follows Rust's conventional test organization:

   #### Unit Tests

   Unit tests live alongside the code they test in `src/`. There are two patterns:

   **Inline module (for small test suites):**
   ```rust
   // src/some_module.rs
   pub fn add(a: i32, b: i32) -> i32 { a + b }

   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn test_add() {
           assert_eq!(add(2, 2), 4);
       }
   }
   ```

   **Separate file (for large test suites, 100+ lines):**
   ```rust
   // src/some_module/mod.rs
   pub fn add(a: i32, b: i32) -> i32 { a + b }

   #[cfg(test)]
   mod tests;

   // src/some_module/tests.rs
   use super::*;

   #[test]
   fn test_add() {
       assert_eq!(add(2, 2), 4);
   }
   ```

   **Key points:**
   - Unit tests can access private items via `use super::*`
   - Use `#[cfg(test)]` to exclude test code from release builds
   - Prefer separate `tests.rs` file when tests exceed ~100 lines

   **Examples in this project:**
   - `src/app/handler/tests.rs` - Handler unit tests
   - `src/tui/widgets/log_view/tests.rs` - Log view widget tests

   #### Integration Tests

   Integration tests live in the `tests/` directory at the project root:

   ```
   tests/
   ├── discovery_integration.rs   # Flutter project discovery tests
   └── common/                    # Shared test utilities (optional)
       └── mod.rs
   ```

   **Key points:**
   - Integration tests can only access the public API
   - Each file in `tests/` is compiled as a separate crate
   - Use `tests/common/mod.rs` for shared helpers (not `tests/common.rs`)
   - Run with `cargo test --test <name>` for specific test files

   #### Running Tests

   ```bash
   # Run all tests
   cargo test

   # Run unit tests only
   cargo test --lib

   # Run integration tests only
   cargo test --test '*'

   # Run specific test file
   cargo test --test discovery_integration

   # Run tests matching a pattern
   cargo test log_view

   # Run with output
   cargo test -- --nocapture
   ```
   ```

2. **Update Table of Contents** if present

3. **Consider adding to Project Structure section**:
   
   Add a note about the `tests/` directory:
   ```markdown
   tests/                   # Integration tests (public API only)
   ├── discovery_integration.rs
   └── common/              # Shared test utilities
   ```

### File Location

The new section should be added in `docs/ARCHITECTURE.md` after line ~91 (after "Error Handling" section, before "Project Structure").

### Acceptance Criteria

1. ARCHITECTURE.md contains a "Testing Strategy" section
2. Section explains unit test placement (inline vs separate file)
3. Section explains integration test placement (`tests/` directory)
4. Section includes practical examples from this project
5. Section includes common `cargo test` commands
6. Documentation renders correctly in markdown viewers
7. No broken links or formatting issues

### Testing

- Verify markdown renders correctly:
  ```bash
  # If you have a markdown previewer
  open docs/ARCHITECTURE.md
  
  # Or use cargo doc
  cargo doc --open
  ```

- Verify documentation is discoverable:
  ```bash
  grep -n "Testing" docs/ARCHITECTURE.md
  grep -n "Unit Test" docs/ARCHITECTURE.md
  grep -n "Integration Test" docs/ARCHITECTURE.md
  ```

### Related Documentation Updates

Consider also updating:

1. **README.md**: Add a brief "Running Tests" section if not present
2. **CONTRIBUTING.md**: Reference the testing documentation (if file exists)

### Notes

- This task can be completed independently of the code refactoring
- The documentation should reflect the patterns already in use
- Use actual file paths from the project as examples
- Keep the documentation concise but complete
- This fulfills Phase 2 of the refactoring plan

---

## Completion Summary

**Status:** Done

**Files modified:**
- `docs/ARCHITECTURE.md`:
  - Added "Testing Strategy" to Table of Contents
  - Updated Project Structure to reflect `log_view/` directory module
  - Added new "Testing Strategy" section with:
    - Unit test patterns (inline vs separate file)
    - Integration test organization
    - Running tests commands
    - Test coverage by module table

**Documentation includes:**
- ✓ Unit test placement explanation (inline vs `tests.rs`)
- ✓ Integration test placement (`tests/` directory)
- ✓ Practical examples from this project
- ✓ Common `cargo test` commands
- ✓ Test coverage table by module

**Verification:**
- Testing section accessible via Table of Contents link
- Project structure updated to show `log_view/` module