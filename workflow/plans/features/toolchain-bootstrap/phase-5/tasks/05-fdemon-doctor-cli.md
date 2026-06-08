## Task: `fdemon doctor` — read-only CLI diagnostics subcommand

**Objective**: Add a `fdemon doctor` subcommand that runs `run_preflight` from any
directory, prints a structured component report plus the captured `flutter doctor`
lines, and exits 0 when all components are `Ok`, 1 otherwise — without breaking the
existing positional-path CLI surface.

**Depends on**: None

**Estimated Time**: 3 hours

### Scope

**Files Modified (Write):**
- `src/main.rs`: convert the flat `Args` into a clap `Commands` enum with a
  default `Run` (existing flags/positional) and a `Doctor` variant; dispatch
  `Doctor` before any `Engine`/TUI init.
- `src/doctor.rs`: **NEW** — `async fn run_doctor(cwd, explicit_sdk_path) -> ExitCode`
  that calls `run_preflight`, formats the report, and computes the exit code.
- `crates/fdemon-daemon/src/toolchain/types.rs`: add a `Display` impl for
  `ComponentStatus` (`Ok`→`OK`, `Partial`→`!`, `Missing`→`MISS`, `Error`→`ERR`,
  `Unknown`→`?`); add the audit-folded `resolve_stable` empty-manifest test.
- `Cargo.toml` (root / binary deps): add `fdemon-daemon` as a **direct** binary dep
  (already transitive via `fdemon-app`, so zero compile-cost) for `run_preflight`.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/mod.rs`: `run_preflight` signature (`:78`).
- `src/headless/runner.rs`: the existing pre-Engine dispatch pattern to mirror.

### Details

**Preserve the existing CLI shape.** `Args` is a flat struct today (positional
`PATH` + `--headless` + `--dap-*`). Use clap 4's default-subcommand idiom so
`fdemon /path` and the DAP flags keep working:

```rust
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    #[command(flatten)]
    run: RunArgs,            // existing positional PATH + flags, used when no subcommand
}

#[derive(Subcommand)]
enum Commands {
    /// Diagnose the Flutter toolchain and exit (no TUI).
    Doctor {
        /// Project dir to probe (defaults to cwd).
        path: Option<PathBuf>,
    },
}
```

- In `main`, if `command == Some(Commands::Doctor { path })`, dispatch
  `doctor::run_doctor(path.unwrap_or(cwd), explicit_sdk)` **before** `Engine::new()`
  and the TUI, then return its `ExitCode`.

**Report formatting.**

```rust
// src/doctor.rs
pub async fn run_doctor(cwd: PathBuf, explicit_sdk: Option<PathBuf>) -> std::process::ExitCode {
    eprintln!("Running toolchain checks…");                 // run_preflight can take up to ~60s
    let report = fdemon_daemon::toolchain::run_preflight(&cwd, explicit_sdk.as_deref()).await;
    let mut all_ok = true;
    for c in &report.components {
        if c.status != ComponentStatus::Ok { all_ok = false; }
        println!("[{:>4}] {} — {}", c.status, c.kind, c.detail);  // uses the new Display
    }
    if let Some(lines) = &report.doctor {
        println!("\nflutter doctor:");
        for l in lines { println!("  {l}"); }                // DoctorLine already renders markers
    }
    if all_ok { ExitCode::SUCCESS } else { ExitCode::from(1) }
}
```

- `run_preflight` "never fails," so there is no error path to handle — just format.
- Print the "Running toolchain checks…" line to **stderr** first; `flutter doctor`
  can take up to the configured timeout (~60s).

**Folded test gap (from audit):** add a `resolve_stable` empty-manifest test in
`types.rs` (zero releases → `None`).

### Acceptance Criteria

1. `fdemon doctor` runs from any directory, prints each component with the new
   `ComponentStatus` `Display` label and detail, and appends `flutter doctor` lines
   when present.
2. Exit code is 0 iff every component is `Ok`, else 1.
3. The pre-existing surface is unchanged: `fdemon`, `fdemon /path`, `--headless`,
   `--dap-port/--dap-stdio/--dap-config` all behave exactly as before.
4. `fdemon-daemon` is a direct binary dep; `cargo build` clean.
5. `ComponentStatus: Display` and `resolve_stable` empty-manifest are unit-tested.

### Testing

```rust
#[test]
fn component_status_display_labels() {
    assert_eq!(ComponentStatus::Ok.to_string(), "OK");
    assert_eq!(ComponentStatus::Missing.to_string(), "MISS");
    // ... Partial/Error/Unknown
}
#[test]
fn resolve_stable_empty_manifest_is_none() {
    let m = FlutterReleaseManifest { /* zero releases */ };
    assert!(m.resolve_stable("x64").is_none());
}
```

- Optional (nice-to-have, may exceed budget): a binary integration test under
  `tests/` invoking `fdemon doctor` as a subprocess and asserting the stdout shape /
  exit code. Mark `#[ignore]` if it depends on a real toolchain.

