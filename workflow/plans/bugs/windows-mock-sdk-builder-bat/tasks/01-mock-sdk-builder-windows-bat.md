# Task 01: MockSdkBuilder writes `flutter.bat` on Windows

**Severity:** BLOCKER (currently breaking 48 Windows CI tests)

**Estimated Time:** 0.25 hours

## Objective

Make `MockSdkBuilder::build()` produce an SDK directory that passes `validate_sdk_path` on every host OS, including Windows. Today it writes `bin/flutter` unconditionally and writes `bin/flutter.bat` only when the caller has previously called `.with_bat_file()`. None of the seven layout helpers in the same file (`create_fvm_layout`, `create_fvm_legacy_layout`, `create_puro_layout`, `create_asdf_layout`, `create_mise_layout`, `create_proto_layout`, `create_flutter_wrapper_layout`) opt in, and neither do the several direct callers — so on Windows every fixture fails the `bin/flutter.bat` check inside `validate_sdk_path` and the test panics with `FlutterNotFound`.

The audit confirmed this is the **last unfixed fixture site** in the workspace. Earlier rounds patched the analogous helpers in `crates/fdemon-daemon/src/flutter_sdk/{cache_scanner,locator,types}.rs` already.

**Depends on:** None

## Scope

**Files Modified (Write):**
- `tests/sdk_detection/fixtures.rs` — modify the `bin/flutter.bat` write block inside `MockSdkBuilder::build()` (currently lines 119–126)

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/flutter_sdk/types.rs` — for reference on what `validate_sdk_path` actually checks for on Windows (`bin/flutter.bat` per lines 148–152)

## Details

The change replaces the existing `if self.create_bat_file { ... }` block. The two concerns are orthogonal and both must be supported:

- **`create_bat_file` (existing flag):** opt-in on any OS, used by tests that exercise Windows-path code paths from a Unix host. **Keep.**
- **Windows host runtime:** `validate_sdk_path` always requires `bin/flutter.bat` on Windows. Must be **automatic, not opt-in**.

**Before** (lines 119–126 in `tests/sdk_detection/fixtures.rs`):

```rust
// Optional: bin/flutter.bat
if self.create_bat_file {
    fs::write(
        self.root.join("bin").join("flutter.bat"),
        "@echo off\nrem mock flutter.bat\n",
    )
    .unwrap();
}
```

**After:**

```rust
// bin/flutter.bat is required by validate_sdk_path on Windows; opt-in on
// other platforms via .with_bat_file() for tests exercising Windows-path
// code paths from a Unix host.
let need_bat = cfg!(target_os = "windows") || self.create_bat_file;
if need_bat {
    fs::write(
        self.root.join("bin").join("flutter.bat"),
        "@echo off\nrem mock flutter.bat\n",
    )
    .unwrap();
}
```

That is the entire change. No other line in `fixtures.rs` needs touching. No callers need updating — the layout helpers and direct callers all transit through `build()` and benefit automatically.

## Acceptance Criteria

- [ ] `MockSdkBuilder::build()` writes `bin/flutter.bat` whenever `cfg!(target_os = "windows")` is true, regardless of whether `.with_bat_file()` was called.
- [ ] On non-Windows platforms, behaviour is unchanged: `.bat` only written when `.with_bat_file()` was called.
- [ ] `cargo test --test sdk_detection` passes locally on macOS (no Unix regression — the `cfg!(windows)` branch is dead code on macOS).
- [ ] `cargo clippy -p flutter-demon --all-targets -- -D warnings` clean.
- [ ] `cargo fmt --all -- --check` clean.

## Out of Scope

- Renaming `with_bat_file` or removing `create_bat_file`. The opt-in mechanism is still useful and orthogonal.
- Refactoring callers to call `.with_bat_file()` explicitly — once `build()` self-heals on Windows, no caller needs to change.
- Touching the production code that reads SDKs.

---

## Completion Summary

**Status:** Done
**Branch:** fix/detect-windows-bat

### Files Modified

| File | Changes |
|------|---------|
| `tests/sdk_detection/fixtures.rs` | Replaced `if self.create_bat_file` block with `let need_bat = cfg!(target_os = "windows") \|\| self.create_bat_file; if need_bat` so `flutter.bat` is automatically written on Windows regardless of whether `.with_bat_file()` was called |

### Notable Decisions/Tradeoffs

1. **Exact task spec applied verbatim**: The change is precisely the one-liner substitution described in the task. The `create_bat_file` flag is preserved unchanged for Unix hosts exercising Windows code paths.

### Testing Performed

- `cargo test --test sdk_detection` - Passed (103 passed, 0 failed, 23 ignored)
- `cargo clippy -p flutter-demon --all-targets -- -D warnings` - Passed (no warnings)
- `cargo fmt --all -- --check` - Passed (no formatting issues)

### Risks/Limitations

1. **Windows-only coverage on CI**: The `cfg!(target_os = "windows")` branch is dead code on macOS/Linux; actual Windows host validation requires CI runners. No risks on Unix hosts.
