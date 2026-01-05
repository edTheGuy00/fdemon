## Task: 02-remove-osc8-code

**Objective**: Remove all OSC 8 terminal hyperlink code from `tui/hyperlinks.rs` and related configuration, as we are replacing this approach with the explicit Link Highlight Mode.

**Depends on**: 01-cleanup-focus-info-auto-detect

### Background

Phase 3 Tasks 05-06 implemented OSC 8 terminal hyperlink support, which:
1. Detects terminal capabilities for hyperlink support
2. Generates file:// URLs and IDE-specific URL schemes
3. Wraps text in OSC 8 escape sequences

This approach has issues:
- Limited terminal support (many terminals don't support OSC 8)
- Complex integration with Ratatui rendering
- The implementation was marked "experimental"

We're replacing this with Link Highlight Mode, so this code is no longer needed.

### Scope

- `src/tui/hyperlinks.rs`:
  - Remove `HyperlinkMode` enum and its impls
  - Remove `HyperlinkSupport` enum and its impls
  - Remove `HYPERLINK_SUPPORT` static
  - Remove `hyperlink_support()` function
  - Remove `detect_hyperlink_support()` function
  - Remove `is_unsupported_terminal()` function
  - Remove `is_supported_terminal()` function
  - Remove `is_terminal_multiplexer()` function
  - Remove `TerminalInfo` struct and its impls
  - Remove `terminal_info()` function
  - Remove `file_url()` function
  - Remove `file_url_with_position()` function
  - Remove `osc8` module (constants START, END, ST)
  - Remove `osc8_wrap()` function
  - Remove `osc8_wrap_file()` function
  - Remove `contains_osc8()` function
  - Remove `HyperlinkRegion` struct and impl
  - Remove `HyperlinkMap` struct and impl
  - Remove `ide_aware_file_url()` function
  - Remove `percent_encode_path()` function
  - Remove `osc8_wrap_ide_aware()` function
  - Remove all associated tests

- `src/config/types.rs`:
  - Remove `hyperlinks: HyperlinkMode` field from `UiSettings`
  - Remove the `use crate::tui::hyperlinks::HyperlinkMode;` import

- `src/tui/mod.rs`:
  - Keep `hyperlinks` module export (we're keeping FileReference types)

### Code to Remove from `hyperlinks.rs`

#### Enums and Statics (Lines ~28-108)
```rust
// REMOVE: HyperlinkMode enum (lines 28-36)
// REMOVE: impl HyperlinkMode (lines 38-71)
// REMOVE: HyperlinkSupport enum (lines 79-86)
// REMOVE: impl HyperlinkSupport (lines 88-98)
// REMOVE: HYPERLINK_SUPPORT static (line 101)
// REMOVE: hyperlink_support() function (lines 106-108)
```

#### Detection Functions (Lines ~114-248)
```rust
// REMOVE: detect_hyperlink_support() (lines 114-132)
// REMOVE: is_unsupported_terminal() (lines 135-154)
// REMOVE: is_supported_terminal() (lines 157-231)
// REMOVE: is_terminal_multiplexer() (lines 234-248)
```

#### TerminalInfo (Lines ~256-301)
```rust
// REMOVE: TerminalInfo struct (lines 256-265)
// REMOVE: impl TerminalInfo (lines 267-281)
// REMOVE: impl Display for TerminalInfo (lines 283-296)
// REMOVE: terminal_info() function (lines 299-301)
```

#### URL Generation (Lines ~427-476)
```rust
// REMOVE: file_url() function (lines 427-466)
// REMOVE: file_url_with_position() function (lines 472-476)
```

#### OSC 8 Module and Functions (Lines ~489-530)
```rust
// REMOVE: osc8 module (lines 489-500)
// REMOVE: osc8_wrap() function (lines 515-517)
// REMOVE: osc8_wrap_file() function (lines 522-525)
// REMOVE: contains_osc8() function (lines 528-530)
```

#### HyperlinkRegion and HyperlinkMap (Lines ~538-628)
```rust
// REMOVE: HyperlinkRegion struct (lines 538-547)
// REMOVE: impl HyperlinkRegion (lines 549-569)
// REMOVE: HyperlinkMap struct (lines 576-578)
// REMOVE: impl HyperlinkMap (lines 580-628)
```

#### IDE-aware Functions (Lines ~660-746)
```rust
// REMOVE: ide_aware_file_url() function (lines 660-711)
// REMOVE: percent_encode_path() function (lines 716-734)
// REMOVE: osc8_wrap_ide_aware() function (lines 739-746)
```

#### Tests to Remove (Lines ~832-1743)
Remove all tests related to the removed functionality:
- `test_hyperlink_mode_*` tests
- `test_hyperlink_support_*` tests
- `test_*_supported` and `test_*_unsupported` terminal tests
- `test_detect_*` tests
- `test_terminal_info_*` tests
- `test_file_url_*` tests
- `test_osc8_*` tests
- `test_hyperlink_region_*` tests
- `test_hyperlink_map_*` tests
- `test_percent_encode_*` tests
- `test_ide_aware_*` tests

### Code to Keep in `hyperlinks.rs`

```rust
// KEEP: Module documentation (update to reflect new purpose)
// KEEP: FileReferenceSource enum (lines 309-316)
// KEEP: FileReference struct (lines 324-333)
// KEEP: impl FileReference (lines 335-412)
// KEEP: FILE_LINE_PATTERN static (lines 760-765)
// KEEP: extract_file_ref_from_message() function (lines 781-806)
// KEEP: split_path_and_location() function (lines 812-825)
// KEEP: Tests for FileReference and extract_file_ref_from_message
```

### Changes to `config/types.rs`

```rust
// REMOVE this import (around line 194-198):
use crate::tui::hyperlinks::HyperlinkMode;

// REMOVE this field from UiSettings (around lines 223-231):
pub struct UiSettings {
    // ... other fields
    #[serde(default)]
    pub hyperlinks: HyperlinkMode,  // REMOVE THIS
}

// REMOVE from Default impl (around lines 234-242):
impl Default for UiSettings {
    fn default() -> Self {
        Self {
            // ... other fields
            hyperlinks: HyperlinkMode::default(),  // REMOVE THIS
        }
    }
}
```

### Updated Module Documentation

After cleanup, `hyperlinks.rs` should have updated documentation:

```rust
//! File reference extraction for Link Highlight Mode.
//!
//! This module provides types and utilities for detecting file references
//! in log messages and stack traces:
//! - `FileReference` for representing file:line:column references
//! - `FileReferenceSource` for tracking where references come from
//! - `extract_file_ref_from_message()` for scanning log text
```

### Acceptance Criteria

1. All OSC 8 related code removed from `hyperlinks.rs`
2. `HyperlinkMode` import removed from `config/types.rs`
3. `hyperlinks` field removed from `UiSettings`
4. `FileReference`, `FileReferenceSource`, and `extract_file_ref_from_message()` preserved
5. Tests for preserved code still pass
6. No compiler errors or warnings
7. No dead code warnings
8. Module documentation updated

### Testing

- **Unit Tests**: 
  - Remove tests for deleted functionality
  - Keep tests for `FileReference` and `extract_file_ref_from_message()`
  - Run `cargo test` to verify remaining tests pass
  
- **Manual Testing**:
  - Verify app compiles and runs
  - Verify config loading works without `hyperlinks` field

### Estimated Lines Removed

Approximately 500-600 lines of code and 300-400 lines of tests.

### Files Changed

| File | Change Type |
|------|-------------|
| `src/tui/hyperlinks.rs` | Modified - major removal |
| `src/config/types.rs` | Modified - remove field and import |

### Estimated Time

2-3 hours

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/hyperlinks.rs` | Complete rewrite: removed ~1,300 lines of OSC 8 code (HyperlinkMode, HyperlinkSupport, TerminalInfo, URL generation, OSC 8 wrapping, HyperlinkRegion, HyperlinkMap, IDE-aware functions, 71 tests); kept FileReference, FileReferenceSource, extract_file_ref_from_message, 26 tests |
| `src/config/types.rs` | Removed `use crate::tui::hyperlinks::HyperlinkMode;` import, removed `hyperlinks` field from UiSettings struct and Default impl |
| `src/tui/mod.rs` | Updated module documentation, removed `HyperlinkMode` from re-exports |

### Notable Decisions/Tradeoffs

1. **Complete file rewrite**: Given the scope of removal (~80% of the file), it was cleaner to rewrite the file with only the kept code rather than making numerous individual edits.

2. **Preserved core types**: `FileReference`, `FileReferenceSource`, and `extract_file_ref_from_message()` are preserved as they're needed for the new Link Highlight Mode (Tasks 03-10).

3. **Backward compatibility**: Old config files with `hyperlinks = "auto"` in `[ui]` will be silently ignored due to `#[serde(default)]` on struct fields.

4. **Module documentation updated**: Changed from "Terminal hyperlink support (OSC 8)" to "File reference extraction for Link Highlight Mode".

### Testing Performed

- `cargo check` - Passed (no errors)
- `cargo test` - 917 tests passed (71 tests removed with OSC 8 code, down from 988)
- No dead code warnings for remaining code

### Lines Removed

- **Code removed**: ~700 lines of implementation code
- **Tests removed**: ~600 lines (71 individual tests)
- **Total reduction**: ~1,300 lines from hyperlinks.rs (1,744 lines → ~424 lines)

### Risks/Limitations

1. **Config migration**: Users with `hyperlinks` in their config will see the field silently ignored. No migration path needed since the field had no visible effect anyway (experimental).

2. **API surface reduced**: External code that imported HyperlinkMode or other removed types will break at compile time (clear error message).