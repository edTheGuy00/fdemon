## Task: Add Timeout to Tool Availability Check

**Objective**: Add a timeout to the tool availability check to prevent permanent loading states if `xcrun` or `emulator` commands hang.

**Priority**: Major

**Depends on**: None

### Scope

- `src/tui/spawn.rs`: `spawn_tool_availability_check()` function
- `src/daemon/tool_availability.rs`: Individual tool check methods
- `src/app/handler/update.rs`: Add timeout message handling (optional)

### Problem Analysis

**Current flow (no timeout):**

1. `runner.rs:68-69` - Spawns tool check at startup
2. `spawn.rs:295-302` - Spawns async task, no timeout
3. `tool_availability.rs:44-52` - Runs `xcrun simctl help`, blocks until complete
4. `update.rs:1031-1051` - Handler waits for message

**Problem:**
- If `xcrun` or `emulator` hangs (misconfigured SDK, Xcode issues), the check never completes
- `bootable_loading` stays `true` forever
- User sees permanent spinner on bootable tab

### Solution Options

#### Option A: Timeout in spawn layer (Recommended)

Add `tokio::time::timeout` around the tool check:

```rust
// spawn.rs
pub fn spawn_tool_availability_check(msg_tx: mpsc::Sender<Message>) {
    tokio::spawn(async move {
        const TIMEOUT: Duration = Duration::from_secs(10);

        let availability = match tokio::time::timeout(TIMEOUT, ToolAvailability::check()).await {
            Ok(result) => result,
            Err(_) => {
                tracing::warn!("Tool availability check timed out after {:?}", TIMEOUT);
                ToolAvailability::default()  // Assume no tools available
            }
        };

        let _ = msg_tx
            .send(Message::ToolAvailabilityChecked { availability })
            .await;
    });
}
```

#### Option B: Timeout in individual checks

Add timeout to each command:

```rust
// tool_availability.rs
async fn check_xcrun_simctl() -> bool {
    const TIMEOUT: Duration = Duration::from_secs(5);

    match tokio::time::timeout(TIMEOUT, async {
        Command::new("xcrun")
            .args(["simctl", "help"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
    }).await {
        Ok(Ok(status)) => status.success(),
        Ok(Err(e)) => {
            tracing::debug!("xcrun simctl check failed: {}", e);
            false
        }
        Err(_) => {
            tracing::warn!("xcrun simctl check timed out");
            false
        }
    }
}
```

#### Option C: Add timeout message (Belt and suspenders)

Add a fallback timer that fires if tool check takes too long:

```rust
// In runner.rs or spawn.rs
tokio::spawn(async move {
    tokio::time::sleep(Duration::from_secs(15)).await;
    let _ = msg_tx.send(Message::ToolAvailabilityTimeout).await;
});

// In update.rs
Message::ToolAvailabilityTimeout => {
    if state.tool_availability == ToolAvailability::default() {
        // Still waiting for check - assume timed out
        tracing::warn!("Tool availability check timed out, assuming no tools available");
        state.new_session_dialog_state.target_selector.bootable_loading = false;
    }
    UpdateResult::none()
}
```

### Recommended Implementation (Option A)

**In `src/tui/spawn.rs`:**

```rust
use tokio::time::{timeout, Duration};

/// Timeout for tool availability checks
const TOOL_CHECK_TIMEOUT: Duration = Duration::from_secs(10);

pub fn spawn_tool_availability_check(msg_tx: mpsc::Sender<Message>) {
    tokio::spawn(async move {
        let availability = match timeout(TOOL_CHECK_TIMEOUT, ToolAvailability::check()).await {
            Ok(result) => result,
            Err(_elapsed) => {
                tracing::warn!(
                    "Tool availability check timed out after {:?}, assuming no tools available",
                    TOOL_CHECK_TIMEOUT
                );
                ToolAvailability::default()
            }
        };

        let _ = msg_tx
            .send(Message::ToolAvailabilityChecked { availability })
            .await;
    });
}
```

**Ensure `ToolAvailability::default()` returns safe defaults:**

```rust
impl Default for ToolAvailability {
    fn default() -> Self {
        Self {
            xcrun_simctl: false,
            android_emulator: false,
            emulator_path: None,
        }
    }
}
```

### Acceptance Criteria

1. Tool availability check has a timeout (10 seconds recommended)
2. If timeout occurs, `bootable_loading` is set to `false`
3. User sees empty bootable tab (not permanent spinner) on timeout
4. Normal case (tools respond quickly) still works
5. Warning logged when timeout occurs
6. All existing tests pass

### Testing

```bash
cargo test tool_availability
cargo test spawn
```

Manual testing:
1. Temporarily rename `xcrun` to cause lookup failure - should timeout gracefully
2. Set `ANDROID_HOME` to invalid path - should timeout gracefully
3. Verify bootable tab shows empty state, not spinner

### Notes

- 10 seconds is generous - most tool checks complete in <1 second
- `tokio::time::timeout` is the idiomatic Tokio approach
- Consider adding per-tool timeouts (5s each) for better granularity

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/spawn.rs` | Added `tokio::time::{timeout, Duration}` import, added `TOOL_CHECK_TIMEOUT` constant (10 seconds), wrapped `ToolAvailability::check()` with timeout in `spawn_tool_availability_check()` function |
| `src/app/handler/update.rs` | Added else branch to `Message::ToolAvailabilityChecked` handler to set `bootable_loading = false` when no tools are available (timeout case) |

### Notable Decisions/Tradeoffs

1. **10-second timeout chosen**: This is generous enough to handle slow SDK paths while preventing permanent hangs. Most tool checks complete in under 1 second, so this gives plenty of buffer.

2. **Timeout at spawn layer (Option A)**: Implemented timeout at the spawn layer rather than within individual tool check methods. This provides a single, clear timeout boundary and simplifies the implementation.

3. **Default values on timeout**: When timeout occurs, `ToolAvailability::default()` returns safe defaults (`xcrun_simctl: false`, `android_emulator: false`, `emulator_path: None`), which results in empty bootable tab instead of permanent loading spinner.

4. **Critical fix in handler**: Added else branch in `Message::ToolAvailabilityChecked` handler to set `bootable_loading = false` when no tools are available. Without this, the bootable tab would show a permanent spinner even after timeout because `bootable_loading` is initialized to `true` when the dialog opens.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo build` - Passed
- `cargo test tool_availability` - Passed (9 tests)
- `cargo clippy -- -D warnings` - Passed

### Risks/Limitations

1. **Pre-existing test compilation errors**: The test suite has compilation errors in unrelated modules (navigation module visibility). These errors existed before this change and are not caused by the timeout implementation. The main codebase compiles successfully with `cargo build` and `cargo check`.

2. **Manual testing needed**: Should manually test timeout behavior by temporarily misconfiguring SDK paths or renaming tool binaries, though this is difficult to automate reliably.

### Acceptance Criteria Met

1. ✅ Tool availability check has a timeout (10 seconds) - Implemented with `tokio::time::timeout` in `spawn.rs`
2. ✅ If timeout occurs, `bootable_loading` is set to `false` - Implemented in else branch of handler in `update.rs`
3. ✅ User sees empty bootable tab (not permanent spinner) on timeout - Follows from criterion 2
4. ✅ Normal case (tools respond quickly) still works - Existing code path unchanged
5. ✅ Warning logged when timeout occurs - `tracing::warn!` added with timeout details
6. ⚠️ All existing tests pass - Pre-existing test compilation errors unrelated to this change; related tests pass
