## Task 4f: Clean Up actions.rs Legacy Code

**Objective**: Remove legacy compatibility code in `actions.rs` including session_id checks and global cmd_sender updates that were only needed for backward compatibility.

**Depends on**: Task 4e (AppState fields must be removed first)

---

### Background

The `actions.rs` file contains several pieces of legacy code:

1. **Global cmd_sender updates**: When a session spawns, it updates a global `cmd_sender` "for backward compatibility"
2. **session_id == 0 checks**: Legacy mode used `session_id: 0` to indicate "use global cmd_sender"
3. **Fallback to global cmd_sender**: When no session sender available, falls back to global

After tasks 4a-4e, all sessions have proper session_ids (> 0) and their own cmd_senders. The global cmd_sender and session_id == 0 patterns are obsolete.

---

### Scope

#### `src/tui/actions.rs`

**1. Remove fallback to global cmd_sender in `handle_action` (lines 38-52)**

Current code:
```rust
UpdateAction::SpawnTask(task) => {
    // Spawn async task for command execution
    // Prefer session-specific cmd_sender, fall back to global
    if let Some(sender) = session_cmd_sender {
        tokio::spawn(async move {
            execute_task(task, msg_tx, Some(sender)).await;
        });
    } else {
        // Fall back to global cmd_sender (legacy mode)  <-- REMOVE
        let cmd_sender_clone = cmd_sender.clone();         // <-- REMOVE
        tokio::spawn(async move {                          // <-- REMOVE
            let sender = cmd_sender_clone.lock().await.clone();
            execute_task(task, msg_tx, sender).await;
        });
    }
}
```

**New code:**
```rust
UpdateAction::SpawnTask(task) => {
    if let Some(sender) = session_cmd_sender {
        tokio::spawn(async move {
            execute_task(task, msg_tx, Some(sender)).await;
        });
    } else {
        // No sender available - task will fail gracefully
        tokio::spawn(async move {
            execute_task(task, msg_tx, None).await;
        });
    }
}
```

---

**2. Remove global cmd_sender parameter from `handle_action` signature**

Since we no longer fall back to global cmd_sender, we can remove the parameter:

Current signature:
```rust
pub fn handle_action(
    action: UpdateAction,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Arc<Mutex<Option<CommandSender>>>,  // <-- REMOVE
    session_cmd_sender: Option<CommandSender>,
    session_senders: Vec<(SessionId, String, CommandSender)>,
    session_tasks: SessionTaskMap,
    shutdown_rx: watch::Receiver<bool>,
    project_path: &Path,
)
```

New signature:
```rust
pub fn handle_action(
    action: UpdateAction,
    msg_tx: mpsc::Sender<Message>,
    session_cmd_sender: Option<CommandSender>,
    session_senders: Vec<(SessionId, String, CommandSender)>,
    session_tasks: SessionTaskMap,
    shutdown_rx: watch::Receiver<bool>,
    project_path: &Path,
)
```

**Note**: This requires updating all call sites of `handle_action`.

---

**3. Remove global cmd_sender update in `spawn_session` (lines 150-160)**

Current code:
```rust
// Send SessionProcessAttached to store cmd_sender in SessionHandle
let _ = msg_tx_clone
    .send(Message::SessionProcessAttached {
        session_id,
        cmd_sender: session_sender.clone(),
    })
    .await;

// Also update legacy global cmd_sender for backward compatibility <-- REMOVE
*cmd_sender_clone.lock().await = Some(session_sender.clone());    // <-- REMOVE
```

**New code:**
```rust
// Send SessionProcessAttached to store cmd_sender in SessionHandle
let _ = msg_tx_clone
    .send(Message::SessionProcessAttached {
        session_id,
        cmd_sender: session_sender.clone(),
    })
    .await;
```

---

**4. Remove global cmd_sender clear on session end (lines 248-256)**

Current code:
```rust
// Clear the global command sender if it was ours
// (only matters for legacy single-session compatibility)
let mut guard = cmd_sender_clone.lock().await;
*guard = None;
drop(guard);
```

**Remove entirely** - no longer needed.

---

**5. Remove cmd_sender parameter from `spawn_session` (lines 93-103)**

Current signature:
```rust
fn spawn_session(
    session_id: SessionId,
    device: Device,
    config: Option<Box<LaunchConfig>>,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Arc<Mutex<Option<CommandSender>>>,  // <-- REMOVE
    session_tasks: SessionTaskMap,
    shutdown_rx: watch::Receiver<bool>,
)
```

New signature:
```rust
fn spawn_session(
    session_id: SessionId,
    device: Device,
    config: Option<Box<LaunchConfig>>,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
    session_tasks: SessionTaskMap,
    shutdown_rx: watch::Receiver<bool>,
)
```

