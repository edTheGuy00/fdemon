# Task: VSCode launch.json args Parsing

**Objective**: Parse `--flavor` from both `toolArgs` and `args` fields in launch.json.

**Depends on**: Task 01 (Config Priority)

## Problem

User's `.vscode/launch.json` uses `args` field for flavor, but we only parse `toolArgs`:

```json
{
    "configurations": [
        {
            "name": "DEV",
            "request": "launch",
            "type": "dart",
            "program": "lib/main.dart",
            "args": [
                "--flavor",
                "develop"
            ]
        }
    ]
}
```

### Root Cause

In `vscode.rs`, the `args` field is defined but marked `#[allow(dead_code)]`:

```rust
/// Arguments passed to the app's main()
#[serde(default)]
#[allow(dead_code)]
args: Vec<String>,
```

Only `toolArgs` is parsed for `--flavor` and `--dart-define`:

```rust
// Extract dart-defines and flavor from toolArgs
let (dart_defines, flavor, extra_args) = parse_tool_args(&vscode.tool_args);
```

### VSCode Dart Extension Semantics

- `toolArgs`: Arguments passed to the Flutter tool (`flutter run --flavor X`)
- `args`: Arguments passed to the Dart VM / app's main()

Some users put `--flavor` in `args` because it "works" in VSCode (the extension handles it). We should be lenient and parse both.

## Scope

- `src/config/vscode.rs` - Parse `args` field for Flutter-specific arguments

## Implementation

### 1. Remove dead_code attribute and use args

```rust
// In vscode.rs VSCodeConfiguration struct
/// Arguments passed to the app's main()
/// Note: Some users put --flavor here, so we parse it too
#[serde(default)]
args: Vec<String>,
```

### 2. Update convert_vscode_config

```rust
fn convert_vscode_config(vscode: VSCodeConfiguration) -> Option<ResolvedLaunchConfig> {
    // Parse flutter mode
    let mode = vscode
        .flutter_mode
        .as_deref()
        .map(parse_flutter_mode)
        .unwrap_or(FlutterMode::Debug);

    // Extract dart-defines and flavor from toolArgs (primary)
    let (mut dart_defines, mut flavor, mut extra_args) = parse_tool_args(&vscode.tool_args);

    // Also parse args field for --flavor (fallback for misconfigured projects)
    // This handles users who put --flavor in args instead of toolArgs
    if flavor.is_none() {
        let (args_defines, args_flavor, args_extra) = parse_tool_args(&vscode.args);

        // Only use args flavor if toolArgs didn't have one
        if args_flavor.is_some() {
            flavor = args_flavor;
            tracing::debug!(
                "Found --flavor in 'args' field for config '{}' (recommend moving to 'toolArgs')",
                vscode.name
            );
        }

        // Merge dart-defines from args (lower priority)
        for (key, value) in args_defines {
            dart_defines.entry(key).or_insert(value);
        }

        // Note: We don't merge extra_args from 'args' as those are meant for Dart VM
    }

    // ... rest unchanged
}
```

### 3. Add test for args-based flavor

```rust
#[test]
fn test_vscode_config_flavor_from_args() {
    let temp = tempdir().unwrap();
    let vscode_dir = temp.path().join(".vscode");
    std::fs::create_dir_all(&vscode_dir).unwrap();

    // This is the user's actual launch.json format
    let content = r#"{
        "version": "0.2.0",
        "configurations": [
            {
                "name": "DEV",
                "request": "launch",
                "type": "dart",
                "program": "lib/main.dart",
                "args": [
                    "--flavor",
                    "develop"
                ]
            },
            {
                "name": "STG",
                "request": "launch",
                "type": "dart",
                "program": "lib/main.dart",
                "args": [
                    "--flavor",
                    "staging"
                ]
            },
            {
                "name": "PROD",
                "request": "launch",
                "type": "dart",
                "program": "lib/main.dart",
                "args": [
                    "--flavor",
                    "production"
                ]
            }
        ]
    }"#;
    std::fs::write(vscode_dir.join("launch.json"), content).unwrap();

    let configs = load_vscode_configs(temp.path());

    assert_eq!(configs.len(), 3);

    assert_eq!(configs[0].config.name, "DEV");
    assert_eq!(configs[0].config.flavor, Some("develop".to_string()));

    assert_eq!(configs[1].config.name, "STG");
    assert_eq!(configs[1].config.flavor, Some("staging".to_string()));

    assert_eq!(configs[2].config.name, "PROD");
    assert_eq!(configs[2].config.flavor, Some("production".to_string()));
}

#[test]
fn test_vscode_config_toolargs_takes_precedence() {
    let temp = tempdir().unwrap();
    let vscode_dir = temp.path().join(".vscode");
    std::fs::create_dir_all(&vscode_dir).unwrap();

    // Both toolArgs and args have --flavor, toolArgs should win
    let content = r#"{
        "configurations": [
            {
                "name": "Test",
                "type": "dart",
                "request": "launch",
                "toolArgs": ["--flavor", "from-toolargs"],
                "args": ["--flavor", "from-args"]
            }
        ]
    }"#;
    std::fs::write(vscode_dir.join("launch.json"), content).unwrap();

    let configs = load_vscode_configs(temp.path());

    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].config.flavor, Some("from-toolargs".to_string()));
}
```

