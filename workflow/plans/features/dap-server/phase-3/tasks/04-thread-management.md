## Task: Thread Management (Isolate-to-Thread Mapping)

**Objective**: Implement the `threads` request handler and thread lifecycle management. Map Dart isolates to DAP thread IDs, handle `attach` flow to connect to the VM Service, and emit `thread` events on isolate start/exit.

**Depends on**: 03-adapter-core-structure

**Estimated Time**: 3-4 hours

### Scope

- `crates/fdemon-dap/src/adapter/threads.rs` — **NEW** ThreadMap implementation, threads handler
- `crates/fdemon-dap/src/adapter/mod.rs` — Wire `handle_attach` and `handle_threads` to dispatch

### Details

#### Thread ID Mapping

Dart VM Service uses string isolate IDs (`"isolates/1234567890"`). DAP requires integer thread IDs. The `ThreadMap` (defined in Task 03) provides this mapping.

```rust
// crates/fdemon-dap/src/adapter/threads.rs

impl ThreadMap {
    pub fn new() -> Self {
        Self {
            isolate_to_thread: HashMap::new(),
            thread_to_isolate: HashMap::new(),
            next_id: 1,
        }
    }

    /// Get or create a thread ID for the given isolate.
    pub fn get_or_create(&mut self, isolate_id: &str) -> i64 {
        if let Some(&id) = self.isolate_to_thread.get(isolate_id) {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.isolate_to_thread.insert(isolate_id.to_string(), id);
        self.thread_to_isolate.insert(id, isolate_id.to_string());
        id
    }

    /// Look up isolate ID from thread ID.
    pub fn isolate_id(&self, thread_id: i64) -> Option<&str> {
        self.thread_to_isolate.get(&thread_id).map(|s| s.as_str())
    }

    /// Look up thread ID from isolate ID.
    pub fn thread_id(&self, isolate_id: &str) -> Option<i64> {
        self.isolate_to_thread.get(isolate_id).copied()
    }

    /// Remove an isolate (on exit).
    pub fn remove(&mut self, isolate_id: &str) -> Option<i64> {
        if let Some(id) = self.isolate_to_thread.remove(isolate_id) {
            self.thread_to_isolate.remove(&id);
            Some(id)
        } else {
            None
        }
    }

    /// Get all current threads as DAP Thread objects.
    pub fn all_threads(&self) -> Vec<(i64, String)> {
        self.thread_to_isolate
            .iter()
            .map(|(&id, isolate_id)| (id, isolate_id.clone()))
            .collect()
    }
}
```

#### `threads` Request Handler

```rust
impl<B: DebugBackend> DapAdapter<B> {
    pub async fn handle_threads(&self, request: &DapRequest) -> DapResponse {
        // Return all known threads from the thread map.
        // Thread names come from isolate names (e.g., "main", "background").
        let threads: Vec<DapThread> = self.thread_map.all_threads()
            .into_iter()
            .map(|(id, _isolate_id)| DapThread {
                id,
                name: self.thread_names.get(&id).cloned()
                    .unwrap_or_else(|| format!("Thread {}", id)),
            })
            .collect();

        let body = serde_json::json!({ "threads": threads });
        DapResponse::success(request, Some(body))
    }
}
```

#### `attach` Request Handler

The `attach` handler replaces the Phase 2 stub. It:
1. Parses `AttachRequestArguments` from the request
2. Signals the adapter to connect to the target session
3. Discovers existing isolates via `getVM()`
4. Subscribes to Debug and Isolate streams (handled by the backend)
5. Populates the thread map with existing isolates
6. Emits `thread` events for each discovered isolate

```rust
pub async fn handle_attach(&mut self, request: &DapRequest) -> DapResponse {
    // Parse attach arguments
    let args: AttachRequestArguments = match &request.arguments {
        Some(v) => serde_json::from_value(v.clone()).unwrap_or_default(),
        None => AttachRequestArguments::default(),
    };

    // Get VM info to discover existing isolates
    match self.backend.get_vm().await {
        Ok(vm_info) => {
            // Extract isolates from VM info
            if let Some(isolates) = vm_info.get("isolates").and_then(|v| v.as_array()) {
                for isolate in isolates {
                    let id = isolate.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let name = isolate.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");

                    if !id.is_empty() {
                        let thread_id = self.thread_map.get_or_create(id);
                        self.thread_names.insert(thread_id, name.to_string());

                        // Emit thread started event
                        let event = DapEvent::thread("started", thread_id);
                        let _ = self.event_tx.send(DapMessage::Event(event)).await;
                    }
                }
            }
            DapResponse::success(request, None)
        }
        Err(e) => DapResponse::error(request, format!("Failed to attach: {}", e)),
    }
}
```

#### Debug Event Handling for Threads

```rust
pub async fn handle_debug_event(&mut self, event: DebugEvent) {
    match event {
        DebugEvent::IsolateStart { isolate_id, name } => {
            let thread_id = self.thread_map.get_or_create(&isolate_id);
            self.thread_names.insert(thread_id, name);
            let event = DapEvent::thread("started", thread_id);
            let _ = self.event_tx.send(DapMessage::Event(event)).await;
        }
        DebugEvent::IsolateExit { isolate_id } => {
            if let Some(thread_id) = self.thread_map.remove(&isolate_id) {
                self.thread_names.remove(&thread_id);
                let event = DapEvent::thread("exited", thread_id);
                let _ = self.event_tx.send(DapMessage::Event(event)).await;
            }
        }
        // Other events handled by other modules...
        _ => {}
    }
}
```

### Acceptance Criteria

1. `ThreadMap` correctly maps isolate IDs to monotonic DAP thread IDs
2. `threads` request returns all known threads with correct names
3. `attach` request discovers existing isolates via `getVM()` and populates the thread map
4. `IsolateStart` events add threads and emit `thread` started events
5. `IsolateExit` events remove threads and emit `thread` exited events
6. Thread names default to `"Thread N"` when the isolate name is unknown
7. Thread IDs are never reused (monotonically increasing even after removal)
8. Unit tests cover mapping, lookup, removal, and event emission

### Testing

```rust
#[test]
fn test_thread_map_monotonic_after_removal() {
    let mut map = ThreadMap::new();
    let id1 = map.get_or_create("isolates/1");
    map.remove("isolates/1");
    let id2 = map.get_or_create("isolates/2");
    assert!(id2 > id1, "IDs must be monotonic even after removal");
}

#[test]
fn test_thread_map_all_threads() {
    let mut map = ThreadMap::new();
    map.get_or_create("isolates/1");
    map.get_or_create("isolates/2");
    let threads = map.all_threads();
    assert_eq!(threads.len(), 2);
}

#[test]
fn test_thread_map_remove_returns_thread_id() {
    let mut map = ThreadMap::new();
    let id = map.get_or_create("isolates/1");
    let removed = map.remove("isolates/1");
    assert_eq!(removed, Some(id));
}

#[test]
fn test_thread_map_remove_unknown_returns_none() {
    let mut map = ThreadMap::new();
    assert_eq!(map.remove("isolates/99"), None);
}
```

### Notes

- Thread IDs start at 1 (DAP convention — 0 is often invalid)
- Isolate names may be absent or generic (`"main()"`, `"isolates/12345"`). Provide sensible defaults.
- The `thread_names` map is separate from `ThreadMap` to keep the mapping logic pure
- Helix sends `supportsVariableType: true` — thread responses don't use this but it validates the init args work correctly
- In Phase 4, multi-session support will namespace thread IDs (session 0 → 1000-1999, etc.). For now, all isolates share a single namespace.

---

## Completion Summary

**Status:** Not Started
