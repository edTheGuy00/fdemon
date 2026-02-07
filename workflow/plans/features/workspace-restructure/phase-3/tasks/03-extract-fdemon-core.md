## Task: Extract fdemon-core Crate

**Objective**: Move `common/` and `core/` modules into the `fdemon-core` crate. This is the foundation crate with zero internal dependencies. All other crates depend on it for `Error`, `Result`, domain types, and prelude.

**Depends on**: 01-create-workspace-scaffold, 02-decouple-app-from-tui-entry

**Estimated Time**: 3-5 hours

### Scope

- `src/common/error.rs` -> `crates/fdemon-core/src/error.rs`
- `src/common/logging.rs` -> `crates/fdemon-core/src/logging.rs`
- `src/common/mod.rs` (prelude) -> `crates/fdemon-core/src/prelude.rs`
- `src/core/types.rs` -> `crates/fdemon-core/src/types.rs`
- `src/core/events.rs` -> `crates/fdemon-core/src/events.rs`
- `src/core/discovery.rs` -> `crates/fdemon-core/src/discovery.rs`
- `src/core/stack_trace.rs` -> `crates/fdemon-core/src/stack_trace.rs`
- `src/core/ansi.rs` -> `crates/fdemon-core/src/ansi.rs`
- `crates/fdemon-core/src/lib.rs`: Wire up all modules and public API

### Details

#### 1. File Moves

Copy all files from `src/common/` and `src/core/` into `crates/fdemon-core/src/`:

```
crates/fdemon-core/src/
  lib.rs          (module declarations + public API)
  error.rs        (from common/error.rs)
  logging.rs      (from common/logging.rs)
  prelude.rs      (from common/mod.rs prelude)
  types.rs        (from core/types.rs)
  events.rs       (from core/events.rs)
  discovery.rs    (from core/discovery.rs)
  stack_trace.rs  (from core/stack_trace.rs)
  ansi.rs         (from core/ansi.rs)
```

#### 2. Write `lib.rs`

```rust
//! fdemon-core - Core domain types for Flutter Demon
//!
//! This crate provides the foundational types shared across all Flutter Demon
//! crates: error handling, domain types, event definitions, and project discovery.

pub mod ansi;
pub mod discovery;
pub mod error;
pub mod events;
pub mod logging;
pub mod stack_trace;
pub mod types;

/// Prelude for common imports used throughout all Flutter Demon crates
pub mod prelude {
    pub use super::error::{Error, Result, ResultExt};
    pub use tracing::{debug, error, info, instrument, trace, warn};
}

// Re-export commonly used types at crate root for convenience
pub use ansi::{contains_word, strip_ansi_codes};
pub use discovery::{
    discover_flutter_projects, get_project_type, is_runnable_flutter_project, ProjectType,
    DEFAULT_MAX_DEPTH,
};
pub use error::{Error, Result};
pub use events::{DaemonEvent, DaemonMessage};
pub use types::{AppPhase, LogEntry, LogLevel, LogSource};
```

#### 3. Update Internal Imports

All `use crate::common::prelude::*` becomes `use crate::prelude::*` within `fdemon-core`.

All `use crate::core::*` or `use crate::common::*` references within these files become `use crate::*` (since they're now in the same crate).

Specific patterns to replace inside `fdemon-core` files:
- `use crate::common::prelude::*` -> remove (prelude is in same crate)
- `use crate::core::ansi::*` -> `use crate::ansi::*`
- `use crate::core::stack_trace::*` -> `use crate::stack_trace::*`
- `use crate::core::types::*` -> `use crate::types::*`

#### 4. Keep Compatibility Shims in Main Crate (Temporary)

During the transition, keep `src/common/mod.rs` and `src/core/mod.rs` in the main crate but change them to re-export from `fdemon-core`:

```rust
// src/common/mod.rs (temporary re-export shim)
pub use fdemon_core::error;
pub use fdemon_core::logging;
pub use fdemon_core::prelude;
```

```rust
// src/core/mod.rs (temporary re-export shim)
pub use fdemon_core::*;
```

This allows all existing `use crate::common::` and `use crate::core::` imports in `app/`, `daemon/`, `tui/`, etc. to continue working while those modules are still in the main crate. The shims are removed when each module is extracted to its own crate (tasks 04-06).

#### 5. Handle `serde_json` in Error Type

`common/error.rs` has `Json(#[from] serde_json::Error)`, which means `fdemon-core` needs `serde_json` as a dependency. This is already accounted for in the `Cargo.toml` from task 01.

#### 6. Handle `dirs` in Logging

`common/logging.rs` uses `dirs::data_local_dir()`. The `dirs` crate is already in `fdemon-core`'s dependencies.

### Acceptance Criteria

1. `crates/fdemon-core/src/` contains all files from `common/` and `core/`
2. `cargo check -p fdemon-core` passes
3. `cargo test -p fdemon-core` passes (all unit tests from `common/` and `core/` run)
4. Compatibility shims in `src/common/mod.rs` and `src/core/mod.rs` re-export from `fdemon-core`
5. `cargo check` (full workspace) passes - existing code still compiles via re-exports
6. `cargo test` (full workspace) passes
7. `fdemon-core` has zero internal crate dependencies (only external crates)

### Testing

```bash
# Test the new crate in isolation
cargo check -p fdemon-core
cargo test -p fdemon-core

# Test full workspace still works
cargo check
cargo test
```

### Notes

- This is the most foundational task. Every other crate depends on `fdemon-core`.
- The compatibility shims are intentionally temporary. They'll be removed as each consuming module moves to its own crate.
- `discovery.rs` has tests using `tempfile` (dev-dependency). These should work as-is since `tempfile` is in `fdemon-core`'s `[dev-dependencies]`.
- The `prelude` module provides `Error`, `Result`, `ResultExt`, and `tracing` macros. All other crates will use `use fdemon_core::prelude::*`.
- Do NOT remove the original files from `src/common/` and `src/core/` in this task. Only add the re-export shims. The originals are deleted when no longer needed (cleanup in task 09).
