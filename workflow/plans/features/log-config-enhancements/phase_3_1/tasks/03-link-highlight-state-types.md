## Task: 03-link-highlight-state-types

**Objective**: Add the new `DetectedLink` and `LinkHighlightState` types to `tui/hyperlinks.rs` that will power the Link Highlight Mode feature.

**Depends on**: 02-remove-osc8-code

### Background

After removing the OSC 8 code, we need to add new types that support the Link Highlight Mode feature. These types will:
1. Represent detected links in the viewport with assigned shortcut keys
2. Track the state of link highlight mode (active/inactive, detected links)
3. Provide methods for scanning and looking up links

### Scope

- `src/tui/hyperlinks.rs`:
  - Add `DetectedLink` struct
  - Add `LinkHighlightState` struct
  - Add helper methods for both structs
  - Add unit tests for new types

### New Types

#### `DetectedLink` Struct

```rust
/// A detected file reference link in the visible viewport.
///
/// Each detected link is assigned a shortcut key (1-9, a-z) that the user
/// can press to open that file in their editor.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectedLink {
    /// The file reference (path, line, column)
    pub file_ref: FileReference,
    
    /// Index of the log entry this link belongs to
    pub entry_index: usize,
    
    /// Stack frame index within the entry (None if from log message)
    pub frame_index: Option<usize>,
    
    /// Shortcut key to select this link ('1'-'9', 'a'-'z')
    pub shortcut: char,
    
    /// Display text shown to user (e.g., "lib/main.dart:42:5")
    pub display_text: String,
    
    /// Relative line number within the viewport (0-based)
    pub viewport_line: usize,
}

impl DetectedLink {
    /// Create a new detected link
    pub fn new(
        file_ref: FileReference,
        entry_index: usize,
        frame_index: Option<usize>,
        shortcut: char,
        viewport_line: usize,
    ) -> Self {
        let display_text = file_ref.display();
        Self {
            file_ref,
            entry_index,
            frame_index,
            shortcut,
            display_text,
            viewport_line,
        }
    }
}
```

#### `LinkHighlightState` Struct

```rust
/// State for link highlight mode.
///
/// When the user presses 'L' to enter link mode, this state is populated
/// by scanning the visible viewport for file references. Links are assigned
/// shortcut keys (1-9, then a-z) that the user can press to open files.
#[derive(Debug, Default, Clone)]
pub struct LinkHighlightState {
    /// Detected links in the current viewport (max 35: 1-9 + a-z)
    pub links: Vec<DetectedLink>,
    
    /// Whether link highlight mode is currently active
    pub active: bool,
}

/// Maximum number of links we can assign shortcuts to (9 + 26 = 35)
pub const MAX_LINK_SHORTCUTS: usize = 35;

impl LinkHighlightState {
    /// Create a new inactive link highlight state
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Check if link mode is active
    pub fn is_active(&self) -> bool {
        self.active
    }
    
    /// Activate link mode (links should be populated first via scan)
    pub fn activate(&mut self) {
        self.active = true;
    }
    
    /// Deactivate link mode and clear detected links
    pub fn deactivate(&mut self) {
        self.active = false;
        self.links.clear();
    }
    
    /// Get a link by its shortcut key
    pub fn link_by_shortcut(&self, c: char) -> Option<&DetectedLink> {
        self.links.iter().find(|link| link.shortcut == c)
    }
    
    /// Get the number of detected links
    pub fn link_count(&self) -> usize {
        self.links.len()
    }
    
    /// Check if there are any detected links
    pub fn has_links(&self) -> bool {
        !self.links.is_empty()
    }
    
    /// Clear links without deactivating (for re-scan on scroll)
    pub fn clear_links(&mut self) {
        self.links.clear();
    }
    
    /// Add a detected link (respects MAX_LINK_SHORTCUTS limit)
    pub fn add_link(&mut self, link: DetectedLink) {
        if self.links.len() < MAX_LINK_SHORTCUTS {
            self.links.push(link);
        }
    }
    
    /// Get the shortcut character for a given index (0-34)
    ///
    /// Returns '1'-'9' for indices 0-8, 'a'-'z' for indices 9-34
    pub fn shortcut_for_index(index: usize) -> Option<char> {
        match index {
            0..=8 => Some((b'1' + index as u8) as char),
            9..=34 => Some((b'a' + (index - 9) as u8) as char),
            _ => None,
        }
    }
    
    /// Get all links on a specific viewport line
    pub fn links_on_line(&self, line: usize) -> Vec<&DetectedLink> {
        self.links
            .iter()
            .filter(|link| link.viewport_line == line)
            .collect()
    }
}
```

