## Task: M1 + frame_analysis_tab.rs cleanup bundle

**Objective:** Close M1 (dead binding with false comment) along with the four `frame_analysis_tab.rs`-local Minors (m4 byte-slice, m9 missing proportional-bar test, m10 raster-remainder allocation, m11 non-saturating u16 arithmetic). All edits live in the same file with no inter-dependency, so bundling them avoids three separate single-file commits.

**Depends on:** — (Wave 1)

**Agent:** implementor

**Estimated Time:** 1–1.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs`

**Files Read (Dependencies):**
- `crates/fdemon-core/src/performance.rs` — `FramePhases` struct for new test fixtures.

### Background

Phase 2 review surfaced five `frame_analysis_tab.rs`-local issues. Four are minor; one is M1 (major) flagged by both the code-quality reviewer and the task validator at merge time:

- **M1 — `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs:165-167`** — `let render_width = label.len().min(width as usize) as u16; … let _ = render_width; // used implicitly by ratatui set_string`. `Buffer::set_string` does not take a width parameter — the comment is false. Rendering is correct today only because the label-selection logic at lines 154–160 already guarantees `label.len() <= width as usize`. Future maintainers will assume a safety mechanism is in place.
- **m4 — lines 151-152** — `&name[..1]` byte-slices the first byte of "Build" / "Layout" / "Paint" / "Raster". Safe for ASCII labels; will panic at UTF-8 boundary if a phase name ever contains a multibyte first character.
- **m9 — tests module** — `renders_proportional_bar_when_phases_and_wide_enough` only asserts the four phase labels appear somewhere in the buffer. The visual contract (segment widths proportional to phase micros; `█` characters present on the bar row) is not test-locked.
- **m10 — lines 129-131** — `raster_cells = area.width.saturating_sub(build_cells + layout_cells + paint_cells)`: the rounding remainder is unconditionally absorbed by raster. When raster_micros is 0 but rounding leaves a non-zero remainder, raster gets a green cell labelled "Raster 0.0ms". Should go to the largest segment.
- **m11 — lines 129-131, 174, 329** — `area.y + 1`, `area.y + 1 + i as u16`, `build_cells + layout_cells + paint_cells` use non-saturating `+`. The project pattern elsewhere uses `saturating_sub`; adopt symmetric style for `saturating_add` and widen the 3-segment sum to u32 before subtraction.

### Details

#### 1. M1 — Remove dead `render_width` binding

The label-selection logic at lines 154–160 already guarantees the chosen `label`'s length is ≤ `width`. The `render_width` variable is dead. Delete both lines (165 and 167) and replace the surrounding code with a short invariant comment at the label-selection block (or simply rely on the immediate code clarity). Example:

```rust
// Label is chosen so that `label.len() <= width as usize` — see selection
// logic above; no further clipping needed before set_string.
let label: &str = if full_label.len() as u16 <= width {
    &full_label
} else if short_label.len() as u16 <= width {
    &short_label
} else {
    min_label
};

let avail = width.saturating_sub(label.len() as u16);
let pad = avail / 2;
let label_x = x + pad;
buf.set_string(label_x, label_y, label, Style::default().fg(color));
x = x.saturating_add(width);  // m11 applies here too
```

If a reviewer prefers explicit defensive clipping over the invariant comment, the equivalent fix is:

```rust
let clipped: String = label.chars().take(width as usize).collect();
buf.set_string(label_x, label_y, &clipped, Style::default().fg(color));
```

Either approach satisfies M1. Pick one and stick to it.

#### 2. m4 — Replace byte-slice with `chars().next()`

At lines 151–152, replace `&name[..1]` with `name.chars().next().unwrap_or(' ')`. Since `name` comes from a static `&str` array of ASCII labels, `unwrap_or(' ')` is unreachable today; the change protects against future renames or i18n. The result type changes from `&str` to `char`; format strings adapt accordingly:

```rust
let first_char = name.chars().next().unwrap_or(' ');
let short_label = format!("{} {:.1}ms", first_char, ms);
// For min_label, we need a String since `first_char` is a char by value:
let min_label = first_char.to_string();
let min_label: &str = &min_label; // adjust borrow as needed
```

#### 3. m10 — Allocate rounding remainder to largest segment

At lines 129–131, the current code computes raster from the remainder:

```rust
let build_cells   = ((build_micros   as f64 / total as f64 * area.width as f64).round()) as u16;
let layout_cells  = ((layout_micros  as f64 / total as f64 * area.width as f64).round()) as u16;
let paint_cells   = ((paint_micros   as f64 / total as f64 * area.width as f64).round()) as u16;
let raster_cells  = area.width.saturating_sub(build_cells + layout_cells + paint_cells);
```

