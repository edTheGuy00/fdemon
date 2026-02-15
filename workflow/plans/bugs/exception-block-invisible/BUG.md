# Bugfix Plan: Exception Blocks Invisible in fdemon

## TL;DR

Exception blocks are invisible when testing with `example/app2`. Three root causes found: (1) the exception parser gets permanently stuck when an empty line arrives during widget capture mode, silently eating ALL subsequent lines including the footer, (2) `parse_flutter_log` strips the "flutter: " prefix before the exception parser sees it, but for `error: true` lines it KEEPS the prefix — causing inconsistent input to the parser, and (3) the example app wraps most error triggers in try-catch, so they produce Logger output instead of raw Flutter exception blocks.

## Bug Reports

### Bug 1: Parser Stuck on Empty Lines During Widget Capture (CRITICAL)

**Symptom:** After the "The relevant error-causing widget was:" marker appears in an exception block, the parser enters `capture_widget_next` mode. When an empty line arrives (common between sections in Flutter error output), `capture_widget_next` is never reset. ALL subsequent lines are consumed by the capture branch and return `FeedResult::Buffered` — including the footer (`════════════`) — so the exception block never completes. The parser eats all subsequent log lines until the 500-line safety limit.

**Expected:** Empty lines after the widget name should reset `capture_widget_next = false` and allow the parser to continue processing normally.

**Root Cause Analysis:**

1. `handle_in_body` (exception_block.rs:264-282) checks `if self.capture_widget_next`
2. Inside: `if !trimmed.is_empty()` gates all processing including the safety reset
3. Empty lines skip the entire inner block → `capture_widget_next` stays `true` → returns `Buffered`
4. All subsequent lines (footer, stack frames, next exception headers, normal logs) are eaten

**Real Flutter exception block showing the empty line:**
```
The relevant error-causing widget was:
  _CodeLine
                    <-- EMPTY LINE: parser gets stuck here
When the exception was thrown, this was the stack:
#0      new Container (package:flutter/...)
════════════════════════════════════════════════════════
```

**Affected Files:**
- `crates/fdemon-core/src/exception_block.rs:264-282` — `handle_in_body` widget capture logic

---

### Bug 2: Inconsistent "flutter: " Prefix Handling in parse_flutter_log

**Symptom:** `parse_flutter_log` handles the "flutter: " prefix differently depending on the `error` flag:
- `error: false` → prefix is STRIPPED → parser receives bare content
- `error: true` → early return with FULL message INCLUDING prefix

This means exception lines can arrive at the parser with or without the prefix depending on what `error` flag Flutter's daemon sets.

**Expected:** The exception parser should handle both cases consistently.

**Root Cause Analysis:**

1. `parse_flutter_log` (protocol.rs:278-306) has an early return for `is_error=true` at line 284
2. This early return skips the `strip_prefix("flutter: ")` at line 289
3. The exception parser's `feed_line` (exception_block.rs:172) strips "flutter: " prefix, which handles both cases
4. **This was already fixed** by adding `strip_prefix("flutter: ")` in `feed_line()` — confirmed working

**Status:** Already fixed in current implementation. No further action needed.

**Affected Files:**
- `crates/fdemon-core/src/exception_block.rs:172` — `feed_line` prefix stripping (already fixed)

---

### Bug 3: Stack Frame Detection Uses Wrong Variable

**Symptom:** Stack frames with leading whitespace (e.g., `"  #0  main..."`) are not detected because `line.chars().nth(1)` checks the original indented line instead of the trimmed version.

**Status:** Already fixed in current implementation.

**Affected Files:**
- `crates/fdemon-core/src/exception_block.rs:247-255` — stack frame detection (already fixed)

---

### Bug 4: Second Exception Header Eaten as Description

**Symptom:** If two exception blocks arrive back-to-back without a footer between them (e.g., when the first block's footer is missing or malformed), the second block's header is treated as a description line instead of triggering a new exception block.

**Expected:** A new exception header in `InBody` state should force-complete the current block and start a new one.

**Root Cause Analysis:**
1. `handle_in_body` never calls `extract_library_from_header`
2. A second header like `══╡ EXCEPTION CAUGHT BY RENDERING LIBRARY ╞═══` is accumulated in `description_lines`
3. The parser stays in its current state, merging both exception blocks

**Affected Files:**
- `crates/fdemon-core/src/exception_block.rs:229-296` — `handle_in_body` and `handle_in_stack_trace`

---

## Affected Modules

- `crates/fdemon-core/src/exception_block.rs`: Fix widget capture empty-line handling, add new-header detection in body/stack_trace states
- `crates/fdemon-app/src/handler/session.rs`: Already modified (routing app.log through exception parser)
- `crates/fdemon-app/src/session.rs`: Already modified (`process_log_line_with_fallback`)

---

## Phases

### Phase 1: Fix Parser State Machine Bugs — Critical

Fix the exception parser's state machine to handle real-world Flutter exception output correctly.

**Steps:**

1. **Fix empty-line handling in widget capture mode** (Bug 1 — CRITICAL)
   - In `handle_in_body`, when `capture_widget_next` is true and `trimmed.is_empty()`:
     - If `widget_name` is already set: reset `capture_widget_next = false` (widget section is done)
     - If `widget_name` is NOT set: just buffer (empty line before widget name, keep waiting)
   - This allows the parser to continue processing footer/stack trace after the widget section

2. **Add new-header detection in InBody and InStackTrace states** (Bug 4)
   - In `handle_in_body`, before accumulating description: check `extract_library_from_header(line)`
   - If matched: force-complete current block, start new one
   - Same check in `handle_in_stack_trace`

3. **Add comprehensive tests for real-world Flutter exception blocks**
   - Test: empty line between widget section and stack trace marker
   - Test: empty line between widget name and footer
   - Test: back-to-back exception blocks without footer separation
   - Test: complete real-world RenderFlex overflow output

**Measurable Outcomes:**
- Exception blocks with empty lines between sections complete correctly
- Back-to-back exceptions are detected as separate blocks
- Normal logs after exception blocks are NOT eaten by the parser
- All existing 29 exception block tests continue to pass

---

## Edge Cases & Risks

### Empty Line Positioning
- **Risk:** Empty lines can appear in different positions within exception blocks (between description lines, between widget info and stack trace, between stack trace and footer)
- **Mitigation:** Only reset `capture_widget_next` on empty lines when widget_name is already set; other empty lines are accumulated normally

### Back-to-Back Exception Headers
- **Risk:** Force-completing on header detection could produce incomplete blocks
- **Mitigation:** This is the correct behavior — an incomplete block with partial info is better than eating all subsequent logs

---

## Task Dependency Graph

```
Phase 1
├── 01-fix-widget-capture-empty-line (CRITICAL - root cause)
├── 02-add-new-header-detection (improvement)
└── 03-add-real-world-tests
    └── depends on: 01, 02
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] Exception blocks with empty lines between sections are detected and complete
- [ ] Back-to-back exceptions without footer produce separate exception entries
- [ ] Normal log lines after exception blocks flow through correctly
- [ ] All 29+ existing exception block tests pass
- [ ] New tests cover empty-line and back-to-back scenarios
- [ ] Manual test with `cargo run -- example/app2` shows exception entries in log view

---

## Milestone Deliverable

Exception blocks from real Flutter framework errors (RenderFlex overflow, build errors, assertion failures) are correctly detected, parsed, and displayed as collapsible error entries in fdemon's log view.
