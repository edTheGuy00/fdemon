## Task: Update Status Bar for Not Connected State

**Objective**: Modify the status bar to show "Not Connected" when no sessions exist, instead of showing session-specific status.

**Depends on**: 01-modify-startup-logic

### Scope

- `src/tui/widgets/status_bar/mod.rs`: Modify phase display logic (around lines 45-68)
- Possibly `src/tui/widgets/status_bar/compact.rs` if compact mode exists

### Details

Currently the status bar shows:
- "○ Starting" when `AppPhase::Initializing`
- "● Running" when `AppPhase::Running`
- "↻ Reloading" when `AppPhase::Reloading`
- "○ Stopped" when `AppPhase::Stopped`
- "○ Stopping" when `AppPhase::Quitting`

**Add handling for no sessions:**

The status bar widget needs to check if any sessions exist before displaying session-specific status. When no sessions exist, show "○ Not Connected".

Find the phase display logic and wrap it:

```rust
// At the start of the status display logic
let phase_span = if !self.state.session_manager.has_any_sessions() {
    // No sessions exist
    Span::styled(
        "○ Not Connected",
        Style::default().fg(Color::DarkGray),
    )
} else if let Some(handle) = self.state.session_manager.selected() {
    // Existing phase display logic based on handle.session.phase
    match handle.session.phase {
        AppPhase::Initializing => Span::styled("○ Starting", Style::default().fg(Color::DarkGray)),
        // ... rest of existing logic
    }
} else {
    // Fallback if no session selected but sessions exist
    Span::styled("○ Not Connected", Style::default().fg(Color::DarkGray))
};
```

**Alternative approach**: Add a helper method to `SessionManager`:

```rust
impl SessionManager {
    /// Check if any sessions exist (regardless of state)
    pub fn has_any_sessions(&self) -> bool {
        !self.sessions.is_empty()
    }
}
```

This may already exist as `len() > 0` or similar.

### Acceptance Criteria

1. Status bar shows "○ Not Connected" when `session_manager.len() == 0`
2. Status bar shows normal phase status when sessions exist
3. Styling matches the "Stopped" state (gray color, no bold)
4. Works in both regular and compact status bar modes
5. Device name area is blank or shows "-" when not connected

### Testing

Visual verification:
```bash
cargo run -- tests/fixtures/simple_app
# Status bar should show "○ Not Connected"
# After starting a session (press 'd' then select device), status should change
```

Unit tests (if status_bar has inline tests):
```bash
cargo test status_bar
```

### Notes

- The status bar may need access to session count; ensure `AppState` is passed correctly
- The compact status bar (`StatusBarCompact`) may need the same update
- Consider what to show in the device/platform area when not connected:
  - Option A: Leave blank
  - Option B: Show "-"
  - Option C: Hide entirely
- For simplicity, start with leaving the device area blank

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (To be filled after implementation)

**Implementation Details:**
(To be filled after implementation)

**Testing Performed:**
- `cargo fmt` - Pending
- `cargo check` - Pending
- `cargo clippy` - Pending
- `cargo test` - Pending