## Acceptance Criteria

1. launch.json with `args: ["--flavor", "develop"]` is parsed correctly
2. `toolArgs` takes precedence over `args` for `--flavor`
3. All three user configs (DEV, STG, PROD) appear in startup dialog
4. Existing toolArgs-based configs continue to work
5. Unit tests cover both scenarios

## Testing

Run the existing test suite plus new tests:
```bash
cargo test vscode
```

## Notes

- This is a lenient parsing approach - we accept "incorrect" configs
- A debug log warns when flavor is found in `args` (recommend `toolArgs`)
- We only extract `--flavor` and `--dart-define` from `args`, not other extras
- The `args` field's actual purpose (Dart VM args) is not affected

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/config/vscode.rs` | Removed `#[allow(dead_code)]` from `args` field, updated `convert_vscode_config()` to parse `args` as fallback for `--flavor` and `--dart-define`, added two new unit tests |

### Implementation Details

1. **Removed dead_code attribute** (line 54-57): The `args` field is now actively used and includes a documentation comment explaining why we parse it for Flutter arguments.

2. **Updated convert_vscode_config function** (lines 133-156):
   - Changed `dart_defines` and `flavor` to mutable bindings
   - Added fallback parsing logic: if `flavor` is None after parsing `toolArgs`, we parse the `args` field
   - `toolArgs` takes precedence (checked first)
   - When flavor is found in `args`, a debug log recommends moving it to `toolArgs`
   - Dart-defines from `args` are merged at lower priority using `entry().or_insert()`
   - Extra args from `args` are intentionally NOT merged (Dart VM args, not Flutter tool args)

3. **Added comprehensive tests** (lines 695-777):
   - `test_vscode_config_flavor_from_args`: Validates that all three user configs (DEV, STG, PROD) with `args`-based flavors are parsed correctly
   - `test_vscode_config_toolargs_takes_precedence`: Verifies that when both `toolArgs` and `args` have `--flavor`, `toolArgs` wins

### Notable Decisions/Tradeoffs

1. **Lenient Parsing Approach**: We intentionally parse `--flavor` from both `toolArgs` and `args` fields, even though technically `args` is meant for Dart VM arguments. This improves user experience by accepting configurations that "work" in VSCode but are technically misconfigured.

2. **Debug Logging**: Added a debug log when flavor is found in `args` to educate users without breaking their workflow.

3. **Precedence Rule**: `toolArgs` takes absolute precedence over `args` - this ensures correct behavior for properly configured projects while still supporting misconfigured ones.

4. **Selective Merging**: We only merge `--flavor` and `--dart-define` from `args`, not extra arguments, since those truly are Dart VM arguments and shouldn't be passed to the Flutter tool.

### Testing Performed

- `cargo test config::vscode::tests` - **PASSED** (21 tests)
- New test `test_vscode_config_flavor_from_args` - **PASSED**
- New test `test_vscode_config_toolargs_takes_precedence` - **PASSED**
- All existing vscode tests continue to pass
- `rustfmt src/config/vscode.rs` - Code properly formatted
- No clippy warnings related to vscode module

### Acceptance Criteria Verification

1. ✅ launch.json with `args: ["--flavor", "develop"]` is parsed correctly
2. ✅ `toolArgs` takes precedence over `args` for `--flavor`
3. ✅ All three user configs (DEV, STG, PROD) appear in startup dialog (test validates config parsing)
4. ✅ Existing toolArgs-based configs continue to work (all existing tests pass)
5. ✅ Unit tests cover both scenarios (2 new tests added)

### Risks/Limitations

1. **None**: The implementation is backward compatible and only adds new functionality. Existing configs continue to work exactly as before.
