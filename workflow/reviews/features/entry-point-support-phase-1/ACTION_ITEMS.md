# Action Items: Entry Point Support - Phase 1

**Review Date:** 2026-01-23
**Verdict:** ✅ APPROVED (after fixes)
**Blocking Issues:** 0 (All resolved)
**Follow-up Required:** No (Critical issues fixed)

---

## Critical Issues - ✅ RESOLVED

### 1. ~~Missing Tests for handle_launch Integration~~ ✅ FIXED

- **Status:** RESOLVED
- **Fix Applied:** Added 4 comprehensive tests in `src/app/handler/new_session/launch_context.rs`:
  - `test_handle_launch_entry_point_creates_config` - Verifies entry_point.is_some() triggers config creation
  - `test_handle_launch_with_entry_point_and_flavor` - Verifies entry_point works with other params
  - `test_handle_launch_without_entry_point_no_config` - Verifies no config when no params set
  - `test_handle_launch_entry_point_from_vscode_config` - Verifies VSCode program → entry_point flow

---

## Major Issues - ✅ RESOLVED

### 1. ~~Unbounded Recursive Directory Traversal~~ ✅ FIXED

- **Status:** RESOLVED
- **Fix Applied:** Added depth limit in `src/core/discovery.rs`:
  - Added `MAX_ENTRY_POINT_DEPTH = 10` constant
  - Modified `discover_entry_points_recursive()` to accept and check depth parameter
  - Added `test_discover_entry_points_respects_depth_limit` test

### 2. Silent File System Errors

- **Source:** Risks Analyzer
- **File:** `src/core/discovery.rs`
- **Line:** 530
- **Problem:** File read errors use `trace!()` which is often disabled, making debugging difficult
- **Suggested Action:** Upgrade to `warn!()`:
  ```rust
  Err(err) => {
      warn!("Cannot read directory {:?}: {}", dir, err);
      return;
  }
  ```
- **Acceptance:** Errors visible in default log level

### 3. Complex Condition Should Be Extracted

- **Source:** Code Quality Inspector
- **File:** `src/app/handler/new_session/launch_context.rs`
- **Lines:** 347-350
- **Problem:** Multi-line condition with 4 checks is hard to read
- **Suggested Action:** Extract to helper method:
  ```rust
  impl LaunchParams {
      fn requires_config(&self) -> bool {
          self.config_name.is_some()
              || self.flavor.is_some()
              || !self.dart_defines.is_empty()
              || self.entry_point.is_some()
      }
  }
  ```
- **Acceptance:** Code is more readable and reusable

---

## Minor Issues (Consider Fixing)

### 1. Add Clarifying Comment for Optional Field Patterns

- **File:** `src/app/new_session_dialog/state.rs`
- **Lines:** 500-517
- **Problem:** Different syntax for optional fields (is_some vs is_empty) is confusing
- **Suggested Action:** Add comment explaining the pattern

### 2. Entry Point Path Validation

- **Files:** `src/app/new_session_dialog/state.rs`, `src/config/launch.rs`
- **Problem:** No validation that entry_point paths exist or are valid Dart files
- **Suggested Action:** Add validation in `set_entry_point()` or at launch time

### 3. Whitespace Trimming

- **File:** `src/config/launch.rs`
- **Lines:** 242-248
- **Problem:** Whitespace-only strings not treated as empty
- **Suggested Action:** Add `.trim()` before `.is_empty()` check

### 4. Doc Comment Examples

- **File:** `src/core/discovery.rs`
- **Lines:** 413, 433
- **Problem:** Doc examples don't compile (missing imports)
- **Suggested Action:** Add hidden imports with `# use` prefix

---

## Recommendations for Phase 2

### High Priority

1. Add critical path tests for handle_launch
2. Add recursion depth limit
3. Add entry_point path validation
4. Extract `LaunchParams::requires_config()` helper

### Medium Priority

1. Upgrade trace!() to warn!() for errors
2. Add clarifying comments for syntax patterns
3. Consider error aggregation for discovery with UI feedback

### Documentation

1. Document regex limitations in user-facing help
2. Add performance expectations (tested up to X files)
3. Document Phase 1 vs Phase 3 feature differences

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] All critical issues resolved (handle_launch tests added)
- [ ] All major issues resolved or justified (depth limit, error logging)
- [ ] Verification commands pass:
  ```bash
  cargo fmt -- --check
  cargo check
  cargo test --lib
  cargo clippy -- -D warnings
  ```
- [ ] New tests cover entry_point flow through handle_launch
- [ ] Recursive discovery has depth limit

---

## Phase 3 Preparation

When implementing Phase 3 (UI for entry point selection):

1. Add `LaunchContextField::EntryPoint` enum variant
2. Update field navigation (next/prev) to include entry point
3. Remove Flavor proxy pattern from `is_entry_point_editable()`
4. Consider AST-based parsing for production accuracy
5. Add lazy loading for entry point discovery in UI
