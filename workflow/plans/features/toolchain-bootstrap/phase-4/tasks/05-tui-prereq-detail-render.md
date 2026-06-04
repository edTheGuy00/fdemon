## Task: Prerequisites detail-pane caption + index-aware copy hint + render tests

**Objective**: Render the Prerequisites guided-command block with a contextual
caption (analogous to the AndroidTools JDK caption), reserve height for it, and move
the `[c] copy` hint + selection highlight to follow `selected_command_index` so the
selected macOS command is visibly the one `c` copies.

**Depends on**: 03-prerequisites-guided-commands, 04-per-command-navigation

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`

**Files Read (Dependencies):**
- `fdemon-app::install_wizard` re-exports: `selected_command_index`, `GuidedCommand`,
  `WizardStepKind`.

### Details

1. **Caption.** In `render_guided_commands` (`step_detail.rs:255-338`) the JDK
   caption block (`step_detail.rs:279-290`) is gated on `WizardStepKind::AndroidTools`.
   Add a `Prerequisites` caption arm — e.g. `"Install the OS build tools below, then
   press r to re-check"` — reusing `JDK_CAPTION_HEIGHT` (`step_detail.rs:75`).
   Prefer a small `match step_kind` over duplicating the `if`-block.

2. **Height reservation.** Update the `bottom_section_height` reservation
   (`step_detail.rs:~439-446`), which currently adds `JDK_CAPTION_HEIGHT` only for
   `AndroidTools` (`step_detail.rs:442`): extend the `caption_rows` condition to also
   include `WizardStepKind::Prerequisites` so the reserved area matches what renders.

3. **Index-aware copy hint + highlight.** The N-command loop (`step_detail.rs:292-336`)
   already renders every command, but the `[c] copy` hint is hardcoded to `i==0`
   (`step_detail.rs:316`). Change it to attach the hint **and** the selection
   highlight to `i == selected_command_index` (read from the wizard state), so the
   command `c` will copy is the visibly-selected one. When a step has a single
   command, index 0 stays selected (unchanged behavior).

4. **Suppress the "later phase" bottom hint for guided Prerequisites.**
   `render_action_hint` (`step_detail.rs:196-238`) already early-returns for
   `AndroidTools` with guided commands (`~step_detail.rs:219-222` via
   `has_guided_commands`); `is_executable` (`step_detail.rs:173-179`) returns
   `false` for `Prerequisites`. Add the same `has_guided_commands` early-return
   guard for `Prerequisites` so the guided block renders **without** also showing the
   muted "Available in a later phase" hint.

No new state is introduced here (`selected_command_index` comes from task 04).

### Acceptance Criteria

1. The Prerequisites step renders a caption above its guided command(s), and the
   reserved bottom-section height accounts for it (no clipping/overlap).
2. The `[c] copy` hint and selection highlight follow `selected_command_index`, not
   a fixed index 0; on a single-command step they stay on index 0.
3. For a Prerequisites step that carries guided commands, the muted "Available in a
   later phase" action hint no longer renders.
4. The per-OS command line(s) render in the detail pane (Linux single line; macOS up
   to three lines).

### Testing

```rust
#[cfg(test)]
mod tests {
    // - Prerequisites caption renders
    // - per-OS command line(s) render (Linux 1, macOS up to 3)
    // - [c] hint/highlight tracks selected_command_index (e.g. index 1 of 3)
    // - "later phase" hint absent for Prerequisites-with-commands
}
```

Update the existing `step_detail.rs` render test asserting "Available in a later
phase" for `Prerequisites` (`~step_detail.rs:725-738`) — it now expects the guided
block instead. The `Doctor` step's behavior is unchanged.

### Notes

- Parallel-safe with task 06 (this writes `step_detail.rs`; task 06 writes
  `KEYBINDINGS.md`).
- Keep the single-command-per-OS rendering for Linux/Windows visually identical to
  the existing JDK command rendering; only macOS gains the multi-line selectable list.
