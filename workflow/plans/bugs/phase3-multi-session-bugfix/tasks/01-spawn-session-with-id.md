## Task: Add SessionId to SpawnSession Action

**Objective**: Extend the `UpdateAction::SpawnSession` variant to include a `SessionId` so that the spawned background task knows which session it's spawning for, enabling proper session-to-process mapping.

**Depends on**: None

---

### Scope

- `src/app/handler.rs`: Update `UpdateAction::SpawnSession` enum variant
- `src/app/session.rs`: Export `SessionId` type for use in handler
- `src/tui/mod.rs`: Update `handle_action` match arm to use the session_id

---

### Current State

```rust
// In src/app/handler.rs
pub enum UpdateAction {
    // ...
    SpawnSession {
        device: Device,
        config: Option<Box<LaunchConfig>>,
    },
}
```

The `SpawnSession` action has no way to associate the spawned process with a specific session in the `SessionManager`.

---

### Implementation Details

#### 1. Update UpdateAction::SpawnSession

```rust
// In src/app/handler.rs
use super::session::SessionId;

pub enum UpdateAction {
    // ... other variants unchanged ...
    
    /// Spawn a new session for a device
    SpawnSession {
        /// The session ID in SessionManager (already created)
        session_id: SessionId,
        /// The device to run on
        device: Device,
        /// Optional launch configuration
        config: Option<Box<LaunchConfig>>,
    },
}
```

#### 2. Update Existing SpawnSession Usage

Find all places that create `UpdateAction::SpawnSession` and add the session_id parameter:

```rust
// In DeviceSelected handler (will be updated in Task 02)
// For now, create a placeholder that will be replaced

// TEMPORARY - Task 02 will properly create session first
UpdateAction::SpawnSession {
    session_id: 0, // Placeholder - will be proper ID from Task 02
    device,
    config: None,
}
```

#### 3. Update handle_action in tui/mod.rs

```rust
// In handle_action function
UpdateAction::SpawnSession { session_id, device, config } => {
    let project_path = project_path.to_path_buf();
    let msg_tx_clone = msg_tx.clone();
    let session_id = session_id; // Capture the session_id for the spawned task
    // ... rest of spawn logic uses session_id
}
```

---

### Acceptance Criteria

1. [ ] `UpdateAction::SpawnSession` includes `session_id: SessionId` field
2. [ ] `SessionId` is properly imported in `handler.rs`
3. [ ] `handle_action` receives and can use the `session_id`
4. [ ] All existing code compiles (may use placeholder `0` for session_id temporarily)
5. [ ] No behavior change yet (this is infrastructure for Task 02)

---

### Testing

```rust
#[test]
fn test_spawn_session_action_has_session_id() {
    let device = Device {
        id: "test-device".to_string(),
        name: "Test Device".to_string(),
        platform: "ios".to_string(),
        emulator: false,
        category: None,
        platform_type: None,
        ephemeral: false,
        emulator_id: None,
    };
    
    let action = UpdateAction::SpawnSession {
        session_id: 42,
        device,
        config: None,
    };
    
    match action {
        UpdateAction::SpawnSession { session_id, .. } => {
            assert_eq!(session_id, 42);
        }
        _ => panic!("Expected SpawnSession variant"),
    }
}
```

---

### Notes

- `SessionId` is already defined in `src/app/session.rs` as `pub type SessionId = u64;`
- This task is foundational - the session_id won't be meaningfully used until Task 02
- Keep backward compatibility by using temporary placeholder values where needed
- The actual session creation happens in Task 02

---

## Completion Summary

**Status:** ✅ Done

**Files Modified:**
- `src/app/handler.rs`:
  - Added `use super::session::SessionId;` import (line 4)
  - Extended `UpdateAction::SpawnSession` with `session_id: SessionId` field (lines 30-37)
  - Updated `DeviceSelected` handler to pass placeholder `session_id: 0` (lines 290-294)
- `src/tui/mod.rs`:
  - Updated `handle_action` match arm to destructure `session_id` (lines 525-540)
  - Captured `session_id` as `_session_id` for future use in Task 02+

**Notable Decisions/Tradeoffs:**
- Used placeholder value `0` for `session_id` in `DeviceSelected` handler as specified - Task 02 will properly create the session before spawning
- Prefixed captured `session_id` with underscore (`_session_id`) in `tui/mod.rs` to suppress unused variable warning until Task 02 uses it
- Added documentation comments to the new `SpawnSession` fields for clarity

**Testing Performed:**
- `cargo check` - Passed (no compilation errors)
- `cargo test` - All 391 tests passed
- `cargo fmt` - Code formatted
- `cargo clippy` - Only pre-existing warning about `run_loop` having too many arguments (unrelated to this task)

**Risks/Limitations:**
- The `session_id` is currently a placeholder (0) and has no effect - this is by design
- No behavior change in this task - purely infrastructure for Task 02

**Acceptance Criteria Status:**
1. [x] `UpdateAction::SpawnSession` includes `session_id: SessionId` field
2. [x] `SessionId` is properly imported in `handler.rs`
3. [x] `handle_action` receives and can use the `session_id`
4. [x] All existing code compiles (using placeholder `0` for session_id)
5. [x] No behavior change (this is infrastructure for Task 02)