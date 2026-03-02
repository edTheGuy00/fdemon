## Task: Write Responsive Layout Guidelines Section

**Objective**: Add a comprehensive "Responsive Layout Guidelines" section to `docs/CODE_STANDARDS.md` that codifies the five responsive layout principles from the feature plan, grounded in the actual patterns implemented in Phases 1-3.

**Depends on**: None

**Estimated Time**: 1-2 hours

### Scope

- `docs/CODE_STANDARDS.md`: Append new `## Responsive Layout Guidelines` section after the existing "Architectural Code Patterns" section (after line 378, end of file)

### Details

The section should cover five principles, each with:
- A clear statement of the principle
- Rationale (why it matters)
- A concrete anti-pattern example (what NOT to do)
- A correct-pattern example (what TO do)

Use real patterns from the Phases 1-3 implementation as the basis for examples. The guidelines must be **general-purpose** — they apply to all widgets, not just the New Session dialog.

#### Principle 1: Decide layout variant based on available space, not orientation

**Rationale**: Layout orientation (horizontal vs vertical) doesn't tell you how much space is available in each dimension. A horizontal layout can be short; a vertical layout can be tall.

**Anti-pattern**: Hardcoding `compact(true)` when layout is vertical and `compact(false)` when horizontal.

**Correct pattern**: Measure the actual `area.height` (or `area.width`) passed to the widget and compare against a named threshold constant. Reference how `render_horizontal()` checks `chunks[2].height < MIN_EXPANDED_LAUNCH_HEIGHT` to decide compact mode even in a horizontal layout.

```rust
// Anti-pattern: compact tied to layout orientation
fn render_vertical(&self, area: Rect, buf: &mut Buffer) {
    let widget = MyWidget::new().compact(true); // always compact in vertical
}
fn render_horizontal(&self, area: Rect, buf: &mut Buffer) {
    let widget = MyWidget::new().compact(false); // always expanded in horizontal
}

// Correct: compact tied to actual available space
fn render(&self, area: Rect, buf: &mut Buffer) {
    let compact = area.height < MIN_EXPANDED_HEIGHT;
    let widget = MyWidget::new().compact(compact);
}
```

#### Principle 2: All content must fit within the allocated area

**Rationale**: Every element rendered must fall within the `Rect` passed to the widget's `render()` method. Manual position arithmetic can produce coordinates outside the area, causing visual corruption or panics.

**Anti-pattern**: Computing a button position by adding offsets to the last field's position — this can exceed the area bounds when the content doesn't fit.

**Correct pattern**: Include all elements (including buttons, spacers, footers) in the `Layout` system. Use `Min(0)` as the final constraint to absorb any overflow gracefully. Reference how `calculate_fields_layout()` includes the button as slot `[11]` with `Constraint::Length(3)` and a `Min(0)` absorber at `[12]`.

```rust
// Anti-pattern: manual position outside layout system
let button_y = last_field.y + last_field.height + 1;
let button_area = Rect { y: button_y, height: 3, ..area }; // can overflow!

// Correct: include button in layout system
let chunks = Layout::vertical([
    Constraint::Length(4), // field
    Constraint::Length(1), // spacer
    Constraint::Length(3), // button — managed by layout
    Constraint::Min(0),    // absorber — clips if space runs out
])
.split(area);
let button_area = chunks[2]; // always within bounds
```

#### Principle 3: Scrollable lists must keep the selected item visible

**Rationale**: Hardcoded viewport height estimates are fragile — the real height varies with terminal size, layout mode, and surrounding content. A hardcoded `10` fails when the real height is `4` (bottom panel) or `30` (full terminal).

**Anti-pattern**: Using `adjust_scroll(DEFAULT_ESTIMATED_VISIBLE_HEIGHT)` with a hardcoded constant.

**Correct pattern**: Feed actual render-time height back to the state layer via `Cell<usize>` interior mutability, so handlers use the real viewport size. Add render-time scroll correction as a safety net.

```rust
// State: Cell<usize> for render-hint feedback
pub struct ListState {
    pub selected_index: usize,
    pub scroll_offset: usize,
    /// Render-hint: actual visible height from last frame.
    /// Defaults to 0 (signals "not yet rendered").
    pub last_known_visible_height: Cell<usize>,
}

// Renderer: write actual height each frame
fn render(&self, area: Rect, buf: &mut Buffer) {
    let visible_height = area.height as usize;
    self.state.last_known_visible_height.set(visible_height);

    // Safety net: clamp scroll so selected item is visible
    let corrected_scroll = calculate_scroll_offset(
        self.state.selected_index, visible_height, self.state.scroll_offset,
    );
    // Use corrected_scroll for rendering (don't write back to state)
}

// Handler: read actual height with fallback
fn handle_scroll(state: &mut AppState) {
    let height = state.list.last_known_visible_height.get();
    let effective = if height > 0 { height } else { DEFAULT_HEIGHT };
    state.list.adjust_scroll(effective);
}
```

**TEA note**: Using `Cell<usize>` for a render-hint is a pragmatic exception to strict unidirectional data flow. It scopes the mutation to a single numeric hint value. Annotate call sites with `// EXCEPTION: TEA render-hint write-back via Cell`.

