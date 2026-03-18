## Task: Extend SDK Info State and TUI Layout for Additional Fields

**Objective**: Add state fields and TUI rendering for framework revision, engine revision, and DevTools version. Make SDK PATH column width dynamic.

**Depends on**: 01-fix-tab-label, 02-fix-vertical-layout

### Scope

- `crates/fdemon-app/src/flutter_version/state.rs`: Add extended metadata fields to `SdkInfoState`
- `crates/fdemon-tui/src/widgets/flutter_version_panel/sdk_info.rs`: Render extended fields, dynamic path width

### Details

**State changes — `SdkInfoState`:**

```rust
/// Read-only display of the currently resolved SDK.
#[derive(Debug, Default)]
pub struct SdkInfoState {
    /// Snapshot of the resolved SDK at panel open time.
    pub resolved_sdk: Option<FlutterSdk>,
    /// Dart SDK version (from file or probe)
    pub dart_version: Option<String>,
    /// Framework git revision (short hash, from probe)
    pub framework_revision: Option<String>,
    /// Engine revision (short hash, from probe)
    pub engine_revision: Option<String>,
    /// DevTools version (from probe)
    pub devtools_version: Option<String>,
}
```

**TUI layout changes — expanded mode (`render_sdk_details_expanded`):**

The current 3-group layout (VERSION/CHANNEL, SOURCE/PATH, DART) gains additional rows:

```
  SDK Info
  ─────────────────────────────
  VERSION         CHANNEL
  3.38.6          stable

  SOURCE          SDK PATH
  system PATH     ~/Dev/flutter

  DART SDK        DEVTOOLS
  3.10.7          2.51.1

  FRAMEWORK       ENGINE
  8b87286849      6f3039bf7c
```

Layout becomes 4 field groups:
```
Length(2)  — VERSION | CHANNEL
Length(1)  — spacer
Length(2)  — SOURCE | SDK PATH
Length(1)  — spacer
Length(2)  — DART SDK | DEVTOOLS
Length(1)  — spacer
Length(2)  — FRAMEWORK | ENGINE
Min(0)    — absorber
```

Total expanded content height: 4×2 + 3×1 = 11 rows. With header(2) = 13 rows.

Update `VERTICAL_SDK_INFO_HEIGHT` in `mod.rs` to `13` (2 header + 11 content).

**TUI layout changes — compact mode (`render_sdk_details_compact`):**

Compact mode fits everything in fewer rows:
```
  3.38.6 stable (system PATH)
  ~/Dev/flutter
  Dart 3.10.7  DevTools 2.51.1
  rev 8b87286849  engine 6f3039bf7c
```

4-5 rows, single-line per concept.

**Dynamic SDK PATH width:**

Replace the hardcoded `MAX_PATH_WIDTH = 28` with a dynamic width based on the actual column area:

```rust
// In render_sdk_details_expanded():
let row2 = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
    .split(chunks[2]);
let max_path_width = row2[1].width.saturating_sub(4) as usize; // 2 label padding + 2 safety margin
let path_str = format_path(&sdk.root, max_path_width);
```

This ensures the path gets all available space in the column, rather than being capped at 28 chars regardless of terminal width.

**Handling None values:**

For probe-sourced fields (`framework_revision`, `engine_revision`, `devtools_version`), use em-dash "—" as placeholder when `None`. Optionally show a subtle "..." if the probe is still in-flight (can coordinate with task 05 for a `probe_pending: bool` field).

### Acceptance Criteria

1. `SdkInfoState` has `framework_revision`, `engine_revision`, `devtools_version` fields (all `Option<String>`)
2. Expanded layout shows 4 field groups with the new FRAMEWORK/ENGINE and DART/DEVTOOLS rows
3. Compact layout includes all fields in a condensed format
4. SDK PATH width is computed dynamically from the available column width, not hardcoded
5. Missing fields (None) display as "—" (em-dash)
6. `VERTICAL_SDK_INFO_HEIGHT` updated to accommodate the additional field rows
7. Existing tests pass with updated layouts; new tests verify extended field rendering

### Testing

```rust
#[test]
fn test_sdk_info_extended_fields_render() {
    let mut state = make_state_with_sdk();
    state.framework_revision = Some("8b87286849".into());
    state.engine_revision = Some("6f3039bf7c".into());
    state.devtools_version = Some("2.51.1".into());
    let pane = SdkInfoPane::new(&state, true);
    let area = Rect::new(0, 0, 50, 20);
    let mut buf = Buffer::empty(area);
    pane.render(area, &mut buf);
    let content: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(content.contains("8b87286849"), "should show framework revision");
    assert!(content.contains("6f3039bf7c"), "should show engine revision");
    assert!(content.contains("2.51.1"), "should show devtools version");
}

#[test]
fn test_sdk_info_missing_extended_fields_show_dash() {
    let state = make_state_with_sdk();
    // framework_revision, engine_revision, devtools_version are None
    let pane = SdkInfoPane::new(&state, true);
    let area = Rect::new(0, 0, 50, 20);
    let mut buf = Buffer::empty(area);
    pane.render(area, &mut buf);
    let content: String = buf.content().iter().map(|c| c.symbol()).collect();
    // Should show em-dash for missing fields
    assert!(content.contains("\u{2014}"), "missing fields should show em-dash");
}

#[test]
fn test_sdk_path_dynamic_width_wide_terminal() {
    let state = make_state_with_sdk();
    let pane = SdkInfoPane::new(&state, true);
    // Wide area — path should not be truncated
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    pane.render(area, &mut buf);
    let content: String = buf.content().iter().map(|c| c.symbol()).collect();
    // Full path should be visible without ellipsis
    assert!(!content.contains("\u{2026}"), "wide terminal should not truncate path");
}
```

### Notes

- The new fields will be `None` until task 05 wires the probe results. Tests should verify em-dash rendering for this initial state.
- Update `make_state_with_sdk()` test helpers to include the new `SdkInfoState` fields.
- The `MIN_EXPANDED_CONTENT_HEIGHT` constant from task 02 may need updating (from 8 to 11) to account for the additional field groups.
- Keep `format_path()` function unchanged — just pass it a larger `max_width` value.

---

## Completion Summary

**Status:** Not Started
