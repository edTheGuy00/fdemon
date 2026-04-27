## Task: Restore MSRV compliance in `fdemon-tui` (DevTools performance charts)

**Objective**: Replace 3 `is_multiple_of(2)` call sites in the DevTools performance chart renderers with `% 2 == 0` and suppress the resulting clippy lint at function scope, restoring compatibility with the workspace's declared MSRV (`1.77.2`).

**Depends on**: None

**Estimated Time**: 0.25–0.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/chart.rs`: Two call sites at lines 111 and 223. Replace each `dot_x.is_multiple_of(2)` with `dot_x % 2 == 0`. Add `#[allow(clippy::manual_is_multiple_of)]` plus the MSRV justification comment on each enclosing function.
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/bars.rs`: One call site at line 180. Replace `(x - line_start_x).is_multiple_of(2)` with `(x - line_start_x) % 2 == 0`. Add `#[allow(...)]` plus the MSRV justification comment on the enclosing function.

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/devtools/network/tests.rs:11-12`: Use as the reference for the exact comment wording and attribute placement.

### Details

#### Site 1: `memory_chart/chart.rs:111`

The current code (around lines 100-114):

```rust
// Allocated line: draw at the allocated capacity level (every dot)
let alloc_y = byte_to_dot_y(sample.allocated);
canvas_allocated.set(dot_x, alloc_y);
// Dashed appearance: also skip every other column
if dot_x.is_multiple_of(2) && alloc_y + 1 < dot_h {
    canvas_allocated.set(dot_x, alloc_y + 1);
}
```

Replace `dot_x.is_multiple_of(2)` with `dot_x % 2 == 0`. Add the `#[allow]` attribute plus MSRV-guard comment to the **enclosing function** (the function that contains line 111 — typically a `render_*` or `paint_*` method on the chart widget).

#### Site 2: `memory_chart/chart.rs:223`

The current code (around lines 215-225):

```rust
for dy in heap_ceil_y..bottom_dot_y {
    canvas_heap.set(dot_x, dy);
}

// Capacity line (dashed)
let alloc_y = byte_to_dot_y(mem.heap_capacity);
canvas_allocated.set(dot_x, alloc_y);
if dot_x.is_multiple_of(2) && alloc_y + 1 < dot_h {
    canvas_allocated.set(dot_x, alloc_y + 1);
}
```

Replace `dot_x.is_multiple_of(2)` with `dot_x % 2 == 0`. Add the `#[allow]` attribute plus MSRV-guard comment to the **enclosing function** (likely a different function from Site 1; if both sites are in the same function, one `#[allow]` covers both).

#### Site 3: `frame_chart/bars.rs:180`

The current code (around lines 170-186):

```rust
// Draw dashed line after label
let line_start_x = area.x + label.len() as u16;
let mut x = line_start_x;
while x < area.right() {
    if let Some(cell) = buf.cell_mut((x, budget_y)) {
        // Skip cells that are part of bar columns (avoid overwriting bars)
        // Use dashed '╌' for every other cell to create a dashed effect
        if (x - line_start_x).is_multiple_of(2) {
            cell.set_char('╌').set_style(line_style);
        }
    }
    x += 1;
}
```

Replace `(x - line_start_x).is_multiple_of(2)` with `(x - line_start_x) % 2 == 0`. **Preserve the parentheses** around the subtraction. Add the `#[allow]` attribute plus MSRV-guard comment to the enclosing function.

#### Pattern (apply to each enclosing function)

```rust
// MSRV guard: `is_multiple_of` requires Rust 1.87; MSRV is 1.77.2 — suppress the lint.
#[allow(clippy::manual_is_multiple_of)]
fn <enclosing_function>(...) { ... }
```

If both sites in `chart.rs` happen to share the same enclosing function, a single `#[allow]` is sufficient. Verify by reading the function structure of the file. Otherwise, add the attribute to each enclosing function independently.

### Acceptance Criteria

1. `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/chart.rs` and `frame_chart/bars.rs` no longer contain `is_multiple_of` (verify with `grep -rn 'is_multiple_of' crates/fdemon-tui/src/widgets/devtools/performance/` → no matches).
2. Each enclosing function carries `#[allow(clippy::manual_is_multiple_of)]` preceded by the MSRV justification comment.
3. `cargo clippy -p fdemon-tui --all-targets -- -D warnings` exits 0.
4. `cargo test -p fdemon-tui` passes — chart-rendering snapshot tests and unit tests still produce the same visual output.
5. `cargo fmt --all` is clean.
6. No other lines in either file are modified.
7. Parentheses preserved on `(x - line_start_x) % 2 == 0` in `bars.rs`.

### Testing

Run the existing performance chart tests to confirm the dashed-line pattern is byte-identical to before:

```bash
cargo test -p fdemon-tui --test '*' performance
cargo test -p fdemon-tui memory_chart
cargo test -p fdemon-tui frame_chart
```

If insta snapshot tests exist for these widgets, they must remain green. The rewrite produces an identical bit pattern (`% 2 == 0` and `is_multiple_of(2)` are observably identical for `u16`/`usize` non-negative values).

### Notes

- The literal `2` divisor is non-zero, so `% 2` is panic-safe.
- For unsigned types, `(x - line_start_x)` could underflow if `x < line_start_x`, but the surrounding `let mut x = line_start_x; while x < area.right()` loop guarantees `x >= line_start_x`, so the subtraction is safe (this invariant pre-dates this fix).
- If both `chart.rs` sites are in the same function, you only need one `#[allow]`. Read the function boundaries before placing attributes.
- Do **not** suppress the lint at module/file scope.

---

## Completion Summary

**Status:** Not Started
**Branch:** _to be filled by implementor_

### Files Modified

| File | Changes |
|------|---------|
| _tbd_ | _tbd_ |

### Notable Decisions/Tradeoffs

_tbd_

### Testing Performed

- `cargo clippy -p fdemon-tui --all-targets -- -D warnings` — _tbd_
- `cargo test -p fdemon-tui` — _tbd_
- `cargo fmt --all -- --check` — _tbd_

### Risks/Limitations

_tbd_