#### Principle 4: Use named constants for layout thresholds

**Rationale**: Magic numbers in layout code are a maintenance burden. Named constants with doc comments explain the derivation and make threshold changes safe (update one constant, not N scattered literals).

**Correct pattern**: Group related thresholds near the widget they control. Include a doc comment explaining how the value was derived.

```rust
/// Minimum content-area height for expanded rendering.
/// Derived from: 5 fields x 4 rows + 4 spacers + 1 button spacer + 3 button rows = 29.
const MIN_EXPANDED_HEIGHT: u16 = 29;

/// Slot index for the launch button in `calculate_fields_layout()`.
const BUTTON_SLOT: usize = 11;
```

#### Principle 5: Add hysteresis at layout breakpoints

**Rationale**: When a widget switches between two modes at a size threshold, a single threshold causes flickering during terminal resize (each row of resize toggles the mode). A hysteresis buffer prevents this.

**Correct pattern**: Define a pair of thresholds — one to switch "up" (e.g., to expanded) and one to switch "down" (e.g., back to compact), with a gap of 3-5 rows between them.

```rust
/// Switch to expanded when height >= 29.
const MIN_EXPANDED_HEIGHT: u16 = 29;

/// Switch back to compact when height <= 24 (5-row hysteresis gap).
const COMPACT_HEIGHT_THRESHOLD: u16 = 24;
```

**Implementation note**: Hysteresis requires remembering the previous mode (stateful). For stateless renderers, start with the expand threshold only and add hysteresis if flickering is observed. Document the intended compact threshold even if unused (with `#[allow(dead_code)]`).

#### Anti-Pattern Summary

End the section with a concise reference table:

| Anti-Pattern | Why It's Wrong | Correct Approach |
|---|---|---|
| `compact(orientation == Vertical)` | Orientation doesn't indicate available space | Check `area.height < THRESHOLD` |
| Manual `Rect` outside layout system | Can overflow parent bounds | Include all elements in `Layout` with `Min(0)` absorber |
| `adjust_scroll(HARDCODED_HEIGHT)` | Real viewport height varies | Feed render-time height via `Cell<usize>`, add render-time clamp |
| Magic numbers for thresholds | Maintenance burden, no rationale | Named constants with doc comments |
| Single threshold for mode switch | Flickering during resize | Hysteresis pair with 3-5 row gap |

### Acceptance Criteria

1. New `## Responsive Layout Guidelines` section exists in `docs/CODE_STANDARDS.md` after "Architectural Code Patterns"
2. All 5 principles have: statement, rationale, anti-pattern example, correct-pattern example
3. Code examples use Rust syntax and follow the project's style conventions
4. Examples are **generalized** — they use `MyWidget`, `ListState`, `MIN_EXPANDED_HEIGHT` etc., not the specific `NewSessionDialog` names (keeping them applicable to any widget)
5. Anti-pattern summary table is present at the end of the section
6. The TEA exception note is included for the `Cell<usize>` render-hint pattern
7. No build or test impact (documentation only)

### Testing

No code tests needed — this is a documentation-only task.

Verification:
- Review the section for accuracy against the actual Phase 1-3 implementations
- Ensure no references to non-existent patterns or types

### Notes

- The section placement (after "Architectural Code Patterns") was specified in the original plan. This puts it at the natural end of the file, after all code patterns and before nothing.
- The examples should be generic enough that a developer working on any widget (log view, devtools panels, etc.) can apply them without needing to understand the New Session dialog.
- The `Cell<usize>` pattern in Principle 3 is the most subtle — make sure the TEA exception rationale is clear and the annotation convention is documented.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `docs/CODE_STANDARDS.md` | Appended new `## Responsive Layout Guidelines` section (lines 382-564) after the existing "Architectural Code Patterns" section |

### Notable Decisions/Tradeoffs

1. **Section separator**: Added a `---` horizontal rule before the new section to visually separate it from "Architectural Code Patterns", matching the style used before that section (line 197).
2. **Introductory paragraph**: Added a brief framing paragraph before Principle 1 to explain scope and origin of the guidelines, helping readers understand applicability without requiring context from the feature plan.
3. **Code block in Principle 4**: The anti-pattern example shows a `Layout::vertical` call with `// ...` placeholder to keep the example focused without implying a fixed number of fields — this is consistent with the generalized intent.
4. **Stray backtick removed**: An edit artifact left a stray closing ` ``` ` after the summary table; it was removed in a follow-up edit.

### Testing Performed

- Documentation review against task acceptance criteria — all 5 principles present with statement, rationale, anti-pattern, and correct-pattern
- Confirmed `## Responsive Layout Guidelines` follows `## Architectural Code Patterns` at line 382
- Confirmed Anti-Pattern Summary table is present with all 5 rows
- Confirmed TEA exception note and `Cell<usize>` annotation convention are documented in Principle 3
- Confirmed all examples use generalized names (`MyWidget`, `ListState`, `MIN_EXPANDED_HEIGHT`) — no `NewSessionDialog` references
- No code changes — no build or test commands required

### Risks/Limitations

1. **Documentation drift**: If the actual implementation diverges from these guidelines in future work, the doc will need updating alongside the code change — no automated enforcement exists.
