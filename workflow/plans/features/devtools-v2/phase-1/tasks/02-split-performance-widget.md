## Task: Split Performance Widget into Directory Module

**Objective**: Decompose `crates/fdemon-tui/src/widgets/devtools/performance.rs` (833 lines) into five files under `performance/` — each under 400 lines — without changing any behavior or test assertions.

**Depends on**: None

### Scope

- `crates/fdemon-tui/src/widgets/devtools/performance.rs` → DELETE (replaced by directory)
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` → **NEW**
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_section.rs` → **NEW**
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_section.rs` → **NEW**
- `crates/fdemon-tui/src/widgets/devtools/performance/stats_section.rs` → **NEW**
- `crates/fdemon-tui/src/widgets/devtools/performance/styles.rs` → **NEW**

No changes to `devtools/mod.rs` — the `pub mod performance;` declaration and `pub use performance::PerformancePanel;` re-export resolve identically.

### Current File Structure

```
Lines   1–15    Imports
Lines  17–44    Constants (8 total: 4 layout, 4 style thresholds + 2 more)
Lines  46–90    PerformancePanel struct + constructor impl
Lines  92–136   impl Widget — render dispatch (disconnected/compact/3-section)
Lines 138–185   render_disconnected()
Lines 187–204   render_compact_summary()
Lines 206–274   render_fps_section() — block, header line, delegates to sparkline
Lines 276–298   render_frame_sparkline() — Sparkline widget from frame history
Lines 300–380   render_memory_section() — block, header, Gauge, detail line
Lines 382–454   render_stats_section() — block, jank/gc/frames stats
Lines 457–504   Free functions: fps_style, gauge_style_for_utilization, jank_style, format_number
Lines 506–833   mod tests (20 tests)
```

### Target File Layout

#### `performance/styles.rs` (~80 lines)

Pure style/format helpers with no widget dependencies.

**Move here:**

1. **Style threshold constants** (lines 31–44):
   - `FPS_GREEN_THRESHOLD` (55.0)
   - `FPS_YELLOW_THRESHOLD` (30.0)
   - `MEM_GREEN_THRESHOLD` (0.6)
   - `MEM_YELLOW_THRESHOLD` (0.8)
   - `JANK_WARN_THRESHOLD` (0.05)
   - `SPARKLINE_MAX_MS` (33)
2. **Style functions** — change from module-private `fn` to `pub(super) fn`:
   - `fps_style(fps: Option<f64>) -> Style` (lines 460–467)
   - `gauge_style_for_utilization(util: f64) -> Style` (lines 470–478)
   - `jank_style(jank_count: u32, total_frames: u64) -> Style` (lines 481–491)
   - `format_number(n: u64) -> String` (lines 494–504)
3. **Style-specific tests** (10 tests):
   - `test_fps_color_green_high_fps`
   - `test_fps_color_yellow_medium_fps`
   - `test_fps_color_red_low_fps`
   - `test_fps_color_none`
   - `test_memory_gauge_color_low_utilization`
   - `test_memory_gauge_color_medium_utilization`
   - `test_memory_gauge_color_high_utilization`
   - `test_format_number_small`
   - `test_format_number_thousands`
   - `test_format_number_millions`

**Required imports:**
```rust
use ratatui::style::{Color, Style};
use crate::theme::palette;
```

#### `performance/frame_section.rs` (~120 lines)

Frame timing section rendering.

**Move here:**

1. **Layout constants:**
   - `FPS_SECTION_HEIGHT` (4)
   - `COMPACT_WIDTH_THRESHOLD` (50)
2. **`impl PerformancePanel<'_>` block** containing:
   - `render_fps_section()` (lines 208–274)
   - `render_frame_sparkline()` (lines 276–298)

**Required imports:**
```rust
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Sparkline};

use super::styles::{fps_style, COMPACT_WIDTH_THRESHOLD, SPARKLINE_MAX_MS};
use super::PerformancePanel;
use crate::theme::palette;
```

#### `performance/memory_section.rs` (~100 lines)

Memory gauge section rendering.

**Move here:**

1. **Layout constant:**
   - `MEMORY_SECTION_HEIGHT` (4)
2. **`impl PerformancePanel<'_>` block** containing:
   - `render_memory_section()` (lines 302–380)

**Required imports:**
```rust
use fdemon_core::performance::MemoryUsage;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Gauge, Paragraph};

use super::styles::gauge_style_for_utilization;
use super::PerformancePanel;
use crate::theme::palette;
```

#### `performance/stats_section.rs` (~90 lines)

Stats section rendering (frames, jank, GC counts).

**Move here:**

1. **Layout constant:**
   - `STATS_SECTION_HEIGHT` (3)
2. **`impl PerformancePanel<'_>` block** containing:
   - `render_stats_section()` (lines 384–454)

**Required imports:**
```rust
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use super::styles::{format_number, jank_style};
use super::PerformancePanel;
use crate::theme::palette;
```

#### `performance/mod.rs` (~350 lines)

Entry point: struct, constructor, Widget impl, dispatch, disconnected/compact states, integration tests.

**Keep here:**