### Shortcut Key Assignment

Links are assigned shortcuts in order of detection:
- First 9 links: `1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`, `9`
- Next 26 links: `a`, `b`, `c`, ... `z`
- Links beyond 35 are not assigned shortcuts (edge case)

This gives users quick access via number keys for the most relevant links.

### Integration Points

These types will be used by:
- **Task 05**: `scan_viewport()` method will populate `LinkHighlightState.links`
- **Task 06**: Key handlers will use `link_by_shortcut()` to find selected link
- **Task 07**: Renderer will iterate `links` to highlight them in the viewport

### Acceptance Criteria

1. `DetectedLink` struct defined with all fields
2. `LinkHighlightState` struct defined with all fields
3. `MAX_LINK_SHORTCUTS` constant defined (35)
4. All methods implemented:
   - `DetectedLink::new()`
   - `LinkHighlightState::new()`, `is_active()`, `activate()`, `deactivate()`
   - `link_by_shortcut()`, `link_count()`, `has_links()`
   - `clear_links()`, `add_link()`, `shortcut_for_index()`, `links_on_line()`
5. Unit tests for all methods
6. Documentation on all public types and methods
7. No compiler errors or warnings

### Testing

#### Unit Tests

```rust
#[cfg(test)]
mod link_highlight_tests {
    use super::*;

    #[test]
    fn test_detected_link_new() {
        let file_ref = FileReference::new("lib/main.dart", 42, 5);
        let link = DetectedLink::new(file_ref, 0, None, '1', 3);
        
        assert_eq!(link.entry_index, 0);
        assert_eq!(link.frame_index, None);
        assert_eq!(link.shortcut, '1');
        assert_eq!(link.viewport_line, 3);
        assert_eq!(link.display_text, "lib/main.dart:42:5");
    }

    #[test]
    fn test_link_highlight_state_default_inactive() {
        let state = LinkHighlightState::new();
        assert!(!state.is_active());
        assert!(!state.has_links());
    }

    #[test]
    fn test_link_highlight_state_activate_deactivate() {
        let mut state = LinkHighlightState::new();
        state.activate();
        assert!(state.is_active());
        
        state.deactivate();
        assert!(!state.is_active());
    }

    #[test]
    fn test_link_by_shortcut() {
        let mut state = LinkHighlightState::new();
        let link = DetectedLink::new(
            FileReference::new("test.dart", 10, 0),
            0, None, 'a', 0
        );
        state.add_link(link);
        
        assert!(state.link_by_shortcut('a').is_some());
        assert!(state.link_by_shortcut('b').is_none());
    }

    #[test]
    fn test_shortcut_for_index() {
        assert_eq!(LinkHighlightState::shortcut_for_index(0), Some('1'));
        assert_eq!(LinkHighlightState::shortcut_for_index(8), Some('9'));
        assert_eq!(LinkHighlightState::shortcut_for_index(9), Some('a'));
        assert_eq!(LinkHighlightState::shortcut_for_index(34), Some('z'));
        assert_eq!(LinkHighlightState::shortcut_for_index(35), None);
    }

    #[test]
    fn test_max_link_limit() {
        let mut state = LinkHighlightState::new();
        
        for i in 0..40 {
            let link = DetectedLink::new(
                FileReference::new("test.dart", i as u32, 0),
                i, None,
                LinkHighlightState::shortcut_for_index(i).unwrap_or('?'),
                i
            );
            state.add_link(link);
        }
        
        // Should cap at MAX_LINK_SHORTCUTS
        assert_eq!(state.link_count(), MAX_LINK_SHORTCUTS);
    }

    #[test]
    fn test_links_on_line() {
        let mut state = LinkHighlightState::new();
        state.add_link(DetectedLink::new(
            FileReference::new("a.dart", 1, 0), 0, None, '1', 0
        ));
        state.add_link(DetectedLink::new(
            FileReference::new("b.dart", 2, 0), 1, None, '2', 0
        ));
        state.add_link(DetectedLink::new(
            FileReference::new("c.dart", 3, 0), 2, None, '3', 1
        ));
        
        let line_0_links = state.links_on_line(0);
        assert_eq!(line_0_links.len(), 2);
        
        let line_1_links = state.links_on_line(1);
        assert_eq!(line_1_links.len(), 1);
    }

    #[test]
    fn test_deactivate_clears_links() {
        let mut state = LinkHighlightState::new();
        state.add_link(DetectedLink::new(
            FileReference::new("test.dart", 1, 0), 0, None, '1', 0
        ));
        state.activate();
        
        assert!(state.has_links());
        
        state.deactivate();
        assert!(!state.has_links());
        assert!(!state.is_active());
    }
}
```

