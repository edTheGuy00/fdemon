## Task: OSC 8 Hyperlink Rendering

**Objective**: Integrate OSC 8 hyperlink escape sequences into terminal output so that file:line references in stack traces become clickable in supported terminals. When running in an IDE's integrated terminal, use IDE-specific URL schemes so Ctrl+click (or Cmd+click on macOS) opens files in that IDE.

**Depends on**: 
- [01-hyperlink-module-url-generation](01-hyperlink-module-url-generation.md) - OSC 8 sequence generation
- [02-editor-configuration](02-editor-configuration.md) - Configuration settings and parent IDE detection
- [05-terminal-capability-detection](05-terminal-capability-detection.md) - Terminal support detection

**Status**: Experimental - May require custom rendering approach

### Performance Benefits from Bug Fix Work

The logger block propagation bug fix implemented virtualization that significantly simplifies this task:

| Optimization | Impact on Hyperlinks |
|--------------|---------------------|
| **Virtualization** | Only ~30-50 entries rendered per frame → HyperlinkMap contains ~30-50 regions max |
| **visible_range()** | Efficient bounds for hyperlink region tracking |
| **VecDeque storage** | Indexing unchanged; no code modifications needed |
| **Buffer lines** | Pre-rendered entries above/below viewport included in hyperlink tracking |

**Key Implication**: `HyperlinkMap` is rebuilt each frame with only visible entries. No need to track hyperlinks for all 10,000+ logs in the buffer.

### Scope

- `src/tui/hyperlinks.rs`: Hyperlink rendering helpers, `HyperlinkMap` struct, IDE-specific URL generation
- `src/tui/render.rs`: Integration with frame rendering
- `src/tui/terminal.rs`: Terminal output handling (if needed)
- `src/tui/widgets/log_view.rs`: Mark hyperlink regions during render (leverages existing virtualization)

### Ctrl+Click / Cmd+Click Support

The key to making hyperlinks work well is using the **correct URL scheme**:

| Scenario | URL Scheme | Example | Ctrl+Click Behavior |
|----------|------------|---------|---------------------|
| Running in VS Code terminal | `vscode://` | `vscode://file/path/file.dart:42:10` | Opens in current VS Code instance |
| Running in Cursor terminal | `cursor://` | `cursor://file/path/file.dart:42:10` | Opens in current Cursor instance |
| Running in Zed terminal | `zed://` | `zed://file/path/file.dart:42` | Opens in current Zed instance |
| Running in IntelliJ terminal | `idea://` | `idea://open?file=/path/file.dart&line=42` | Opens in current IntelliJ instance |
| Running in plain terminal | `file://` | `file:///path/file.dart` | Opens with default handler (may not work well) |

**Key Insight**: `file://` URLs often just reveal files in Finder/Explorer. IDE-specific URL schemes actually open the file in the editor at the correct line!

### Background

Ratatui uses crossterm as its backend, rendering to an internal buffer and then diffing/flushing to the terminal. OSC 8 hyperlinks require escape sequences to wrap text:

```
ESC ] 8 ; ; URI ST text ESC ] 8 ; ; ST
```

Where:
- `ESC ] 8 ; ;` starts the hyperlink with empty params
- `URI` is the target (e.g., `file:///path/to/file.dart#L42`)
- `ST` is String Terminator (`ESC \`)
- `text` is the visible clickable text
- `ESC ] 8 ; ; ST` closes the hyperlink

The challenge is that Ratatui's cell-based rendering doesn't natively support OSC 8. We need to inject these sequences at the right point in the rendering pipeline.

Additionally, we need to generate the **right URL scheme** based on the detected parent IDE (from Task 02) so that Ctrl+click opens files in the current IDE instance.

### Approach Options

#### Option A: Post-Process Buffer Output (Recommended)

Intercept the terminal write after Ratatui's flush and inject OSC 8 sequences for marked cells.

```rust
// Concept: Custom backend wrapper
struct HyperlinkBackend<B: Backend> {
    inner: B,
    hyperlink_regions: Vec<HyperlinkRegion>,
}

