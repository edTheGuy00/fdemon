# Bug: Settings Page Not Appearing in PTY E2E Tests

## Problem Statement

All 16 settings page E2E tests are marked with `#[ignore]` because the settings page doesn't appear when the comma key is pressed in the PTY test environment. The tests time out waiting for the "Settings" text to appear.

**Test file:** `tests/e2e/settings_page.rs`
**Error:** `ExpectTimeout` when calling `session.expect("Settings")`

## Root Cause Analysis

### Investigation Summary

The comma key (`,`) triggers `Message::ShowSettings`, which transitions the app to `UiMode::Settings`. However, this only works when the app is in `UiMode::Normal`.

**Code flow:**
```
Comma key pressed
    → handle_key() (keys.rs:8-20)
        → Dispatches based on ui_mode
            → Normal: handle_key_normal() → Message::ShowSettings ✅
            → DeviceSelector: handle_key_device_selector() → None (ignored) ❌
            → Loading: handle_key_loading() → None (ignored) ❌
```

### Why Tests Fail

1. **Test fixture configuration:** `tests/fixtures/simple_app/.fdemon/config.toml` has `auto_start = false`
2. **App startup behavior:** With `auto_start = false`, the app shows the device selector immediately (`UiMode::DeviceSelector`)
3. **Key handling in DeviceSelector:** The comma key is not handled in `handle_key_device_selector()` - it falls through to `_ => None` (line 55)
4. **Test sequence:**
   - Test spawns fdemon → app enters `DeviceSelector` mode
   - Test calls `expect_header()` → succeeds (header is visible)
   - Test sends comma `,` → key is ignored (wrong UiMode)
   - Test times out waiting for "Settings" text

### Key Evidence

| Location | Finding |
|----------|---------|
| `keys.rs:23-56` | `handle_key_device_selector()` doesn't handle comma |
| `keys.rs:318` | Comma → ShowSettings only in `handle_key_normal()` |
| `config.toml:5` | `auto_start = false` in test fixture |
| `pty_utils.rs:220-223` | `expect_header()` only checks for "Flutter Demon" text |

### Why Unit Tests Pass

Unit tests in `keys.rs` call `handle_key_normal()` directly with `AppState::new()` which defaults to `UiMode::Normal`. They don't go through the real startup flow that results in `UiMode::DeviceSelector`.

## Solution Options

### Option A: Transition to Normal Mode Before Testing (Recommended)

Modify tests to first transition to `UiMode::Normal` by selecting a device or pressing Escape.

**Pros:**
- Tests real user flow more accurately
- No changes to source code
- Works with existing test infrastructure

**Cons:**
- Tests are slightly more complex
- Depends on device selector behavior

### Option B: Allow Settings Access from DeviceSelector Mode

Add comma key handling to `handle_key_device_selector()` to allow opening settings.

**Pros:**
- Settings accessible from more UI modes
- Simpler test code

**Cons:**
- Changes app behavior (may be desirable UX)
- Need to decide if settings should be accessible from all modes

### Chosen Approach: Option A + Option B (Both Required)

1. **Fix tests (Option A):** Add proper state transitions in E2E tests
2. **Enhance UX (Option B):** Allow settings access from DeviceSelector mode

Both options will be implemented to ensure tests work correctly AND provide better UX.

## Tasks

### Task 1: Fix E2E Test State Transitions

**File:** `tests/e2e/settings_page.rs`

Modify all 16 tests to properly transition to Normal mode before testing settings:

```rust
// Before (broken):
session.expect_header().expect("header");
open_settings(&mut session).await.expect("open settings");

// After (fixed):
session.expect_header().expect("header");
// Transition from DeviceSelector to Normal by pressing Escape
session.send_special(SpecialKey::Escape).expect("close device selector");
tokio::time::sleep(Duration::from_millis(INPUT_DELAY_MS)).await;
// Now we're in Normal mode
open_settings(&mut session).await.expect("open settings");
```

**Acceptance criteria:**
- [ ] All 16 tests transition to Normal mode before testing settings
- [ ] Remove `#[ignore]` from all tests
- [ ] Tests pass with `cargo nextest run --test e2e settings_page`

### Task 2: Allow Settings Access from DeviceSelector (Enhancement)

**File:** `src/app/handler/keys.rs`

Add comma key handling to `handle_key_device_selector()`:

```rust
fn handle_key_device_selector(state: &AppState, key: KeyEvent) -> Option<Message> {
    match key.code {
        // ... existing handlers ...

        // Settings (allows access from device selector)
        KeyCode::Char(',') => Some(Message::ShowSettings),

        _ => None,
    }
}
```

**Acceptance criteria:**
- [ ] Settings can be opened with comma from DeviceSelector mode
- [ ] Existing unit tests pass
- [ ] Add unit test for new behavior

### Task 3: Verify All Settings Tests Pass

Run full verification:
- [ ] `cargo test --test e2e settings_page` - All 16 tests pass
- [ ] `cargo test --test e2e` - No regressions in other tests
- [ ] `cargo clippy -- -D warnings` - No warnings

## Files to Modify

| File | Changes |
|------|---------|
| `tests/e2e/settings_page.rs` | Add state transitions, remove `#[ignore]` |
| `src/app/handler/keys.rs` | Add comma handler to device selector (optional) |

## Dependencies

None - this is a self-contained fix.

## Testing Strategy

1. **Before fix:** Run `cargo test --test e2e settings_page` - all 16 ignored
2. **After Task 1:** Tests should run and pass
3. **After Task 2:** Verify settings opens from device selector mode
4. **Final:** Full E2E suite passes with no regressions
