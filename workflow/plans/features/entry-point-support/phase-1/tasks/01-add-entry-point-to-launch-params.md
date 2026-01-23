## Task: Add entry_point field to LaunchParams

**Objective**: Add the `entry_point` field to the `LaunchParams` struct so it can be passed through the launch flow.

**Depends on**: None

### Scope

- `src/app/new_session_dialog/types.rs`: Add `entry_point` field to `LaunchParams`

### Details

Add `entry_point: Option<PathBuf>` to the `LaunchParams` struct. This struct is used to pass launch parameters from the dialog state to the launch handler.

```rust
use std::path::PathBuf;

/// Parameters for launching a Flutter session
#[derive(Debug, Clone)]
pub struct LaunchParams {
    pub device_id: String,
    pub mode: crate::config::FlutterMode,
    pub flavor: Option<String>,
    pub dart_defines: Vec<String>,
    pub config_name: Option<String>,
    pub entry_point: Option<PathBuf>,  // ADD THIS
}
```

### Acceptance Criteria

1. `LaunchParams` struct has `entry_point: Option<PathBuf>` field
2. Import `std::path::PathBuf` at top of file (if not already imported)
3. Code compiles without errors

### Testing

```rust
#[test]
fn test_launch_params_has_entry_point() {
    let params = LaunchParams {
        device_id: "test".to_string(),
        mode: FlutterMode::Debug,
        flavor: None,
        dart_defines: vec![],
        config_name: None,
        entry_point: Some(PathBuf::from("lib/main_dev.dart")),
    };

    assert_eq!(params.entry_point, Some(PathBuf::from("lib/main_dev.dart")));
}
```

### Notes

- This is a simple struct field addition
- Will cause compile errors in `build_launch_params()` until Task 04 is complete
- Can be done in parallel with Task 02

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/new_session_dialog/types.rs` | Added `use std::path::PathBuf;` import and `entry_point: Option<PathBuf>` field to `LaunchParams` struct |

### Notable Decisions/Tradeoffs

1. **Field Position**: Added `entry_point` as the last field in `LaunchParams` struct to maintain consistency with the existing field ordering (required fields first, then optional fields)

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (compiles without errors)
- `cargo test --lib` - Passed (1496 tests passed, 0 failed)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

None. This is a simple struct field addition that integrates cleanly with the existing codebase. The `build_launch_params()` method in `src/app/new_session_dialog/state.rs` already properly initializes the field with `self.launch_context.entry_point.clone()`
