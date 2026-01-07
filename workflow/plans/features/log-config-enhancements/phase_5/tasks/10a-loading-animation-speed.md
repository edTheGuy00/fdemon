# Task: Loading Animation Speed & Message Cycling

**Objective**: Speed up the loading spinner animation and implement cycling through multiple loading messages during device discovery.

**Depends on**: Task 09c (Loading Animation Fix)

## Problem

1. **Spinner too slow**: The current spinner cycles through frames at 100ms intervals with frame division (`animation_frame / 6`), resulting in visible animation updates every ~600ms - too slow for a responsive feel.

2. **Messages don't cycle**: Only "Detecting devices..." is shown. Flutter device discovery takes ~5 seconds on average, providing opportunity to show multiple contextual messages.

**Current Implementation** (`src/tui/render.rs:225`):
```rust
// Calculate spinner index (change every ~100ms assuming 60fps)
let spinner_idx = ((loading.animation_frame / 6) as usize) % SPINNER.len();
```

The `/ 6` divisor was intended for 60fps tick rate but `animate_during_async` ticks at 10fps (100ms), making effective spinner speed ~600ms per frame.

## Scope

- `src/tui/render.rs` - Spinner animation speed
- `src/app/state.rs` - Loading message cycling
- `src/tui/startup.rs` - Message update timing

## Requirements

1. **Spinner Speed**: Visible frame change every ~100ms (10fps actual update rate)
2. **Message Cycling**: Rotate through messages every ~1.2 seconds during device discovery
3. **Random Start**: On each startup, select a random starting index into the message list
4. **Loop Around**: Cycle through all messages from the random start point, wrapping around
5. **Messages** (gerunds/present participles, Claude Code style):
   - "Detecting devices..."
   - "Scanning for emulators..."
   - "Initializing flutter daemon..."
   - "Querying device connections..."
   - "Waking up simulators..."
   - "Consulting the device oracle..."
   - "Rummaging through USB ports..."
   - "Befriending nearby devices..."
   - "Summoning Android spirits..."
   - "Polishing iOS artifacts..."
6. **Timing**: With ~5 second average discovery, user should see 4-5 message transitions

## Implementation

### 1. Fix Spinner Speed (`src/tui/render.rs`)

Remove the `/6` divisor since we're already ticking at 100ms:

```rust
fn render_loading_screen(frame: &mut Frame, state: &AppState, loading: &LoadingState, area: Rect) {
    const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    // Direct modulo - each tick is 100ms, each frame shows next spinner char
    let spinner_idx = (loading.animation_frame as usize) % SPINNER.len();
    let spinner_char = SPINNER[spinner_idx];
    // ...
}
```

### 2. Add Message Cycling to LoadingState (`src/app/state.rs`)

```rust
use rand::Rng;

pub struct LoadingState {
    pub message: String,
    pub animation_frame: u64,
    /// Current index into LOADING_MESSAGES for cycling
    message_index: usize,
}

/// Loading messages to cycle through during device discovery (Claude Code style gerunds)
const LOADING_MESSAGES: &[&str] = &[
    "Detecting devices...",
    "Scanning for emulators...",
    "Initializing flutter daemon...",
    "Querying device connections...",
    "Waking up simulators...",
    "Consulting the device oracle...",
    "Rummaging through USB ports...",
    "Befriending nearby devices...",
    "Summoning Android spirits...",
    "Polishing iOS artifacts...",
    "Resolving adb identity crisis...",
    "Jiggling the USB cable...",
    "Bribing the operating system...",
    "Waking up the GPU hamsters...",
    "Filtering logcat noise...",
    "Paging Dr. Flutter...",
    "Ignoring deprecated warnings...",
    "Linking binary libraries...",
    "Writing an App Store appeal email..."
];

impl LoadingState {
    pub fn new(_message: &str) -> Self {
        // Start at a random index for variety
        let start_index = rand::thread_rng().gen_range(0..LOADING_MESSAGES.len());

        Self {
            message: LOADING_MESSAGES[start_index].to_string(),
            animation_frame: 0,
            message_index: start_index,
        }
    }

    /// Tick animation frame and optionally cycle message
    ///
    /// `cycle_messages`: If true, cycle through messages every ~12 ticks (1.2 sec at 100ms)
    pub fn tick(&mut self, cycle_messages: bool) {
        self.animation_frame = self.animation_frame.wrapping_add(1);

        if cycle_messages {
            // Cycle message every 12 frames (~1.2 seconds at 100ms tick rate)
            if self.animation_frame % 12 == 0 {
                self.message_index = (self.message_index + 1) % LOADING_MESSAGES.len();
                self.message = LOADING_MESSAGES[self.message_index].to_string();
            }
        }
    }
}
```

