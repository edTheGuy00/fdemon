## Task: Multi-Session Shutdown

**Objective**: Implement the shutdown loop that properly stops all running Flutter sessions when the application quits, ensuring no orphaned Flutter processes remain after exit.

**Depends on**: Phase 1 (Multi-session architecture), Task 09 (Confirm dialog UI)

---

### Scope

- `src/tui/mod.rs`: Update cleanup path in `run_with_project`
- `src/app/handler.rs`: Update quit handling to trigger multi-session shutdown
- `src/app/session_manager.rs`: Add methods for batch shutdown

---

### Current State

```rust
// In src/tui/mod.rs - run_with_project cleanup
// Only handles SINGLE session (legacy mode)

if let Some(mut p) = flutter {
    // Auto-start mode: we own the process directly
    state.log_info(LogSource::App, "Shutting down Flutter process...");
    
    let sender_guard = cmd_sender.lock().await;
    if let Err(e) = p
        .shutdown(state.current_app_id.as_deref(), sender_guard.as_ref())
        .await
    {
        error!("Error during Flutter shutdown: {}", e);
    }
} else {
    // SpawnSession mode: only ONE task handled
    let task_handle = session_task.lock().await.take();
    
    if let Some(handle) = task_handle {
        // Only waits for ONE task!
        let _ = shutdown_tx.send(true);
        match tokio::time::timeout(std::time::Duration::from_secs(10), handle).await {
            Ok(Ok(())) => info!("Session task completed cleanly"),
            // ...
        }
    }
}
```

**Problem:**
- Only one task is waited on
- Other sessions' Flutter processes continue running
- `ps aux | grep flutter` shows orphaned processes after quit

---

### Implementation Details

#### 1. Update Cleanup to Handle All Sessions

```rust
// In src/tui/mod.rs - run_with_project cleanup section

// Run the main loop
let result = run_loop(
    &mut term,
    &mut state,
    // ... params ...
);

// Stop file watcher
file_watcher.stop();

// ─────────────────────────────────────────────────────────────────
// Multi-Session Cleanup
// ─────────────────────────────────────────────────────────────────

// Signal all background tasks to shut down
info!("Sending shutdown signal to all sessions...");
let _ = shutdown_tx.send(true);

// Draw shutdown message
state.log_info(LogSource::App, "Shutting down all Flutter sessions...");
let _ = term.draw(|frame| render::view(frame, &mut state));

// Collect all session tasks
let tasks: Vec<(SessionId, tokio::task::JoinHandle<()>)> = {
    let mut guard = session_tasks.lock().await;
    guard.drain().collect()
};

let task_count = tasks.len();
if task_count > 0 {
    info!("Waiting for {} session task(s) to complete...", task_count);
    
    // Wait for all tasks with individual timeouts
    for (session_id, handle) in tasks {
        match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
            Ok(Ok(())) => {
                info!("Session {} completed cleanly", session_id);
            }
            Ok(Err(e)) => {
                warn!("Session {} task panicked: {}", session_id, e);
            }
            Err(_) => {
                warn!("Timeout waiting for session {}, process may be orphaned", session_id);
            }
        }
    }
}

// Handle legacy single-session mode (auto-start with direct process ownership)
if let Some(mut p) = flutter {
    state.log_info(LogSource::App, "Shutting down legacy Flutter process...");
    let sender_guard = cmd_sender.lock().await;
    if let Err(e) = p
        .shutdown(state.current_app_id.as_deref(), sender_guard.as_ref())
        .await
    {
        error!("Error during Flutter shutdown: {}", e);
    }
}

// Restore terminal
ratatui::restore();

result
```

#### 2. Add Graceful Stop to Session Tasks

Each spawned session task should handle shutdown properly:

