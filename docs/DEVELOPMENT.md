# Development Guide

This document provides the development workflow, build commands, and tooling information for Flutter Demon.

## Build System

**Language:** Rust
**Build Tool:** Cargo (workspace with 4 library crates + 1 binary)
**Minimum Rust Version:** 1.70+

### Workspace Structure

Flutter Demon is organized as a Cargo workspace:
- **fdemon-core** — Domain types (zero internal deps)
- **fdemon-daemon** — Flutter process management
- **fdemon-app** — Application state and orchestration
- **fdemon-tui** — Terminal UI
- **flutter-demon** — Binary crate (main.rs + headless mode)

### Build Commands

| Command | Purpose |
|---------|---------|
| `cargo build` | Build all crates and binary |
| `cargo build --release` | Build optimized release binary |
| `cargo build -p fdemon-core` | Build specific crate only |
| `cargo run` | Run the binary application |
| `cargo run -- <args>` | Run binary with arguments |

### Verification Commands

Run these commands before committing changes:

```bash
cargo fmt --all              # Format all crates
cargo check --workspace      # Check all crates compile
cargo test --workspace       # Test all crates
cargo clippy --workspace     # Lint all crates
```

**Full verification (quality gate):**

```bash
cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings
```

### Test Commands

| Command | Purpose |
|---------|---------|
| `cargo test --workspace` | Run all tests in all crates |
| `cargo test --lib` | Run unit tests only (all crates) |
| `cargo test -p fdemon-core` | Test specific crate only |
| `cargo test <pattern>` | Run tests matching pattern |
| `cargo test -- --nocapture` | Show println! output |
| `cargo nextest run --test e2e` | Run E2E tests with retry (requires nextest) |
| `./scripts/test-e2e.sh` | Run E2E tests with retry if nextest available |

**Install cargo-nextest for enhanced testing with automatic retry:**
```bash
cargo install cargo-nextest --locked
```

### Per-Crate Commands

Test crate isolation:

```bash
# Check each crate builds independently
cargo check -p fdemon-core
cargo check -p fdemon-daemon
cargo check -p fdemon-app
cargo check -p fdemon-tui

# Test each crate independently
cargo test -p fdemon-core
cargo test -p fdemon-daemon
cargo test -p fdemon-app
cargo test -p fdemon-tui
```

## File Extensions

| Extension | Type |
|-----------|------|
| `.rs` | Rust source files |
| `.toml` | Configuration (Cargo.toml, config files) |
| `.md` | Documentation |

## Workflow Locations

### Planning & Implementation

```
workflow/
├── plans/
│   ├── features/          # Feature plans with phases and tasks
│   │   └── <feature-name>/
│   │       ├── PLAN.md
│   │       └── <phase>/
│   │           ├── TASKS.md
│   │           └── tasks/
│   │               └── ##-task-slug.md
│   └── bugs/              # Bug reports and fix tasks
│       └── <bug-name>/
│           ├── BUG.md
│           └── tasks/
│               └── fix-*.md
└── reviews/
    ├── features/          # Feature implementation reviews
    └── bugs/              # Bug fix reviews
```

### Task File Structure

Each task file should include a **Completion Summary** after implementation:

```markdown
---

## Completion Summary

**Status:** Done / Blocked / Failed

### Files Modified

| File | Changes |
|------|---------|
| `src/path/file.rs` | <what changed> |

### Notable Decisions/Tradeoffs

1. **<Decision>**: <Rationale and implications>

### Testing Performed

- `cargo check` - Passed/Failed
- `cargo test` - Passed/Failed (X tests)
- `cargo clippy` - Passed/Failed

### Risks/Limitations

1. **<Risk>**: <Description and mitigation if any>
```

## Quality Gates

Before a task is considered complete:

- [ ] `cargo fmt` — Code is formatted
- [ ] `cargo check` — No compilation errors
- [ ] `cargo test` — All tests pass
- [ ] `cargo clippy -- -D warnings` — No clippy warnings

## Dependencies

### Runtime Dependencies

See `Cargo.toml` for full dependency list.

**Key Crates:**
- `ratatui` — Terminal UI framework
- `crossterm` — Cross-platform terminal manipulation
- `tokio` — Async runtime
- `serde` / `serde_json` — Serialization
- `tracing` — Logging and diagnostics
- `notify` — File system watching

### Development Dependencies

- `tempfile` — Temporary directories for tests

## Editor Setup

### VS Code

Recommended extensions:
- `rust-analyzer` — Rust language support
- `Even Better TOML` — TOML file support
- `Error Lens` — Inline error display

### IntelliJ / CLion

- Install the Rust plugin
- Enable Cargo check on save

## Logging

Flutter Demon uses file-based logging via `tracing` (stdout is owned by the TUI).

Log files are written to the system temp directory.

**Log Macros:**
```rust
use tracing::{info, warn, error, debug, trace};

info!("Application started");
warn!("Potential issue: {}", message);
error!("Failed to connect: {}", err);
debug!("Processing item: {:?}", item);
```

## Running the Application

### From Flutter Project Directory

```bash
cd /path/to/flutter/app
fdemon
```

### With Explicit Path

```bash
fdemon /path/to/flutter/app
```

### Development Mode

```bash
cargo run -- /path/to/flutter/app
```

## Common Issues

### Build Fails

1. Ensure Rust 1.70+ is installed: `rustup update`
2. Clear build cache: `cargo clean && cargo build`

### Tests Fail

1. Run single failing test: `cargo test <test_name> -- --nocapture`
2. Check for file system test isolation issues

### Clippy Warnings

Fix all warnings before committing:
```bash
cargo clippy --fix --allow-dirty
```
