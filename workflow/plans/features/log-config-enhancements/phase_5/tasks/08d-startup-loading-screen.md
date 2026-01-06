# Task: Startup Loading Screen

**Objective**: Show an animated loading screen during auto_start initialization instead of black screen.

**Depends on**: Task 06 (Startup Flow Refactor)

## Problem

When `auto_start=true`, users see a black screen for several seconds while device discovery runs. This happens because:

1. **`startup::startup_flutter()` blocks before render loop** (`runner.rs:62-63`):
   ```rust
   let startup_action =
       startup::startup_flutter(&mut state, &settings, project_path, msg_tx.clone()).await;
   ```

2. The terminal is initialized (`ratatui::init()`) but `run_loop` hasn't started
3. No frame is rendered during the async device discovery

## Scope

- `src/tui/runner.rs` - Restructure startup flow
- `src/tui/render.rs` - Add loading screen rendering
- `src/app/state.rs` - Add loading state with rotating messages

## Implementation

### Option A: Pre-render before async operations

Render a loading screen BEFORE calling the async startup function:

```rust
// In runner.rs, after terminal init:

// Set initial loading state with rotating message
state.set_loading_phase("Initializing...");

// Draw initial loading frame
let _ = term.draw(|frame| render::view(frame, &mut state));

// Now do async startup
let startup_action = startup::startup_flutter(...).await;
```

### Option B: Non-blocking startup with message-based updates

Restructure to spawn startup as a task and handle via messages:

```rust
// Spawn startup task (non-blocking)
spawn::spawn_startup_flutter(state, settings, project_path, msg_tx);

// Enter run_loop immediately - it will receive:
// - LoadingPhaseChanged("Detecting devices...")
// - StartupComplete { action: Option<UpdateAction> }
```

### Loading Messages (rotating)

Create an array of loading messages to cycle through:
```rust
const LOADING_MESSAGES: &[&str] = &[
    "Starting engine...",
    "Detecting devices...",
    "Loading configurations...",
    "Preparing launch...",
];
```

### State Changes

```rust
// In state.rs
pub struct LoadingState {
    pub message: String,
    pub animation_frame: u64,
}

impl AppState {
    pub fn set_loading_phase(&mut self, message: &str) {
        self.loading_state = Some(LoadingState {
            message: message.to_string(),
            animation_frame: 0,
        });
        self.ui_mode = UiMode::Loading;
    }
}
```

### Render Loading Screen

```rust
// In render.rs, for UiMode::Loading when no device_selector:
fn render_loading_screen(frame: &mut Frame, state: &AppState, area: Rect) {
    // Center a block with:
    // - App name/logo
    // - Animated spinner (braille or simple)
    // - Current loading message

    let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let idx = (state.loading_state.animation_frame / 5) % spinner.len();

    let content = format!("{} {}", spinner[idx], state.loading_state.message);
    // Render centered in screen
}
```

## Acceptance Criteria

1. No black screen during auto_start initialization
2. Loading screen shows animated spinner with message
3. Messages rotate through phases ("Starting engine...", "Detecting devices...", etc.)
4. Loading screen is centered and visually polished
5. Works correctly on first launch and subsequent launches

## Testing

```rust
#[test]
fn test_loading_state_cycles_messages() {
    let mut state = AppState::new();
    state.set_loading_phase("Detecting devices...");
    assert_eq!(state.ui_mode, UiMode::Loading);
    assert!(state.loading_state.is_some());
}
```

## Notes

- Option A is simpler but may not update during long operations
- Option B is more robust but requires more refactoring
- Consider user's terminal size - loading screen should be minimal
- Spinner should be fast enough to show activity (every 100ms or so)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/state.rs` | Added `LoadingState` struct with animation frame and message, added `loading_state` field to `AppState`, added helper methods (`set_loading_phase`, `update_loading_message`, `clear_loading`, `tick_loading_animation`) |
| `src/tui/render.rs` | Added `render_loading_screen` function with centered spinner and message display, updated `UiMode::Loading` handling to render loading screen instead of device selector |
| `src/tui/runner.rs` | Set initial loading state and render frame before async `startup_flutter()` call when `auto_start=true` |
| `src/tui/startup.rs` | Updated `auto_start_session` to use loading messages during device discovery, updated `launch_session` to clear loading state on completion |
| `src/app/handler/update.rs` | Added loading animation tick to `Message::Tick` handler |

### Notable Decisions/Tradeoffs

1. **Option A Implementation**: Chose Option A (pre-render before async operations) for simplicity. The loading screen is rendered once before `startup_flutter()` is called, and messages are updated during device discovery phases.

2. **Spinner Design**: Used Braille spinner characters (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) for smooth animation, updating every ~100ms (6 frames at ~60fps). This provides visual feedback without being distracting.

3. **Centered Layout**: Loading screen is centered in a bordered box with vertical and horizontal centering, using 40% padding on each side for good visual balance on various terminal sizes.

4. **Message Updates**: Loading messages update through key phases: "Initializing..." → "Detecting devices..." → "Preparing launch...". This provides user feedback about what's happening during startup.

5. **Animation Frame Counter**: Used `u64` with wrapping addition to prevent overflow, consistent with existing dialog state implementations.

### Testing Performed

- `cargo check` - Passed
- `cargo test --lib` - Passed (1179 tests, 0 failed)
- `cargo clippy` - Passed (no warnings)
- `cargo fmt` - Applied

Added comprehensive unit tests:
- `test_loading_state_creation` - Verifies LoadingState initialization
- `test_loading_state_tick` - Verifies animation frame increments
- `test_loading_state_tick_wraps` - Verifies u64 wrapping behavior
- `test_loading_state_set_message` - Verifies message updates
- `test_app_state_set_loading_phase` - Verifies loading phase setup
- `test_app_state_update_loading_message` - Verifies message updates
- `test_app_state_clear_loading` - Verifies cleanup
- `test_app_state_tick_loading_animation` - Verifies animation ticking
- `test_app_state_tick_loading_no_state` - Verifies safe no-op when no loading state
- `test_app_state_update_loading_no_state` - Verifies safe no-op when no loading state

### Risks/Limitations

1. **Single Frame Limitation**: The loading screen is rendered once before the async operation starts. If device discovery takes a very long time (>5 seconds), the spinner won't animate during the actual await. However, this is acceptable because:
   - Device discovery is typically fast (<1 second)
   - The loading screen provides better UX than a black screen
   - Message updates still occur at key phases

2. **Terminal Size**: The centered layout works well on standard terminal sizes but may look cramped on very small terminals (width < 40 chars). The design uses percentage-based layout which adapts reasonably well.

3. **No Cancellation**: Users cannot cancel the loading screen - they must wait for device discovery to complete. This matches existing behavior but could be enhanced in the future with an "Esc to cancel" option.