struct HyperlinkRegion {
    start_x: u16,
    start_y: u16,
    end_x: u16,
    end_y: u16,
    url: String,
}
```

**Pros:**
- Clean separation from Ratatui internals
- Works with existing rendering code

**Cons:**
- Requires tracking regions separately
- May have performance overhead

#### Option B: Direct Terminal Write

Write OSC 8 sequences directly to stdout at specific moments, bypassing Ratatui for hyperlinked content.

```rust
use std::io::Write;
use crate::config::{detect_parent_ide, ParentIde};

fn write_hyperlink(stdout: &mut impl Write, text: &str, url: &str) -> io::Result<()> {
    write!(stdout, "\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, text)
}

/// Generate a URL using the appropriate scheme for the detected parent IDE
/// This enables Ctrl+click to open files in the current IDE instance
fn ide_aware_file_url(file_ref: &FileReference, project_root: &Path) -> String {
    // Resolve the absolute path first
    let abs_path = file_ref.resolve_path(project_root);
    let path_str = abs_path.display().to_string();
    
    if let Some(ide) = detect_parent_ide() {
        match ide {
            ParentIde::VSCode => {
                // vscode://file/path:line:column
                format!("vscode://file{}:{}:{}", path_str, file_ref.line, file_ref.column.max(1))
            }
            ParentIde::VSCodeInsiders => {
                format!("vscode-insiders://file{}:{}:{}", path_str, file_ref.line, file_ref.column.max(1))
            }
            ParentIde::Cursor => {
                format!("cursor://file{}:{}:{}", path_str, file_ref.line, file_ref.column.max(1))
            }
            ParentIde::Zed => {
                // Zed uses zed://file/path:line (no column)
                format!("zed://file{}:{}", path_str, file_ref.line)
            }
            ParentIde::IntelliJ | ParentIde::AndroidStudio => {
                // JetBrains uses idea://open?file=path&line=N
                format!("idea://open?file={}&line={}", 
                    urlencoding::encode(&path_str), 
                    file_ref.line)
            }
            ParentIde::Neovim => {
                // Neovim doesn't have a URL scheme, fall back to file://
                file_url_with_position(file_ref)
            }
        }
    } else {
        // No parent IDE detected, use standard file:// URL
        file_url_with_position(file_ref)
    }
}
```

**Note**: Add `urlencoding` to Cargo.toml dependencies for JetBrains URL encoding, or use a simple percent-encoding function.

**Pros:**
- Simple and direct
- No Ratatui modifications needed

**Cons:**
- May conflict with Ratatui's cursor positioning
- Harder to integrate with diffing

#### Option C: Custom Cell Attribute

Use Ratatui's style system to mark cells, then handle specially during flush.

```rust
// Mark cells with a special attribute
let hyperlink_style = Style::default()
    .add_modifier(Modifier::UNDERLINED)
    .fg(Color::Blue);

// Store URL in a parallel data structure keyed by position
```

### Implementation (Option A - Post-Process)

#### 1. Hyperlink Region Tracking

```rust
// In src/tui/hyperlinks.rs

use std::collections::HashMap;

/// Tracks hyperlink regions on the screen
#[derive(Debug, Default)]
pub struct HyperlinkMap {
    /// Map of (y, x_start, x_end) -> URL
    regions: Vec<HyperlinkRegion>,
}

#[derive(Debug, Clone)]
pub struct HyperlinkRegion {
    pub y: u16,
    pub x_start: u16,
    pub x_end: u16,
    pub url: String,
}

impl HyperlinkMap {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn clear(&mut self) {
        self.regions.clear();
    }
    
    pub fn add_region(&mut self, y: u16, x_start: u16, x_end: u16, url: String) {
        self.regions.push(HyperlinkRegion { y, x_start, x_end, url });
    }
    
    /// Get all regions, sorted by position
    pub fn regions(&self) -> &[HyperlinkRegion] {
        &self.regions
    }
    
    /// Get regions for a specific line
    pub fn regions_for_line(&self, y: u16) -> impl Iterator<Item = &HyperlinkRegion> {
        self.regions.iter().filter(move |r| r.y == y)
    }
}
```

#### 2. Mark Hyperlinks During Render

```rust
// In src/tui/widgets/log_view.rs

impl<'a> StatefulWidget for LogView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut LogViewState) {
        // Clear previous hyperlink regions (rebuilt each frame)
        state.hyperlink_map.clear();
        
        // ... existing rendering logic ...
        
        // PERFORMANCE: Thanks to virtualization, we only process visible entries
        // visible_range() returns ~30-50 entries max (visible + buffer_lines)
        let (visible_start, visible_end) = state.visible_range();
        
        // When rendering a stack frame with file:line
        // Note: Only visible frames are iterated - not all 10,000+ logs
        for frame in visible_frames {
            if !frame.is_async_gap && hyperlinks_enabled {
                // Calculate the screen position of the file:line text
                let x_start = /* calculated position */;
                let x_end = /* calculated end position */;
                let y = /* current line */;
                
                /// Create file URL with appropriate scheme for parent IDE
                                let file_ref = FileReference::from_stack_frame(frame)?;
                                let url = ide_aware_file_url(&file_ref, project_root);
                
                // Register hyperlink region
                // HyperlinkMap typically contains ~30-50 regions (not thousands)
                state.hyperlink_map.add_region(y, x_start, x_end, url);
            }
        }
    }
}
```

#### 3. Custom Terminal Writer

```rust
// In src/tui/terminal.rs

