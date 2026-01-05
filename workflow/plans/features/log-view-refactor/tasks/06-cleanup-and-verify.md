## Task: Cleanup and Verify Migration

**Objective**: Remove the original `log_view.rs` file, verify all migrations are complete, and ensure the project compiles and all tests pass.

**Depends on**: `01-create-module-directory`, `02-extract-styles-module`, `03-extract-state-module`, `04-migrate-widget-implementation`, `05-extract-tests-module`

### Scope

- `src/tui/widgets/log_view.rs`: Delete original file
- `src/tui/widgets/mod.rs`: Verify module declaration still works
- `src/tui/widgets/log_view/mod.rs`: Verify complete and correct
- Full project compilation and test verification

### Implementation Details

1. **Verify all content has been migrated**:

   Run a diff or manual comparison to ensure nothing was left behind:
   
   | Original Location | New Location | Lines |
   |-------------------|--------------|-------|
   | L1-20 (imports) | `mod.rs` | ~20 |
   | L22-58 (stack_trace_styles) | `styles.rs` | ~37 |
   | L61-252 (state types) | `state.rs` | ~192 |
   | L255-1212 (widget impl) | `mod.rs` | ~958 |
   | L1215-2262 (tests) | `tests.rs` | ~1047 |

2. **Delete the original file**:
   ```bash
   rm src/tui/widgets/log_view.rs
   ```

3. **Verify `src/tui/widgets/mod.rs`** still has correct imports:
   ```rust
   mod log_view;  // Now points to log_view/ directory
   
   pub use log_view::{LogView, LogViewState};
   ```

4. **Run full verification**:
   ```bash
   # Check compilation
   cargo check
   
   # Check for warnings
   cargo check 2>&1 | grep -i warning
   
   # Run all tests
   cargo test
   
   # Run log_view specific tests
   cargo test log_view
   
   # Build release to catch any optimization issues
   cargo build --release
   
   # Generate docs to verify documentation
   cargo doc --no-deps
   ```

5. **Verify public API unchanged**:
   
   The following should still be importable from external code:
   ```rust
   use flutter_demon::tui::widgets::{LogView, LogViewState};
   ```

### File Structure After This Task

```
src/tui/widgets/
├── log_view/            # NEW directory module
│   ├── mod.rs           # ~980 lines - widget impl + module declarations
│   ├── state.rs         # ~175 lines - LogViewState, FocusInfo
│   ├── styles.rs        # ~40 lines - stack trace styling constants
│   └── tests.rs         # ~1050 lines - all unit tests
├── mod.rs               # UNCHANGED (module declaration still works)
├── confirm_dialog.rs
├── device_selector.rs
├── header.rs
├── search_input.rs
├── status_bar.rs
└── tabs.rs

# DELETED:
# src/tui/widgets/log_view.rs  (original 2262-line file)
```

### Line Count Comparison

| Before | After | Difference |
|--------|-------|------------|
| 1 file × 2262 lines | 4 files | Same total |
| - | mod.rs: ~980 lines | Largest file < 1000 |
| - | state.rs: ~175 lines | Focused module |
| - | styles.rs: ~40 lines | Constants only |
| - | tests.rs: ~1050 lines | Tests isolated |

### Acceptance Criteria

1. Original `log_view.rs` file is deleted
2. `cargo check` passes with no errors
3. `cargo check` has no new warnings (existing warnings acceptable)
4. `cargo test` passes - all tests green
5. `cargo test log_view` runs ~68 tests successfully
6. `cargo build --release` succeeds
7. `cargo doc` generates documentation without errors
8. No file in the new module exceeds 1100 lines
9. Public API is unchanged (`LogView`, `LogViewState` accessible)

### Testing

**Full verification script:**

```bash
#!/bin/bash
set -e

echo "=== Step 1: Check compilation ==="
cargo check

echo "=== Step 2: Run all tests ==="
cargo test

echo "=== Step 3: Run log_view tests specifically ==="
cargo test log_view -- --test-threads=1

echo "=== Step 4: Count log_view tests ==="
TEST_COUNT=$(cargo test log_view 2>&1 | grep "^test " | wc -l)
echo "Found $TEST_COUNT log_view tests"

echo "=== Step 5: Build release ==="
cargo build --release

echo "=== Step 6: Generate docs ==="
cargo doc --no-deps

echo "=== Step 7: Verify file structure ==="
ls -la src/tui/widgets/log_view/

echo "=== Step 8: Verify no original file ==="
if [ -f src/tui/widgets/log_view.rs ]; then
    echo "ERROR: Original log_view.rs still exists!"
    exit 1
fi

echo "=== All verifications passed! ==="
```

### Rollback Plan

If issues are discovered after deletion:

1. The original file should be in git history:
   ```bash
   git checkout HEAD~1 -- src/tui/widgets/log_view.rs
   ```

2. Remove the new directory:
   ```bash
   rm -rf src/tui/widgets/log_view/
   ```

3. Update `mod.rs` if needed

### Post-Migration Checklist

- [ ] Original `log_view.rs` deleted
- [ ] `cargo check` passes
- [ ] `cargo test` passes
- [ ] `cargo build --release` passes
- [ ] `cargo doc` generates correctly
- [ ] No regression in functionality
- [ ] IDE navigation works (rust-analyzer picks up new structure)
- [ ] All 68+ tests still run
- [ ] No new compiler warnings introduced

### Notes

- Commit the changes in a single commit for easy rollback
- Suggested commit message: "refactor(tui): split log_view.rs into module directory"
- Tag the commit for reference: `git tag log-view-refactor-complete`
- This completes Phase 1 of the plan