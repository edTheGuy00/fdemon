## Task: Move entry point discovery to async task

**Objective**: Move `discover_entry_points()` from synchronous handler to async task to prevent UI freezing on large projects.

**Depends on**: Task 08 (file size guard should be in place first)

### Scope

- `src/app/handler/mod.rs`: Add `UpdateAction::DiscoverEntryPoints` variant
- `src/app/message.rs`: Add `EntryPointsDiscovered` message
- `src/tui/spawn.rs`: Add `spawn_entry_point_discovery()` function
- `src/tui/actions.rs`: Handle `DiscoverEntryPoints` action
- `src/app/handler/new_session/fuzzy_modal.rs`: Refactor to use async pattern
- `src/app/handler/update.rs`: Handle `EntryPointsDiscovered` message
- `src/app/new_session_dialog/types.rs`: Add loading flag

### Details

The current implementation at `fuzzy_modal.rs:40-42` performs synchronous filesystem I/O:

```rust
// CURRENT (blocking):
FuzzyModalType::EntryPoint => {
    let entry_points = discover_entry_points(&state.project_path); // BLOCKING I/O
    // ...
}
```

This should be refactored to follow the device discovery pattern:

#### 1. Add UpdateAction variant

```rust
// In handler/mod.rs
pub enum UpdateAction {
    // ... existing variants ...

    /// Discover entry points in background
    DiscoverEntryPoints { project_path: PathBuf },
}
```

#### 2. Add Message variants

```rust
// In message.rs
pub enum Message {
    // ... existing variants ...

    /// Entry point discovery completed
    EntryPointsDiscovered { entry_points: Vec<PathBuf> },
}
```

#### 3. Add loading flag to state

```rust
// In new_session_dialog/types.rs (LaunchContextState)
pub struct LaunchContextState {
    // ... existing fields ...

    /// True while discovering entry points
    pub entry_points_loading: bool,
}
```

#### 4. Add spawn function

```rust
// In tui/spawn.rs
pub fn spawn_entry_point_discovery(
    msg_tx: mpsc::Sender<Message>,
    project_path: PathBuf,
) {
    tokio::spawn(async move {
        // Use spawn_blocking since discover_entry_points is sync I/O
        let entry_points = tokio::task::spawn_blocking(move || {
            crate::core::discovery::discover_entry_points(&project_path)
        })
        .await
        .unwrap_or_default();

        let _ = msg_tx
            .send(Message::EntryPointsDiscovered { entry_points })
            .await;
    });
}
```

#### 5. Handle action dispatch

```rust
// In tui/actions.rs
UpdateAction::DiscoverEntryPoints { project_path } => {
    spawn::spawn_entry_point_discovery(msg_tx, project_path);
}
```

#### 6. Refactor handler to start async

```rust
// In handler/new_session/fuzzy_modal.rs
FuzzyModalType::EntryPoint => {
    // Set loading state
    state.new_session_dialog_state.launch_context.entry_points_loading = true;

    // Open modal with placeholder (will be populated when discovery completes)
    use crate::app::new_session_dialog::FuzzyModalState;
    state.new_session_dialog_state.fuzzy_modal = Some(FuzzyModalState::new(
        FuzzyModalType::EntryPoint,
        vec!["(discovering...)".to_string()],
    ));

    // Return action to spawn async discovery
    return UpdateResult::action(UpdateAction::DiscoverEntryPoints {
        project_path: state.project_path.clone(),
    });
}
```

#### 7. Handle completion message

```rust
// In handler/update.rs
Message::EntryPointsDiscovered { entry_points } => {
    // Clear loading flag
    state.new_session_dialog_state.launch_context.entry_points_loading = false;

    // Cache discovered entry points
    state.new_session_dialog_state.launch_context
        .set_available_entry_points(entry_points);

    // Update modal if open
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        if modal.modal_type == FuzzyModalType::EntryPoint {
            let items = state.new_session_dialog_state.launch_context
                .entry_point_modal_items();
            modal.items = items.clone();
            modal.update_filter(items);
        }
    }

    UpdateResult::none()
}
```

### Acceptance Criteria

1. `UpdateAction::DiscoverEntryPoints` variant added
2. `Message::EntryPointsDiscovered` variant added
3. `spawn_entry_point_discovery()` function added
4. Entry point modal opens immediately (doesn't block)
5. Modal updates when discovery completes
6. Loading flag set during discovery
7. UI remains responsive during discovery
8. Large projects (500+ files) don't freeze UI
9. All existing tests pass
10. Code compiles without warnings

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_point_modal_returns_action() {
        let mut state = create_test_state();

        let result = handle_open_fuzzy_modal(&mut state, FuzzyModalType::EntryPoint);

        // Should return action to spawn discovery
        assert!(matches!(
            result.action,
            Some(UpdateAction::DiscoverEntryPoints { .. })
        ));

        // Loading flag should be set
        assert!(state.new_session_dialog_state.launch_context.entry_points_loading);
    }

    #[test]
    fn test_entry_points_discovered_updates_modal() {
        let mut state = create_test_state();

        // Simulate modal open with placeholder
        state.new_session_dialog_state.fuzzy_modal = Some(FuzzyModalState::new(
            FuzzyModalType::EntryPoint,
            vec!["(discovering...)".to_string()],
        ));
        state.new_session_dialog_state.launch_context.entry_points_loading = true;

        // Simulate discovery completion
        let entry_points = vec![
            PathBuf::from("lib/main.dart"),
            PathBuf::from("lib/main_dev.dart"),
        ];
        let result = update(&mut state, Message::EntryPointsDiscovered { entry_points });

        // Loading flag should be cleared
        assert!(!state.new_session_dialog_state.launch_context.entry_points_loading);

        // Modal should have items (including "(default)")
        let modal = state.new_session_dialog_state.fuzzy_modal.as_ref().unwrap();
        assert!(modal.items.contains(&"(default)".to_string()));
        assert!(modal.items.iter().any(|i| i.contains("main.dart")));
    }
}
```

### Notes

- Uses `spawn_blocking` because `discover_entry_points` is sync I/O
- The "(discovering...)" placeholder provides immediate feedback
- Modal can be closed before discovery completes (no special handling needed)
- Pattern matches device discovery flow exactly
- Discovery results are cached in `available_entry_points` for reuse