use std::io::{self, Write};
use crossterm::{cursor, terminal, QueueableCommand};

/// A writer that can inject OSC 8 sequences
pub struct HyperlinkWriter<W: Write> {
    inner: W,
    hyperlink_map: HyperlinkMap,
    current_line: u16,
    current_col: u16,
    in_hyperlink: bool,
}

impl<W: Write> HyperlinkWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            hyperlink_map: HyperlinkMap::new(),
            current_line: 0,
            current_col: 0,
            in_hyperlink: false,
        }
    }
    
    pub fn set_hyperlink_map(&mut self, map: HyperlinkMap) {
        self.hyperlink_map = map;
    }
    
    fn start_hyperlink(&mut self, url: &str) -> io::Result<()> {
        if !self.in_hyperlink {
            write!(self.inner, "\x1b]8;;{}\x1b\\", url)?;
            self.in_hyperlink = true;
        }
        Ok(())
    }
    
    fn end_hyperlink(&mut self) -> io::Result<()> {
        if self.in_hyperlink {
            write!(self.inner, "\x1b]8;;\x1b\\")?;
            self.in_hyperlink = false;
        }
        Ok(())
    }
}
```

#### 4. Integration with Terminal Flush

```rust
// In src/tui/runner.rs or main loop

use crate::tui::hyperlinks::{HyperlinkMode, hyperlink_support};

fn render_frame(terminal: &mut Terminal<impl Backend>, state: &mut AppState) -> Result<()> {
    let hyperlinks_enabled = state.settings.ui.hyperlinks.should_enable();
    
    terminal.draw(|frame| {
        view(frame, state);
    })?;
    
    // If hyperlinks are enabled and we have regions, inject OSC 8 sequences
    if hyperlinks_enabled {
        if let Some(session) = state.session_manager.selected() {
            inject_hyperlinks(
                &mut std::io::stdout(),
                &session.session.log_view_state.hyperlink_map,
            )?;
        }
    }
    
    Ok(())
}

/// Inject OSC 8 sequences after Ratatui has rendered
fn inject_hyperlinks<W: Write>(
    writer: &mut W,
    hyperlink_map: &HyperlinkMap,
) -> io::Result<()> {
    use crossterm::{cursor::MoveTo, ExecutableCommand};
    
    for region in hyperlink_map.regions() {
        // Save cursor position
        writer.execute(cursor::SavePosition)?;
        
        // Move to start of hyperlink region
        writer.execute(MoveTo(region.x_start, region.y))?;
        
        // Start hyperlink
        write!(writer, "\x1b]8;;{}\x1b\\", region.url)?;
        
        // Move to end of hyperlink region
        writer.execute(MoveTo(region.x_end, region.y))?;
        
        // End hyperlink
        write!(writer, "\x1b]8;;\x1b\\")?;
        
        // Restore cursor position
        writer.execute(cursor::RestorePosition)?;
    }
    
    writer.flush()?;
    Ok(())
}
```

### Alternative: Simpler Inline Approach

If the post-process approach proves too complex, use a simpler inline approach for Phase 3:

```rust
// In log_view.rs, when formatting stack frame file references