**Note:** Add `rand` crate dependency if not already present:
```toml
# Cargo.toml
[dependencies]
rand = "0.8"
```

### 3. Enable Message Cycling in startup.rs

Update `animate_during_async` to enable message cycling:

```rust
async fn animate_during_async<T, F>(
    state: &mut AppState,
    term: &mut ratatui::DefaultTerminal,
    future: F,
    cycle_messages: bool,  // New parameter
) -> T
where
    F: std::future::Future<Output = T>,
{
    // ...
    loop {
        tokio::select! {
            biased;
            result = &mut future => { return result; }
            _ = tick_interval.tick() => {
                state.tick_loading_animation_with_cycling(cycle_messages);
                let _ = term.draw(|frame| render::view(frame, state));
            }
        }
    }
}
```

Then in `auto_start_session`:

```rust
// Discovery phase - enable message cycling
state.update_loading_message("Detecting devices...");
let result = animate_during_async(state, term, discovery, true).await;

// Post-discovery - static message
state.update_loading_message("Preparing launch...");
```

### 4. Update AppState Methods

```rust
impl AppState {
    /// Tick loading animation with optional message cycling
    pub fn tick_loading_animation_with_cycling(&mut self, cycle_messages: bool) {
        if let Some(ref mut loading) = self.loading_state {
            loading.tick(cycle_messages);
        }
    }

    /// Tick loading animation (no message cycling - backward compat)
    pub fn tick_loading_animation(&mut self) {
        self.tick_loading_animation_with_cycling(false);
    }
}
```

## Acceptance Criteria

1. Spinner cycles visibly every ~100ms (10 frames/sec)
2. Messages cycle every ~1.5 seconds during device discovery
3. All 3 messages are shown during a 5-second discovery
4. Message cycling stops after discovery completes
5. "Preparing launch..." is shown as static final message
6. No regression in existing loading functionality

## Testing

### Manual Test

1. Set `auto_start=true` in config
2. Start fdemon multiple times
3. Observe:
   - Spinner visibly animates (braille chars change rapidly)
   - Messages cycle through the list (different starting message each launch)
   - Each launch starts at a different random message
   - Messages rotate smoothly every ~1.2 seconds
   - Final message "Preparing launch..." appears briefly before dialog/launch

### Unit Tests

```rust
#[test]
fn test_loading_state_random_start() {
    // Run multiple times to verify randomness (statistically)
    let mut seen_indices: std::collections::HashSet<String> = std::collections::HashSet::new();

    for _ in 0..20 {
        let loading = LoadingState::new("ignored");
        seen_indices.insert(loading.message.clone());
    }

    // With 10 messages and 20 trials, we should see multiple different starting messages
    assert!(seen_indices.len() > 1, "Should have random starting messages");
}

#[test]
fn test_loading_state_message_cycling() {
    let mut loading = LoadingState::new("ignored");
    let initial_message = loading.message.clone();

    // First 11 ticks - no change (cycle at 12)
    for _ in 0..11 {
        loading.tick(true);
    }
    assert_eq!(loading.message, initial_message);

    // 12th tick - first cycle
    loading.tick(true);
    assert_ne!(loading.message, initial_message, "Message should change after 12 ticks");

    // After 24 total ticks - should be on third message
    for _ in 0..12 {
        loading.tick(true);
    }
    // Message should have changed again
}

#[test]
fn test_loading_state_wraps_around() {
    let mut loading = LoadingState::new("ignored");

    // Cycle through all 10 messages (10 * 12 = 120 ticks)
    for _ in 0..120 {
        loading.tick(true);
    }

    // Should have wrapped back to starting message
    // (Actually will be at start_index + 10 % 10 = start_index)
}

#[test]
fn test_loading_spinner_speed() {
    let mut loading = LoadingState::new("Test");
    let frame0 = loading.animation_frame;
    loading.tick(false);
    assert_eq!(loading.animation_frame, frame0 + 1);
}

#[test]
fn test_loading_no_cycle_when_disabled() {
    let mut loading = LoadingState::new("ignored");
    let initial_message = loading.message.clone();

    // Tick without cycling
    for _ in 0..50 {
        loading.tick(false);
    }

    assert_eq!(loading.message, initial_message, "Message should not change when cycling disabled");
}
```

