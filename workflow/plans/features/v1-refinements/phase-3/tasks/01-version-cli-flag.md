## Task: Add `--version` CLI Flag

**Objective**: Enable `fdemon --version` to print the binary version, using clap's built-in version support. This is required both for end-user visibility and for the install script to detect the currently installed version.

**Depends on**: None

**Estimated Time**: 0.5-1 hour

### Scope

- `src/main.rs`: Add `version` attribute to clap `#[command(...)]`

### Details

#### Current state

The CLI is defined in `src/main.rs:17-29` using clap's derive API:

```rust
#[derive(Parser, Debug)]
#[command(name = "fdemon")]
#[command(about = "A high-performance TUI for Flutter development", long_about = None)]
struct Args {
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,
    #[arg(long)]
    headless: bool,
}
```

Running `fdemon --version` currently produces a clap error because no `version` attribute is set.

#### Change

Add `version` to the `#[command(...)]` attribute on `src/main.rs:19`. Clap's derive API automatically reads the `CARGO_PKG_VERSION` compile-time env var when `version` is specified without a value:

```rust
#[command(name = "fdemon", version)]
```

This makes `fdemon --version` print:
```
fdemon 0.1.0
```

The version comes from `Cargo.toml:7` (`[workspace.package] version = "0.1.0"`) via the binary crate's `version.workspace = true` at `Cargo.toml:63`.

### Acceptance Criteria

1. `fdemon --version` prints `fdemon 0.1.0` (or whatever the current workspace version is)
2. `fdemon -V` also works (clap auto-generates the short flag)
3. `fdemon --help` output includes the version in the header line
4. All existing functionality is unchanged
5. `cargo test --workspace` passes

### Testing

This is a CLI flag change that doesn't affect library code. Verify manually:

```bash
cargo run -- --version
# Expected: fdemon 0.1.0

cargo run -- -V
# Expected: fdemon 0.1.0

cargo run -- --help
# Expected: includes version in output
```

No unit test changes needed — the existing integration tests in `tests/` don't test `--version` and won't be affected.

### Notes

- This is a one-line change but it's a prerequisite for task 05 (install script), which parses `fdemon --version` output to detect the installed version
- The `version` attribute with no value is equivalent to `version = env!("CARGO_PKG_VERSION")` — clap handles this automatically
- No `build.rs` is needed for this — `CARGO_PKG_VERSION` is a built-in Cargo compile-time variable
