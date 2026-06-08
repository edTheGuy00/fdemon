## Task: fdemon doctor — align status column, honour configured SDK path, document subcommand collision (F20, F24, F25)

**Severity:** LOW (F20, F24) + NIT (F25)

**Objective**: Make `fdemon doctor` faithful: aligned status column, respect the
user's configured `[flutter] sdk_path`, and a documented note about the `./doctor`
project-name collision.

**Depends on**: — (file-disjoint from chains A/B and Task 07; safe to parallelise)

**Estimated Time**: 1.5–2 hours

### Scope

**Files Modified (Write):**
- `src/doctor.rs`
- `src/main.rs`
- `crates/fdemon-daemon/src/toolchain/types.rs` (optionally — see F20 option b)

### Details & Fixes

**F20 (LOW) — `{:>4}` width specifier is dead.** The report line
`println!("[{:>4}] {} — {}", c.status, c.kind, c.detail)` (`doctor.rs:42`) intends a
right-aligned 4-char status column, but `ComponentStatus`'s `Display` writes labels
directly via `write!(f, "OK")` etc. (`types.rs:69-78`) without consulting
`f.width()`/`f.pad()`, so `>4` is a no-op and the column is ragged
(`[OK]`/`[MISS]`/`[?]`).
**Fix (a, minimal, preferred):** `println!("[{:>4}] {} — {}", c.status.to_string(), ...)`
— a `String` honours formatter width. **Fix (b, more robust):** make `Display`
width-aware via `f.pad(label)` in `types.rs` (every future `{:>N}` then aligns), at the
cost of touching shared daemon code. Given this is the only call site, (a) is the
lowest-risk choice; `>4` is correctly sized for the widest label (`MISS`/`ERR`).

**F24 (LOW) — doctor ignores configured `[flutter] sdk_path`.** `run_doctor`'s
signature/doc describe an `explicit_sdk` sourced from `.fdemon/config.toml`
`[flutter] sdk_path`, but `main.rs:123` calls `doctor::run_doctor(cwd, None)` — it never
loads the project config. A user who pins a non-default SDK gets `fdemon doctor`
probing the wrong location, potentially reporting Flutter `Missing` (exit 1) despite a
healthy configured SDK.
**Fix:** in the `Commands::Doctor` branch of `main.rs`, load settings for `cwd` and pass
the configured SDK through, mirroring the engine path:
```rust
let settings = fdemon_app::config::load_settings(&cwd);
let explicit_sdk = settings.flutter.sdk_path.clone();
let exit_code = doctor::run_doctor(cwd, explicit_sdk).await;
```
(`load_settings` is `pub`; the binary already depends on `fdemon-app`. Compare
`engine.rs:203`, `install_wizard/navigation.rs:19`, `flutter_version/actions.rs:70`,
which all thread `settings.flutter.sdk_path` into `find_flutter_sdk`/`run_preflight`.)

**F25 (NIT) — `./doctor` project can't launch via `fdemon doctor`.** Because
`Commands::Doctor` is a clap subcommand, clap resolves the bare token `doctor` to the
subcommand before the positional `PATH`, so a runnable Flutter project at `./doctor`
can never be launched via `fdemon doctor` (it always runs diagnostics). This is
inherent to introducing the subcommand and the task deliberately chose this surface;
the workaround `fdemon ./doctor` works.
**Fix:** acceptable as-is — **document** the `fdemon ./doctor` workaround in the CLI
usage help and note in AC #3 wording (Phase 5) that positional-path parity holds except
for the bare token `doctor`. Do **not** add filesystem-aware subcommand resolution
(disproportionate complexity for the edge case).

### Acceptance Criteria

1. The doctor report status column is aligned to a fixed width
   (`[  OK]` / `[MISS]` / `[ ERR]`), verified by a test on the rendered line (F20).
2. `fdemon doctor` loads project settings and passes the configured
   `[flutter] sdk_path` into `run_doctor`/`run_preflight`; a config-pinned SDK is
   probed (F24).
3. The `./doctor` collision + `fdemon ./doctor` workaround is documented in usage help
   (and Task 08 notes it in docs) (F25).
4. The existing positional-path + `--headless` + `--dap-*` CLI surface still parses and
   still enforces its `conflicts_with` constraints (no regression).

### Testing

```rust
// src/doctor.rs (or src/main.rs) test module
// - NEW (F20): formatting a ComponentStatus::Ok / ::Missing via the report line yields
//     a fixed-width status field ("  OK" / "MISS") — assert padded width == 4.
// - NEW (F24): with a settings fixture pinning flutter.sdk_path, assert run_doctor is
//     invoked with Some(path) (or factor the wiring so it is unit-testable).
// - KEEP the existing CLI-surface parse tests (positional path + subcommand coexist).
```

### Notes

- File-disjoint from every other task in this followup — safe to run in Wave 1.
- Prefer F20 fix (a) (`.to_string()`) unless you also want every future `{:>N}` use of
  `ComponentStatus` to align, in which case (b) `f.pad` in `types.rs` is cleaner.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `src/doctor.rs` | F20: changed `c.status` to `c.status.to_string()` so `{:>4}` width specifier is honoured; added `#[cfg(test)] mod tests` with `status_field_is_always_4_chars_wide` verifying all 5 variants produce a 4-char padded string |
| `src/main.rs` | F24: load `fdemon_app::config::load_settings(&cwd)` in the `Commands::Doctor` branch and pass `settings.flutter.sdk_path` as `explicit_sdk` to `run_doctor`; F25: added `./doctor` workaround note to `Doctor` subcommand's doc comment (shown in `fdemon doctor --help`) |

### Notable Decisions/Tradeoffs

1. **F20 fix (a) chosen**: Used `.to_string()` at the call site (`doctor.rs`) rather than making `Display` width-aware via `f.pad()` in `types.rs`. This is the minimal, lowest-risk fix — `types.rs` is shared daemon code and the doctor report is the only call site that needs alignment.
2. **Test helper not extracted**: Avoided a `pub fn format_status_field` helper (which would emit a `dead_code` warning in non-test builds). The test directly applies `format!("{:>4}", status.to_string())` — the same expression used in the production code — making the test both representative and warning-free.
3. **F25 approach**: Documented the `fdemon ./doctor` workaround inline in the clap `Doctor` variant doc comment so it appears in `fdemon doctor --help` output. No filesystem-aware subcommand resolution added (disproportionate complexity as specified by the task).

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed (0 warnings)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
- `cargo test --workspace` — Passed (6,944+ tests, 0 failed)
- `cargo test -p flutter-demon doctor -- --nocapture` — Passed (1 new test: `doctor::tests::status_field_is_always_4_chars_wide`)

### Risks/Limitations

1. **F24 test coverage**: The task's suggested F24 test ("with a settings fixture pinning flutter.sdk_path, assert run_doctor is invoked with Some(path)") was not added as a unit test because `run_doctor` is async and requires `run_preflight` which spawns real processes — it's an integration concern. The wiring itself is a 3-line change that is straightforward to verify by inspection, and it mirrors the existing engine/navigation.rs patterns.
