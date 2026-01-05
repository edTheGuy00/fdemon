## Task: 05-viewport-scanning

**Objective**: Implement the viewport scanning logic that detects all file references in the visible log entries and populates `LinkHighlightState` with `DetectedLink` entries.

**Depends on**: 03-link-highlight-state-types, 04-ui-mode-and-messages

### Background

When the user enters Link Highlight Mode, we need to scan all visible log entries in the viewport to find file references. Each detected reference is assigned a shortcut key and stored in `LinkHighlightState`.

The scanning must:
1. Iterate over only the visible entries (virtualized rendering)
2. Respect the current filter state (only scan filtered entries)
3. Extract file refs from both log messages and stack frames
4. Assign shortcut keys in order (1-9, then a-z)
5. Track viewport line numbers for rendering

### Scope

- `src/tui/hyperlinks.rs`:
  - Add `LinkHighlightState::scan_viewport()` method
  - Add helper method `scan_entry_for_links()`

- `src/app/session.rs`:
  - Add `link_highlight_state: LinkHighlightState` field to `Session` struct
  - Initialize in constructor

### New Method: `scan_viewport()`

```rust
impl LinkHighlightState {
    /// Scan visible log entries for file references and populate links.
    ///
    /// This method is called when entering link highlight mode. It scans
    /// the entries in the visible viewport range, extracts file references
    /// from both log messages and stack frames, and assigns shortcut keys.
    ///
    /// # Arguments
    ///
    /// * `logs` - The log entries (VecDeque for virtualization)
    /// * `visible_start` - Start index of visible range
    /// * `visible_end` - End index of visible range (exclusive)
    /// * `filter_state` - Optional filter to apply
    /// * `collapse_state` - Stack trace collapse state for determining visible frames
    ///
    /// # Example
    ///
    /// ```
    /// let mut state = LinkHighlightState::new();
    /// state.scan_viewport(&logs, 0, 20, Some(&filter), &collapse_state);
    /// state.activate();
    /// ```
    pub fn scan_viewport(
        &mut self,
        logs: &VecDeque<LogEntry>,
        visible_start: usize,
        visible_end: usize,
        filter_state: Option<&FilterState>,
        collapse_state: &CollapseState,
        default_collapsed: bool,
        max_collapsed_frames: usize,
    ) {
        self.clear_links();
        
        let mut link_index: usize = 0;
        let mut viewport_line: usize = 0;
        
        // Build filtered indices if filter is active
        let filtered_indices: Vec<usize> = if let Some(filter) = filter_state {
            logs.iter()
                .enumerate()
                .filter(|(_, entry)| filter.matches(entry))
                .map(|(i, _)| i)
                .collect()
        } else {
            (0..logs.len()).collect()
        };
        
        // Iterate visible entries
        for &idx in filtered_indices
            .iter()
            .skip(visible_start)
            .take(visible_end.saturating_sub(visible_start))
        {
            if link_index >= MAX_LINK_SHORTCUTS {
                break;
            }
            
            let entry = &logs[idx];
            
            // Check log message for file reference
            if let Some(file_ref) = extract_file_ref_from_message(&entry.message) {
                if let Some(shortcut) = Self::shortcut_for_index(link_index) {
                    self.add_link(DetectedLink::new(
                        file_ref,
                        idx,
                        None, // No frame index - from message
                        shortcut,
                        viewport_line,
                    ));
                    link_index += 1;
                }
            }
            
            viewport_line += 1; // Count the message line
            
            // Check stack trace frames if present
            if let Some(ref trace) = entry.stack_trace {
                let is_expanded = collapse_state
                    .expanded_entries
                    .contains(&entry.id)
                    ^ default_collapsed;
                
                let frames_to_scan = if is_expanded {
                    trace.frames.len()
                } else {
                    max_collapsed_frames.min(trace.frames.len())
                };
                
                for (frame_idx, frame) in trace.frames.iter().take(frames_to_scan).enumerate() {
                    if link_index >= MAX_LINK_SHORTCUTS {
                        break;
                    }
                    
                    if let Some(file_ref) = FileReference::from_stack_frame(frame) {
                        if let Some(shortcut) = Self::shortcut_for_index(link_index) {
                            self.add_link(DetectedLink::new(
                                file_ref,
                                idx,
                                Some(frame_idx),
                                shortcut,
                                viewport_line,
                            ));
                            link_index += 1;
                        }
                    }
                    
                    viewport_line += 1; // Count each frame line
                }
                
                // Account for collapsed indicator line if applicable
                if !is_expanded && trace.frames.len() > max_collapsed_frames {
                    viewport_line += 1;
                }
            }
        }
    }
}
```

### Changes to `src/app/session.rs`

Add the `link_highlight_state` field to the `Session` struct:

```rust
use crate::tui::hyperlinks::LinkHighlightState;

