## Task: Update Binary and Headless Module

**Objective**: Update `src/main.rs` to import from workspace crates instead of the monolithic library. Move `headless/` into the binary crate since it's a runner like TUI (or keep it in the binary source tree). Remove the old `src/lib.rs` and all compatibility shims.

**Depends on**: 06-extract-fdemon-tui

**Estimated Time**: 2-3 hours

### Scope

- `src/main.rs`: Update to import from `fdemon_core`, `fdemon_app`, `fdemon_tui`
- `src/headless/mod.rs` -> stays at `src/headless/mod.rs` (part of binary crate)
- `src/headless/runner.rs` -> stays at `src/headless/runner.rs` (part of binary crate)
- `src/lib.rs`: **DELETE** (no longer needed - each crate has its own lib.rs)
- `src/common/`, `src/core/`, `src/daemon/`, `src/app/`, `src/config/`, `src/services/`, `src/watcher/`, `src/tui/`: **DELETE** (all re-export shims)

### Details

#### 1. Update `src/main.rs`

Replace all `flutter_demon::*` imports with direct crate imports:

```rust
//! Flutter Demon - A high-performance TUI for Flutter development
//!
//! This is the binary entry point.

mod headless;

use std::path::PathBuf;

use clap::Parser;
use fdemon_core::prelude::*;
use fdemon_core::{
    discover_flutter_projects, get_project_type, is_runnable_flutter_project, ProjectType,
    DEFAULT_MAX_DEPTH,
};
use fdemon_tui::{select_project, SelectionResult};

/// Flutter Demon - A high-performance TUI for Flutter development
#[derive(Parser, Debug)]
#[command(name = "fdemon")]
#[command(about = "A high-performance TUI for Flutter development", long_about = None)]
struct Args {
    /// Path to Flutter project
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    /// Run in headless mode (JSON output, no TUI)
    #[arg(long)]
    headless: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize error handling and logging
    color_eyre::install().map_err(|e| Error::terminal(e.to_string()))?;
    fdemon_core::logging::init()?;

    let args = Args::parse();
    // ... rest of main() with updated imports ...
    // flutter_demon::run_with_project -> fdemon_tui::run_with_project
    // flutter_demon::run_headless -> headless::runner::run_headless
}
```

#### 2. Update `src/headless/` Imports

The headless module stays in the binary crate but imports from workspace crates:

```rust
// headless/runner.rs
use fdemon_core::prelude::*;
use fdemon_app::{Engine, message::Message, state::AppState};
use fdemon_app::actions::handle_action;
use fdemon_daemon::devices;
```

```rust
// headless/mod.rs
use chrono::Utc;
use serde::Serialize;
// ... (external deps only, plus fdemon_core types if needed)
```

#### 3. Remove `src/lib.rs`

The monolithic library crate is no longer needed. Each crate (`fdemon-core`, `fdemon-daemon`, `fdemon-app`, `fdemon-tui`) has its own `lib.rs`. Delete `src/lib.rs`.

#### 4. Remove All Compatibility Shim Directories

Delete the temporary re-export shim files:
- `src/common/` (entire directory)
- `src/core/` (entire directory)
- `src/daemon/` (entire directory)
- `src/app/` (entire directory)
- `src/config/` (entire directory)
- `src/services/` (entire directory)
- `src/watcher/` (entire directory)
- `src/tui/` (entire directory)

After this, `src/` contains only:
```
src/
  main.rs
  headless/
    mod.rs
    runner.rs
```

#### 5. Update Root `Cargo.toml`

Remove the `[lib]` section since there's no more library crate at root:

```toml
# REMOVE these lines:
# [lib]
# name = "flutter_demon"
# path = "src/lib.rs"
```

The root `Cargo.toml` now only has `[[bin]]`, `[dependencies]`, and `[workspace]` sections.

Also add dependencies needed by `headless/`:
```toml
[dependencies]
fdemon-core.workspace = true
fdemon-daemon.workspace = true
fdemon-app.workspace = true
fdemon-tui.workspace = true
clap.workspace = true
tokio.workspace = true
color-eyre.workspace = true
tracing.workspace = true
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
```

#### 6. Alternative: Headless as Separate Crate

If headless grows complex enough, it could become `fdemon-headless` crate. But for now, keeping it in the binary crate is simpler since it's just 2 files and is an alternative runner to TUI.

### Acceptance Criteria

1. `src/main.rs` imports from `fdemon_core`, `fdemon_app`, `fdemon_tui` (not `flutter_demon`)
2. `src/headless/` imports from workspace crates (not `crate::*` main lib)
3. `src/lib.rs` is deleted
4. All compatibility shim directories are deleted
5. `src/` contains only `main.rs` and `headless/`
6. Root `Cargo.toml` has no `[lib]` section
7. `cargo build` produces the `fdemon` binary
8. `cargo run -- --help` works
9. `cargo test --workspace` passes

### Testing

```bash
# Build the binary
cargo build

# Verify binary works
cargo run -- --help

# Test all workspace crates
cargo test --workspace

# Verify no leftover source in src/ (except main.rs and headless/)
ls src/
# Should show: main.rs headless/
```

### Notes

- This is the "big switch" task where we go from the shim-based hybrid to a clean workspace.
- Do NOT delete the compatibility shims until ALL imports in main.rs and headless/ are updated and compiling.
- The binary's `[dependencies]` in root `Cargo.toml` should include all 4 internal crates plus external deps used directly by `main.rs` and `headless/`.
- `headless/runner.rs` is the more complex file (~200+ lines). It creates an `Engine` and runs the headless event loop. All its imports need updating from `crate::app::*` / `crate::daemon::*` to `fdemon_app::*` / `fdemon_daemon::*`.
- After this task, `cargo build` should produce the same `fdemon` binary as before. Test both TUI and headless modes if possible.
