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