pub struct Session {
    // ... existing fields ...
    
    /// State for link highlight mode (Phase 3.1)
    pub link_highlight_state: LinkHighlightState,
}

impl Session {
    pub fn new(/* existing params */) -> Self {
        Self {
            // ... existing field initializations ...
            link_highlight_state: LinkHighlightState::new(),
        }
    }
}
```

### Scanning Strategy

The scanning follows the same logic as the renderer to ensure consistency:

1. **Filter Application**: Only scan entries that pass the current filter
2. **Collapse State**: Respect whether stack traces are expanded or collapsed
3. **Viewport Lines**: Track line numbers to help the renderer position highlights
4. **Order Preservation**: Scan top-to-bottom so shortcuts are assigned in reading order

### Integration with Existing Code

The scanning logic mirrors `LogView::render()` in these ways:
- Uses the same filter matching logic
- Respects the same collapse state
- Uses `extract_file_ref_from_message()` for log messages
- Uses `FileReference::from_stack_frame()` for stack frames

### Required Imports

In `hyperlinks.rs`:
```rust
use std::collections::VecDeque;
use crate::core::{FilterState, LogEntry};
use crate::tui::widgets::CollapseState;
```

### Acceptance Criteria

1. `LinkHighlightState::scan_viewport()` implemented
2. Correctly extracts file refs from log messages
3. Correctly extracts file refs from visible stack frames
4. Respects filter state (only scans filtered entries)
5. Respects collapse state (only scans visible frames)
6. Assigns shortcut keys in order (1-9, a-z)
7. Stops at MAX_LINK_SHORTCUTS (35) links
8. Tracks viewport line numbers correctly
9. `Session.link_highlight_state` field added
10. Unit tests for scan_viewport()

### Testing

#### Unit Tests

```rust
#[cfg(test)]
mod scan_tests {
    use super::*;
    use crate::core::{LogEntry, LogLevel, LogSource, StackTrace, StackFrame};
    use std::collections::VecDeque;

    fn make_entry(id: u64, message: &str) -> LogEntry {
        LogEntry {
            id,
            timestamp: chrono::Local::now(),
            level: LogLevel::Info,
            source: LogSource::App,
            message: message.to_string(),
            stack_trace: None,
            raw: None,
        }
    }

    fn make_entry_with_trace(id: u64, message: &str, frames: Vec<&str>) -> LogEntry {
        let mut entry = make_entry(id, message);
        entry.stack_trace = Some(StackTrace {
            frames: frames.iter().map(|f| StackFrame {
                number: 0,
                function: "test".to_string(),
                location: f.to_string(),
                is_async_gap: false,
                is_project_frame: true,
            }).collect(),
        });
        entry
    }

    #[test]
    fn test_scan_finds_message_links() {
        let mut logs = VecDeque::new();
        logs.push_back(make_entry(1, "Error at lib/main.dart:42"));
        logs.push_back(make_entry(2, "No link here"));
        logs.push_back(make_entry(3, "Another at lib/test.dart:10:5"));
        
        let mut state = LinkHighlightState::new();
        let collapse = CollapseState::default();
        state.scan_viewport(&logs, 0, 3, None, &collapse, true, 3);
        
        assert_eq!(state.link_count(), 2);
        assert_eq!(state.links[0].shortcut, '1');
        assert_eq!(state.links[1].shortcut, '2');
    }

    #[test]
    fn test_scan_finds_stack_frame_links() {
        let mut logs = VecDeque::new();
        logs.push_back(make_entry_with_trace(
            1,
            "Exception occurred",
            vec!["lib/widget.dart:15:3", "lib/app.dart:100"],
        ));
        
        let mut state = LinkHighlightState::new();
        let collapse = CollapseState::default();
        state.scan_viewport(&logs, 0, 1, None, &collapse, false, 10);
        
        // Should find 2 frame links (message has no dart file ref)
        assert_eq!(state.link_count(), 2);
        assert!(state.links[0].frame_index.is_some());
        assert!(state.links[1].frame_index.is_some());
    }

