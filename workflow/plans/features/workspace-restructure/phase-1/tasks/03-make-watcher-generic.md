## Task: Make FileWatcher Generic (Remove watcher/ -> app/ Dependency)

**Objective**: Eliminate the `watcher/ -> app/` dependency violation by making `FileWatcher` produce its own `WatcherEvent` enum instead of constructing `Message` variants directly.

**Depends on**: None

**Estimated Time**: 1-2 hours

### Scope

- `src/watcher/mod.rs`: Remove `Message` import, define `WatcherEvent`, change API
- `src/tui/runner.rs`: Add bridge to convert `WatcherEvent` -> `Message`
- `src/headless/runner.rs`: Add bridge to convert `WatcherEvent` -> `Message`

### Details

#### The Violation

`src/watcher/mod.rs:14` imports `use crate::app::message::Message;`

The watcher constructs 3 specific `Message` variants internally:
- `Message::AutoReloadTriggered` (line 175)
- `Message::FilesChanged { count }` (line 178)
- `Message::WatcherError { message }` (lines 186, 199)

#### Step 1: Define `WatcherEvent` enum in `watcher/mod.rs`

Add a new enum at the top of the file:

```rust
/// Events emitted by the file watcher.
/// Consumers map these to their own message types.
#[derive(Debug, Clone)]
pub enum WatcherEvent {
    /// File changes detected and auto-reload is enabled
    AutoReloadTriggered,
    /// File changes detected but auto-reload is disabled
    FilesChanged { count: usize },
    /// Watcher encountered an error
    Error { message: String },
}
```

#### Step 2: Change `FileWatcher::start()` signature

Before:
```rust
pub fn start(&mut self, message_tx: mpsc::Sender<Message>) -> Result<(), String>
```

After:
```rust
pub fn start(&mut self, event_tx: mpsc::Sender<WatcherEvent>) -> Result<(), String>
```

#### Step 3: Update `run_watcher()` to use `WatcherEvent`

Before (line 175-186):
```rust
if config.auto_reload {
    let _ = message_tx.send(Message::AutoReloadTriggered).await;
} else {
    let _ = message_tx.send(Message::FilesChanged { count: changed_count }).await;
}
// ...
let _ = message_tx.send(Message::WatcherError { message: err_msg }).await;
```

After:
```rust
if config.auto_reload {
    let _ = event_tx.send(WatcherEvent::AutoReloadTriggered).await;
} else {
    let _ = event_tx.send(WatcherEvent::FilesChanged { count: changed_count }).await;
}
// ...
let _ = event_tx.send(WatcherEvent::Error { message: err_msg }).await;
```

#### Step 4: Remove the `Message` import

Delete from `watcher/mod.rs`:
```rust
use crate::app::message::Message;  // DELETE THIS LINE
```

#### Step 5: Update `src/tui/runner.rs` -- add bridge

The TUI runner currently does:
```rust
file_watcher.start(msg_tx.clone());
```

Change to create a bridge channel:

```rust
// Create watcher-specific channel
let (watcher_tx, mut watcher_rx) = mpsc::channel::<WatcherEvent>(32);
file_watcher.start(watcher_tx)?;

// Bridge watcher events to app messages
let watcher_msg_tx = msg_tx.clone();
tokio::spawn(async move {
    while let Some(event) = watcher_rx.recv().await {
        let msg = match event {
            WatcherEvent::AutoReloadTriggered => Message::AutoReloadTriggered,
            WatcherEvent::FilesChanged { count } => Message::FilesChanged { count },
            WatcherEvent::Error { message } => Message::WatcherError { message },
        };
        let _ = watcher_msg_tx.send(msg).await;
    }
});
```

Add import:
```rust
use crate::watcher::WatcherEvent;
```

#### Step 6: Update `src/headless/runner.rs` -- add bridge

Same pattern as the TUI runner. The headless runner currently does:
```rust
file_watcher.start(msg_tx.clone());
```

Change to the same bridge pattern. Import `WatcherEvent` from `crate::watcher`.

### Acceptance Criteria

