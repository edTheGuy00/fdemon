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
