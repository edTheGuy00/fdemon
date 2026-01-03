## Task: Per-Session Task Tracking

**Objective**: Replace the single `session_task: Arc<Mutex<Option<JoinHandle>>>` with a per-session task map `session_tasks: Arc<Mutex<HashMap<SessionId, JoinHandle>>>` so multiple Flutter processes can run concurrently without overwriting each other's task handles.

**Depends on**: Task 02 (Session created before spawn)

---

### Scope

- `src/tui/mod.rs`: Change task tracking data structure and update all usages
- `src/app/session.rs`: Ensure SessionId is Copy/Clone for HashMap key usage

---

### Current State

```rust
// In src/tui/mod.rs - run_with_project
let session_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>> = Arc::new(Mutex::new(None));

// In handle_action - SpawnSession
if let Ok(mut guard) = session_task.try_lock() {
    *guard = Some(handle);  // OVERWRITES previous task!
}

// In cleanup
let task_handle = session_task.lock().await.take();  // Only ONE task!
```

**Problem:** Only one task can be tracked. Starting a second session overwrites the first task handle, causing:
- First process runs but is untracked
- Cleanup only waits for the last-started process
- First process becomes orphaned on quit

---

### Implementation Details

#### 1. Update Data Structure

```rust
// In src/tui/mod.rs - run_with_project
use std::collections::HashMap;
use crate::app::session::SessionId;

// Replace single task with HashMap
let session_tasks: Arc<Mutex<HashMap<SessionId, tokio::task::JoinHandle<()>>>> = 
    Arc::new(Mutex::new(HashMap::new()));
```

#### 2. Update handle_action for SpawnSession

```rust
UpdateAction::SpawnSession { session_id, device, config } => {
    let project_path = project_path.to_path_buf();
    let msg_tx_clone = msg_tx.clone();
    let session_tasks_clone = session_tasks.clone();
    let session_id = session_id;
    // ... other clones ...

    let handle = tokio::spawn(async move {
        // ... existing spawn logic ...
        
        // At the end of the spawned task, remove self from tracking
        session_tasks_clone.lock().await.remove(&session_id);
    });

    // Store handle with session_id as key
    if let Ok(mut guard) = session_tasks.try_lock() {
        guard.insert(session_id, handle);
    }
}
```

#### 3. Update Cleanup Path

```rust
// In run_with_project cleanup section
// Instead of taking single task, iterate all tasks

// Collect all task handles
let tasks: Vec<(SessionId, tokio::task::JoinHandle<()>)> = {
    let mut guard = session_tasks.lock().await;
    guard.drain().collect()
};

// Wait for all tasks with timeout
for (session_id, handle) in tasks {
    info!("Waiting for session {} to complete shutdown...", session_id);
    match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
        Ok(Ok(())) => info!("Session {} completed cleanly", session_id),
        Ok(Err(e)) => warn!("Session {} task panicked: {}", session_id, e),
        Err(_) => warn!("Timeout waiting for session {}, may be orphaned", session_id),
    }
}
```

#### 4. Update run_loop and process_message Signatures

```rust
fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut AppState,
    mut msg_rx: mpsc::Receiver<Message>,
    mut daemon_rx: mpsc::Receiver<DaemonEvent>,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Arc<Mutex<Option<CommandSender>>>,  // Will be replaced in Task 04
    session_tasks: Arc<Mutex<HashMap<SessionId, JoinHandle<()>>>>,  // Updated
    shutdown_rx: watch::Receiver<bool>,
    project_path: &Path,
) -> Result<()>
```

#### 5. Update process_message and handle_action Signatures

All functions that pass session_task need to be updated to pass session_tasks.

---

### Session Task Lifecycle After This Task

```
Session Created (Task 02)
         │
         ▼
SpawnSession action with session_id
         │
         ▼
tokio::spawn() creates handle
         │
         ▼
session_tasks.insert(session_id, handle)
         │
         ▼
Task runs (forwarding events, etc.)
         │
         ▼
Task completes or shutdown signal
         │
         ▼
session_tasks.remove(session_id)
```

---

### Acceptance Criteria

1. [ ] `session_tasks` is a `HashMap<SessionId, JoinHandle>`
2. [ ] Starting second session doesn't overwrite first task
3. [ ] Each spawned task is tracked by its session_id
4. [ ] Tasks remove themselves from map on completion
5. [ ] Cleanup iterates and waits for ALL tasks
6. [ ] No compilation errors with new signatures

