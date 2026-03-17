## Task: TUI Widget — Flutter Version Panel

**Objective**: Build the Flutter Version panel widget as a centered overlay with two panes (SDK info and version list), following the New Session Dialog widget pattern with responsive layout support.

**Depends on**: 01-state-types

### Scope

- `crates/fdemon-tui/src/widgets/flutter_version_panel/mod.rs`: **NEW** Widget root, layout dispatch, overlay rendering
- `crates/fdemon-tui/src/widgets/flutter_version_panel/sdk_info.rs`: **NEW** Left pane — current SDK details
- `crates/fdemon-tui/src/widgets/flutter_version_panel/version_list.rs`: **NEW** Right pane — installed versions list
- `crates/fdemon-tui/src/widgets/mod.rs`: Re-export `flutter_version_panel` module

### Details

#### 1. Module Structure

```
crates/fdemon-tui/src/widgets/flutter_version_panel/
├── mod.rs              — FlutterVersionPanel widget, layout dispatch, overlay
├── sdk_info.rs         — SdkInfoPane widget (left pane)
└── version_list.rs     — VersionListPane widget (right pane)
```

#### 2. Main Widget (`mod.rs`)

```rust
//! # Flutter Version Panel
//!
//! Centered overlay panel for viewing and managing Flutter SDK versions.
//! Follows the New Session Dialog widget pattern.

use ratatui::prelude::*;
use ratatui::widgets::*;
use fdemon_app::flutter_version::{FlutterVersionState, FlutterVersionPane};
use crate::widgets::modal_overlay;

mod sdk_info;
mod version_list;

use sdk_info::SdkInfoPane;
use version_list::VersionListPane;

/// Minimum terminal width for horizontal (side-by-side) layout.
/// Derived from: 30 chars left pane + 1 separator + 35 chars right pane + 4 border = 70.
const MIN_HORIZONTAL_WIDTH: u16 = 70;

/// Minimum terminal height for any rendering.
/// Derived from: 3 header + 1 sep + 5 content + 1 sep + 1 footer + 2 border = 13.
const MIN_RENDER_HEIGHT: u16 = 13;

/// Left pane width as percentage of content area.
const LEFT_PANE_PERCENT: u16 = 40;

pub struct FlutterVersionPanel<'a> {
    state: &'a FlutterVersionState,
    icons: &'a IconSet,
}

impl<'a> FlutterVersionPanel<'a> {
    pub fn new(state: &'a FlutterVersionState, icons: &'a IconSet) -> Self {
        Self { state, icons }
    }
}
```

#### 3. Overlay Rendering Pattern

Follow the exact sequence from `NewSessionDialog::render()`:

```rust
impl Widget for FlutterVersionPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // 1. Dim the entire background
        modal_overlay::dim_background(buf, area);

        // 2. Calculate centered dialog area (80% width, 70% height)
        let dialog_area = Self::centered_rect(area);

        // 3. Check minimum size
        if dialog_area.width < 40 || dialog_area.height < MIN_RENDER_HEIGHT {
            self.render_too_small(dialog_area, buf);
            return;
        }

        // 4. Render drop shadow
        modal_overlay::render_shadow(buf, dialog_area);

        // 5. Clear the dialog area
        modal_overlay::clear_area(buf, dialog_area);

        // 6. Render border block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_COLOR))
            .style(Style::default().bg(POPUP_BG));
        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        // 7. Layout: header | separator | panes | separator | footer
        let chunks = Layout::vertical([
            Constraint::Length(3),   // header (title + subtitle)
            Constraint::Length(1),   // separator
            Constraint::Min(5),     // panes (flexible)
            Constraint::Length(1),   // separator
            Constraint::Length(1),   // footer (keybinding hints)
        ])
        .split(inner);

        self.render_header(chunks[0], buf);
        self.render_separator(chunks[1], buf);

        // Choose horizontal vs vertical pane layout
        if inner.width >= MIN_HORIZONTAL_WIDTH {
            self.render_horizontal_panes(chunks[2], buf);
        } else {
            self.render_vertical_panes(chunks[2], buf);
        }

        self.render_separator(chunks[3], buf);
        self.render_footer(chunks[4], buf);
    }
}
```

