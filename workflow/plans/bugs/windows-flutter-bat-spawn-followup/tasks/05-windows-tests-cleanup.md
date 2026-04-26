## Task: `windows_tests.rs` cleanup + new shim-installer Windows tests

**Objective**: Address the test-quality issues raised in `ACTION_ITEMS.md` (Minors 5, 6, 11, 12, 13) and add the Windows-only tests for Strategy 12 (the shim-installer fix from Task 04). Single file in scope; cleanly parallel-isolated from Task 06.

**Depends on**: Task 01 (uses the relocated `windows_hint()` from `flutter_sdk::diagnostics`), Task 04 (provides the Strategy 12 implementation that the new shim tests exercise) — Wave B

**Estimated Time**: 1.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/flutter_sdk/windows_tests.rs`:
  - Rename all 8 existing test functions to follow `test_<function>_<scenario>_<expected_result>` per `docs/CODE_STANDARDS.md`.
  - Strengthen `windows_batch_command_works_with_path_containing_spaces` so the fake `.bat` echoes back its arguments and the test asserts the argument arrived intact.
  - Replace `unwrap()` with `.expect("...")` and a descriptive message in test bodies.
  - Add inline comments next to each `#[serial]` annotation explaining that PATH mutation requires serialization (matches the convention in `locator.rs`'s test module).
  - Replace `r"bin\cache\dart-sdk"` literals in `create_fake_sdk` with `Path::new("bin").join("cache").join("dart-sdk")` (and similarly for `bin\flutter.bat`).
  - Add two new tests for Strategy 12 shim layouts: `test_find_flutter_sdk_scoop_shim_resolves_via_strategy_12` and `test_find_flutter_sdk_winget_shim_resolves_via_strategy_12`.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/flutter_sdk/locator.rs` (read-only — for the Strategy 12 implementation, the existing `path_prepend_guard` helper, and the test naming convention used in its `mod tests`).
- `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs` (read-only — produced by Task 01).

### Details

#### Test rename mapping

| Current name | New name |
|--------------|----------|
| `validate_sdk_path_returns_windows_batch_variant` | `test_validate_sdk_path_windows_returns_windows_batch_variant` |
| `windows_batch_command_invokes_path_directly` | `test_command_windows_batch_invokes_bat_directly_not_cmd` |
| `windows_batch_command_executes_successfully` | `test_command_windows_batch_executes_bat_returns_zero` |
| `windows_batch_command_works_with_path_containing_spaces` | `test_command_windows_batch_path_with_spaces_passes_args_intact` |
| `which_resolves_flutter_bat_via_pathext` | `test_which_finds_flutter_bat_via_pathext` |
| `dunce_canonicalize_strips_unc_prefix` | `test_dunce_canonicalize_strips_unc_prefix_on_windows` |
| `resolve_sdk_root_from_binary_returns_sdk_root` | `test_resolve_sdk_root_from_binary_walks_up_two_parents` |
| `find_flutter_sdk_resolves_via_path` | `test_find_flutter_sdk_via_path_returns_system_source` |

#### Strengthen the spaces-in-path test

Modify the fake `.bat` shim used by `test_command_windows_batch_path_with_spaces_passes_args_intact` (formerly `windows_batch_command_works_with_path_containing_spaces`) to echo `%*` so we can verify the argument arrived intact:

```rust
// In create_fake_sdk (or a new helper):
fs::write(
    &batch_path,
    "@echo off\r\necho FAKE_FLUTTER %*\r\n",
).expect("write fake flutter.bat");
```

Then in the test body:

```rust
let output = rt.block_on(async {
    exe.command()
        .arg("devices")
        .output()
        .await
        .expect("spawn flutter.bat with spaces in path")
});
assert!(
    output.status.success(),
    "exit code: {:?}",
    output.status.code()
);
let stdout = String::from_utf8_lossy(&output.stdout);
assert!(
    stdout.contains("FAKE_FLUTTER") && stdout.contains("devices"),
    "expected stdout to contain `FAKE_FLUTTER devices`, got: {stdout}"
);
```

#### Replace `unwrap()` with `.expect("...")` in test bodies

Targets: `windows_tests.rs:87-88, 107-108` and any other `.unwrap()` introduced after the rename. Each `.expect(...)` should describe what was being attempted, not just "failed". Examples:
- `Runtime::new().unwrap()` → `Runtime::new().expect("create tokio runtime")`.
- `output().await.unwrap()` → `output().await.expect("execute fake flutter.bat")`.

#### `#[serial]` rationale comments

