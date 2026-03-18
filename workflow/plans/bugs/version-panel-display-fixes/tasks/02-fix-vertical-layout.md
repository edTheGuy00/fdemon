## Task: Fix SDK Info Clipping in Vertical Layout and Small Terminals

**Objective**: Ensure all SDK info fields are visible at supported terminal sizes by fixing the vertical layout height constant and adding a compact rendering mode for very constrained heights.

**Depends on**: 01-fix-tab-label (label now always consumes 2 rows)

### Scope

- `crates/fdemon-tui/src/widgets/flutter_version_panel/mod.rs`: Update `VERTICAL_SDK_INFO_HEIGHT` constant
- `crates/fdemon-tui/src/widgets/flutter_version_panel/sdk_info.rs`: Add compact field layout for tight heights

### Details

**Current issue:**
- `VERTICAL_SDK_INFO_HEIGHT = 6` (mod.rs:70) is used in `render_vertical_panes()` to cap the SDK info pane height
- After task 01, the SDK info needs: 2 (header) + 2 (VERSION/CHANNEL) + 1 (spacer) + 2 (SOURCE/PATH) + 1 (spacer) + 2 (DART) = 10 rows for expanded layout
- At 6 rows, the DART SDK field and part of SOURCE/PATH are clipped

**Fix approach:**

1. **Update `VERTICAL_SDK_INFO_HEIGHT`** from `6` to `10`:

```rust
/// Height of the left pane in vertical (stacked) layout.
///
/// Derived from: 2 header + 2 VERSION/CHANNEL + 1 spacer + 2 SOURCE/PATH + 1 spacer + 2 DART = 10.
const VERTICAL_SDK_INFO_HEIGHT: u16 = 10;
```

2. **Add compact mode to `SdkInfoPane`** — When the content area height is too small for the expanded 3-row-group layout (< 8 rows after header), use a compact single-line-per-field layout:

```rust
/// Minimum content-area height for expanded (2-row-per-field) layout.
///
/// Derived from: 3 field groups × 2 rows + 2 spacers = 8 rows.
const MIN_EXPANDED_CONTENT_HEIGHT: u16 = 8;

fn render_sdk_details(&self, sdk: &FlutterSdk, area: Rect, buf: &mut Buffer) {
    if area.height < MIN_EXPANDED_CONTENT_HEIGHT {
        self.render_sdk_details_compact(sdk, area, buf);
    } else {
        self.render_sdk_details_expanded(sdk, area, buf);
    }
}
```

Compact layout renders each field as a single line: `"  LABEL: value"` with no spacer rows between groups. This fits the essential info in ~5 rows:

```
  VERSION: 3.38.6  CHANNEL: stable
  SOURCE: system PATH
  SDK PATH: ~/Dev/flutter
  DART: 3.10.7
```

3. **Ensure `MIN_RENDER_HEIGHT` is correct**: Currently 13. With the updated layout: 2 border + 3 header + 1 sep + 5 min content + 1 sep + 1 footer = 13 — still correct.

### Acceptance Criteria

1. All 5 SDK info fields are visible in vertical (stacked) layout at `VERTICAL_SDK_INFO_HEIGHT`
2. All 5 SDK info fields are visible in horizontal layout at minimum dialog height
3. Compact mode activates when content area height < `MIN_EXPANDED_CONTENT_HEIGHT`
4. Compact mode shows VERSION, CHANNEL, SOURCE, SDK PATH, and DART SDK on fewer rows
5. No field clipping at any supported terminal size (minimum: `MIN_RENDER_HEIGHT`)
6. Layout decisions follow Architecture Principle 1 (based on available space, not orientation)
7. All constants have doc comments with derivation (Architecture Principle 4)

### Testing

```rust
#[test]
fn test_sdk_info_compact_mode_all_fields_visible() {
    let state = make_state_with_sdk();
    let pane = SdkInfoPane::new(&state, true);
    // Very tight area: only 6 rows for content (compact mode)
    let area = Rect::new(0, 0, 40, 8);
    let mut buf = Buffer::empty(area);
    pane.render(area, &mut buf);
    let content: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(content.contains("3.19.0"), "compact should show version");
    assert!(content.contains("stable"), "compact should show channel");
    assert!(content.contains("3.3.0"), "compact should show dart version");
}

#[test]
fn test_sdk_info_expanded_mode_with_spacers() {
    let state = make_state_with_sdk();
    let pane = SdkInfoPane::new(&state, true);
    // Comfortable area: expanded mode
    let area = Rect::new(0, 0, 40, 15);
    let mut buf = Buffer::empty(area);
    pane.render(area, &mut buf);
    let content: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(content.contains("3.19.0"), "expanded should show version");
    assert!(content.contains("3.3.0"), "expanded should show dart version");
}
```

### Notes

- The compact mode should be minimal — just a different layout for the same data, not a different set of fields.
- After this task, the SDK info pane gracefully handles all sizes from MIN_RENDER_HEIGHT up.
- Future task 04 will extend the field grid; compact mode will need to accommodate additional fields too.

---

## Completion Summary

**Status:** Not Started
