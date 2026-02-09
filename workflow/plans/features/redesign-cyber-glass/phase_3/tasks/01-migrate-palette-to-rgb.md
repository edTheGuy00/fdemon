## Task: Migrate Palette from Named Colors to RGB Design Tokens

**Objective**: Replace all named `Color::*` constants in `theme/palette.rs` with `Color::Rgb()` values from the Cyber-Glass design token specification. This gives all Phase 3+ tasks accurate colors from the start instead of deferring migration.

**Depends on**: None

### Scope

- `crates/fdemon-tui/src/theme/palette.rs` — Replace all named color constants with RGB values

### Details

#### Color Migration Table

Replace each constant's value. Keep the constant name and doc comment. Remove `// Phase 2: Rgb(...)` comments since the values are now applied.

**Background layers:**

| Constant | Current | Target |
|----------|---------|--------|
| `DEEPEST_BG` | `Color::Black` | `Color::Rgb(10, 12, 16)` |
| `CARD_BG` | `Color::Black` | `Color::Rgb(18, 21, 28)` |
| `POPUP_BG` | `Color::DarkGray` | `Color::Rgb(28, 33, 43)` |
| `SURFACE` | `Color::Black` | `Color::Rgb(22, 27, 34)` |

**Borders:**

| Constant | Current | Target |
|----------|---------|--------|
| `BORDER_DIM` | `Color::DarkGray` | `Color::Rgb(45, 51, 59)` |
| `BORDER_ACTIVE` | `Color::Cyan` | `Color::Rgb(88, 166, 255)` |

**Accent:**

| Constant | Current | Target |
|----------|---------|--------|
| `ACCENT` | `Color::Cyan` | `Color::Rgb(88, 166, 255)` |
| `ACCENT_DIM` | `Color::DarkGray` | `Color::Rgb(56, 107, 163)` |

**Text:**

| Constant | Current | Target |
|----------|---------|--------|
| `TEXT_PRIMARY` | `Color::White` | `Color::Rgb(201, 209, 217)` |
| `TEXT_SECONDARY` | `Color::Gray` | `Color::Rgb(125, 133, 144)` |
| `TEXT_MUTED` | `Color::DarkGray` | `Color::Rgb(72, 79, 88)` |
| `TEXT_BRIGHT` | `Color::White` | `Color::Rgb(240, 246, 252)` |

**Status:**

| Constant | Current | Target |
|----------|---------|--------|
| `STATUS_GREEN` | `Color::Green` | `Color::Rgb(16, 185, 129)` |
| `STATUS_RED` | `Color::Red` | `Color::Rgb(244, 63, 94)` |
| `STATUS_YELLOW` | `Color::Yellow` | `Color::Rgb(234, 179, 8)` |
| `STATUS_BLUE` | `Color::Blue` | `Color::Rgb(56, 189, 248)` |
| `STATUS_INDIGO` | `Color::Magenta` | `Color::Rgb(129, 140, 248)` |

**Effects:**

| Constant | Current | Target |
|----------|---------|--------|
| `SHADOW` | `Color::Black` | `Color::Rgb(5, 6, 8)` |
| `CONTRAST_FG` | `Color::Black` | `Color::Rgb(0, 0, 0)` |

**Gradients:**

| Constant | Current | Target |
|----------|---------|--------|
| `GRADIENT_BLUE` | `Color::Blue` | `Color::Rgb(37, 99, 235)` |
| `GRADIENT_INDIGO` | `Color::Magenta` | `Color::Rgb(99, 102, 241)` |

**Log level colors** — migrate to use design token status colors:

| Constant | Current | Target |
|----------|---------|--------|
| `LOG_ERROR` | `Color::Red` | `Color::Rgb(244, 63, 94)` |
| `LOG_ERROR_MSG` | `Color::LightRed` | `Color::Rgb(251, 113, 133)` |
| `LOG_WARNING` | `Color::Yellow` | `Color::Rgb(234, 179, 8)` |
| `LOG_WARNING_MSG` | `Color::Yellow` | `Color::Rgb(250, 204, 21)` |
| `LOG_INFO` | `Color::Green` | `Color::Rgb(16, 185, 129)` |
| `LOG_INFO_MSG` | `Color::White` | `Color::Rgb(201, 209, 217)` |
| `LOG_DEBUG` | `Color::DarkGray` | `Color::Rgb(72, 79, 88)` |
| `LOG_DEBUG_MSG` | `Color::DarkGray` | `Color::Rgb(100, 116, 139)` |

