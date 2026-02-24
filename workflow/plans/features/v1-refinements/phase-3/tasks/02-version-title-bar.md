## Task: Show Version in Title Bar

**Objective**: Display the app version next to "Flutter Demon" in the TUI title bar header, so users always know which version they're running.

**Depends on**: None

**Estimated Time**: 1-2 hours

### Scope

- `crates/fdemon-tui/src/widgets/header.rs`: Add version constant and modify title rendering

### Details

#### Current title bar rendering

The title bar is rendered by `MainHeader` in `crates/fdemon-tui/src/widgets/header.rs`. The `render_title_row` method (line 100) builds the left section at lines 138-152:

```rust
let left_spans = vec![
    Span::raw(" "),
    Span::styled(status_icon, status_style),
    Span::raw(" "),
    Span::styled("Flutter Demon", Style::default().fg(palette::ACCENT).add_modifier(Modifier::BOLD)),
    Span::raw(" "),
    Span::styled("/", Style::default().fg(palette::TEXT_MUTED)),
    Span::raw(" "),
    Span::styled(project_name, Style::default().fg(palette::TEXT_SECONDARY)),
];
```

This renders as: `[dot] Flutter Demon / my_app`

#### Target rendering

After this change: `[dot] Flutter Demon v0.1.0 / my_app`

The version should use a muted style (`palette::TEXT_MUTED`) to avoid visual competition with the bold, accented "Flutter Demon" title.

#### Implementation

1. Add a version constant at the top of `header.rs`:

```rust
/// App version from Cargo.toml, surfaced in the title bar
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
```

2. Modify the `left_spans` in `render_title_row` to insert the version after the "Flutter Demon" span:

```rust
let left_spans = vec![
    Span::raw(" "),
    Span::styled(status_icon, status_style),
    Span::raw(" "),
    Span::styled(
        "Flutter Demon",
        Style::default().fg(palette::ACCENT).add_modifier(Modifier::BOLD),
    ),
    Span::raw(" "),
    Span::styled(
        format!("v{}", APP_VERSION),
        Style::default().fg(palette::TEXT_MUTED),
    ),
    Span::raw(" "),
    Span::styled("/", Style::default().fg(palette::TEXT_MUTED)),
    Span::raw(" "),
    Span::styled(project_name, Style::default().fg(palette::TEXT_SECONDARY)),
];
```

Note: the `format!("v{}", APP_VERSION)` creates a `String`, which is fine since `Span::styled` accepts `Into<Cow<'_, str>>`.

#### Test updates

The existing `test_header_renders_title` test at line 257 checks for `"Flutter Demon"` — this will still pass since the title text hasn't changed.

Add a new test to verify the version appears:

```rust
#[test]
fn test_header_renders_version() {
    let mut term = TestTerminal::new();
    let icons = IconSet::new(IconMode::Unicode);
    let header = MainHeader::new(None, icons);
    term.render_widget(header, term.area());

    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    assert!(
        term.buffer_contains(&version),
        "Header should contain version string"
    );
}
```

### Acceptance Criteria

1. The title bar displays `Flutter Demon v0.1.0 / project_name` (version between title and separator)
2. The version uses `palette::TEXT_MUTED` style (not bold, not accented)
3. The version updates automatically when `Cargo.toml` workspace version changes (compile-time)
4. All existing header tests continue to pass
5. New test verifies version presence in header
6. `cargo test -p fdemon-tui` passes

### Testing

```rust
#[test]
fn test_header_renders_version() {
    let mut term = TestTerminal::new();
    let icons = IconSet::new(IconMode::Unicode);
    let header = MainHeader::new(None, icons);
    term.render_widget(header, term.area());

    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    assert!(
        term.buffer_contains(&version),
        "Header should contain version string"
    );
}

#[test]
fn test_header_version_visible_in_narrow_terminal() {
    // Version is part of the left section which is always rendered
    let mut term = TestTerminal::with_size(50, 5);
    let icons = IconSet::new(IconMode::Unicode);
    let header = MainHeader::new(Some("app"), icons);
    term.render_widget(header, term.area());

    assert!(term.buffer_contains("Flutter Demon"), "Title should show");
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    assert!(term.buffer_contains(&version), "Version should show");
}
```

### Notes

- `env!("CARGO_PKG_VERSION")` in `fdemon-tui` resolves to the TUI crate's version, which inherits from the workspace — same value as the binary crate
- The version string adds ~7 characters to the left section width (`" v0.1.0"`), which slightly reduces space for shortcuts in narrow terminals — acceptable since shortcuts already degrade gracefully (lines 221-233 handle the fallback)
- The project selector modal at `selector.rs:172` also uses `" Flutter Demon "` as a block title — this is a border decoration and does NOT need the version added
- The loading screen at `render/mod.rs:289` uses `"Flutter Demon"` as a fallback project name — this also does NOT need the version

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/header.rs` | Added `APP_VERSION` constant; inserted version span after "Flutter Demon" in `left_spans`; added 2 new tests |
| `crates/fdemon-tui/src/render/snapshots/fdemon_tui__render__tests__normal_initializing.snap` | Updated snapshot to include `v0.1.0` in header line |
| `crates/fdemon-tui/src/render/snapshots/fdemon_tui__render__tests__normal_reloading.snap` | Updated snapshot to include `v0.1.0` in header line |
| `crates/fdemon-tui/src/render/snapshots/fdemon_tui__render__tests__normal_running.snap` | Updated snapshot to include `v0.1.0` in header line |
| `crates/fdemon-tui/src/render/snapshots/fdemon_tui__render__tests__normal_stopped.snap` | Updated snapshot to include `v0.1.0` in header line |

### Notable Decisions/Tradeoffs

1. **Snapshot updates required**: Four existing insta snapshot tests captured the old header line `Flutter Demon / flutter_app`. These were updated via `cargo insta test --accept` to reflect `Flutter Demon v0.1.0 / flutter_app`. This is correct behaviour — snapshots document the expected rendered output and must be updated when intentional rendering changes are made.
2. **`format!` string ownership**: The version span uses `format!("v{}", APP_VERSION)` which creates an owned `String`. This is accepted by `Span::styled` via `Into<Cow<'_, str>>` and is the approach specified by the task.

### Testing Performed

- `cargo fmt --all` — Passed (no formatting changes needed)
- `cargo check --workspace` — Passed
- `cargo insta test -p fdemon-tui --accept` — 4 snapshots accepted, all tests pass
- `cargo test -p fdemon-tui` — Passed (536 tests: 532 existing + 2 new + 4 snapshot recoveries + 7 doc tests)
- `cargo clippy --workspace -- -D warnings` — Passed

### Risks/Limitations

1. **Narrow terminal truncation**: Adding `v0.1.0` (~7 characters) to the left section slightly reduces available width for shortcuts. The existing graceful degradation logic at lines 221-233 handles this correctly — shortcuts are dropped before the left section is truncated.
