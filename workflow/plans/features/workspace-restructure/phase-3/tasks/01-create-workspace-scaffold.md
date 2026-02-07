## Task: Create Workspace Scaffold

**Objective**: Set up the Cargo workspace root `Cargo.toml` and create empty crate directories with their `Cargo.toml` files. This establishes the physical structure that subsequent tasks will populate with source files.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

- `Cargo.toml`: Convert from single-crate to workspace root
- `crates/fdemon-core/Cargo.toml`: **NEW**
- `crates/fdemon-daemon/Cargo.toml`: **NEW**
- `crates/fdemon-app/Cargo.toml`: **NEW**
- `crates/fdemon-tui/Cargo.toml`: **NEW**
- `crates/fdemon-core/src/lib.rs`: **NEW** (empty placeholder)
- `crates/fdemon-daemon/src/lib.rs`: **NEW** (empty placeholder)
- `crates/fdemon-app/src/lib.rs`: **NEW** (empty placeholder)
- `crates/fdemon-tui/src/lib.rs`: **NEW** (empty placeholder)

### Details

#### 1. Create Directory Structure

```
crates/
  fdemon-core/
    Cargo.toml
    src/
      lib.rs
  fdemon-daemon/
    Cargo.toml
    src/
      lib.rs
  fdemon-app/
    Cargo.toml
    src/
      lib.rs
  fdemon-tui/
    Cargo.toml
    src/
      lib.rs
```

#### 2. Transform Root `Cargo.toml`

The root `Cargo.toml` becomes a workspace definition. The binary stays at root level (`src/main.rs`).

```toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "BSL-1.1"

[workspace.dependencies]
# Internal crate dependencies
fdemon-core = { path = "crates/fdemon-core" }
fdemon-daemon = { path = "crates/fdemon-daemon" }
fdemon-app = { path = "crates/fdemon-app" }
fdemon-tui = { path = "crates/fdemon-tui" }

# Shared external dependencies (deduplicate versions)
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "chrono"] }
tracing-appender = "0.2"
thiserror = "2"
regex = "1"
color-eyre = "0.6"

# TUI-specific
ratatui = { version = "0.30", features = ["all-widgets"] }
crossterm = "0.29"

# Config
toml = "0.8"
dirs = "5"
fs2 = "0.4"

# Async
trait-variant = "0.1"
notify = "7"
notify-debouncer-full = "0.4"

# CLI
clap = { version = "4", features = ["derive"] }

# Misc
rand = "0.8"

# Dev
tempfile = "3"
tokio-test = "0.4"
mockall = "0.13"
serial_test = "3"
expectrl = "0.7"
insta = { version = "1.41", features = ["filters"] }

# The binary crate
[[bin]]
name = "fdemon"
path = "src/main.rs"

[dependencies]
fdemon-core.workspace = true
fdemon-daemon.workspace = true
fdemon-app.workspace = true
fdemon-tui.workspace = true
clap.workspace = true
tokio.workspace = true
color-eyre.workspace = true
tracing.workspace = true

[dev-dependencies]
tempfile.workspace = true
tokio-test.workspace = true
```

#### 3. Crate-Specific `Cargo.toml` Files

**`crates/fdemon-core/Cargo.toml`:**
```toml
[package]
name = "fdemon-core"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Core domain types for Flutter Demon"

[dependencies]
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
thiserror.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
tracing-appender.workspace = true
regex.workspace = true
dirs.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

**`crates/fdemon-daemon/Cargo.toml`:**
```toml
[package]
name = "fdemon-daemon"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Flutter process management for Flutter Demon"

[dependencies]
fdemon-core.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
tracing.workspace = true
regex.workspace = true

[dev-dependencies]
tempfile.workspace = true
tokio-test.workspace = true
```

**`crates/fdemon-app/Cargo.toml`:**
```toml
[package]
name = "fdemon-app"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Application state and orchestration for Flutter Demon"

[dependencies]
fdemon-core.workspace = true
fdemon-daemon.workspace = true
tokio.workspace = true
crossterm.workspace = true
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
tracing.workspace = true
thiserror.workspace = true
regex.workspace = true
rand.workspace = true
color-eyre.workspace = true
toml.workspace = true
dirs.workspace = true
fs2.workspace = true
notify.workspace = true
notify-debouncer-full.workspace = true
trait-variant.workspace = true

[dev-dependencies]
tempfile.workspace = true
tokio-test.workspace = true
mockall.workspace = true
```

**`crates/fdemon-tui/Cargo.toml`:**
```toml
[package]
name = "fdemon-tui"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Terminal UI for Flutter Demon"

[dependencies]
fdemon-core.workspace = true
fdemon-app.workspace = true
ratatui.workspace = true
crossterm.workspace = true
tokio.workspace = true
tracing.workspace = true
chrono.workspace = true

[dev-dependencies]
tempfile.workspace = true
insta.workspace = true
fdemon-daemon.workspace = true
```

#### 4. Placeholder `lib.rs` Files

Each `crates/<name>/src/lib.rs` starts as a minimal placeholder:

```rust
//! <Crate description>
```

These will be populated by subsequent tasks (03-06).

### Acceptance Criteria

1. `crates/` directory exists with 4 subdirectories, each containing `Cargo.toml` and `src/lib.rs`
2. Root `Cargo.toml` is a valid workspace definition with `[workspace]` section
3. Root `Cargo.toml` still defines the `fdemon` binary pointing to `src/main.rs`
4. All workspace dependencies are declared in `[workspace.dependencies]`
5. Each crate's `Cargo.toml` uses `version.workspace = true` and `dep.workspace = true`
6. `cargo check` passes (binary still builds from existing `src/` code; crate lib.rs files are empty)
7. No functional changes to the application

### Testing

```bash
# Verify workspace structure is valid
cargo check

# Verify existing tests still pass (source hasn't moved yet)
cargo test

# Verify each crate's Cargo.toml is valid
cargo check -p fdemon-core
cargo check -p fdemon-daemon
cargo check -p fdemon-app
cargo check -p fdemon-tui
```

### Notes

- The existing `src/lib.rs` and `src/main.rs` remain unchanged in this task. Source files move in tasks 03-07.
- Keep the existing `[lib]` section in root `Cargo.toml` temporarily so existing code compiles. It will be removed in task 07.
- The root `Cargo.toml` needs both the workspace definition AND the binary/lib sections during the transition.
- `fdemon-tui` has `fdemon-daemon` as a **dev-dependency** only (for test utilities like `Device` construction in tests).
- `crossterm` is a dependency of `fdemon-app` because `Message::Key(KeyEvent)` uses `crossterm::event::KeyEvent`.
