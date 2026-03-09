# Task 06: Verify `build_flutter_args` includes `extra_args` (existing coverage check)

**File:** `crates/fdemon-app/src/config/launch.rs`
**Depends on:** None
**Wave:** 2 (parallel with Task 05)

## What to do

1. Check existing tests in `launch.rs` for `build_flutter_args` — confirm there's a test that verifies `extra_args` are appended to the argument list.

2. If no such test exists, add one:
   ```rust
   #[test]
   fn test_build_flutter_args_includes_extra_args() {
       let config = LaunchConfig {
           extra_args: vec![
               "--dart-define-from-file=envs/staging.env.json".to_string(),
               "--no-sound-null-safety".to_string(),
           ],
           ..Default::default()
       };
       let args = config.build_flutter_args("device1");
       assert!(args.contains(&"--dart-define-from-file=envs/staging.env.json".to_string()));
       assert!(args.contains(&"--no-sound-null-safety".to_string()));
   }
   ```

## Verification

- `cargo test -p fdemon-app -- build_flutter_args` passes

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/config/launch.rs` | No changes needed — coverage already existed |

### Notable Decisions/Tradeoffs

1. **Test already present**: The test `test_build_flutter_args_with_extra_args` (lines 525–539) was already in the module. It creates a `LaunchConfig` with `extra_args: vec!["--verbose", "--no-sound-null-safety"]` and asserts both args appear in the output of `build_flutter_args`. This satisfies the task's acceptance criterion exactly; no new test was added.

### Testing Performed

- `cargo test -p fdemon-app -- build_flutter_args` — Passed (6 tests: basic, with_flavor, with_entry_point, with_extra_args, with_dart_defines, full)

### Risks/Limitations

1. **None**: The existing test covers the case directly. The broader `test_build_flutter_args_full` test also exercises `extra_args`, providing redundant coverage.
