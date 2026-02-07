## Task: Move Signal Handler from common/ to app/

**Objective**: Eliminate the `common/ -> app/` dependency violation by moving `signals.rs` from `common/` to `app/`, where the `Message` type it depends on is defined.

**Depends on**: None

**Estimated Time**: 1 hour

### Scope

- `src/common/signals.rs`: Move to `src/app/signals.rs`
- `src/common/mod.rs`: Remove `pub mod signals;`
- `src/app/mod.rs`: Add `pub mod signals;`
- `src/tui/runner.rs`: Update import path
- `src/headless/runner.rs`: Consolidate duplicate signal handler

### Details

#### The Violation

`src/common/signals.rs:5` imports `use crate::app::message::Message;`

The `common/` module is documented as having **no dependencies** (ARCHITECTURE.md line 82), but `signals.rs` breaks this by importing `Message::Quit` from `app/`.

#### Step 1: Move the file

Move `src/common/signals.rs` to `src/app/signals.rs` (no content changes needed).

The file contains:
```rust
// Two items:
pub fn spawn_signal_handler(tx: mpsc::Sender<Message>)  // line 9
async fn wait_for_signal() -> Result<()>                // line 22 (private)
```

The import changes from:
```rust
use crate::app::message::Message;  // old - cross-module
```
to:
```rust
use super::message::Message;  // new - same module (app/)
// or
use crate::app::message::Message;  // still works, just no longer cross-layer
```

#### Step 2: Update `src/common/mod.rs`

Remove the signals module declaration:
```rust
// REMOVE this line:
pub mod signals;
```

#### Step 3: Update `src/app/mod.rs`

Add the signals module declaration:
```rust
// ADD:
pub mod signals;
```

#### Step 4: Update consumer -- `src/tui/runner.rs`

Current import (line 16):
```rust
use crate::common::{prelude::*, signals};
```

Change to:
```rust
use crate::app::signals;
use crate::common::prelude::*;
```

Usage at line 52 (`signals::spawn_signal_handler(msg_tx.clone())`) stays the same.

#### Step 5: Consolidate headless signal handler (optional but recommended)

The headless runner at `src/headless/runner.rs:287-321` has a **duplicate** inline signal handler (`spawn_signal_handler`) that does the same thing plus emits a `HeadlessEvent`. After moving `signals.rs` to `app/`, the headless runner could:

**Option A (minimal):** Keep the duplicate. The headless handler has extra logic (emitting `HeadlessEvent::shutdown_requested`). This is fine for now.

**Option B (cleaner):** Refactor `app/signals.rs` to accept an optional callback for pre-quit actions:
```rust
pub fn spawn_signal_handler(
    tx: mpsc::Sender<Message>,
    on_signal: Option<Box<dyn Fn() + Send>>,
)
```
Then headless passes a closure that emits `HeadlessEvent`. But this adds scope -- defer to a future task.

**Recommendation**: Option A for this task. Just move the file and update imports.

### Acceptance Criteria

1. `src/common/signals.rs` no longer exists
2. `src/app/signals.rs` exists with identical content (updated import path)
3. `src/common/mod.rs` does not declare `signals` module
4. `src/app/mod.rs` declares `pub mod signals;`
5. `src/tui/runner.rs` imports signals from `crate::app::signals`
6. `common/` has zero imports from `app/`
7. `cargo build` succeeds
8. `cargo test` passes
9. `cargo clippy` is clean

### Testing

```bash
cargo test            # Full test suite
cargo clippy          # Lint check
```

No test files should need changes -- `signals.rs` has no tests of its own, and its consumers are runtime code (not tested in unit tests).

### Notes

- This is the simplest task in Phase 1. The file has no tests, two consumers, and the change is purely organizational.
- The headless duplicate signal handler is a code smell but not a dependency violation. Cleaning it up is deferred.