#### 4. Centered Rect Calculation

Reuse the same approach as `NewSessionDialog`:

```rust
impl FlutterVersionPanel<'_> {
    fn centered_rect(area: Rect) -> Rect {
        let vertical = Layout::vertical([
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(area);

        Layout::horizontal([
            Constraint::Percentage(10),
            Constraint::Percentage(80),
            Constraint::Percentage(10),
        ])
        .split(vertical[1])[1]
    }
}
```

#### 5. Header

```
┌──────────────────────────────────────────────────────┐
│  Flutter SDK                          [Esc] Close    │
│  Manage Flutter SDK versions and channels.           │
└──────────────────────────────────────────────────────┘
```

- Title: "Flutter SDK" (bold)
- Right-aligned: "[Esc] Close"
- Subtitle line: "Manage Flutter SDK versions and channels." (dimmed)

#### 6. Horizontal Pane Layout

```rust
fn render_horizontal_panes(&self, area: Rect, buf: &mut Buffer) {
    let chunks = Layout::horizontal([
        Constraint::Percentage(LEFT_PANE_PERCENT), // left: SDK info
        Constraint::Length(1),                      // separator
        Constraint::Min(20),                        // right: version list
    ])
    .split(area);

    let sdk_info = SdkInfoPane::new(
        &self.state.sdk_info,
        self.state.focused_pane == FlutterVersionPane::SdkInfo,
        self.icons,
    );
    sdk_info.render(chunks[0], buf);

    // Vertical separator
    self.render_vertical_separator(chunks[1], buf);

    let version_list = VersionListPane::new(
        &self.state.version_list,
        self.state.focused_pane == FlutterVersionPane::VersionList,
        self.icons,
    );
    version_list.render(chunks[2], buf);
}
```

#### 7. Vertical Pane Layout (Stacked)

When width < `MIN_HORIZONTAL_WIDTH`, stack panes vertically:

```rust
fn render_vertical_panes(&self, area: Rect, buf: &mut Buffer) {
    let chunks = Layout::vertical([
        Constraint::Length(6),    // SDK info (compact)
        Constraint::Length(1),    // separator
        Constraint::Min(5),      // version list (fills remaining)
    ])
    .split(area);

    let sdk_info = SdkInfoPane::new(
        &self.state.sdk_info,
        self.state.focused_pane == FlutterVersionPane::SdkInfo,
        self.icons,
    );
    sdk_info.render(chunks[0], buf);

    self.render_separator(chunks[1], buf);

    let version_list = VersionListPane::new(
        &self.state.version_list,
        self.state.focused_pane == FlutterVersionPane::VersionList,
        self.icons,
    );
    version_list.render(chunks[2], buf);
}
```

#### 8. Footer

Show keyboard shortcuts based on focused pane:

```rust
fn render_footer(&self, area: Rect, buf: &mut Buffer) {
    let hints = match self.state.focused_pane {
        FlutterVersionPane::SdkInfo => "[Tab] Versions  [Esc] Close",
        FlutterVersionPane::VersionList => "[Tab] Info  [Enter] Switch  [d] Remove  [Esc] Close",
    };

    // If status message exists, show it on the left
    let text = if let Some(ref msg) = self.state.status_message {
        format!("{msg}  │  {hints}")
    } else {
        hints.to_string()
    };

    Paragraph::new(text)
        .style(Style::default().fg(TEXT_MUTED))
        .render(area, buf);
}
```

#### 9. Left Pane — SDK Info (`sdk_info.rs`)

Read-only display of the resolved SDK:

```
  VERSION            CHANNEL
  3.19.0             stable

  SOURCE             SDK PATH
  FVM (.fvmrc)       ~/fvm/versions/3.19.0/

  DART SDK
  3.3.0
```

**When no SDK is resolved**: Show "No Flutter SDK found" with a hint to install or configure.

