## Task: Consolidate Phase-to-Icon/Color Mapping

**Objective**: Eliminate the 4-5 duplicated `AppPhase → (icon, color)` mappings across `tabs.rs` and `status_bar/mod.rs` by creating a single canonical mapping in `theme::styles`.

**Depends on**: 01-create-theme-module

### Scope

- `crates/fdemon-tui/src/theme/styles.rs` — Add `phase_indicator()` function
- `crates/fdemon-tui/src/widgets/tabs.rs` — Replace 3 duplicated mappings
- `crates/fdemon-tui/src/widgets/status_bar/mod.rs` — Replace 2 duplicated mappings

### Details

#### Current Duplication

The same `AppPhase → (icon, color)` mapping is repeated **5 times**:

| Location | File | Lines | Variant |
|----------|------|-------|---------|
| `tab_titles()` | `tabs.rs` | 34-39 | `(char, Color)` |
| `render_single_session()` | `tabs.rs` | 57-62 | `(char, Color)` |
| `render_single_session_header()` | `tabs.rs` | 265-270 | `(char, Color)` |
| `state_indicator()` | `status_bar/mod.rs` | 49-73 | `(&str text, Color, Modifier)` — expanded with labels |
| `StatusBarCompact` render | `status_bar/mod.rs` | 302-306 | `(char, Color)` — minimal |

**Canonical mapping:**

| AppPhase | Icon | Color | Label |
|----------|------|-------|-------|
| Running | `●` | Green | "Running" |
| Running (busy) | `↻` | Yellow | "Reloading" |
| Reloading | `↻` | Yellow | "Reloading" |
| Initializing | `○` | DarkGray | "Starting" |
| Stopped | `○` | DarkGray | "Stopped" |
| Quitting | `✗` | Red | "Stopping" |

Note: "Running (busy)" is a sub-state where `phase == Running && is_busy == true`. The status bar differentiates this, while tabs do not.

#### New Function in `theme/styles.rs`

```rust
use fdemon_core::types::AppPhase;

/// Phase indicator for session tabs and status displays.
///
/// Returns `(icon_char, label, Style)` for the given AppPhase.
/// The label is the human-readable status text (e.g., "Running", "Stopped").
pub fn phase_indicator(phase: &AppPhase) -> (&'static str, &'static str, Style) {
    match phase {
        AppPhase::Running => (
            "●",
            "Running",
            Style::default().fg(palette::STATUS_GREEN).add_modifier(Modifier::BOLD),
        ),
        AppPhase::Reloading => (
            "↻",
            "Reloading",
            Style::default().fg(palette::STATUS_YELLOW).add_modifier(Modifier::BOLD),
        ),
        AppPhase::Initializing => (
            "○",
            "Starting",
            Style::default().fg(palette::TEXT_MUTED),
        ),
        AppPhase::Stopped => (
            "○",
            "Stopped",
            Style::default().fg(palette::TEXT_MUTED),
        ),
        AppPhase::Quitting => (
            "✗",
            "Stopping",
            Style::default().fg(palette::STATUS_RED),
        ),
    }
}

/// Phase indicator for "busy" override (running but currently reloading).
///
/// When a session is Running but has pending operations, show the reload indicator.
pub fn phase_indicator_busy() -> (&'static str, &'static str, Style) {
    (
        "↻",
        "Reloading",
        Style::default().fg(palette::STATUS_YELLOW).add_modifier(Modifier::BOLD),
    )
}

/// "Not connected" indicator for when no sessions exist.
pub fn phase_indicator_disconnected() -> (&'static str, &'static str, Style) {
    (
        "○",
        "Not Connected",
        Style::default().fg(palette::TEXT_MUTED),
    )
}
```

#### Migration in `tabs.rs`

Replace the 3 inline match blocks with calls to `theme::styles::phase_indicator()`:

```rust
// Before (in tab_titles, line 34-39)
let (icon, color) = match session.phase {
    AppPhase::Running => ('●', Color::Green),
    AppPhase::Reloading => ('↻', Color::Yellow),
    AppPhase::Initializing => ('○', Color::DarkGray),
    AppPhase::Stopped => ('○', Color::DarkGray),
    AppPhase::Quitting => ('✗', Color::Red),
};

// After
let (icon, _label, style) = crate::theme::styles::phase_indicator(&session.phase);
```

