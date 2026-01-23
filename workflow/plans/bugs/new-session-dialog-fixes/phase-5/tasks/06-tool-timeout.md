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

**Status:** Not Started