---

**6. Remove session_id > 0 checks in `execute_task` (lines 290-380)**

Current code has patterns like:
```rust
// Use session-specific message for multi-session mode (session_id > 0)
// Use legacy message for single-session mode (session_id == 0)
if session_id > 0 {
    let _ = msg_tx
        .send(Message::SessionReloadCompleted { session_id, time_ms })
        .await;
} else {
    let _ = msg_tx.send(Message::ReloadCompleted { time_ms }).await;
}
```

**New code** - always use session-specific messages:
```rust
let _ = msg_tx
    .send(Message::SessionReloadCompleted { session_id, time_ms })
    .await;
```

Apply this pattern to:
- Reload success (lines 318-334)
- Reload failure (lines 336-346)
- Restart success (lines 356-364)
- Restart failure (lines 366-378)

---

### Call Site Updates

#### `src/tui/process.rs`

Update call to `handle_action`:
```rust
// Before:
handle_action(
    action,
    msg_tx.clone(),
    cmd_sender.clone(),  // <-- REMOVE
    session_cmd_sender,
    session_senders,
    session_tasks.clone(),
    shutdown_rx.clone(),
    project_path,
);

// After:
handle_action(
    action,
    msg_tx.clone(),
    session_cmd_sender,
    session_senders,
    session_tasks.clone(),
    shutdown_rx.clone(),
    project_path,
);
```

---

### Implementation Steps

1. **Update `execute_task` to remove session_id checks**
   - Remove all `if session_id > 0` branches
   - Always send session-specific messages

2. **Remove global cmd_sender usage from `spawn_session`**
   - Remove the `*cmd_sender_clone.lock().await = ...` line
   - Remove the cleanup at session end
   - Remove the parameter

3. **Remove global cmd_sender fallback from `handle_action`**
   - Remove the `else` branch that uses global sender
   - Remove the parameter

4. **Update call sites**
   - Update `process.rs` to not pass cmd_sender

5. **Consider removing global cmd_sender entirely**
   - If no longer used anywhere, remove from runner.rs too
   - This may be deferred if other code still references it

---

### Files Changed Summary

| File | Lines Removed | Lines Changed |
|------|---------------|---------------|
| `actions.rs` | ~40 | ~20 |
| `process.rs` | 0 | ~5 |

**Total: ~40 lines removed, ~25 lines changed**

---

### Acceptance Criteria

1. ✅ No `session_id > 0` checks in execute_task
2. ✅ No `session_id == 0` patterns anywhere
3. ✅ No global cmd_sender updates in spawn_session
4. ✅ No global cmd_sender fallback in handle_action
5. ✅ `cmd_sender` parameter removed from spawn_session
6. ✅ All tasks use session-specific messages (SessionReloadCompleted, etc.)
7. ✅ `cargo check` passes
8. ✅ `cargo clippy` shows no warnings
9. ✅ All session operations still work

---

### Testing

#### Compile-Time Verification
- `cargo check` passes
- `cargo clippy` shows no warnings
- No unused parameter warnings

#### Verification Commands

```bash
# Verify no session_id == 0 patterns
grep -n "session_id: 0" src/tui/actions.rs
# Should return no matches

# Verify no session_id > 0 checks
grep -n "session_id > 0" src/tui/actions.rs
# Should return no matches

# Verify no legacy comments
grep -n "legacy" src/tui/actions.rs
# Should return no matches
```

#### Runtime Testing
1. Start session → verify cmd_sender attached
2. Hot reload → verify SessionReloadCompleted message sent
3. Hot restart → verify SessionRestartCompleted message sent
4. Multiple sessions → verify each has independent cmd_sender
5. Session close → verify no global state affected

---

### Edge Cases

1. **Task with no cmd_sender**
   - `execute_task` already handles this gracefully
   - Sends appropriate failure message

2. **Session spawn failure**
   - SessionSpawnFailed message still sent correctly
   - No global state to clean up

---

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Breaking task execution | Verify session-specific messages work |
| Missing cmd_sender | Already handled with failure path |
| Call site not updated | Compiler will catch parameter mismatch |

---

### Post-Task Cleanup

After this task, consider whether the global `cmd_sender: Arc<Mutex<Option<CommandSender>>>` in `runner.rs` is still needed:

- If only used for spawn_session parameter (now removed), can delete
- If still referenced elsewhere, may need to keep or remove in separate task

---

### Estimated Effort

**30 minutes**

- 15 minutes: Remove session_id checks and global sender updates
- 10 minutes: Update signatures and call sites  
- 5 minutes: Compile and verify