```rust
pub struct SdkInfoPane<'a> {
    state: &'a SdkInfoState,
    focused: bool,
    icons: &'a IconSet,
}

impl Widget for SdkInfoPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Highlight border if focused
        let border_style = if self.focused {
            Style::default().fg(ACCENT_COLOR)
        } else {
            Style::default().fg(BORDER_DIM)
        };

        match &self.state.resolved_sdk {
            Some(sdk) => self.render_sdk_details(sdk, area, buf),
            None => self.render_no_sdk(area, buf),
        }
    }
}
```

**Fields to display:**

| Label | Source |
|-------|--------|
| VERSION | `sdk.version` |
| CHANNEL | `sdk.channel.as_deref().unwrap_or("unknown")` |
| SOURCE | `sdk.source.to_string()` (Display impl from Phase 1) |
| SDK PATH | `sdk.root.display()` (truncate if too long) |
| DART SDK | `self.state.dart_version.as_deref().unwrap_or("—")` |

Use a vertical layout with label/value pairs. Labels in dimmed text, values in normal text. Use `Constraint::Length(2)` per field (label + value) with `Constraint::Length(1)` spacers.

#### 10. Right Pane — Version List (`version_list.rs`)

Scrollable list of installed versions:

```
  Installed Versions
  ─────────────────
  ● 3.19.0 (stable) ← active
    3.16.0
    3.22.0-beta (beta)
```

```rust
pub struct VersionListPane<'a> {
    state: &'a VersionListState,
    focused: bool,
    icons: &'a IconSet,
}
```

**Rendering logic:**

1. **Loading state**: Show spinner or "Scanning..." text when `state.loading`
2. **Error state**: Show error message when `state.error.is_some()`
3. **Empty state**: Show "No versions found in ~/fvm/versions/" when list is empty
4. **List rendering**: Iterate visible items (accounting for scroll_offset), highlight selected

**Active version indicator**: Use `●` (filled circle) for the active version, ` ` (space) for others. Active version text is bold/highlighted.

**Selection highlight**: Invert colors (or use accent background) for the selected item when pane is focused.

**Render-hint write-back**: Set `last_known_visible_height` during render:

```rust
impl Widget for VersionListPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // ... layout ...
        let list_area = chunks[1]; // area for the list items

        // EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md
        let visible_height = list_area.height as usize;
        self.state.last_known_visible_height.set(visible_height);

        // Render visible items
        let start = self.state.scroll_offset;
        let end = (start + visible_height).min(self.state.installed_versions.len());

        for (i, sdk) in self.state.installed_versions[start..end].iter().enumerate() {
            let y = list_area.y + i as u16;
            let is_selected = (start + i) == self.state.selected_index;
            self.render_version_item(sdk, y, list_area.x, list_area.width, is_selected, buf);
        }
    }
}
```

**Version item format:**

```
  ● 3.19.0 (stable)     — active, selected
    3.16.0               — inactive, not selected
    3.22.0-beta (beta)   — inactive, selected
```

- Column 1: `●` or ` ` (1 char + 1 space)
- Column 2: version string
- Column 3: ` (channel)` if channel is Some and different from version string

### Acceptance Criteria

1. Panel renders as a centered overlay with rounded border and drop shadow
2. Background is dimmed behind the panel
3. Header shows "Flutter SDK" title and "[Esc] Close"
4. Horizontal layout (2 side-by-side panes) when width >= 70
5. Vertical layout (stacked panes) when width < 70
6. "Too small" message when dialog area < 40 wide or < 13 tall
7. Left pane shows SDK version, channel, source, path, Dart version
8. Left pane shows "No Flutter SDK found" when `resolved_sdk` is `None`
9. Right pane shows scrollable installed versions list
10. Active version marked with `●` indicator
11. Selected item highlighted with accent color when pane is focused
12. Loading spinner shown during cache scan
13. Empty state shown when no versions installed
14. Footer shows context-appropriate keybinding hints
15. `last_known_visible_height` render-hint is written every frame
16. Scroll offset clamp applied as safety net during render
17. `cargo check --workspace` compiles
18. `cargo test --workspace` passes
19. `cargo clippy --workspace -- -D warnings` passes