**Search highlight** — use bright, high-contrast values:

| Constant | Current | Target |
|----------|---------|--------|
| `SEARCH_HIGHLIGHT_FG` | `Color::Black` | `Color::Rgb(0, 0, 0)` |
| `SEARCH_HIGHLIGHT_BG` | `Color::Yellow` | `Color::Rgb(234, 179, 8)` |
| `SEARCH_CURRENT_FG` | `Color::Black` | `Color::Rgb(0, 0, 0)` |
| `SEARCH_CURRENT_BG` | `Color::LightYellow` | `Color::Rgb(250, 204, 21)` |

**Stack trace** — use consistent design token shades:

| Constant | Current | Target |
|----------|---------|--------|
| `STACK_FRAME_NUMBER` | `Color::DarkGray` | `Color::Rgb(72, 79, 88)` |
| `STACK_FUNCTION_PROJECT` | `Color::White` | `Color::Rgb(201, 209, 217)` |
| `STACK_FUNCTION_PACKAGE` | `Color::DarkGray` | `Color::Rgb(72, 79, 88)` |
| `STACK_FILE_PROJECT` | `Color::Blue` | `Color::Rgb(56, 189, 248)` |
| `STACK_FILE_PACKAGE` | `Color::DarkGray` | `Color::Rgb(72, 79, 88)` |
| `STACK_LOCATION_PROJECT` | `Color::Cyan` | `Color::Rgb(88, 166, 255)` |
| `STACK_LOCATION_PACKAGE` | `Color::DarkGray` | `Color::Rgb(72, 79, 88)` |
| `STACK_ASYNC_GAP` | `Color::DarkGray` | `Color::Rgb(72, 79, 88)` |
| `STACK_PUNCTUATION` | `Color::DarkGray` | `Color::Rgb(72, 79, 88)` |

**Modal backgrounds** — keep existing RGB values as-is (they are already correct):

No changes needed for `MODAL_FUZZY_BG`, `MODAL_FUZZY_QUERY_BG`, `MODAL_DART_DEFINES_*`, and `LINK_BAR_BG`.

#### Cleanup

- Remove all `// Phase 2: Rgb(...)` comments since the migration is complete
- Remove `#[allow(dead_code)]` annotations from `SURFACE`, `ACCENT_DIM`, `TEXT_BRIGHT`, `GRADIENT_BLUE`, `GRADIENT_INDIGO` — these will be used in Phase 3 tasks
- Update the module doc comment from "Phase 1: Maps to existing named colors" to "Cyber-Glass design tokens using true-color RGB values"
- Update the `test_palette_constants_are_valid` test — the existing test just checks they compile, which still passes. Add a test that verifies a few representative constants are `Color::Rgb()` variant.

### Acceptance Criteria

1. All named color constants (`Color::Black`, `Color::Cyan`, etc.) replaced with `Color::Rgb()` values
2. RGB values match the Cyber-Glass design token specification exactly
3. No `// Phase 2:` comments remain
4. `#[allow(dead_code)]` removed from constants that will be used in Phase 3
5. Module doc comment updated
6. `cargo check --workspace` passes (all crates that import palette still compile)
7. `cargo clippy --workspace` passes
8. `cargo test -p fdemon-tui` passes (test assertions may need updating if they compare exact color values)

### Testing

- Verify compilation across all crates: `cargo check --workspace`
- Run full test suite: `cargo test --workspace`
- If tests compare exact `Color::` values (e.g., `assert_eq!(style.fg, Some(Color::Cyan))`), update them to match the new RGB values
- Visual spot check: run the app briefly to verify the UI looks reasonable with RGB colors

### Notes

- **Test breakage is expected**: Tests in `theme/styles.rs` that assert `Some(palette::STATUS_GREEN)` will still pass since they compare against the constant, not the raw value. But tests that hardcode `Some(Color::Green)` will fail — update these.
- **Terminal compatibility**: `Color::Rgb()` requires true-color terminal support. Most modern terminals support this. On terminals without true-color, ratatui/crossterm auto-fallback to nearest 256-color match. Document this in the module header.
- **Log readability**: The RGB log colors should maintain sufficient contrast against `DEEPEST_BG` (Rgb(10,12,16)). All chosen values have high contrast ratios.
