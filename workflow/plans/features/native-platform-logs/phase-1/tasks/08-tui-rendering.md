## Task: TUI Native Source Rendering

**Objective**: Add visual styling for `LogSource::Native` in the TUI — a dedicated palette color, `source_style()` match arm, and proper display of the dynamic tag name in the log view.

**Depends on**: 01-core-types (for `LogSource::Native` variant)

### Scope

- `crates/fdemon-tui/src/theme/palette.rs`: Add `SOURCE_NATIVE` color constant
- `crates/fdemon-tui/src/widgets/log_view/mod.rs`: Update `source_style()` match arm, verify `format_entry()` renders native tag correctly

### Details

#### 1. Add palette color

In `crates/fdemon-tui/src/theme/palette.rs` (lines 62–66), add a new source color:

```rust
// Existing source colors:
pub const SOURCE_APP: Color = STATUS_GREEN;           // Green
pub const SOURCE_DAEMON: Color = STATUS_YELLOW;       // Yellow
pub const SOURCE_FLUTTER: Color = STATUS_INDIGO;      // Indigo
pub const SOURCE_FLUTTER_ERROR: Color = STATUS_RED;   // Red
pub const SOURCE_WATCHER: Color = STATUS_BLUE;        // Blue

// NEW:
/// Color for native platform log sources (Android logcat, macOS log stream).
pub const SOURCE_NATIVE: Color = Color::Rgb(206, 147, 216);  // Light purple / lavender
```

The color choice (light purple/lavender) provides visual distinction from all existing source colors:
- Green (App), Yellow (Daemon), Indigo (Flutter), Red (FlutterError), Blue (Watcher)
- Purple/lavender is unoccupied and immediately recognizable as "different from Flutter framework logs"

Alternative color options if purple doesn't fit the theme:
- `Color::Rgb(255, 183, 77)` — Orange/amber
- `Color::Rgb(129, 199, 132)` — Light green (distinct from STATUS_GREEN)
- `Color::Rgb(224, 224, 224)` — Light gray (neutral)

#### 2. Update `source_style()`

In `crates/fdemon-tui/src/widgets/log_view/mod.rs` (lines 207–217), add the match arm:

```rust
fn source_style(source: &LogSource) -> Style {
    match source {
        LogSource::App          => Style::default().fg(palette::SOURCE_APP),
        LogSource::Daemon       => Style::default().fg(palette::SOURCE_DAEMON),
        LogSource::Flutter      => Style::default().fg(palette::SOURCE_FLUTTER),
        LogSource::FlutterError => Style::default().fg(palette::SOURCE_FLUTTER_ERROR),
        LogSource::Watcher      => Style::default().fg(palette::SOURCE_WATCHER),
        LogSource::VmService    => Style::default().fg(palette::ACCENT),
        LogSource::Native { .. } => Style::default().fg(palette::SOURCE_NATIVE),
    }
}
```

**Note on function signature**: The existing `source_style()` takes `LogSource` by value (not reference). Since `LogSource::Native { tag: String }` is not `Copy`, this function may need to change to take `&LogSource` instead. Check all call sites and update accordingly. If the function already takes a reference, just add the match arm.

#### 3. Verify `format_entry()` renders tag correctly

In `format_entry()` (line 290), the source prefix is rendered as:

```rust
// Line ~316:
Span::styled(
    format!("[{}] ", entry.source.prefix()),
    source_style(entry.source.clone()), // or &entry.source
)
```

For `LogSource::Native { tag: "GoLog".into() }`, `prefix()` returns `"GoLog"`, so the rendered output is `[GoLog] `. This is the desired display format — no changes needed to `format_entry()` itself.

Verify that:
- Long tags (e.g., `"com.example.myplugin.logging"`) render without truncation in the prefix
- Empty tags (defensive) render as `[] ` — acceptable edge case
- The prefix doesn't break horizontal scrolling

#### 4. Update `LogSourceFilter` display in metadata bar

The log view's metadata bar shows the active filter via `filter_state.source_filter.display_name()`. Task 01 already adds `"Native logs"` as the display name for `LogSourceFilter::Native`. Verify this renders correctly in the metadata bar.

#### 5. Fix any `source_style()` signature issues

If `source_style()` takes `LogSource` by value and `LogSource` is no longer `Copy` (because `Native { tag: String }`), the function signature must change to `&LogSource`:

```rust
fn source_style(source: &LogSource) -> Style {
    // ...
}
```

Update all call sites to pass a reference instead of moving. Common pattern:
```rust
// Before:
source_style(entry.source.clone())
// After:
source_style(&entry.source)
```

### Acceptance Criteria

1. `palette::SOURCE_NATIVE` is defined as a distinct color
2. `source_style()` returns the correct style for `LogSource::Native { .. }`
3. Native log entries render with `[GoLog]`, `[OkHttp]`, `[com.example.plugin]` prefixes in the log view
4. The prefix color is `SOURCE_NATIVE` (purple/lavender)
5. Existing source colors are unchanged
6. `LogSourceFilter::Native` display name `"Native logs"` renders in the metadata bar
7. No visual regressions in existing log rendering
8. `cargo check --workspace` compiles
9. TUI rendering tests pass

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::{LogEntry, LogLevel, LogSource};

    #[test]
    fn test_source_style_native() {
        let style = source_style(&LogSource::Native { tag: "GoLog".into() });
        assert_eq!(style.fg, Some(palette::SOURCE_NATIVE));
    }

    #[test]
    fn test_native_log_entry_prefix_rendering() {
        let entry = LogEntry::new(
            LogLevel::Info,
            LogSource::Native { tag: "GoLog".into() },
            "Hello from Go".to_string(),
        );
        assert_eq!(entry.source.prefix(), "GoLog");
        // The format_entry function will render as "[GoLog] Hello from Go"
    }

    #[test]
    fn test_native_log_entry_long_tag() {
        let entry = LogEntry::new(
            LogLevel::Debug,
            LogSource::Native { tag: "com.example.myplugin.logging".into() },
            "verbose message".to_string(),
        );
        assert_eq!(entry.source.prefix(), "com.example.myplugin.logging");
    }

    #[test]
    fn test_source_style_existing_sources_unchanged() {
        // Verify existing source styles haven't regressed
        assert_eq!(source_style(&LogSource::App).fg, Some(palette::SOURCE_APP));
        assert_eq!(source_style(&LogSource::Daemon).fg, Some(palette::SOURCE_DAEMON));
        assert_eq!(source_style(&LogSource::Flutter).fg, Some(palette::SOURCE_FLUTTER));
        assert_eq!(source_style(&LogSource::FlutterError).fg, Some(palette::SOURCE_FLUTTER_ERROR));
        assert_eq!(source_style(&LogSource::Watcher).fg, Some(palette::SOURCE_WATCHER));
    }
}
```

### Notes

- **`source_style()` signature change**: If `LogSource` was previously `Copy` (all variants were fieldless enums), adding `Native { tag: String }` makes it non-`Copy`. This is a breaking change that affects the function signature and all call sites. The fix is straightforward (take `&LogSource`) but touches multiple files in the TUI crate.
- **Color choice**: Purple/lavender was chosen to be visually distinct from all existing colors. If the team prefers a different color, it's a single constant change. The important thing is that native logs are immediately distinguishable from Flutter/App/Daemon sources.
- **This task is intentionally small** — the heavy lifting is in core types (task 01) and app integration (task 07). The TUI changes are a thin presentation layer addition.

---

## Completion Summary

**Status:** Not Started
