## Task: Checks Correctness, ANSI Hygiene & Test Isolation (fdemon-daemon)

**Objective**: Fix the JDK version classification so an unparseable major version is not reported as
`Ok`, strip ANSI from process-derived `detail` strings, isolate the env-mutating test so the
default parallel `cargo test` is deterministic, and remove a duplicated test. Addresses review
findings **m5**, **n12**, **m8**, **m10**.

**Depends on**: 01-doctor-process-memory-hardening (consumes the shared `strip_ansi` helper)

**Agent:** implementor

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/checks.rs` — JDK classification fix, ANSI strip on `detail`,
  serialize env tests.
- `crates/fdemon-daemon/src/toolchain/mod.rs` — remove the duplicate
  `test_host_platform_detect_matches_cfg`.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs` — the shared `strip_ansi` (extended in
  task 01).
- `crates/fdemon-daemon/src/toolchain/types.rs` — `ComponentCheck`, `ComponentStatus`.

### Details

**m5 — JDK unparseable-major must not be `Ok`** (`checks.rs:200-226`, `parse_jdk_output`):

Today, when `extract_quoted_version` returns `Some(v)` but `parse_java_major_version(&v)` returns
`None` (e.g. a bare `"1"`), the `None` arm returns `ComponentStatus::Ok`. Change that arm to
`Partial` (preferred) — an unknown major version is not a confirmed-good JDK:

```rust
None => ComponentCheck {
    kind: ComponentKind::Jdk,
    status: ComponentStatus::Partial,
    detail: format!("Java {v} (could not determine major version)"),
},
```

Keep the existing `>= 17 → Ok`, `< 17 → Partial`, and the outer `None` (no quoted version) `Error`
arms unchanged.

**n12 — strip ANSI from `detail`** sourced from process stderr/stdout:

In `check_git`, `check_jdk`, and any other probe that stores raw stderr/stdout into
`ComponentCheck::detail` (e.g. the catch-all in `check_android_platform_tools`), pass the string
through `flutter_sdk::diagnostics::strip_ansi` before storing, and truncate to a sane column bound:

```rust
/// Cap stored probe detail so a misbehaving tool's first line cannot bloat the report.
const MAX_DETAIL_LEN: usize = 256;
```

Apply strip-then-truncate at the point each `detail` is built from external output. Do not strip
the static, code-authored detail strings (e.g. `"java not found on PATH"`).

**m8 — serialize env-mutating tests** (`checks.rs`, around `test_android_sdk_root_*`):

`test_android_sdk_root_from_env_android_home` (and any sibling that mutates `ANDROID_HOME` /
`ANDROID_SDK_ROOT`) uses process-global `std::env::set_var`/`remove_var` and races other tests under
the default parallel runner. Add `#[serial_test::serial]` to every test in the module that touches
those vars (confirm `serial_test` is a `fdemon-daemon` dev-dependency; if absent, add it under
`[dev-dependencies]` — it is already used elsewhere in the workspace). Wrap `set_var`/`remove_var`
in `unsafe { }` as required by the Rust 1.77+ MSRV. Prefer restoring prior values in a guard so the
tests are hermetic.

**m10 — remove duplicate test** (`toolchain/mod.rs`):

`test_host_platform_detect_matches_cfg` exists identically in both `toolchain/types.rs` and
`toolchain/mod.rs`. Remove the copy in `mod.rs`; keep it in `types.rs` (home of `HostPlatform::detect`).

### Acceptance Criteria

1. `parse_jdk_output` on a version string with no parseable major (e.g. `"1"`) returns
   `ComponentStatus::Partial` (or `Error`), never `Ok`. Existing `>=17`/`<17` paths unchanged.
2. `detail` strings built from external process output are ANSI-stripped and length-bounded;
   code-authored detail strings are untouched.
3. All env-mutating tests carry `#[serial_test::serial]`; `cargo test --workspace` (default
   parallel) is deterministic across repeated runs.
4. `test_host_platform_detect_matches_cfg` exists in exactly one module (`types.rs`).
5. `checks.rs` quality gate (fmt/check/test/clippy `-D warnings`) is green.

### Testing

```rust
#[test] fn test_parse_jdk_unparseable_major_is_not_ok() { /* "1" -> Partial */ }
#[test] fn test_parse_jdk_modern_17_is_ok() { /* regression guard */ }
#[test] fn test_parse_jdk_legacy_8_is_partial() { /* "1.8.0_291" -> Partial */ }
#[test] fn test_detail_strips_ansi_and_truncates() { /* embed \x1b[..m + long line */ }
```

### Notes

- This task **must** run after task 01 — it calls the shared `strip_ansi` that task 01 consolidates.
- Do not change the Android probe logic here beyond ANSI/truncation hygiene — the file split is
  task 03.
- `std::env::set_var` is `unsafe` on the workspace MSRV (1.77.2); the `unsafe` block is expected and
  not a clippy violation when annotated.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a2097366872094786

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/checks.rs` | JDK Partial fix (m5), ANSI strip+truncate on process-derived details (n12), serialized env tests with EnvGuard (m8), new acceptance-criteria tests |
| `crates/fdemon-daemon/src/toolchain/mod.rs` | Removed duplicate `test_host_platform_detect_matches_cfg` (m10) |

### Notable Decisions/Tradeoffs

1. **EnvGuard RAII pattern**: Instead of bare `set_var`/`remove_var` in test bodies, introduced a local `EnvGuard` struct that saves the prior value on construction and restores it on drop. This makes each env-mutating test hermetic even under `panic!`, and the `unsafe` blocks are co-located with the safety rationale comment.

2. **`strip_and_truncate` helper**: Rather than inlining `strip_ansi` + truncation at every call-site, a private `strip_and_truncate` combines both operations. It is applied only to process-derived strings (stdout/stderr). Code-authored static strings like `"java not found on PATH"` are intentionally not run through it.

3. **Truncation at char boundary**: Used `char_indices().nth(MAX_DETAIL_LEN)` to truncate at a character boundary, avoiding the potential panic from slicing a multi-byte character.

4. **m5 — `None` arm returns `Partial`**: A bare `"1"` string (legacy format, first component is 1 but no second component) now returns `Partial` with a descriptive message rather than `Ok`. The message wording follows the task spec exactly: `"Java {v} (could not determine major version)"`.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test --workspace` — Passed (all 6,486 tests across all crates)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **Truncation is char-count not byte-count**: `MAX_DETAIL_LEN = 256` means 256 Unicode codepoints. For purely ASCII tool output (git, java) this is equivalent to 256 bytes. If a tool produces multi-byte UTF-8, the stored string may be up to 4×256 bytes in the worst case. This is acceptable for the intended use (diagnostic display).