```rust
// In handle_action - SpawnSession, inside the spawned task
loop {
    tokio::select! {
        event = daemon_rx.recv() => {
            // ... event handling ...
        }
        _ = shutdown_rx_clone.changed() => {
            if *shutdown_rx_clone.borrow() {
                info!("Shutdown signal received for session {}", session_id);
                break;
            }
        }
    }
}

// Graceful shutdown when loop ends
info!("Session {} ending, initiating shutdown...", session_id);

// Stop the app first if running
if let Some(ref app_id) = app_id {
    let sender_guard = cmd_sender_clone.lock().await;
    if let Some(ref sender) = *sender_guard {
        info!("Sending stop command for app {}", app_id);
        let stop_result = sender.send(DaemonCommand::Stop { 
            app_id: app_id.clone() 
        }).await;
        if let Err(e) = stop_result {
            warn!("Failed to send stop command: {}", e);
        }
    }
}

// Shutdown the process
if let Err(e) = process
    .shutdown(app_id.as_deref(), sender_guard.as_ref())
    .await
{
    warn!("Shutdown error for session {}: {}", session_id, e);
}

// Remove from session tasks map
session_tasks_clone.lock().await.remove(&session_id);
```

#### 3. Add SessionManager Batch Methods (Optional Helper)

```rust
// In src/app/session_manager.rs
impl SessionManager {
    /// Get all session IDs with running apps
    pub fn running_session_ids(&self) -> Vec<SessionId> {
        self.sessions
            .iter()
            .filter(|(_, h)| h.session.is_running())
            .map(|(id, _)| *id)
            .collect()
    }
    
    /// Get count of running sessions
    pub fn running_count(&self) -> usize {
        self.sessions.values().filter(|h| h.session.is_running()).count()
    }
    
    /// Get all app_ids for running sessions
    pub fn running_app_ids(&self) -> Vec<String> {
        self.sessions
            .values()
            .filter_map(|h| h.session.app_id.clone())
            .collect()
    }
}
```

#### 4. Update Shutdown Progress Display

```rust
// In render.rs or a new shutdown indicator
// Show shutdown progress in the UI

fn render_shutdown_status(frame: &mut Frame, state: &AppState) {
    if state.phase == AppPhase::Quitting {
        let message = format!(
            "Shutting down {} session(s)...", 
            state.session_manager.running_count()
        );
        
        // Render centered message
        let area = frame.area();
        let popup = Paragraph::new(message)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Yellow));
        
        // Position at bottom of screen or as modal
        frame.render_widget(popup, area);
    }
}
```

---

### Shutdown Flow After Implementation

```
User confirms quit (Message::ConfirmQuit)
              │
              ▼
     state.phase = Quitting
              │
              ▼
     Main loop exits (should_quit() returns true)
              │
              ▼
     shutdown_tx.send(true)
              │
              ├──────────────────┬──────────────────┐
              ▼                  ▼                  ▼
       Session 1 Task      Session 2 Task    Session 3 Task
              │                  │                  │
              ▼                  ▼                  ▼
    shutdown_rx.changed()  shutdown_rx.changed()  shutdown_rx.changed()
              │                  │                  │
              ▼                  ▼                  ▼
    Send Stop command      Send Stop command    Send Stop command
              │                  │                  │
              ▼                  ▼                  ▼
    process.shutdown()     process.shutdown()   process.shutdown()
              │                  │                  │
              ▼                  ▼                  ▼
    Task completes         Task completes       Task completes
              │                  │                  │
              └──────────────────┴──────────────────┘
                                 │
                                 ▼
                    Main thread waits for all tasks
                                 │
                                 ▼
                    Terminal restored, app exits
                                 │
                                 ▼
                    No orphaned Flutter processes
```

---

### Acceptance Criteria

1. [ ] Shutdown signal is broadcast to all session tasks
2. [ ] Each session task handles shutdown signal and stops its app
3. [ ] Main thread waits for all session tasks with timeout
4. [ ] Per-session timeout prevents hanging on unresponsive processes
5. [ ] Logs indicate which sessions are shutting down
6. [ ] After quit, `ps aux | grep flutter` shows no orphaned processes
7. [ ] Legacy single-session mode still works
8. [ ] UI shows shutdown progress message

---

### Testing

