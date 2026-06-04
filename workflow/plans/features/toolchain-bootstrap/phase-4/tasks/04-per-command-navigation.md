## Task: Per-command navigation for guided steps (`[`/`]` + index-aware copy)

**Objective**: Let a step with multiple guided commands (macOS Prerequisites:
Xcode CLT / CocoaPods / Rosetta) select and copy each command individually. Add a
`selected_command_index` to `InstallWizardState`, `[`/`]` navigation keys, and make
`selected_guided_command()` (and thus `c`) index-aware instead of always `.first()`.

**Depends on**: 03-prerequisites-guided-commands (same file `state.rs` — sequential)

**Estimated Time**: 4-6 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/state.rs`: add `selected_command_index`;
  index-aware `selected_guided_command`; navigation mutators; reset on step change.
- `crates/fdemon-app/src/message.rs`: add `InstallWizardPrevCommand`,
  `InstallWizardNextCommand`.
- `crates/fdemon-app/src/handler/keys.rs`: route `[`/`]` in
  `handle_key_install_wizard`.
- `crates/fdemon-app/src/handler/update.rs`: wire the two new messages.
- `crates/fdemon-app/src/handler/install_wizard/navigation.rs`: add
  `handle_prev_command` / `handle_next_command`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/types.rs`: `GuidedCommand`, `WizardPane`.

### Details

1. **State** (`install_wizard/state.rs`):
   - Add `pub selected_command_index: usize` to `InstallWizardState`
     (`state.rs:42-76`); it derives `Default` to `0`.
   - Change `selected_guided_command()` (`state.rs:115-117`) to return
     `guided_commands.get(self.selected_command_index)` instead of `.first()`,
     clamping defensively (treat out-of-range as `None` or clamp to last).
   - Add pure mutators:
     ```rust
     pub fn select_next_command(&mut self) { /* clamp to guided_commands.len()-1 */ }
     pub fn select_prev_command(&mut self) { /* saturating_sub(1) */ }
     ```
     Both no-op when the selected step has 0 or 1 guided commands.
   - **Reset `selected_command_index = 0`** wherever the selected step changes:
     the `StepList` arms of `handle_up`/`handle_down` (`navigation.rs:62-66`,
     `87-92`, alongside the existing `detail_scroll = 0` reset) and in
     `apply_report` (`state.rs:95-102`, alongside the `selected_index` clamp).

2. **Message** (`message.rs`): add `InstallWizardPrevCommand` and
   `InstallWizardNextCommand` next to the existing `InstallWizardCopyCommand`
   (`message.rs:1737`).

3. **Keys** (`handler/keys.rs`): in `handle_key_install_wizard` (`keys.rs:~413-441`,
   beside the `c`/`r` arms at `keys.rs:438-440`):
   ```rust
   InputKey::Char('[') => Some(Message::InstallWizardPrevCommand),
   InputKey::Char(']') => Some(Message::InstallWizardNextCommand),
   ```
   Update the doc comment listing the wizard keys.

4. **Update wiring** (`handler/update.rs`): dispatch the two messages to the new
   navigation handlers (pure state mutation → `UpdateResult::none()`).

5. **Handlers** (`handler/install_wizard/navigation.rs`): add `handle_prev_command`
   / `handle_next_command` calling the state mutators. These work regardless of
   `focused_pane` (the commands live in the detail pane but are navigable from
   either pane), and are a no-op when the step has ≤1 guided command.

### Acceptance Criteria

1. `selected_command_index` defaults to 0 and resets to 0 on every step change
   (`handle_up`/`handle_down` StepList arms and `apply_report`).
2. `]`/`[` advance/retreat `selected_command_index`, clamped to
   `[0, guided_commands.len()-1]`; both no-op for steps with ≤1 command.
3. `selected_guided_command()` returns the command at `selected_command_index`
   (not always the first); `c` (`handle_copy_command`, `actions.rs:372-378`) copies
   the selected command unchanged otherwise.
4. `[`/`]` in `UiMode::InstallWizard` emit `InstallWizardPrev/NextCommand`; the
   messages reach the navigation handlers.

### Testing

```rust
#[cfg(test)]
mod tests {
    // state.rs:
    //   - select_next/prev clamp within [0, len-1]; no-op for 0 or 1 command
    //   - selected_guided_command returns the indexed command
    //   - step change (handle_up/down StepList) resets selected_command_index
    //   - apply_report resets selected_command_index
    // keys.rs:
    //   - '[' -> InstallWizardPrevCommand, ']' -> InstallWizardNextCommand
    // navigation.rs:
    //   - handlers mutate index; no-op on single-command steps
}
```

### Notes

- No new `UpdateAction` — this is pure state mutation; `c`'s existing
  `WriteClipboard` action is reused unchanged.
- `[`/`]` were chosen to avoid colliding with `j`/`k` (step nav / detail scroll),
  `Tab` (pane switch), and `c`/`r`. Document them in task 06.
- Only the macOS Prerequisites step currently carries >1 guided command; the keys
  are harmless no-ops elsewhere (AndroidTools has one JDK command, Linux/Windows
  Prerequisites one command).