fn format_file_reference_with_hyperlink(
    frame: &StackFrame,
    hyperlinks_enabled: bool,
) -> String {
    let display_text = frame.display_location();
    
    if hyperlinks_enabled && !frame.is_async_gap {
        let file_ref = FileReference::from_stack_frame(frame).unwrap();
        let url = file_url_with_position(&file_ref);
        osc8_wrap(&display_text, &url)
    } else {
        display_text
    }
}

// This embeds OSC 8 in the Span text directly
// May or may not work depending on crossterm's handling
```

### Configuration

Add to UiSettings:

```rust
// In src/config/types.rs

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UiSettings {
    // ... existing fields ...
    
    /// Enable terminal hyperlinks (OSC 8)
    #[serde(default)]
    pub hyperlinks: HyperlinkMode,
}
```

```toml
# .fdemon/config.toml

[ui]
# Terminal hyperlinks: "auto", "enabled", "disabled"
hyperlinks = "auto"
```

### LogViewState Update

```rust
// Add to LogViewState (note: existing fields from bug fix work)
pub struct LogViewState {
    // ... existing fields (offset, h_offset, auto_scroll, etc.) ...
    
    /// Buffer lines above/below viewport (added in Task 05)
    pub buffer_lines: usize,
    
    /// Focus info for file opening (added in Task 03)
    pub focus_info: FocusInfo,
    
    /// Map of hyperlink regions for OSC 8 rendering (NEW)
    /// Rebuilt each frame with only visible entries (~30-50 max)
    pub hyperlink_map: HyperlinkMap,
}
```

### Acceptance Criteria

1. [ ] `HyperlinkMap` struct tracks hyperlink regions on screen
2. [ ] Stack frame file references marked as hyperlink regions during render
3. [ ] OSC 8 sequences injected after Ratatui flush (or inline)
4. [ ] Hyperlinks only enabled when `hyperlinks != disabled` AND terminal supports
5. [ ] **IDE-specific URL schemes used when parent IDE detected**
6. [ ] **Ctrl+click in VS Code terminal opens file in that VS Code instance**
7. [ ] **Ctrl+click in Cursor/Zed/IntelliJ terminals opens in those IDEs**
8. [ ] Clicking hyperlink in iTerm2/Kitty opens file URL
9. [ ] No visual artifacts in unsupported terminals
10. [ ] Hyperlinks don't interfere with Ratatui's screen diffing
11. [ ] Configuration option to force enable/disable
12. [ ] Graceful degradation when detection fails

### Testing

#### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hyperlink_map_add_region() {
        let mut map = HyperlinkMap::new();
        map.add_region(10, 5, 20, "file:///test.dart".to_string());
        
        assert_eq!(map.regions().len(), 1);
        assert_eq!(map.regions()[0].y, 10);
        assert_eq!(map.regions()[0].x_start, 5);
    }
    
    #[test]
    fn test_hyperlink_map_regions_for_line() {
        let mut map = HyperlinkMap::new();
        map.add_region(10, 5, 20, "url1".to_string());
        map.add_region(10, 25, 40, "url2".to_string());
        map.add_region(11, 5, 20, "url3".to_string());
        
        let line_10: Vec<_> = map.regions_for_line(10).collect();
        assert_eq!(line_10.len(), 2);
    }
    
    #[test]
    fn test_hyperlink_map_clear() {
        let mut map = HyperlinkMap::new();
        map.add_region(10, 5, 20, "url".to_string());
        map.clear();
        
        assert!(map.regions().is_empty());
    }
}
```

#### Manual Testing

**In VS Code Terminal:**
1. Run Flutter Demon from VS Code's integrated terminal
2. Trigger an error with stack trace
3. Hover over file:line reference - cursor should change
4. Ctrl+click (or Cmd+click on macOS) on hyperlink
5. Verify file opens **in the same VS Code window** at the correct line

