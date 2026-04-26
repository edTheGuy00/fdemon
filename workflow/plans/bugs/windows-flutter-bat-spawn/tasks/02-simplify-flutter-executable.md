## Task: Simplify `FlutterExecutable::command()` — drop manual `cmd /c` wrapper

**Objective**: Make both `FlutterExecutable` variants invoke the resolved absolute path directly via `Command::new(path)`. Modern Rust (≥ 1.77.2, well above our MSRV of 1.70 — though our MSRV requires bumping if we want a guarantee here) handles `.bat` invocation correctly when the program path has an explicit extension. Keeping the enum preserves the API and the metadata distinction (`Direct` vs `WindowsBatch`) but removes the buggy `cmd /c` wrapper that was triggering the user-reported "The system cannot find the path specified" error on paths containing whitespace.

**Depends on**: 01-add-windows-deps (no direct code use, but the task ordering ensures Cargo.toml is updated first)

**Estimated Time**: 1-2h

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/flutter_sdk/types.rs`:
  - Rewrite `FlutterExecutable::command()` (lines 74-83) to use `Command::new(path)` for both variants.
  - Update the doc comment on `WindowsBatch` to explain it's a metadata marker (the runtime invocation is identical to `Direct` because Rust's stdlib handles `.bat` correctly when given an absolute path).
  - Update the existing tests in the same file (lines 391-409) — `test_flutter_executable_direct_command` should now also cover `WindowsBatch` behavior.
  - Confirm `FlutterExecutable::path()` (lines 64-68) is unchanged.
  - Confirm `validate_sdk_path` and `validate_sdk_path_lenient` (lines 134-207) are unchanged (they still pick `flutter.bat` on Windows).

**Files Read (Dependencies):**
- `Cargo.toml` (to confirm MSRV).
- `docs/CODE_STANDARDS.md` (for any project-specific patterns around Windows `cfg` blocks).

### Details

Current code (`crates/fdemon-daemon/src/flutter_sdk/types.rs:74-83`):

```rust
pub fn command(&self) -> tokio::process::Command {
    match self {
        Self::Direct(path) => tokio::process::Command::new(path),
        Self::WindowsBatch(path) => {
            let mut cmd = tokio::process::Command::new("cmd");
            cmd.args(["/c", &*path.to_string_lossy()]);
            cmd
        }
    }
}
```

Replacement:

```rust
/// Configures a [`tokio::process::Command`] for this executable.
///
/// Both variants now invoke the resolved absolute path directly. Rust's
/// stdlib (≥ 1.77.2 — our MSRV is 1.70 but the runtime requirement is
/// effectively a recent toolchain) handles `.bat` / `.cmd` invocation
/// safely when the program path has an explicit extension, including
/// the `cmd.exe` argument-escape rules covered by CVE-2024-24576.
///
/// The `WindowsBatch` variant is retained as a *metadata* marker so callers
/// and logs can tell that the underlying executable is a batch file. The
/// previous `cmd /c <path>` wrapper has been removed because it caused
/// quote-stripping failures on paths containing whitespace
/// (see issues #32, #34).
pub fn command(&self) -> tokio::process::Command {
    match self {
        Self::Direct(path) | Self::WindowsBatch(path) => {
            tokio::process::Command::new(path)
        }
    }
}
```

The `validate_sdk_path*` functions still produce `WindowsBatch(<root>/bin/flutter.bat)` on Windows — that is unchanged. The path is absolute and includes the `.bat` extension, which is exactly what `Command::new` needs to safely invoke it.

Also rewrite `test_flutter_executable_direct_command` at line 402-408 to cover both variants (and add a unit test that verifies `command().get_program()` returns the path itself — not `"cmd"` — for `WindowsBatch`):

```rust
#[test]
fn test_flutter_executable_direct_command_invokes_path() {
    let path = PathBuf::from("/usr/local/flutter/bin/flutter");
    let exe = FlutterExecutable::Direct(path.clone());
    let cmd = exe.command();
    assert_eq!(cmd.as_std().get_program(), path.as_os_str());
}

#[test]
fn test_flutter_executable_windows_batch_command_invokes_path() {
    let path = PathBuf::from("C:\\flutter\\bin\\flutter.bat");
    let exe = FlutterExecutable::WindowsBatch(path.clone());
    let cmd = exe.command();
    // After the fix, WindowsBatch invokes the .bat directly (not cmd.exe)
    assert_eq!(cmd.as_std().get_program(), path.as_os_str());
}
```

(Note: `tokio::process::Command::as_std()` returns `&std::process::Command`; `get_program()` is stable. Confirm at write time.)

### Acceptance Criteria

1. `FlutterExecutable::command()` returns a `Command` whose program is the path itself for *both* variants.
2. No reference to `"cmd"` or `"/c"` remains in `types.rs`.
3. The doc comment on `WindowsBatch` and on `command()` accurately describes the metadata-marker role.
4. Existing tests still pass on Unix.
5. The two new tests above pass on the platform that matches each variant (the `WindowsBatch` test should be runnable everywhere because we are only inspecting the program field — no actual spawn).
6. `cargo clippy -p fdemon-daemon -- -D warnings` is clean.

### Testing

```bash
cargo test -p fdemon-daemon flutter_sdk::types
cargo clippy -p fdemon-daemon -- -D warnings
```

### Notes

- **Do not** delete the `WindowsBatch` variant — keep the enum. We have explicit user agreement to retain it.
- **Do not** add a `current_dir(...)` call here. Spawn-site callers (e.g. `process.rs`) decide CWD.
- The unit-test additions above are the *only* tests that go in `types.rs`. Cross-platform end-to-end Windows assertions live in `windows_tests.rs` (task 05).
- If the project's MSRV is below 1.77.2 (currently `1.70+` per `docs/DEVELOPMENT.md`), bump it to `1.77.2` in the workspace `Cargo.toml`'s `rust-version = "..."` field and in `docs/DEVELOPMENT.md`. The CVE-2024-24576 fix is what makes direct `.bat` invocation safe; we should not silently rely on a newer compiler than declared.
