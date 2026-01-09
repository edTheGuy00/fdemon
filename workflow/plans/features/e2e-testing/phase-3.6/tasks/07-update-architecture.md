## Task: Update ARCHITECTURE.md for TEA Compliance

**Objective**: Update the architecture documentation to accurately reflect the TUI→App dependency required by the TEA pattern, and document the new `render/` module structure.

**Depends on**: Wave 3 complete

### Scope

- `docs/ARCHITECTURE.md`: Update layer dependencies and module documentation

### Details

**Issue 1: Incorrect TUI dependencies**

The current documentation says TUI depends only on Core, but the TEA pattern requires TUI (View) to access App (Model):

**Current (line ~79):**
```markdown
| **TUI** | Presentation | Core |
```

**Fix:**
```markdown
| **TUI** | Presentation | Core, App (TEA View pattern) |
```

**Issue 2: Missing explanation of TEA View pattern**

Add clarification in the Design Principles section:

```markdown
### Layer Dependencies Note

The TUI layer depends on App because of the TEA pattern:
- **View** (`tui::render`) must receive **Model** (`AppState`) to render it
- This is the fundamental TEA contract: `View: State → UI`
- The dependency is intentional and necessary, not a violation
```

**Issue 3: Missing render/ module documentation**

Update the TUI section to reflect the new module structure:

```markdown
### `tui/` — Terminal UI

| File | Purpose |
|------|---------|
| `mod.rs` | Main event loop, message channel setup |
| `render/mod.rs` | State → UI rendering (was render.rs) |
| `render/tests.rs` | Full-screen snapshot and transition tests |
| `layout.rs` | Layout calculations for different UI modes |
| `event.rs` | Terminal event polling |
| `terminal.rs` | Terminal initialization, cleanup, panic hook |
| `selector.rs` | Interactive project selection |
| `test_utils.rs` | TestTerminal wrapper and test helpers |
```

**Issue 4: Update test coverage table**

Add new test files to the coverage table:

```markdown
| Module | Test File | Coverage |
|--------|-----------|----------|
| ... existing ... |
| `tui/render` | `render/tests.rs` | Full-screen snapshots, UI transitions |
| `tui/widgets/status_bar` | `status_bar/tests.rs` | Widget rendering, phase display |
```

### Acceptance Criteria

1. Layer dependency table shows `TUI | Presentation | Core, App`
2. TEA View pattern explanation added
3. `render/` module documented
4. `test_utils.rs` documented
5. Test coverage table updated with new test files

### Testing

```bash
# Verify docs compile (no broken links)
cargo doc --no-deps

# Manual review: read through updated sections
```

### Notes

- This documents reality, not changing behavior
- The TUI→App dependency is correct per TEA pattern
- Future readers will understand why View needs Model access

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Updated layer dependency table to show TUI → App dependency; added TEA View pattern explanation; updated tui/ module table with render/ structure and test_utils.rs; updated test coverage table with render/tests.rs and status_bar tests |

### Notable Decisions/Tradeoffs

1. **TEA View Pattern Explanation**: Added explicit documentation that the TUI→App dependency is intentional and required by the TEA pattern. This clarifies what could appear to be a layer violation to future maintainers.
2. **Test Coverage**: Listed status_bar tests as "inline" rather than separate file, reflecting actual test organization in the codebase.

### Testing Performed

- `cargo doc --no-deps` - Passed (documentation builds successfully)
- `cargo check` - Passed (code compiles without errors)

### Risks/Limitations

None - This is a documentation-only change that accurately reflects the existing architecture and test structure.