1. `src/watcher/mod.rs` has zero imports from `crate::app`
2. `WatcherEvent` enum is defined in `watcher/mod.rs`
3. `FileWatcher::start()` accepts `mpsc::Sender<WatcherEvent>`
4. TUI runner bridges `WatcherEvent` -> `Message`
5. Headless runner bridges `WatcherEvent` -> `Message`
6. `cargo build` succeeds
7. `cargo test` passes
8. `cargo clippy` is clean
9. Auto-reload still works (manual verification)

### Testing

```bash
cargo test            # Full test suite
cargo test watcher    # Watcher-specific tests (if any)
cargo clippy          # Lint check
```

Verify manually that file watcher behavior is unchanged:
1. Start fdemon in a Flutter project
2. Edit a `.dart` file in `lib/`
3. Confirm hot reload triggers automatically

### Notes

- The bridge pattern adds one extra async task per runner but the overhead is negligible (one channel forward per file change event).
- `WatcherConfig` and the public constants (`DEFAULT_DEBOUNCE_MS`, etc.) are unchanged.
- If `watcher/mod.rs` has any inline tests that construct `Message`, they need to change to construct `WatcherEvent` instead.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/watcher/mod.rs` | Added `WatcherEvent` enum, changed `FileWatcher::start()` signature to accept `mpsc::Sender<WatcherEvent>`, updated all internal event sending to use `WatcherEvent` variants, removed `use crate::app::message::Message` import, updated test to use `WatcherEvent` channel type |
| `src/tui/runner.rs` | Added import for `WatcherEvent`, created bridge channel for watcher events, spawned async task to convert `WatcherEvent` -> `Message` |
| `src/headless/runner.rs` | Added import for `WatcherEvent`, created bridge channel for watcher events, spawned async task to convert `WatcherEvent` -> `Message` |

### Notable Decisions/Tradeoffs

1. **Bridge Pattern**: Used the bridge pattern as specified in the task - creates a separate channel for `WatcherEvent` and spawns an async task to convert events to `Message` variants. This adds minimal overhead (one async task per runner, one channel forward per file change event) but provides clean separation of concerns and eliminates the layer violation.

2. **Consistent Implementation**: Applied the same bridge pattern to both TUI and headless runners for consistency and maintainability.

3. **Event Naming**: Used `WatcherEvent::Error` instead of `WatcherEvent::WatcherError` to avoid redundancy since the enum name already indicates these are watcher events.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo build` - Passed (5.25s)
- `cargo test` - Pre-existing test failures unrelated to this task (daemon/protocol.rs tests reference missing `daemon::events` module)
- `cargo clippy -- -D warnings` - Passed

### Verification of Acceptance Criteria

1. `src/watcher/mod.rs` has zero imports from `crate::app` - VERIFIED (grep shows no matches)
2. `WatcherEvent` enum is defined in `watcher/mod.rs` - VERIFIED (lines 17-25)
3. `FileWatcher::start()` accepts `mpsc::Sender<WatcherEvent>` - VERIFIED (line 112)
4. TUI runner bridges `WatcherEvent` -> `Message` - VERIFIED (lines 84-106 in runner.rs)
5. Headless runner bridges `WatcherEvent` -> `Message` - VERIFIED (lines 98-117 in headless/runner.rs)
6. `cargo build` succeeds - VERIFIED
7. `cargo test` passes - BLOCKED (pre-existing unrelated test failures)
8. `cargo clippy` is clean - VERIFIED
9. Auto-reload still works - REQUIRES MANUAL VERIFICATION

### Risks/Limitations

1. **Pre-existing Test Failures**: The test suite has pre-existing failures related to missing `daemon::events` module that prevent running tests. These failures existed before this implementation and are unrelated to the watcher changes. The watcher module compiles successfully and all verification commands that don't depend on the test suite pass.

2. **Manual Verification Required**: Auto-reload functionality should be manually verified by running fdemon in a Flutter project and editing a `.dart` file to confirm hot reload triggers automatically. The implementation preserves all existing behavior through the bridge pattern.