Replace with: compute all four cells via `.round()`, then if the sum differs from `area.width`, add the difference to the segment with the largest cell count (ties: prefer raster to preserve current behaviour). Helper:

```rust
fn distribute_remainder(cells: &mut [u16; 4], target_width: u16) {
    let sum: u32 = cells.iter().map(|&c| c as u32).sum();
    let target = target_width as u32;
    if sum == target {
        return;
    }
    // Add (target - sum) — may be negative if we rounded up too much.
    let diff = target as i32 - sum as i32;
    // Apply to the index with the largest value (ties: highest index = raster).
    let max_idx = cells
        .iter()
        .enumerate()
        .max_by_key(|(_, &c)| c)
        .map(|(i, _)| i)
        .unwrap_or(3);
    let new_val = cells[max_idx] as i32 + diff;
    cells[max_idx] = new_val.max(0) as u16;
}
```

Use `[build_cells, layout_cells, paint_cells, raster_cells]` as the input array (raster computed via `.round()` like the others).

#### 4. m11 — Saturating / u32-widened arithmetic

Throughout the file, replace:

- `area.y + 1` → `area.y.saturating_add(1)` (lines 174, 329, and any other `area.y + N` patterns)
- `x + dx` → use saturating where the loop bound makes overflow theoretically possible
- The 3-segment sum: `build_cells + layout_cells + paint_cells` computed as u16 → cast to u32 before subtraction:

  ```rust
  let three_sum = build_cells as u32 + layout_cells as u32 + paint_cells as u32;
  let raster_cells = (area.width as u32).saturating_sub(three_sum) as u16;
  ```

  (This becomes moot if m10 reorders the cell computation, but apply the u32-widening pattern wherever segment sums are accumulated.)

Audit the file for any other `+` on u16 coordinates and apply the same pattern. Do NOT change `+` on f64 (phase-micros math).

#### 5. m9 — Add proportional-bar regression test

Add a new `#[test]` in the existing `mod tests` block that asserts the `█` (U+2588) count per segment matches the expected proportion within ±1 column:

```rust
#[test]
fn proportional_bar_segment_widths_match_phase_proportions() {
    // Build a frame with phases: build=2ms, layout=4ms, paint=6ms, raster=8ms (total=20ms).
    // At width=80: expected cells are roughly build=8, layout=16, paint=24, raster=32 (sum=80).
    // Render to an 80-column buffer.
    // For each segment, count `█` characters on the bar row (row 1).
    // Assert each segment's count is within ±1 of the expected proportion.
}
```

Use the existing test fixtures and helper functions in this file's `mod tests`. The test should:

1. Construct a `FrameTiming` with explicit `phases` populated.
2. Render `render_proportional_phase_bar` to an 80-column buffer.
3. Walk the buffer row 1 (the `█` row), counting consecutive `█` runs.
4. Assert each run's length matches the expected `(phase_micros / total * 80).round()` value within ±1.

The test guards against regressions in the proportional math (segment widths summing wrong, colours swapping, the `█` character being replaced with something else).

### Acceptance Criteria

1. `frame_analysis_tab.rs` contains no dead bindings. `cargo clippy --workspace --all-targets -- -D warnings` is green.
2. The `render_width = …; let _ =` pattern is gone. The label-fit invariant is either documented inline or enforced via explicit clipping.
3. No `&name[..1]` byte-slice on phase names. `chars().next()` is used instead.
4. The proportional-bar remainder goes to the largest segment, not unconditionally to raster. A new or updated test verifies this for an asymmetric phase distribution.
5. All u16 coordinate arithmetic in this file uses `saturating_add` or u32-widened intermediates.
6. The new `proportional_bar_segment_widths_match_phase_proportions` test passes.
7. All previously passing tests still pass.
8. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` is green.

### Testing

- `cargo test -p fdemon-tui widgets::devtools::performance::details::frame_analysis_tab` — runs all tab tests including the new one.
- `cargo test --workspace` — full quality gate.

### Risk

- The m10 remainder-redistribution may shift `█` counts by 1 in existing tests that assert exact segment widths. If any tests fail, update their expected counts.
- The m4 type change from `&str` to `char` may cascade through format strings; double-check `format!` width consistency.

### Out of Scope

- Do NOT touch `frame_chart/*` — that's T01's scope.
- Do NOT touch `performance/mod.rs` constants or callsites — T01 (dual_pane plumbing) and T04 (const visibility) own that file.
- Do NOT modify the no-data fallback, no-selection prompt, hint list rendering, or verdict line. M1+m4+m9+m10+m11 only touch the proportional-bar / label-rendering region of this file.
