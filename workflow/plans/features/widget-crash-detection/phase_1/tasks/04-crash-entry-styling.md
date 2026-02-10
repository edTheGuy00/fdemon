## Task: Crash Entry Styling (Optional)

**Objective**: Add visual distinction for widget crash entries in the log view so they stand out from regular error log entries.

**Depends on**: [03-handler-integration](03-handler-integration.md)

**Estimated Time**: 1-2 hours

### Scope

- `crates/fdemon-core/src/types.rs` — Add crash origin marker to `LogEntry`
- `crates/fdemon-tui/src/widgets/log_view/mod.rs` — Conditional styling for crash entries
- `crates/fdemon-tui/src/widgets/log_view/styles.rs` — Crash-specific style constants

### Approach

Widget crash entries already work with the existing rendering (they're `LogLevel::Error` with a `stack_trace`). This task adds **optional** visual enhancements to make crashes more prominent.

#### Option A: Message Prefix (Minimal)

Add a crash icon prefix to the exception summary message in `ExceptionBlock::to_log_entry()`:

```
 ERROR  WIDGETS LIBRARY: Failed assertion: 'margin.isNonNegative' — _CodeLine
```

This requires no TUI changes — the message itself contains the visual marker.

#### Option B: LogSource Variant (Moderate)

Add a `LogSource::WidgetCrash` variant (or a boolean flag on LogEntry) so the TUI can style crash entries differently:

```rust
// In types.rs
pub enum LogSource {
    App,
    Flutter,
    Daemon,
    Watcher,
    WidgetCrash, // NEW: Distinct source for framework exception blocks
}
```

The log view can then apply a distinct style:
- Red left border marker for crash entries
- Bold error message
- Distinct source label (e.g., `[crash]` instead of `[flutter]`)

#### Option C: Crash Summary Header Line (Rich)

Render a compact header above the stack trace:

```
 CRASH  WIDGETS LIBRARY — _CodeLine (ide_code_viewer.dart:72:22)
        Failed assertion: 'margin == null || margin.isNonNegative'
        ▶ 297 more frames...
```

This would require minor changes to the LogView rendering to detect crash entries and format them with a two-line header.

### Recommendation

**Start with Option A** (message prefix) as it requires zero TUI changes and is immediately effective. Option B or C can be added later if users want more visual distinction.

### Acceptance Criteria

1. [ ] Widget crash entries are visually distinguishable from regular error entries
2. [ ] The library name and widget name are visible in the log message
3. [ ] No regression in non-crash error entry rendering
4. [ ] Style is consistent with existing log view design

### Testing

Visual testing only — verify in the TUI that crash entries look distinct.

### Notes

- This task is intentionally minimal. The primary value comes from Tasks 01-03 (detection and collapsibility). Styling is a polish step.
- The exact visual treatment can be decided during implementation based on what looks best in the terminal.
- Consider the cyber-glass theme (Phase 4 redesign) when choosing colors and markers.
