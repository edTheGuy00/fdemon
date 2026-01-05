## Task: Create log_view Module Directory Structure

**Objective**: Create the directory structure for the new `log_view` module and set up the initial `mod.rs` file with module declarations and re-exports.

**Depends on**: None

### Scope

- Create `src/tui/widgets/log_view/` directory
- Create `src/tui/widgets/log_view/mod.rs` with module structure
- Update `src/tui/widgets/mod.rs` to use the new module directory

### Implementation Details

1. **Create directory**: `src/tui/widgets/log_view/`

2. **Create `mod.rs`** with the following structure:
   ```rust
   //! Scrollable log view widget with rich formatting
   
   mod state;
   mod styles;
   
   #[cfg(test)]
   mod tests;
   
   // Re-export public types
   pub use state::{FocusInfo, LogViewState};
   pub use styles::stack_trace_styles;
   
   // ... LogView struct and impl will be added in task 04
   ```

3. **Update `src/tui/widgets/mod.rs`**:
   - Change `mod log_view;` to reference the directory module (no change needed if using standard module resolution)
   - Verify `pub use log_view::{LogView, LogViewState};` still works

### File Structure After This Task

```
src/tui/widgets/
├── log_view/
│   └── mod.rs          # Initial skeleton
├── mod.rs              # Updated to use log_view directory
├── confirm_dialog.rs
├── device_selector.rs
├── header.rs
├── search_input.rs
├── status_bar.rs
└── tabs.rs
```

### Acceptance Criteria

1. Directory `src/tui/widgets/log_view/` exists
2. `mod.rs` contains module declarations (even if submodules don't exist yet)
3. `cargo check` passes (may need placeholder files)
4. No changes to public API

### Testing

- Run `cargo check` to verify module structure is valid
- Temporarily comment out submodule declarations if files don't exist yet

### Notes

This task creates the scaffolding. Subsequent tasks will populate the submodules. The original `log_view.rs` file will be removed in the final task after all content is migrated.