### Notes

- **`fdemon setup` is deferred** (resolved scope): a headless, TUI-decoupled install
  runner is materially larger and adds little over the interactive wizard. Note it as
  a Future Enhancement only.
- Independent of the wizard tasks (disjoint files) — Wave 1, parallelizable with 01
  and 07.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `src/main.rs` | Converted flat `Args` struct to `Cli` (with `#[command(subcommand)] command: Option<Commands>` + `#[command(flatten)] run: RunArgs`); added `Commands::Doctor { path }` variant; dispatched doctor before Engine/TUI init using `doctor::run_doctor`; added `mod doctor` declaration |
| `src/doctor.rs` | **NEW** — `async fn run_doctor(cwd, explicit_sdk) -> ExitCode` calls `run_preflight`, formats component report with `[{status:>4}] {kind} — {detail}`, appends `flutter doctor` lines when present, exits 0 if all `Ok` else 1 |
| `crates/fdemon-daemon/src/toolchain/types.rs` | Added `Display` impl for `ComponentStatus` (`Ok`→`"OK"`, `Partial`→`"!"`, `Missing`→`"MISS"`, `Error`→`"ERR"`, `Unknown`→`"?"`); added `component_status_display_labels` test; added `resolve_stable_empty_manifest_is_none` test (all 3 arch variants) |

### Notable Decisions/Tradeoffs

1. **`ExitCode` conversion**: Rust stable has no `u8::from(ExitCode)` — compared against `ExitCode::SUCCESS` and called `std::process::exit(0/1)` directly. This is the correct approach for `Result<()>` main.
2. **`fdemon-daemon` already a direct dep**: The task specified adding it as a direct dep but it was already present in `[dependencies]` — no `Cargo.toml` change needed.
3. **`resolve_stable` uses `HostArch`**: The task's sketch test showed `resolve_stable("x64")` (string), but the existing implementation takes `HostArch`. The test was implemented using `HostArch::X64/Arm64/Unknown` which covers the same correctness intent.
4. **Existing `test_resolve_stable_returns_none_for_empty_manifest`**: Already covered the empty-manifest case; added the task-named `resolve_stable_empty_manifest_is_none` as a separate shorter test to satisfy the AC explicitly.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all test suites green)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed
- `cargo test -p fdemon-daemon component_status_display_labels` - Passed (1 test)
- `cargo test -p fdemon-daemon resolve_stable_empty_manifest` - Passed (1 test)

### Risks/Limitations

1. **`fdemon doctor` with explicit SDK path**: The `run_doctor` function accepts `explicit_sdk: Option<PathBuf>` but the CLI currently always passes `None`. A future enhancement could add a `--sdk-path` flag to `fdemon doctor`. This is not in scope per the task spec.
2. **Clap subcommand conflict handling**: The `#[command(flatten)]` approach on `RunArgs` means both `path` (positional) and `RunArgs.path` can potentially conflict when the `doctor` subcommand is used. Clap handles this correctly — the `doctor` variant's `path` field takes the positional argument and `RunArgs` is ignored for that subcommand.