### Testing

Widget rendering tests use `Buffer` comparison (same pattern as existing widget tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    fn test_state() -> FlutterVersionState {
        FlutterVersionState {
            sdk_info: SdkInfoState {
                resolved_sdk: Some(FlutterSdk { /* test data */ }),
                dart_version: Some("3.3.0".into()),
            },
            version_list: VersionListState {
                installed_versions: vec![/* test data */],
                selected_index: 0,
                scroll_offset: 0,
                loading: false,
                error: None,
                last_known_visible_height: Cell::new(0),
            },
            focused_pane: FlutterVersionPane::SdkInfo,
            visible: true,
            status_message: None,
        }
    }

    #[test]
    fn test_panel_renders_without_panic() {
        let state = test_state();
        let widget = FlutterVersionPanel::new(&state, &IconSet::default());
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
    }

    #[test]
    fn test_too_small_renders_message() {
        let state = test_state();
        let widget = FlutterVersionPanel::new(&state, &IconSet::default());
        let area = Rect::new(0, 0, 30, 8); // too small
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        // Should contain "too small" or similar message
    }

    #[test]
    fn test_no_sdk_shows_not_found() {
        let mut state = test_state();
        state.sdk_info.resolved_sdk = None;
        let pane = SdkInfoPane::new(&state.sdk_info, true, &IconSet::default());
        let area = Rect::new(0, 0, 30, 10);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        // Buffer should contain "No Flutter SDK"
    }

    #[test]
    fn test_loading_state_shows_spinner() {
        let mut state = test_state();
        state.version_list.loading = true;
        let pane = VersionListPane::new(&state.version_list, true, &IconSet::default());
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        // Buffer should contain "Scanning" or similar
    }

    #[test]
    fn test_render_hint_set_during_render() {
        let state = test_state();
        assert_eq!(state.version_list.last_known_visible_height.get(), 0);
        let pane = VersionListPane::new(&state.version_list, true, &IconSet::default());
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        assert!(state.version_list.last_known_visible_height.get() > 0);
    }

    #[test]
    fn test_active_version_has_indicator() {
        let mut state = test_state();
        state.version_list.installed_versions = vec![
            InstalledSdk {
                version: "3.19.0".into(),
                channel: Some("stable".into()),
                path: PathBuf::from("/test"),
                is_active: true,
            },
        ];
        let pane = VersionListPane::new(&state.version_list, true, &IconSet::default());
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        // Check buffer contains the active indicator
    }

    #[test]
    fn test_empty_list_shows_message() {
        let mut state = test_state();
        state.version_list.installed_versions = vec![];
        state.version_list.loading = false;
        let pane = VersionListPane::new(&state.version_list, true, &IconSet::default());
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        // Should contain "No versions" or similar
    }
}
```

### Notes

- **Color constants**: Use the existing theme constants from the codebase (`POPUP_BG`, `BORDER_COLOR`, `TEXT_MUTED`, `ACCENT_COLOR`, etc.). Find them in the existing widget code (e.g., `new_session_dialog/mod.rs` or a shared theme module).
- **`IconSet`**: The widget takes `&IconSet` for emoji/nerd-font icon rendering (same as `NewSessionDialog`). Check the `IconSet` type definition for available icons.
- **`modal_overlay` functions** are in `crates/fdemon-tui/src/widgets/modal_overlay.rs` — `dim_background()`, `render_shadow()`, `clear_area()`, `centered_rect_percent()`. Import and reuse directly.
- **Path truncation**: For the SDK PATH display, truncate from the left if the path exceeds the available width. Use `~` prefix for home directory paths (e.g., `~/fvm/versions/3.19.0/`).
- **Responsive layout**: The horizontal/vertical threshold is based on the **inner** content area width (after border), not the full terminal width. This is consistent with how `NewSessionDialog` decides layout mode.
- **No `channel_selector.rs`**: The PLAN mentions a channel selector widget, but for Phase 2 this is deferred. Channels are shown in the version list alongside version numbers. Phase 3 may add a dedicated channel switching UI.