---

### Testing

```rust
#[tokio::test]
async fn test_multiple_session_tasks_tracked() {
    let session_tasks: Arc<Mutex<HashMap<SessionId, JoinHandle<()>>>> = 
        Arc::new(Mutex::new(HashMap::new()));
    
    // Simulate adding two tasks
    let tasks_clone = session_tasks.clone();
    let handle1 = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(100)).await;
    });
    
    let handle2 = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(100)).await;
    });
    
    {
        let mut guard = session_tasks.lock().await;
        guard.insert(1, handle1);
        guard.insert(2, handle2);
    }
    
    // Both tasks should be tracked
    assert_eq!(session_tasks.lock().await.len(), 2);
    
    // Wait for completion
    tokio::time::sleep(Duration::from_millis(150)).await;
}

#[tokio::test]
async fn test_cleanup_waits_for_all_tasks() {
    use std::sync::atomic::{AtomicU32, Ordering};
    
    let completed = Arc::new(AtomicU32::new(0));
    let session_tasks: Arc<Mutex<HashMap<SessionId, JoinHandle<()>>>> = 
        Arc::new(Mutex::new(HashMap::new()));
    
    // Create 3 tasks that increment counter on completion
    for i in 1..=3 {
        let completed_clone = completed.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            completed_clone.fetch_add(1, Ordering::SeqCst);
        });
        session_tasks.lock().await.insert(i, handle);
    }
    
    // Drain and wait for all
    let tasks: Vec<_> = session_tasks.lock().await.drain().collect();
    for (_, handle) in tasks {
        let _ = handle.await;
    }
    
    // All should have completed
    assert_eq!(completed.load(Ordering::SeqCst), 3);
}
```

---

### Notes

- `SessionId` is `u64` which implements `Copy`, `Clone`, `Hash`, `Eq` - suitable for HashMap key
- Tasks self-remove on completion to avoid stale entries
- The shutdown signal broadcast will cause all tasks to exit their loops
- This task focuses on task tracking; command sender routing is Task 04
- Consider using `tokio::task::JoinSet` as an alternative, but HashMap gives us session_id lookup capability

---

## Completion Summary

**Status:** ✅ Done

**Files Modified:**
- `src/tui/mod.rs`:
  - Added imports: `use std::collections::HashMap;` and `use crate::app::session::SessionId;` (lines 12, 18)
  - Changed `session_task: Arc<Mutex<Option<JoinHandle>>>` to `session_tasks: Arc<Mutex<HashMap<SessionId, JoinHandle>>>` (lines 62-64)
  - Updated `run_loop` signature to use `session_tasks` (line 393)
  - Updated `process_message` signature to use `session_tasks` (line 471)
  - Updated `handle_action` signature to use `session_tasks` (line 521)
  - Updated `SpawnSession` handler to:
    - Use `session_id` in log messages (lines 557-559, 575-578, etc.)
    - Insert task with `session_id` key (lines 675-682)
    - Self-remove from HashMap on task completion (lines 670-672)
  - Updated cleanup code to iterate and wait for ALL tasks (lines 231-263)
  - Updated `run()` test/demo function to use new type (lines 366-368)

**Notable Decisions/Tradeoffs:**
- Used `HashMap<SessionId, JoinHandle>` instead of `JoinSet` to maintain lookup by session_id
- Each task removes itself from the map on completion (whether success, failure, or shutdown)
- Cleanup uses 5-second timeout per task (vs 10 seconds for single task before)
- Added session_id to log messages for better debugging

**Testing Performed:**
- `cargo check` - Passed (no compilation errors)
- `cargo test` - All 395 tests passed
- `cargo fmt` - Code formatted
- `cargo clippy` - Only pre-existing warning about `run_loop` having too many arguments

**Risks/Limitations:**
- Command sender is still single (shared) - Task 04 will create per-session senders
- Session IDs must be unique (guaranteed by SessionManager)
- If a task panics before removing itself, the entry remains in map (but JoinHandle can still be awaited)

**Acceptance Criteria Status:**
1. [x] `session_tasks` is a `HashMap<SessionId, JoinHandle>`
2. [x] Starting second session doesn't overwrite first task
3. [x] Each spawned task is tracked by its session_id
4. [x] Tasks remove themselves from map on completion
5. [x] Cleanup iterates and waits for ALL tasks
6. [x] No compilation errors with new signatures