1. **Submodule declarations:**
   ```rust
   mod frame_section;
   mod memory_section;
   mod stats_section;
   pub(super) mod styles;
   ```
2. **`PerformancePanel` struct** (lines 52–63) and all fields
3. **Constructor impl** (lines 65–90): `new()`, `with_connection_error()`
4. **`impl Widget for PerformancePanel<'_>`** (lines 92–136): render dispatch logic including the 3-section `Layout::vertical` split using height constants imported from section files
5. **`render_disconnected()`** (lines 141–185)
6. **`render_compact_summary()`** (lines 189–204) — calls `fps_style` via `styles::fps_style`
7. **Integration render tests** (10 tests):
   - `test_performance_panel_renders_without_panic`
   - `test_performance_panel_shows_fps`
   - `test_performance_panel_disconnected_state`
   - `test_performance_panel_small_terminal`
   - `test_performance_panel_zero_area`
   - `test_performance_panel_shows_connection_error`
   - `test_performance_panel_no_error_shows_generic_disconnected`
   - `test_monitoring_inactive_shows_disconnected`
   - `test_performance_panel_reconnecting_shows_attempt_count`

**Note:** The section height constants (`FPS_SECTION_HEIGHT`, `MEMORY_SECTION_HEIGHT`, `STATS_SECTION_HEIGHT`) are used in the `render()` dispatch to calculate the 3-way layout split. Import them via `use frame_section::FPS_SECTION_HEIGHT;` etc. — make the constants `pub(super)` in their section files.

**Required imports:**
```rust
use fdemon_app::session::PerformanceState;
use fdemon_app::state::VmConnectionStatus;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap};

use crate::theme::{icons::IconSet, palette};

use frame_section::FPS_SECTION_HEIGHT;
use memory_section::MEMORY_SECTION_HEIGHT;
use stats_section::STATS_SECTION_HEIGHT;
use styles::fps_style;
```

### Implementation Steps

1. Create `crates/fdemon-tui/src/widgets/devtools/performance/` directory
2. Create `performance/styles.rs` with style constants, helper functions (`pub(super)`), and 10 style tests
3. Create `performance/frame_section.rs` with `FPS_SECTION_HEIGHT`, `COMPACT_WIDTH_THRESHOLD` as `pub(super)`, and `render_fps_section` + `render_frame_sparkline` in an `impl PerformancePanel<'_>` block
4. Create `performance/memory_section.rs` with `MEMORY_SECTION_HEIGHT` as `pub(super)` and `render_memory_section`
5. Create `performance/stats_section.rs` with `STATS_SECTION_HEIGHT` as `pub(super)` and `render_stats_section`
6. Create `performance/mod.rs` with struct, constructors, Widget impl, disconnected/compact renderers, and 10 integration tests
7. Delete `performance.rs`
8. Verify: `cargo test -p fdemon-tui -- performance` (all 20 tests pass)
9. Verify: `cargo clippy -p fdemon-tui`

### Call Graph (for reference during split)

```
Widget::render [mod.rs]
  ├── render_disconnected [mod.rs]
  ├── render_compact_summary [mod.rs]
  │     └── fps_style [styles.rs]
  ├── render_fps_section [frame_section.rs]
  │     ├── fps_style [styles.rs]
  │     └── render_frame_sparkline [frame_section.rs]
  ├── render_memory_section [memory_section.rs]
  │     ├── MemoryUsage::format_bytes [fdemon_core]
  │     ├── MemoryUsage::utilization [fdemon_core]
  │     └── gauge_style_for_utilization [styles.rs]
  └── render_stats_section [stats_section.rs]
        ├── format_number [styles.rs]
        └── jank_style [styles.rs]
```

### Acceptance Criteria

1. `performance.rs` no longer exists — replaced by `performance/` directory with 5 files
2. Each file is under 400 lines
3. All 20 existing tests pass with zero changes to test assertions
4. `cargo clippy -p fdemon-tui` produces no warnings
5. No changes to `devtools/mod.rs` — the `pub mod performance;` and `pub use performance::PerformancePanel;` lines remain untouched
6. No public API changes — `PerformancePanel::new()` and `with_connection_error()` remain `pub`

### Testing

Run the specific performance tests:

```bash
cargo test -p fdemon-tui -- performance
```

All 20 tests should pass without any modifications to test assertions.

### Notes

- Rust allows multiple `impl` blocks for the same type across files within a module — each section file adds its own `impl PerformancePanel<'_>` block with the relevant render methods.
- The `render()` method in `mod.rs` calls private methods defined in other files (e.g., `self.render_fps_section()`). This works because all files are in the same module (`performance`) — Rust's visibility resolution is module-based, not file-based.
- `styles.rs` is `pub(super)` visibility so the parent `devtools/mod.rs` could also access these helpers if needed in the future.
- The `frame_section`, `memory_section`, and `stats_section` modules are private (no `pub`) — they only need to be visible within the `performance` module.
- In Phase 3, `frame_section.rs` will be **replaced** with a bar chart, `memory_section.rs` with a time-series chart, and `stats_section.rs` will be **deleted**. Extracting them now makes those replacements surgical single-file operations.