    #[test]
    fn test_scan_respects_collapsed_frames() {
        let mut logs = VecDeque::new();
        logs.push_back(make_entry_with_trace(
            1,
            "Error",
            vec![
                "lib/a.dart:1",
                "lib/b.dart:2",
                "lib/c.dart:3",
                "lib/d.dart:4",
                "lib/e.dart:5",
            ],
        ));
        
        let mut state = LinkHighlightState::new();
        let collapse = CollapseState::default();
        // max_collapsed_frames = 2, default_collapsed = true
        state.scan_viewport(&logs, 0, 1, None, &collapse, true, 2);
        
        // Should only find 2 frames (collapsed)
        assert_eq!(state.link_count(), 2);
    }

    #[test]
    fn test_scan_stops_at_max_shortcuts() {
        let mut logs = VecDeque::new();
        for i in 0..50 {
            logs.push_back(make_entry(i, &format!("Error at lib/file{}.dart:{}", i, i)));
        }
        
        let mut state = LinkHighlightState::new();
        let collapse = CollapseState::default();
        state.scan_viewport(&logs, 0, 50, None, &collapse, true, 3);
        
        assert_eq!(state.link_count(), MAX_LINK_SHORTCUTS);
    }

    #[test]
    fn test_scan_empty_logs() {
        let logs: VecDeque<LogEntry> = VecDeque::new();
        
        let mut state = LinkHighlightState::new();
        let collapse = CollapseState::default();
        state.scan_viewport(&logs, 0, 0, None, &collapse, true, 3);
        
        assert!(!state.has_links());
    }
}
```

### Edge Cases Handled

1. **Empty logs**: Returns empty links list
2. **No file refs in viewport**: Returns empty links list
3. **More than 35 links**: Stops after assigning all shortcuts
4. **Collapsed stack traces**: Only scans visible frames
5. **Filter active**: Only scans entries matching filter
6. **Mixed content**: Messages and frames both scanned

### Performance Considerations

- Only scans visible viewport (not entire log buffer)
- Early exit when MAX_LINK_SHORTCUTS reached
- No regex compilation per-call (uses static FILE_LINE_PATTERN)
- O(n) where n = visible entries × frames per entry

### Files Changed

| File | Change Type |
|------|-------------|
| `src/tui/hyperlinks.rs` | Modified - add scan_viewport() |
| `src/app/session.rs` | Modified - add link_highlight_state field |

### Estimated Time

3-4 hours

---

## Completion Summary

**Status:** ✅ Done

### Files Modified

| File | Change |
|------|--------|
| `src/tui/hyperlinks.rs` | Added `scan_viewport()` method to `LinkHighlightState`, added 10 unit tests |
| `src/app/session.rs` | Added `link_highlight_state: LinkHighlightState` field to `Session`, initialized in constructor |

### Implementation Details

1. **`scan_viewport()` method** - Scans visible log entries for file references:
   - Iterates only visible entries (respects `visible_start`, `visible_end`)
   - Applies filter if active (only scans filtered entries)
   - Respects collapse state (only scans visible frames)
   - Extracts file refs from both log messages and stack frames
   - Assigns shortcut keys in order ('1'-'9', then 'a'-'z')
   - Tracks viewport line numbers for rendering
   - Stops at `MAX_LINK_SHORTCUTS` (35) links

2. **Session integration**:
   - Added `link_highlight_state: LinkHighlightState` field
   - Initialized with `LinkHighlightState::new()` in constructor
   - Imported from `crate::tui::hyperlinks::LinkHighlightState`

### Testing Performed

```
cargo check    # ✅ Passed
cargo test hyperlinks    # ✅ 48 tests passed (10 new tests for scan_viewport)
```

### New Tests Added

- `test_scan_finds_message_links` - Finds file refs in log messages
- `test_scan_finds_stack_frame_links` - Finds file refs in stack frames
- `test_scan_respects_collapsed_frames` - Only scans visible frames when collapsed
- `test_scan_stops_at_max_shortcuts` - Caps at 35 links
- `test_scan_empty_logs` - Handles empty log buffer
- `test_scan_respects_visible_range` - Only scans specified range
- `test_scan_with_filter` - Only scans entries matching filter
- `test_scan_mixed_message_and_frames` - Both message and frame refs detected
- `test_scan_shortcuts_in_order` - Shortcuts assigned correctly ('1'-'9', 'a'-'z')
- `test_scan_clears_previous_links` - Clears links before re-scan

### Notable Decisions

- Filter check: Only builds filtered indices if filter is actually active (`filter.is_active()`)
- Collapse state: Uses `collapse_state.is_expanded()` which handles both default modes
- Viewport line tracking: Increments for messages, frames, AND collapsed indicator line
- Message before frames: If message has file ref, it gets first shortcut, then frames

### Risks/Limitations

None identified. The scanning logic mirrors `LogView::render()` to ensure consistency.