### Files Changed

| File | Change Type |
|------|-------------|
| `src/tui/hyperlinks.rs` | Modified - add new types |

### Estimated Time

2-3 hours

---

## Completion Summary

**Status:** ✅ Done

### Files Modified

| File | Change |
|------|--------|
| `src/tui/hyperlinks.rs` | Added `DetectedLink`, `LinkHighlightState`, `MAX_LINK_SHORTCUTS`, and 18 unit tests |

### Implementation Details

1. **`DetectedLink` struct** - Represents a detected file reference link with:
   - `file_ref: FileReference` - the actual file path, line, and column
   - `entry_index: usize` - which log entry this belongs to
   - `frame_index: Option<usize>` - optional stack frame index
   - `shortcut: char` - assigned shortcut key ('1'-'9', 'a'-'z')
   - `display_text: String` - formatted display text
   - `viewport_line: usize` - relative line in viewport

2. **`LinkHighlightState` struct** - Manages link highlight mode:
   - `links: Vec<DetectedLink>` - detected links (max 35)
   - `active: bool` - whether link mode is active
   - Methods: `new()`, `is_active()`, `activate()`, `deactivate()`, `link_by_shortcut()`, `link_count()`, `has_links()`, `clear_links()`, `add_link()`, `shortcut_for_index()`, `links_on_line()`

3. **`MAX_LINK_SHORTCUTS` constant** - Set to 35 (9 digits + 26 letters)

### Testing Performed

```
cargo check                    # ✅ Passed
cargo test hyperlinks          # ✅ 38 tests passed (18 new tests)
cargo clippy                   # ✅ No new warnings (pre-existing issue in unrelated file)
```

### New Tests Added

- `test_detected_link_new`
- `test_detected_link_with_frame_index`
- `test_link_highlight_state_default_inactive`
- `test_link_highlight_state_activate_deactivate`
- `test_link_by_shortcut`
- `test_link_by_shortcut_returns_correct_link`
- `test_shortcut_for_index_digits`
- `test_shortcut_for_index_letters`
- `test_shortcut_for_index_out_of_range`
- `test_max_link_limit`
- `test_links_on_line`
- `test_deactivate_clears_links`
- `test_clear_links_preserves_active_state`
- `test_has_links`
- `test_link_count`

### Notable Decisions

- `clear_links()` preserves active state (for re-scan on scroll without exiting link mode)
- `deactivate()` clears links (complete cleanup when exiting link mode)
- Shortcut assignment: first 9 links get '1'-'9', next 26 get 'a'-'z'
- `add_link()` silently ignores links beyond MAX_LINK_SHORTCUTS (edge case handling)

### Risks/Limitations

None identified. Types are pure data structures with simple logic, ready for integration in Tasks 05-07.