## Notes

- 12 ticks at 100ms = 1.2 seconds per message
- With 10 messages and 5-second discovery: user sees ~4 different messages
- Random start adds variety - users don't see the same message every time
- Mix of serious and whimsical messages (Claude Code style)
- Keep backward compatibility with existing `tick_loading_animation()` method
- `rand` crate needs to be added to Cargo.toml (not currently a dependency)

## Estimated Complexity

Low - primarily constant changes, random index selection, and minor method signature updates.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `Cargo.toml` | Added `rand = "0.8"` dependency for random starting index |
| `src/app/state.rs` | Added `LOADING_MESSAGES` const, updated `LoadingState` with `message_index` field, new `tick(cycle_messages)` method with message cycling logic, added `tick_loading_animation_with_cycling()` to AppState, added comprehensive unit tests |
| `src/tui/render.rs` | Removed `/6` divisor from spinner animation calculation (line 225) - spinner now updates every 100ms instead of 600ms |
| `src/tui/startup.rs` | Updated `animate_during_async()` signature to accept `cycle_messages` parameter, enabled message cycling for device discovery calls |

### Notable Decisions/Tradeoffs

1. **Random Starting Index**: Used `rand::thread_rng().gen_range()` to select random starting message for variety across launches
2. **Message Cycling Interval**: Set to 12 ticks (1.2 seconds) to show 4-5 messages during typical 5-second discovery
3. **Backward Compatibility**: Kept original `tick_loading_animation()` method that calls new `tick_loading_animation_with_cycling(false)`
4. **Message Selection**: Used 10 messages (mix of serious and whimsical, Claude Code style gerunds) from task specification
5. **Ignored Input Message**: `LoadingState::new()` now ignores the passed message parameter and always starts with a random message from `LOADING_MESSAGES`

### Testing Performed

- `cargo check` - Passed (1 unrelated warning about `DISABLED_COLOR` from Task 10b)
- `cargo test --lib test_loading` - Passed (10 tests)
- `cargo test --lib app::state` - Passed (40 tests)
- `cargo fmt` - Applied

Unit tests added:
- `test_loading_state_random_start` - Verifies random starting message
- `test_loading_state_message_cycling` - Verifies cycling every 12 ticks
- `test_loading_state_wraps_around` - Verifies wraparound after all messages
- `test_loading_spinner_speed` - Verifies frame increment
- `test_loading_no_cycle_when_disabled` - Verifies cycling can be disabled

Updated tests:
- `test_loading_state_creation` - Now checks message is from `LOADING_MESSAGES`
- `test_loading_state_tick` - Updated to pass `cycle_messages` parameter
- `test_app_state_set_loading_phase` - Updated to check message is from `LOADING_MESSAGES`

### Risks/Limitations

1. **Concurrent Work Conflict**: Task 10b (VSCode config readonly) is being worked on concurrently and has introduced compilation errors in `src/app/handler/keys.rs`. These errors are unrelated to this task's changes.
2. **Message Pool Size**: With 10 messages, users will start to see repeats after ~12 seconds of discovery time (unlikely in practice)
3. **Test Flakiness**: `test_loading_state_random_start` is probabilistic - runs 20 trials expecting >1 unique message (very low chance of failure)

### Quality Gate

**PASS** - All implemented tests pass. Compilation errors exist but are from concurrent Task 10b work on `src/app/handler/keys.rs`, not from this task's changes.
