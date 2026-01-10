## Task: Add Auto-Launch Message Variants

**Objective**: Add three new message variants to support the auto-launch flow through the TEA message loop.

**Depends on**: None

**Estimated Time**: 0.5 hours

### Scope

- `src/app/message.rs`: Add new message variants

### Details

Add the following message variants to the `Message` enum:

```rust
/// Trigger auto-launch flow from Normal mode
/// Sent by runner after first render when auto_start=true
StartAutoLaunch {
    /// Pre-loaded configs to avoid re-loading in handler
    configs: crate::config::LoadedConfigs,
},

/// Update loading screen message during auto-launch
/// Sent by auto-launch task during device discovery
AutoLaunchProgress {
    /// Message to display on loading screen
    message: String,
},

/// Report auto-launch result (success or failure)
/// Sent by auto-launch task when device discovery completes
AutoLaunchResult {
    /// Ok: device and optional config to launch with
    /// Err: error message to display in StartupDialog
    result: Result<AutoLaunchSuccess, String>,
},
```

Also add a supporting struct for the success case:

```rust
/// Successful auto-launch discovery result
#[derive(Debug, Clone)]
pub struct AutoLaunchSuccess {
    /// Device to launch on
    pub device: crate::daemon::Device,
    /// Optional launch config (None = bare flutter run)
    pub config: Option<crate::config::LaunchConfig>,
}
```

### Import Requirements

The `Message` enum will need these imports (if not already present):
- `crate::config::LoadedConfigs`
- `crate::config::LaunchConfig`
- `crate::daemon::Device`

### Acceptance Criteria

1. `Message::StartAutoLaunch` variant exists with `configs` field
2. `Message::AutoLaunchProgress` variant exists with `message` field
3. `Message::AutoLaunchResult` variant exists with `result` field
4. `AutoLaunchSuccess` struct is defined and derives `Debug, Clone`
5. All variants are documented with `///` comments
6. `cargo check` passes
7. `cargo clippy -- -D warnings` passes

### Testing

No unit tests needed for this task (message enum doesn't have logic).
Compilation verification is sufficient.

```bash
cargo check
cargo clippy -- -D warnings
```

### Notes

- The `LoadedConfigs` type is already used elsewhere in the codebase
- Keep the message variants grouped with other startup-related messages
- The `AutoLaunchSuccess` struct could be placed in `message.rs` or a separate types module

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (pending)

**Implementation Details:**

(pending)

**Testing Performed:**
- (pending)

**Notable Decisions:**
- (pending)

**Risks/Limitations:**
- (pending)
