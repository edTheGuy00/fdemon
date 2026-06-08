## Task: Phase 3 wizard protocol additions (params, action fields, copy key)

**Objective**: Add the app-side protocol surface Phase 3 needs: `AndroidStepParams`,
extend `UpdateAction::RunWizardStep` to carry Android install + Android-env params,
add the `Message::InstallWizardCopyCommand` variant, and route the `c` key in
`UiMode::InstallWizard` to it.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mod.rs`: add `AndroidStepParams`; extend
  `UpdateAction::RunWizardStep` with `android: Option<AndroidStepParams>` and the
  PathConfig step's `android_sdk_root: Option<PathBuf>`.
- `crates/fdemon-app/src/message.rs`: add `Message::InstallWizardCopyCommand`.
- `crates/fdemon-app/src/handler/keys.rs`: route `c` →
  `Message::InstallWizardCopyCommand` in `handle_key_install_wizard`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/types.rs`: `WizardStepKind` (field type).
- `crates/fdemon-app/src/config/types.rs`: `ToolchainSettings` (field types for
  the params: `android_sdk_root`, `android_api_level`, `cmdline_tools_build`,
  `jdk_path`).
- Existing `FlutterStepParams` + `RunWizardStep` definition as the template.

### Details

Mirror `FlutterStepParams`. The `RunWizardStep` action gains a parallel
`android` slot; the existing `install` (FlutterSdk) and `path_bin_dir` (PathConfig)
fields stay. PathConfig additionally needs the Android SDK root so it can write
`ANDROID_HOME`.

```rust
pub struct AndroidStepParams {
    pub sdk_root: Option<std::path::PathBuf>,   // None → daemon resolves default
    pub api_level: u32,
    pub cmdline_tools_build: Option<String>,
    pub jdk_path: Option<std::path::PathBuf>,
}

pub enum UpdateAction {
    // ...
    RunWizardStep {
        kind: WizardStepKind,
        install: Option<FlutterStepParams>,         // FlutterSdk
        path_bin_dir: Option<std::path::PathBuf>,   // PathConfig (Flutter bin)
        android_sdk_root: Option<std::path::PathBuf>, // PathConfig (ANDROID_HOME) — NEW
        android: Option<AndroidStepParams>,          // AndroidTools — NEW
    },
}
```

`message.rs`:

```rust
/// `c` in the Install Wizard — copy the selected step's guided command to the
/// clipboard (e.g. the JDK install command). No-op when the step has no command.
InstallWizardCopyCommand,
```

`keys.rs` — add to `handle_key_install_wizard` next to the `r` arm:

```rust
InputKey::Char('c') => Some(Message::InstallWizardCopyCommand),
```

Update the doc comment block above `handle_key_install_wizard` to list `c`.

### Acceptance Criteria

1. `AndroidStepParams` is defined and `UpdateAction::RunWizardStep` carries the new
   `android` and `android_sdk_root` fields; all existing constructors of
   `RunWizardStep` (task 06, and any tests) are updated to the new shape.
2. `Message::InstallWizardCopyCommand` exists.
3. `c` in `UiMode::InstallWizard` produces `Message::InstallWizardCopyCommand`
   (unit-tested in the existing `handle_key_install_wizard` test module).
4. Adding the new action fields does not break exhaustive matches — any `match` on
   `RunWizardStep` (e.g. the Phase 2 executor) compiles; coordinate with task 06,
   which owns the executor and will consume the new fields.
5. `cargo check -p fdemon-app` passes.

### Testing

```rust
#[test]
fn test_c_in_install_wizard_emits_copy_command() {
    let mut state = AppState::for_test();
    state.ui_mode = UiMode::InstallWizard;
    let msg = handle_key(InputKey::Char('c'), &mut state);
    assert!(matches!(msg, Some(Message::InstallWizardCopyCommand)));
}
```

### Notes

- This is a pure additive-protocol task (mirrors Phase 2 task 05). All consumption
  is in tasks 06 (executor) and 07 (handlers).
- Because `RunWizardStep` is a struct-variant, adding fields forces every
  construction site to update. The only existing site is the Phase 2 executor in
  `actions/mod.rs` (task 06 rewrites that arm anyway) and any dispatch tests. If
  task 06 has not landed yet, set the new fields to `None` at the existing
  construction/test sites to keep the tree compiling.
- Keep `c` distinct from existing wizard keys (`Esc`/`Tab`/`j`/`k`/`Enter`/`r`).

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mod.rs` | Added `AndroidStepParams` struct; extended `UpdateAction::RunWizardStep` with `android_sdk_root: Option<PathBuf>` and `android: Option<AndroidStepParams>` fields |
| `crates/fdemon-app/src/message.rs` | Added `Message::InstallWizardCopyCommand` variant with doc comment |
| `crates/fdemon-app/src/handler/keys.rs` | Added `c` → `Message::InstallWizardCopyCommand` arm in `handle_key_install_wizard`; updated doc comment to list `c`; added `test_c_in_install_wizard_emits_copy_command` test |
| `crates/fdemon-app/src/handler/update.rs` | Added no-op stub dispatch arm for `Message::InstallWizardCopyCommand` to keep match exhaustive |
| `crates/fdemon-app/src/actions/mod.rs` | Updated `RunWizardStep` pattern match to destructure new fields (as `_android_sdk_root`, `_android`); updated all 5 test construction sites to pass `android_sdk_root: None, android: None` |
| `crates/fdemon-app/src/handler/install_wizard/actions.rs` | Updated 2 `RunWizardStep` construction sites to pass `android_sdk_root: None, android: None`; updated 2 pattern matches to use `..` |

### Notable Decisions/Tradeoffs

1. **No-op stub for `InstallWizardCopyCommand` in `update.rs`**: Task 07 will implement the full handler. The stub was necessary to satisfy Rust's exhaustive match requirement.
2. **`..` in existing pattern matches**: The two `matches!` assertions in `handler/install_wizard/actions.rs` that checked `RunWizardStep` fields used explicit field bindings. Added `..` to ignore the new fields rather than repeating `android_sdk_root: None, android: None` — this keeps the test assertions focused on the fields they are testing.
3. **`_android_sdk_root` / `_android` prefixed bindings in executor**: The Phase 2 executor in `actions/mod.rs` destructures the new fields with underscore prefixes to suppress unused-variable warnings while signaling clearly that task 06 will consume them.

### Testing Performed

- `cargo check -p fdemon-app` — Passed
- `cargo test -p fdemon-app` — Passed (2,723 tests, 0 failed)
- `cargo check --workspace --all-targets` — Passed
- `cargo fmt --all -- --check` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **No executor logic**: The new `android_sdk_root` and `android` fields are wired into the protocol but not yet consumed by the executor. Task 06 owns that implementation.
