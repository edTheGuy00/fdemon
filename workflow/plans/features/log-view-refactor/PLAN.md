# Plan: Log View Module Refactoring & Test Organization

## TL;DR

Refactor the oversized `log_view.rs` widget (2262 lines) into a modular directory structure following Rust idioms and the project's existing patterns. The file will be split into logical submodules: state management, styling constants, widget implementation, and tests. Additionally, clarify test organization best practices: unit tests remain in `src/` (using separate `tests.rs` files), while integration tests belong in the `tests/` directory.

## Current State Analysis

### log_view.rs Breakdown (2262 lines)

| Section | Lines | Description |
|---------|-------|-------------|
| `mod stack_trace_styles` | L22-58 (~37) | Styling constants for stack traces |
| `FocusInfo` | L73-87 (~15) | Focus tracking struct |
| `LogViewState` | L95-252 (~158) | Scroll state management |
| `LogView` struct + impl | L255-1000 (~746) | Main widget with formatting methods |
| `StatefulWidget` impl | L1002-1204 (~203) | Widget trait implementation |
| `Widget` impl | L1207-1212 (~6) | Simple widget wrapper |
| **`mod tests`** | **L1215-2262 (~1047)** | **Unit tests (46% of file!)** |

### Existing Test Patterns in Project

The project already uses the separate `tests.rs` file pattern:
- `src/app/handler/tests.rs` - Tests in sibling file with `#[cfg(test)] mod tests;` in `mod.rs`

### Rust Test Organization Best Practices

| Test Type | Location | Access Level | Purpose |
|-----------|----------|--------------|---------|
| **Unit Tests** | `src/**/*.rs` or `src/**/tests.rs` | Private items via `use super::*` | Test implementation details |
| **Integration Tests** | `tests/*.rs` | Public API only | Test public interface |

**Key Insight:** Moving unit tests to `tests/` directory would break their ability to test private functions. The project's existing pattern (`handler/tests.rs`) is the correct approach.

## Affected Modules

- `src/tui/widgets/log_view.rs` → Becomes `src/tui/widgets/log_view/` directory
- `src/tui/widgets/mod.rs` → Update module declaration
- `tests/` → Add clarification; remains for integration tests only

## Phases

### Phase 1: Convert log_view.rs to Module Directory

Transform the single 2262-line file into a well-organized module directory, maintaining the same public API.

**Target Structure:**
```
src/tui/widgets/log_view/
├── mod.rs           # Module declarations, re-exports, LogView struct
├── state.rs         # LogViewState, FocusInfo structs and impls
├── styles.rs        # stack_trace_styles constants module
└── tests.rs         # All unit tests (~1047 lines)
```

**Steps:**
1. Create `src/tui/widgets/log_view/` directory
2. Extract `stack_trace_styles` module to `styles.rs`
3. Extract `FocusInfo` and `LogViewState` to `state.rs`
4. Move test module to `tests.rs` with `#[cfg(test)] mod tests;` declaration
5. Create `mod.rs` with `LogView` struct, impls, and re-exports
6. Update `src/tui/widgets/mod.rs` to use the new module
7. Verify compilation with `cargo check`
8. Verify all tests pass with `cargo test`

**Measurable Outcomes:**
- No single file exceeds 800 lines
- All existing tests pass
- Public API unchanged (`LogView`, `LogViewState` exports)
- `cargo doc` generates same documentation

### Phase 2: Document Test Organization Strategy

Create clear documentation about the project's test organization approach.

**Steps:**
1. Add "Testing" section to `docs/ARCHITECTURE.md`
2. Document unit test pattern (inline `mod tests` or sibling `tests.rs`)
3. Document integration test location (`tests/` directory)
4. Optionally add integration tests for `LogView` public API in `tests/`

**Measurable Outcomes:**
- ARCHITECTURE.md contains testing guidelines
- New contributors understand test placement

### Phase 3: Apply Pattern to Other Large Modules (Optional/Future)

Identify other modules with large inline test sections and apply the same refactoring pattern.

**Candidates (modules with 100+ lines of tests):**
- `src/core/types.rs`
- `src/core/stack_trace.rs`
- `src/daemon/protocol.rs`
- `src/tui/widgets/device_selector.rs`
- `src/tui/hyperlinks.rs`

**Steps:**
1. For each candidate, extract tests to sibling `tests.rs` file
2. Use `#[cfg(test)] mod tests;` pattern
3. Verify tests pass

## Edge Cases & Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Breaking public imports | High | Keep re-exports in `mod.rs`; run `cargo check` |
| Test visibility issues | Medium | Tests can access `pub(crate)` and `pub(super)` items via `use super::*` |
| IDE navigation breaks | Low | Modern Rust IDEs handle module directories well |
| Merge conflicts with in-flight work | Medium | Coordinate timing; refactor is mechanical |
| `#[allow(dead_code)]` on test helpers | Low | Keep helpers in `tests.rs` scope or mark `pub(crate)` |

## Further Considerations

1. **Should we consolidate all widget tests into one file?**
   - Recommendation: No, keep tests colocated with their modules for maintainability

2. **Should we add integration tests for LogView?**
   - Could add `tests/log_view_integration.rs` for testing the widget's public API with ratatui's `TestBackend`

3. **Should we extract more from LogView::impl?**
   - The formatting methods could become a separate `formatting.rs`, but current split is sufficient

4. **What about the `#[allow(dead_code)]` annotations?**
   - `format_stack_frame` and `format_stack_frame_line` are only used in tests
   - After refactor, consider making them `pub(crate)` or keeping the annotation

5. **Consistency across project?**
   - Phase 3 would apply this pattern project-wide, but it's optional and lower priority