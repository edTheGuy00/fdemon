# Task: Fix Loading Screen Animation

**Objective**: Make the loading screen spinner animate during auto_start initialization.

**Depends on**: None

## Problem

Loading screen appears but spinner is frozen. User sees:
```
⠋ Loading...
```
...but it never changes to ⠙, ⠹, etc.

**Root Cause**: The render loop (`run_loop`) starts AFTER `startup_flutter()` completes:

```rust
// runner.rs
if settings.behavior.auto_start {
    state.set_loading_phase("Initializing...");
    let _ = term.draw(|frame| render::view(frame, &mut state));  // One frame
}

// BLOCKING CALL - no ticks happen here
let startup_action = startup::startup_flutter(...).await;

// Loop starts here - too late!
run_loop(...);
```

`Message::Tick` only processes in `run_loop`, which hasn't started during async device discovery.

## Scope

- `src/tui/runner.rs` - Restructure to enable animation during startup

## Implementation Options

### Option A: Periodic re-render during async phases (Simpler)

Modify `startup.rs` to accept terminal reference and periodically render:

```rust
// In startup.rs
pub async fn startup_flutter(
    state: &mut AppState,
    settings: &config::Settings,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
    term: &mut ratatui::DefaultTerminal,  // Add terminal ref
) -> Option<UpdateAction> {
    if settings.behavior.auto_start {
        auto_start_session_with_animation(state, &configs, project_path, msg_tx, term).await
    } else {
        show_startup_dialog(state, configs, msg_tx)
    }
}

async fn auto_start_session_with_animation(
    state: &mut AppState,
    configs: &LoadedConfigs,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
    term: &mut ratatui::DefaultTerminal,
) -> Option<UpdateAction> {
    // Spawn animation task
    let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();

    // Can't easily share state, so just do periodic renders inline
    // Use select! to animate while waiting for discovery

    state.update_loading_message("Detecting devices...");

    let discovery_future = devices::discover_devices();
    tokio::pin!(discovery_future);

    let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));

    loop {
        tokio::select! {
            result = &mut discovery_future => {
                match result {
                    Ok(discovery_result) => {
                        state.set_device_cache(discovery_result.devices.clone());
                        // Continue with rest of startup logic...
                        break;
                    }
                    Err(e) => {
                        // Handle error...
                        break;
                    }
                }
            }
            _ = interval.tick() => {
                state.tick_loading_animation();
                let _ = term.draw(|frame| super::render::view(frame, state));
            }
        }
    }

    // ... rest of logic
}
```

### Option B: Non-blocking startup (Task 08d Option B - More Complex)

Restructure to spawn startup as background task and enter run_loop immediately:

```rust
// In runner.rs
if settings.behavior.auto_start {
    state.set_loading_phase("Initializing...");
    // Spawn non-blocking startup
    spawn_startup_flutter(state, settings, project_path, msg_tx.clone());
} else {
    // Show dialog normally
}

// Enter run_loop immediately - it will receive:
// - Message::LoadingPhaseChanged("Detecting devices...")
// - Message::StartupComplete { action }
run_loop(...);
```

This requires new messages and more refactoring.

### Recommended: Option A (Inline Animation)

Option A is simpler and localizes changes to startup.rs. It's the pragmatic fix.

## Implementation (Option A Detail)

### 1. Update startup_flutter signature

```rust
pub async fn startup_flutter(
    state: &mut AppState,
    settings: &config::Settings,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
    term: &mut ratatui::DefaultTerminal,
) -> Option<UpdateAction>
```

### 2. Update caller in runner.rs

```rust
let startup_action =
    startup::startup_flutter(&mut state, &settings, project_path, msg_tx.clone(), &mut term).await;
```

### 3. Implement animated discovery in startup.rs

