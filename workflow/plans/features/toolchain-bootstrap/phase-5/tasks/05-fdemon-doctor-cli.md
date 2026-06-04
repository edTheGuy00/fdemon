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

**Status:** Not Started
**Branch:** feat/toolchain-bootstrap