Repeat for the other 2 locations in `tabs.rs`.

#### Migration in `status_bar/mod.rs`

The status bar's `state_indicator()` is more complex because it adds:
- The "busy" check (`session.is_busy` → show reload icon even when Running)
- Labels like "Running", "Stopped", etc.
- The "Not Connected" fallback for no sessions

Replace with:

```rust
// Before (lines 49-73): long match with labels and modifiers
// After
fn state_indicator(&self) -> Span {
    let session = self.state.active_session();
    let (icon, label, style) = match session {
        None => crate::theme::styles::phase_indicator_disconnected(),
        Some(s) if s.is_busy => crate::theme::styles::phase_indicator_busy(),
        Some(s) => crate::theme::styles::phase_indicator(&s.phase),
    };
    Span::styled(format!("{icon} {label}"), style)
}
```

For the compact status bar (lines 302-306), same pattern but only use the icon:

```rust
let (icon, _label, style) = match session {
    Some(s) if s.is_busy => crate::theme::styles::phase_indicator_busy(),
    Some(s) => crate::theme::styles::phase_indicator(&s.phase),
    None => crate::theme::styles::phase_indicator_disconnected(),
};
Span::styled(icon, style)
```

### Acceptance Criteria

1. `theme::styles::phase_indicator()` function exists and returns `(&str, &str, Style)`
2. `theme::styles::phase_indicator_busy()` and `phase_indicator_disconnected()` helper functions exist
3. All 3 duplicated mappings in `tabs.rs` are replaced with `phase_indicator()` calls
4. Both mappings in `status_bar/mod.rs` are replaced with `phase_indicator()` calls
5. No `AppPhase` match → color/icon mapping exists outside of `theme/styles.rs`
6. `cargo check -p fdemon-tui` passes
7. `cargo clippy -p fdemon-tui` passes with no warnings
8. Visual behavior is preserved (same icons, same colors, same modifiers)

### Testing

Existing tests in `status_bar/tests.rs` and `tabs.rs` test modules may need updating since the exact Style values (bold modifiers, etc.) might differ slightly from the current per-site implementations. Any test failures are addressed in Task 05.

Add unit tests for the new function:

```rust
#[test]
fn test_phase_indicator_running() {
    let (icon, label, style) = phase_indicator(&AppPhase::Running);
    assert_eq!(icon, "●");
    assert_eq!(label, "Running");
    assert_eq!(style.fg, Some(palette::STATUS_GREEN));
}

#[test]
fn test_phase_indicator_all_phases_covered() {
    // Ensure every AppPhase variant returns valid data
    for phase in [AppPhase::Running, AppPhase::Reloading, AppPhase::Initializing, AppPhase::Stopped, AppPhase::Quitting] {
        let (icon, label, _style) = phase_indicator(&phase);
        assert!(!icon.is_empty());
        assert!(!label.is_empty());
    }
}
```

### Notes

- **Dependency on `fdemon-core`**: The `phase_indicator` function takes `&AppPhase`, which is defined in `fdemon-core`. The `fdemon-tui` crate already depends on `fdemon-core`, so this is fine.
- **Style differences**: The current implementations have minor inconsistencies:
  - `tabs.rs` does NOT apply `Modifier::BOLD` to Running/Reloading, but `status_bar` DOES
  - The consolidated function should apply BOLD for Running and Reloading (matching the status bar behavior), since tabs apply their own styling on top
  - This may cause minor visual differences in tabs (icons becoming bold) — acceptable and arguably an improvement
- **The "busy" concept**: Only the status bar currently checks `is_busy`. The `phase_indicator_busy()` helper keeps this case explicit without complicating the main function.
- **`phase_indicator` returns `&'static str` not `char`**: The current tabs code uses `char` for icons, but `&str` is more flexible for future Nerd Font icons and avoids `char`-to-`String` conversion at call sites.
