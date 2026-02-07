## Task: Verify and Document

**Objective**: Final verification pass. Run the full quality gate (`fmt`, `check`, `test`, `clippy`), verify binary behavior is identical, and update `docs/ARCHITECTURE.md` to reflect the workspace structure.

**Depends on**: 09-cleanup-re-exports-and-paths

**Estimated Time**: 2-3 hours

### Scope

- Full quality gate verification
- `docs/ARCHITECTURE.md`: Update to reflect workspace crate structure
- `docs/DEVELOPMENT.md`: Update build/test commands for workspace
- `CLAUDE.md`: Update if needed
- `Cargo.toml` (root): Final audit

### Details

#### 1. Run Full Quality Gate

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

All four must pass cleanly.

#### 2. Verify Binary Behavior

```bash
# Build the binary
cargo build

# Verify it runs
./target/debug/fdemon --help

# If a Flutter project is available:
./target/debug/fdemon /path/to/flutter/project  # TUI mode
./target/debug/fdemon --headless /path/to/flutter/project  # Headless mode
```

#### 3. Verify Crate Isolation

Test each crate builds and tests independently:

```bash
cargo check -p fdemon-core
cargo check -p fdemon-daemon
cargo check -p fdemon-app
cargo check -p fdemon-tui

cargo test -p fdemon-core
cargo test -p fdemon-daemon
cargo test -p fdemon-app
cargo test -p fdemon-tui
```

#### 4. Verify Dependency Graph

Confirm the clean dependency invariants:

```bash
# fdemon-core should have no internal deps
cargo tree -p fdemon-core --depth 1 | grep -c "fdemon-"
# Expected: 0 (only the crate itself)

# fdemon-daemon should depend only on fdemon-core
cargo tree -p fdemon-daemon --depth 1 | grep "fdemon-"
# Expected: fdemon-core only

# fdemon-app should depend on fdemon-core + fdemon-daemon
cargo tree -p fdemon-app --depth 1 | grep "fdemon-"
# Expected: fdemon-core, fdemon-daemon

# fdemon-tui should depend on fdemon-core + fdemon-app (not fdemon-daemon in regular deps)
cargo tree -p fdemon-tui --depth 1 | grep "fdemon-"
# Expected: fdemon-core, fdemon-app (fdemon-daemon only in dev-deps)
```

#### 5. Update `docs/ARCHITECTURE.md`

Update the architecture document to reflect the workspace structure:

**Project Structure section**: Replace the single-crate file tree with the workspace layout:
```
flutter-demon/
  Cargo.toml                    (workspace root + binary)
  crates/
    fdemon-core/                (domain types, error handling)
    fdemon-daemon/              (Flutter process management)
    fdemon-app/                 (TEA state, Engine, services)
    fdemon-tui/                 (terminal UI)
  src/
    main.rs                     (binary entry point)
    headless/                   (headless runner)
  tests/                        (integration tests)
```

**Dependency graph**: Update to show crate-level dependencies.

**Module Reference**: Update to show which files are in which crate.

**Build Commands**: May need updates for workspace commands.

#### 6. Update `docs/DEVELOPMENT.md`

Update build/test commands:
```bash
cargo build                   # Build everything
cargo test --workspace        # Test all crates
cargo test -p fdemon-core     # Test specific crate
cargo clippy --workspace      # Lint all crates
cargo fmt --all               # Format all crates
```

#### 7. Update `CLAUDE.md`

Update the architecture diagram and build commands section to reflect the workspace structure. Update the Key Modules section to mention crate boundaries.

#### 8. Final Root `Cargo.toml` Audit

Verify:
- `[workspace]` section lists `members = ["crates/*"]`
- `[workspace.dependencies]` contains all shared deps
- `[[bin]]` section correctly points to `src/main.rs`
- `[dependencies]` section has the binary's direct dependencies
- No `[lib]` section (removed in task 07)
- `[dev-dependencies]` has workspace integration test deps

### Acceptance Criteria

1. `cargo fmt --all` makes no changes (code is formatted)
2. `cargo check --workspace` passes
3. `cargo test --workspace` passes with 0 failures
4. `cargo clippy --workspace -- -D warnings` passes with 0 warnings
5. `cargo tree -p fdemon-core --depth 1` shows no internal deps
6. `cargo tree -p fdemon-daemon --depth 1` shows only `fdemon-core`
7. `cargo tree -p fdemon-app --depth 1` shows only `fdemon-core` + `fdemon-daemon`
8. `cargo tree -p fdemon-tui --depth 1` shows only `fdemon-core` + `fdemon-app`
9. Binary behavior is unchanged (TUI and headless modes work)
10. `docs/ARCHITECTURE.md` accurately reflects workspace structure
11. `docs/DEVELOPMENT.md` has updated commands
12. `CLAUDE.md` has updated architecture info

### Testing

```bash
# Full verification (this is the quality gate)
cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings

# Dependency verification
cargo tree -p fdemon-core --depth 1
cargo tree -p fdemon-daemon --depth 1
cargo tree -p fdemon-app --depth 1
cargo tree -p fdemon-tui --depth 1
```

### Notes

- This is the final task of Phase 3. After it completes, the workspace split is done.
- If any test fails, fix it in this task rather than going back to prior tasks.
- The `cargo tree` commands are the definitive verification of the dependency graph.
- Binary behavior testing may be limited if no Flutter SDK is available. At minimum, verify `--help` and project discovery work.
- The documentation updates should be thorough since the architecture has fundamentally changed from single-crate to workspace.