```rust
use tokio::time::{interval, Duration};
use super::render;

async fn auto_start_session(
    state: &mut AppState,
    configs: &LoadedConfigs,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
    term: &mut ratatui::DefaultTerminal,
) -> Option<UpdateAction> {
    state.ui_mode = UiMode::Loading;

    // Helper to animate during an async operation
    async fn animate_during<T, F: std::future::Future<Output = T>>(
        state: &mut AppState,
        term: &mut ratatui::DefaultTerminal,
        future: F,
    ) -> T {
        tokio::pin!(future);
        let mut tick_interval = interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                result = &mut future => {
                    return result;
                }
                _ = tick_interval.tick() => {
                    state.tick_loading_animation();
                    let _ = term.draw(|frame| render::view(frame, state));
                }
            }
        }
    }

    state.update_loading_message("Detecting devices...");

    let discovery_result = animate_during(
        state,
        term,
        devices::discover_devices()
    ).await;

    match discovery_result {
        Ok(result) => {
            state.set_device_cache(result.devices.clone());
            state.update_loading_message("Preparing launch...");
            // ... continue with config matching logic
        }
        Err(e) => {
            // Handle error
        }
    }

    // ... rest of function
}
```

### 4. Handle borrow checker issues

The `animate_during` helper takes mutable borrows. May need to restructure to avoid simultaneous borrows:

```rust
// Alternative: inline the select! at each await point
state.update_loading_message("Detecting devices...");

let discovery = devices::discover_devices();
tokio::pin!(discovery);
let mut tick_interval = interval(Duration::from_millis(100));

let result = loop {
    tokio::select! {
        biased;  // Prioritize completion over animation
        result = &mut discovery => break result,
        _ = tick_interval.tick() => {
            state.tick_loading_animation();
            let _ = term.draw(|frame| render::view(frame, state));
        }
    }
};
```

## Acceptance Criteria

1. Loading spinner animates (cycles through ⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏)
2. Loading message updates ("Detecting devices...", "Preparing launch...")
3. Animation runs at ~10fps (100ms interval)
4. No visual glitches or flicker
5. Works correctly when devices are found quickly
6. Works correctly when device discovery is slow

## Testing

### Manual Test

1. Set `auto_start=true` in config
2. Start fdemon
3. Observe loading screen - spinner should animate
4. Message should change from "Initializing..." to "Detecting devices..."

### Automated

```rust
#[tokio::test]
async fn test_loading_animation_ticks() {
    let mut state = AppState::new();
    state.set_loading_phase("Test");

    let frame0 = state.loading_state.as_ref().unwrap().animation_frame;
    state.tick_loading_animation();
    let frame1 = state.loading_state.as_ref().unwrap().animation_frame;

    assert!(frame1 > frame0);
}
```

## Notes

- 100ms tick rate = 10fps animation, good balance of smoothness vs CPU
- `biased` in select! ensures we don't miss completion
- Terminal draw is cheap when nothing changed visually
- Consider debouncing if draw becomes expensive

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/startup.rs` | Added `animate_during_async()` helper function; updated `startup_flutter()` and `auto_start_session()` signatures to accept terminal reference; integrated animation during device discovery using `tokio::select!` |
| `src/tui/runner.rs` | Updated `startup_flutter()` call to pass `&mut term` reference |

### Notable Decisions/Tradeoffs

1. **Option A - Inline Animation**: Chose to implement inline animation using `tokio::select!` rather than Option B (non-blocking startup) as it is simpler and localizes changes to startup.rs without requiring new message types or major refactoring.

2. **Generic Helper Function**: Created `animate_during_async<T, F>()` as a reusable generic helper that can wrap any future with loading animation. This makes it easy to add animation to other async operations in the future.

3. **Biased Select**: Used `biased` flag in `tokio::select!` to prioritize completion over animation, ensuring device discovery completes as soon as possible rather than potentially delaying by up to 100ms.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test` - Passed (all 1200 tests)
- `cargo clippy -- -D warnings` - Passed
- `cargo build` - Passed

### Risks/Limitations

1. **Animation Frame Rate**: 100ms interval (10fps) provides smooth enough animation for a loading spinner without excessive CPU usage. If device discovery completes very quickly (< 100ms), user may only see 1-2 animation frames.

2. **Terminal Draw Performance**: Each animation tick calls `term.draw()`. Ratatui is efficient with minimal redraws, but this does add overhead during the async operation. Performance should be acceptable given the 100ms interval.

3. **Manual Testing Required**: Automated tests verify compilation and existing functionality, but manual testing is needed to verify the visual animation behavior under different device discovery scenarios (fast vs slow).