```rust
#[tokio::test]
async fn test_shutdown_signal_broadcast() {
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    
    let received = Arc::new(AtomicBool::new(false));
    let received_clone = received.clone();
    
    // Spawn a task that waits for shutdown
    let handle = tokio::spawn(async move {
        shutdown_rx.changed().await.ok();
        received_clone.store(true, Ordering::SeqCst);
    });
    
    // Send shutdown signal
    shutdown_tx.send(true).unwrap();
    
    // Wait for task
    tokio::time::timeout(Duration::from_millis(100), handle)
        .await
        .unwrap()
        .unwrap();
    
    assert!(received.load(Ordering::SeqCst));
}

#[tokio::test]
async fn test_multiple_tasks_receive_shutdown() {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    
    let completed = Arc::new(AtomicU32::new(0));
    
    // Spawn 3 tasks
    let mut handles = Vec::new();
    for _ in 0..3 {
        let mut rx = shutdown_rx.clone();
        let completed = completed.clone();
        
        let handle = tokio::spawn(async move {
            rx.changed().await.ok();
            completed.fetch_add(1, Ordering::SeqCst);
        });
        handles.push(handle);
    }
    
    // Send shutdown
    shutdown_tx.send(true).unwrap();
    
    // Wait for all
    for handle in handles {
        tokio::time::timeout(Duration::from_millis(100), handle)
            .await
            .unwrap()
            .unwrap();
    }
    
    assert_eq!(completed.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_cleanup_with_timeout() {
    let session_tasks: Arc<Mutex<HashMap<SessionId, JoinHandle<()>>>> = 
        Arc::new(Mutex::new(HashMap::new()));
    
    // Add a task that takes too long
    let slow_handle = tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(60)).await;
    });
    session_tasks.lock().await.insert(1, slow_handle);
    
    // Add a task that completes quickly
    let fast_handle = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(10)).await;
    });
    session_tasks.lock().await.insert(2, fast_handle);
    
    // Drain and wait with timeout
    let tasks: Vec<_> = session_tasks.lock().await.drain().collect();
    
    let mut completed = 0;
    let mut timed_out = 0;
    
    for (session_id, handle) in tasks {
        match tokio::time::timeout(Duration::from_millis(100), handle).await {
            Ok(_) => completed += 1,
            Err(_) => timed_out += 1,
        }
    }
    
    // Fast task completed, slow task timed out
    assert_eq!(completed, 1);
    assert_eq!(timed_out, 1);
}

#[test]
fn test_running_session_ids() {
    let mut manager = SessionManager::new();
    
    let d1 = test_device("d1", "Device 1");
    let d2 = test_device("d2", "Device 2");
    let d3 = test_device("d3", "Device 3");
    
    let id1 = manager.create_session(&d1).unwrap();
    let id2 = manager.create_session(&d2).unwrap();
    let id3 = manager.create_session(&d3).unwrap();
    
    // Mark some as running
    manager.get_mut(id1).unwrap().session.mark_started("app-1".into());
    manager.get_mut(id3).unwrap().session.mark_started("app-3".into());
    
    let running = manager.running_session_ids();
    
    assert_eq!(running.len(), 2);
    assert!(running.contains(&id1));
    assert!(!running.contains(&id2));
    assert!(running.contains(&id3));
}

#[test]
fn test_running_count() {
    let mut manager = SessionManager::new();
    
    let device = test_device("d1", "Device 1");
    let id = manager.create_session(&device).unwrap();
    
    assert_eq!(manager.running_count(), 0);
    
    manager.get_mut(id).unwrap().session.mark_started("app-1".into());
    
    assert_eq!(manager.running_count(), 1);
}
```

---

### Integration Test (Manual)

```bash
# Start Flutter Demon
cd sample
cargo run

# Start multiple sessions:
# - Press 'n' to show device selector
# - Select first device (e.g., iPhone Simulator)
# - Wait for session to start
# - Press 'n' again
# - Select second device (e.g., Android Emulator)
# - Wait for session to start

# Verify both sessions running:
# - Tab bar shows both devices
# - `ps aux | grep flutter` shows 2 flutter processes

# Quit:
# - Press 'q'
# - Confirm with 'y' in dialog

# Verify cleanup:
ps aux | grep flutter
# Should show NO flutter processes

# Alternative force quit test:
# - Start sessions as above
# - Press Ctrl+C (force quit)
# - Verify `ps aux | grep flutter` shows no orphans
```

---

### Notes

- Individual task timeouts (5s each) are shorter than the old single-task timeout (10s)
- If a process is unresponsive, we log a warning but continue cleanup
- The shutdown_tx is a `watch` channel - all receivers get the signal
- Legacy mode (auto-start with direct process ownership) is still supported
- Consider adding a kill fallback for truly stuck processes
- The UI should show which sessions are being shut down (nice-to-have)