For each `#[serial]`-tagged test, add a one-line comment immediately above:

```rust
// PATH mutation — must run serially (set_var is process-wide and not thread-safe).
#[test]
#[serial]
fn test_which_finds_flutter_bat_via_pathext() {
    ...
}
```

Apply to both `test_which_finds_flutter_bat_via_pathext` and `test_find_flutter_sdk_via_path_returns_system_source`. The comment should match the style used in `locator.rs`'s test module.

#### Portable `Path::join` in `create_fake_sdk`

Current (lines 21-22):

```rust
fs::create_dir_all(root.join(r"bin\cache\dart-sdk")).unwrap();
fs::write(root.join(r"bin\flutter.bat"), ...).unwrap();
```

Replacement:

```rust
fs::create_dir_all(root.join("bin").join("cache").join("dart-sdk"))
    .expect("create fake bin/cache/dart-sdk dir");
fs::write(
    root.join("bin").join("flutter.bat"),
    "@echo off\r\necho FAKE_FLUTTER %*\r\n",
)
.expect("write fake flutter.bat");
```

(Note the `.bat` content change — `%*` echoes args for the strengthened spaces test. If `create_fake_sdk` is reused across multiple tests where args don't need echoing, accept that the prefix `FAKE_FLUTTER` plus a trailing space is harmless on stdout.)

#### New tests — Scoop and winget shim layouts (Strategy 12)

Add to `windows_tests.rs`:

```rust
/// Helper: build a scoop-style shim layout under a tempdir.
/// scoop installs flutter shims at <root>/scoop/shims/flutter.bat — there is
/// no <root>/scoop/bin/, so strategies 10 and 11 must reject the inferred root
/// and Strategy 12 must fire.
fn create_scoop_shim_layout(root: &Path) -> PathBuf {
    let shims = root.join("scoop").join("shims");
    fs::create_dir_all(&shims).expect("create scoop shims dir");
    let bat = shims.join("flutter.bat");
    fs::write(&bat, "@echo off\r\necho FAKE_FLUTTER %*\r\n")
        .expect("write scoop flutter.bat");
    bat
}

/// Helper: build a winget-style shim layout under a tempdir.
/// winget shims live at <root>/Links/flutter.bat with no surrounding SDK tree.
fn create_winget_shim_layout(root: &Path) -> PathBuf {
    let links = root.join("Links");
    fs::create_dir_all(&links).expect("create winget Links dir");
    let bat = links.join("flutter.bat");
    fs::write(&bat, "@echo off\r\necho FAKE_FLUTTER %*\r\n")
        .expect("write winget flutter.bat");
    bat
}

// PATH mutation — must run serially.
#[test]
#[serial]
fn test_find_flutter_sdk_scoop_shim_resolves_via_strategy_12() {
    let temp = TempDir::new().expect("tempdir");
    let bat = create_scoop_shim_layout(temp.path());
    let bin_dir = bat.parent().expect("bat parent (scoop/shims)");
    let _path_guard = path_prepend_guard(bin_dir);
    std::env::remove_var("FLUTTER_ROOT");
    let project = TempDir::new().expect("project tempdir");

    let sdk = find_flutter_sdk(project.path(), None)
        .expect("Strategy 12 should resolve scoop shim");
    assert_eq!(sdk.source, SdkSource::PathInferred);
    assert_eq!(sdk.version, "unknown");
    assert!(sdk.executable.path().ends_with("flutter.bat"));
}

// PATH mutation — must run serially.
#[test]
#[serial]
fn test_find_flutter_sdk_winget_shim_resolves_via_strategy_12() {
    let temp = TempDir::new().expect("tempdir");
    let bat = create_winget_shim_layout(temp.path());
    let bin_dir = bat.parent().expect("bat parent (Links)");
    let _path_guard = path_prepend_guard(bin_dir);
    std::env::remove_var("FLUTTER_ROOT");
    let project = TempDir::new().expect("project tempdir");

    let sdk = find_flutter_sdk(project.path(), None)
        .expect("Strategy 12 should resolve winget shim");
    assert_eq!(sdk.source, SdkSource::PathInferred);
    assert_eq!(sdk.version, "unknown");
    assert!(sdk.executable.path().ends_with("flutter.bat"));
}
```

Note: the `path_prepend_guard` helper is defined in `locator.rs`'s test module. Either import it via `use super::tests::path_prepend_guard;` if visibility allows, or duplicate a minimal version inside `windows_tests.rs`. (Visibility is restricted because `path_prepend_guard` is in `#[cfg(test)] mod tests`. The cleanest approach is to copy the helper into `windows_tests.rs` since both modules are test-only.)

### Acceptance Criteria

1. All 8 existing tests in `windows_tests.rs` use `test_<function>_<scenario>_<expected_result>` naming.
2. The renamed `test_command_windows_batch_path_with_spaces_passes_args_intact` asserts that the argument was passed to the fake `.bat` (via `%*` echo), not just that spawn succeeded.
3. All `unwrap()` calls in test bodies are replaced with `.expect("...")` and a descriptive message.
4. Each `#[serial]`-tagged test has a one-line rationale comment immediately above the attribute.
5. `create_fake_sdk` uses chained `Path::join` calls instead of `r"bin\..."` literals.
6. Two new Windows-only tests, `test_find_flutter_sdk_scoop_shim_resolves_via_strategy_12` and `test_find_flutter_sdk_winget_shim_resolves_via_strategy_12`, exercise Strategy 12 against scoop and winget shim layouts respectively.
7. `cargo check -p fdemon-daemon` succeeds on macOS (the file is `#[cfg(all(test, target_os = "windows"))]`-gated, so checks but does not actually compile the file body on macOS).
8. On Windows CI, `cargo test -p fdemon-daemon` runs all tests in `windows_tests.rs` (10 total: 8 renamed + 2 new) and they pass.

### Testing

```bash
# Local (macOS) — cfg gate compiles file out, but macOS check confirms the syntax is well-formed
cargo check -p fdemon-daemon
cargo test -p fdemon-daemon flutter_sdk::types
cargo test -p fdemon-daemon flutter_sdk::locator

# Windows CI — actually executes windows_tests.rs
# (verified via .github/workflows/ci.yml after Task 03 lands)
```

### Notes

- The new tests rely on Strategy 12 from Task 04. Verify Task 04 is merged before Wave B starts; the orchestrator will sequence this automatically because Task 05 depends on Task 04.
- The two new shim-layout helpers (`create_scoop_shim_layout`, `create_winget_shim_layout`) are intentionally similar — extracting a shared parametric helper would be premature, since each represents a distinct user-visible scenario.
- If `path_prepend_guard` is not accessible from `windows_tests.rs`, copy it locally rather than making the locator test helper public — keep the public surface minimal.
- Do NOT add tests that require a real Flutter SDK. The existing tests already use fake `.bat` shims; the new tests follow the same pattern.
- Do NOT change the inner `#![cfg(all(test, target_os = "windows"))]` attribute on `windows_tests.rs`. The file remains Windows-only.
