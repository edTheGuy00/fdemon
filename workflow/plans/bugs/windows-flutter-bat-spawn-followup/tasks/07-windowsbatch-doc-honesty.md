## Task: Honest doc-comment on `FlutterExecutable::WindowsBatch`

**Objective**: Replace the current "metadata marker" doc-comment on the `WindowsBatch` enum variant with honest text describing its actual semantics. After Wave-1, both `Direct(p)` and `WindowsBatch(p)` are operationally identical (`Command::new(p)` for both). The doc-comment claims callers can use the variant as a metadata marker, but no production code actually does.

This is a doc-only edit — the variant itself is preserved per the original BUG.md decision to avoid API churn.

**Depends on**: nothing — Wave A

**Estimated Time**: 0.25h

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/flutter_sdk/types.rs`:
  - Update the doc-comment on the `WindowsBatch` enum variant (currently around lines 54-66) to reflect actual behavior.
  - Update the doc-comment on `FlutterExecutable::command()` to remove the now-misleading "metadata marker so callers and logs can tell that the underlying executable is a batch file" language.

**Files Read (Dependencies):**
- `workflow/plans/bugs/windows-flutter-bat-spawn/BUG.md` (Risks/Open Questions #3 — for context on the variant-retention decision).

### Details

#### Current doc-comment

```rust
/// Represents how to invoke the Flutter binary.
pub enum FlutterExecutable {
    /// Direct invocation of the binary.
    Direct(PathBuf),
    /// On Windows, `.bat` files cannot be invoked directly via
    /// `Command::new`. This variant marks a `.bat` file so we know to wrap
    /// it via `cmd /c` at spawn time.
    ///
    /// (Historical note: kept as a metadata marker even though the runtime
    /// invocation is now identical to `Direct` because Rust's stdlib handles
    /// `.bat` correctly when given an absolute path.)
    WindowsBatch(PathBuf),
}
```

#### Replacement

```rust
/// Represents how to invoke the Flutter binary.
pub enum FlutterExecutable {
    /// Direct invocation of the binary by absolute path.
    Direct(PathBuf),
    /// Windows `.bat` shim path.
    ///
    /// **Operationally identical to `Direct`** — both variants spawn via
    /// `Command::new(path)` directly. The Rust stdlib (≥ 1.77.2, our
    /// declared MSRV) handles `.bat` argument escaping safely per
    /// CVE-2024-24576, so no `cmd /c` wrapper is needed.
    ///
    /// Retained as a separate variant for backward compatibility with
    /// callers that pattern-match on it. New code should rely on the path's
    /// extension (`.bat` / `.cmd`) for batch-file detection rather than the
    /// variant tag.
    WindowsBatch(PathBuf),
}
```

#### `command()` doc-comment update

Current (after Wave-1):

```rust
/// Configures a [`tokio::process::Command`] for this executable.
///
/// Both variants now invoke the resolved absolute path directly. Rust's
/// stdlib (≥ 1.77.2 — our MSRV) handles `.bat` / `.cmd` invocation
/// safely when the program path has an explicit extension, including
/// the `cmd.exe` argument-escape rules covered by CVE-2024-24576.
///
/// The `WindowsBatch` variant is retained as a *metadata* marker so callers
/// and logs can tell that the underlying executable is a batch file. The
/// previous `cmd /c <path>` wrapper has been removed because it caused
/// quote-stripping failures on paths containing whitespace
/// (see issues #32, #34).
pub fn command(&self) -> tokio::process::Command {
    ...
}
```

Replacement:

```rust
/// Configures a [`tokio::process::Command`] for this executable.
///
/// Both variants invoke the resolved absolute path directly via
/// `Command::new(path)`. Rust's stdlib (≥ 1.77.2 — our MSRV) handles
/// `.bat` / `.cmd` invocation safely per CVE-2024-24576 when the program
/// path has an explicit extension. The previous `cmd /c <path>` wrapper
/// (removed in #32/#34's fix) caused quote-stripping failures on paths
/// containing whitespace.
///
/// The two variants are operationally identical at this layer; callers
/// distinguish batch files via the path extension if needed.
pub fn command(&self) -> tokio::process::Command {
    ...
}
```

### Acceptance Criteria

1. The doc-comment on `WindowsBatch` no longer claims it is a "metadata marker" that callers consume.
2. The doc-comment honestly states that the two variants are operationally identical at the `command()` boundary and recommends extension-based detection for batch files.
3. The doc-comment on `command()` removes the "metadata marker" language and instead describes both variants behaving identically.
4. No code change — only doc-comment text.
5. `cargo doc -p fdemon-daemon --no-deps` builds without warnings.

### Testing

```bash
cargo doc -p fdemon-daemon --no-deps
cargo check -p fdemon-daemon
```

### Notes

- The `WindowsBatch` variant is intentionally preserved per `workflow/plans/bugs/windows-flutter-bat-spawn/BUG.md` Risks/Open Questions #3. Do NOT collapse it.
- Task 04 introduces a private `flutter_executable_from_binary_path` helper inside `locator.rs` that branches on path extension. After Task 04 lands, the new doc-comment's recommendation ("rely on the path's extension") will be reflected in real code.
- Task 04 keeps its helper inside `locator.rs` precisely to avoid overlap with this task. Task 07 owns `types.rs` outright in Wave A; no sequencing constraint is needed.

---

## Completion Summary

**Status:** Done
**Branch:** fix/detect-windows-bat

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/flutter_sdk/types.rs` | Updated doc-comment on `FlutterExecutable` enum (top-level), `Direct` variant, `WindowsBatch` variant, and `command()` method to remove "metadata marker" language and accurately describe operational identity |

### Notable Decisions/Tradeoffs

1. **Doc-only change**: No code logic was altered, only `///` doc-comment text was updated as specified by the task.
2. **Retained pre-existing warnings**: `cargo doc` emits 18 pre-existing warnings in other files (`native_logs/formats.rs`, `process.rs`, etc.); none originate from `types.rs`. These were not introduced by this task.

### Testing Performed

- `cargo doc -p fdemon-daemon --no-deps` - Passed (18 pre-existing warnings from other files, zero new warnings from `types.rs`)
- `cargo check -p fdemon-daemon` - Passed

### Risks/Limitations

1. **None**: This is a pure documentation change. The enum variant and implementation are unchanged.
