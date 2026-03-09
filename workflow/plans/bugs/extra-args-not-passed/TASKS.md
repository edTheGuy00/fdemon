# Tasks: Fix `extra_args` Not Passed Through Dialog Launch

## Wave 1: Core Fix

Tasks 01 & 02 can run in parallel. Tasks 03 & 04 depend on them.

- [x] [Task 01](tasks/01-add-extra-args-to-launch-context-state.md): Add `extra_args` to `LaunchContextState` + copy in `select_config()`
- [x] [Task 02](tasks/02-add-extra-args-to-launch-params.md): Add `extra_args` to `LaunchParams`
- [x] [Task 03](tasks/03-populate-extra-args-in-build-launch-params.md): Populate `extra_args` in `build_launch_params()` *(depends on 01, 02)*
- [x] [Task 04](tasks/04-use-extra-args-in-handle-launch.md): Use `extra_args` in `handle_launch()` *(depends on 02)*

## Wave 2: Tests (parallel)

- [x] [Task 05](tasks/05-unit-tests-dialog-pipeline.md): Unit tests for `extra_args` in dialog pipeline *(depends on 01-04)*
- [x] [Task 06](tasks/06-verify-build-flutter-args-coverage.md): Verify `build_flutter_args` test coverage

## Wave 3: Build verification

- [x] [Task 07](tasks/07-build-verification.md): Full workspace build, test, clippy, fmt *(depends on all)*
