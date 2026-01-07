# Development Guide

This document provides the development workflow, build commands, and tooling information for Flutter Demon.

## Build System

**Language:** Rust
**Build Tool:** Cargo
**Minimum Rust Version:** 1.70+

### Commands

| Command | Purpose |
|---------|---------|
| `cargo build` | Build the project |
| `cargo build --release` | Build optimized release binary |
| `cargo run` | Run the application |
| `cargo run -- <args>` | Run with arguments |

### Verification Commands

Run these commands before committing changes:

```bash
cargo fmt              # Format code
cargo check            # Fast compilation check
cargo test             # Run all tests
cargo clippy           # Run lints
```

**Full verification:**

```bash
cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings
```

### Test Commands

| Command | Purpose |
|---------|---------|
| `cargo test` | Run all tests |
| `cargo test --lib` | Run unit tests only |
| `cargo test <pattern>` | Run tests matching pattern |
| `cargo test -- --nocapture` | Show println! output |

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