**In Cursor/Zed Terminal:**
1. Run Flutter Demon from Cursor's or Zed's integrated terminal
2. Trigger an error with stack trace
3. Ctrl+click on file reference
4. Verify file opens **in that IDE instance**

**In Plain Terminal (iTerm2, Kitty, etc.):**
1. Configure `hyperlinks = "enabled"` in config
2. Run in iTerm2 (known to support OSC 8)
3. Trigger an error with stack trace
4. Hover over file:line reference - cursor should change
5. Cmd+click on hyperlink
6. Verify file opens (may open in default handler for file:// URLs)

#### Compatibility Testing Matrix

| Terminal | Test Status | URL Scheme | Notes |
|----------|-------------|------------|-------|
| VS Code Terminal | | `vscode://` | Ctrl+click opens in same VS Code instance |
| Cursor Terminal | | `cursor://` | Ctrl+click opens in same Cursor instance |
| Zed Terminal | | `zed://` | Ctrl+click opens in same Zed instance |
| IntelliJ Terminal | | `idea://` | Ctrl+click opens in same IntelliJ instance |
| iTerm2 | | `file://` | Cmd+click should work |
| Kitty | | `file://` | Click should work |
| WezTerm | | `file://` | Click should work |
| macOS Terminal | | N/A | Should not show garbage (OSC 8 unsupported) |
| Alacritty | | `file://` | Click should work |
| tmux | | Passthrough | May require passthrough config |

### Known Issues & Limitations

1. **Ratatui Diffing**: OSC 8 sequences may be stripped or cause issues with Ratatui's diff algorithm
2. **Cursor Positioning**: Injecting sequences after render may cause cursor position issues
3. **Screen Refresh**: Full redraws may be needed to maintain hyperlinks
4. **URL Length**: Very long file paths may cause issues in some terminals
5. **file:// Protocol**: Some terminals may not handle file:// URLs, only http(s)://
6. **IDE URL Scheme Registration**: IDE URL schemes require the IDE to be properly installed and registered with the OS
7. **Nested Terminals**: Running in tmux inside VS Code terminal may not detect the parent IDE correctly

### Fallback Plan

If OSC 8 integration proves too complex or unreliable:

1. Mark this task as "Deferred"
2. Focus on the `o` key functionality (Task 04) as the primary file-opening method
3. Document OSC 8 as "experimental" and provide manual enable config
4. Consider revisiting when Ratatui has native hyperlink support

### Notes

- This is the most complex task in Phase 3
- Success depends on terminal behavior and Ratatui's handling of escape sequences
- The `o` key (Task 04) provides fallback functionality regardless of OSC 8 success
- Consider adding `--hyperlinks=on|off|auto` CLI flag for quick testing
- **Performance**: With virtualization from bug fix work, HyperlinkMap operations are O(visible_entries) not O(total_logs)
- **VecDeque Compatibility**: Log storage is now `VecDeque<LogEntry>` but indexing works identically to `Vec`
- **IDE URL Schemes are Key**: Using `vscode://`, `cursor://`, `zed://`, `idea://` schemes makes Ctrl+click work properly
- **Parent IDE Detection**: Leverages `detect_parent_ide()` from Task 02 to choose the right URL scheme
- **Fallback**: If no parent IDE detected, falls back to `file://` URLs (which may not open in an editor)

### References

- [OSC 8 Hyperlink Specification](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda)
- [Ratatui Backend Trait](https://docs.rs/ratatui/latest/ratatui/backend/trait.Backend.html)
- [Crossterm Terminal Control](https://docs.rs/crossterm/latest/crossterm/)

### Estimated Time

3-4 hours (reduced due to virtualization limiting hyperlink scope; may require additional iteration)

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/hyperlinks.rs` | Add `HyperlinkMap`, `HyperlinkRegion`, `ide_aware_file_url()` |
| `src/tui/widgets/log_view.rs` | Track hyperlink regions during render |
| `src/tui/render.rs` or `runner.rs` | Inject OSC 8 after frame flush |
| `src/config/types.rs` | Add `hyperlinks` field to `UiSettings` |
| `src/tui/terminal.rs` | Potentially add custom writer |
| `Cargo.toml` | Add `urlencoding` dependency (for JetBrains URL encoding) |