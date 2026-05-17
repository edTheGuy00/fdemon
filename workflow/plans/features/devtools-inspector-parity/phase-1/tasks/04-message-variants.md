## Task: Add new `Message` variants for details + tab + toggle

**Objective**: Add the four new `Message` enum variants required by Phase 1 handlers and key bindings.

**Depends on**: 02-state-inspector-extensions (uses `DetailsTab` indirectly — variants are pure-data, but the cycle direction parameter mirrors `DetailsTab::next/prev`).

**Estimated Time**: ~1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/message.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` (only to confirm `DetailsTab` exists from task 02 — no struct usage in the message itself).

### Details

Add the following variants to the existing `pub enum Message` block in `message.rs`. Group them near the other `DevToolsInspector*` variants for readability (the existing variants live around line 997 and 1588–1596 — pick the natural insertion point near `DevToolsInspectorSelectRow`).

```rust
/// Opens the Details view for the currently selected widget in the
/// Inspector tree. Snapshots the selected value_id into
/// `details_node_id` and sets `details_open = true`.
///
/// In Phase 1 this also fires `FetchLayoutData` for the snapshotted node
/// if it isn't already cached. Phase 2 will additionally fire
/// `FetchInspectorProperties`.
DevToolsInspectorOpenDetails,

/// Closes the Details view and returns the Inspector tab to tree mode.
/// Tied to the first Esc press while details is open (tiered Esc).
DevToolsInspectorCloseDetails,

/// Cycles the active Details tab. `forward = true` → next tab; `false` →
/// previous tab.
DevToolsInspectorCycleTab { forward: bool },

/// Toggles `inspector.hide_implementation_widgets`. Rebuilds the row list
/// and persists the new value to `.fdemon/config.toml` (see task 03 and
/// task 05 for the persistence path).
DevToolsInspectorToggleHideImplementation,
```

#### Doc comments

Each variant needs a `///` doc block explaining:
- What action it triggers.
- What state it expects to read/mutate.
- Which key binding(s) produce it (forward reference to task 06).

#### `Debug` / `PartialEq` / `Eq` derives

Match the existing message derives. No need to add `Clone` unless the existing enum already derives it (it does).

#### No new tests required in this task

`Message` is a tagged union; the variants are tested via the handlers (task 05) and key binding tests (task 06).

### Acceptance Criteria

1. `cargo check --workspace --all-targets` passes (the new variants compile but are unused at this point; that's expected — task 05 and task 06 wire them up).
2. Each new variant has a `///` doc comment.
3. `cargo clippy --workspace --all-targets -- -D warnings` passes — including the `dead_code` lint, which means the variants must NOT trigger dead-code warnings (the enum is `pub`, so dead-code is suppressed by visibility).
4. `cargo fmt --all -- --check` passes.

### Testing

This task does not add tests of its own. The variants are exercised by:
- Task 05 (handler dispatch tests).
- Task 06 (key binding → message tests).

### Notes

- Use `forward: bool` rather than a custom `Direction` enum — keeps the message small and the pattern matches existing `Message` style elsewhere in the file.
- The `DevToolsInspectorToggleHideImplementation` variant is intentionally parameterless. The handler reads the current value, flips it, and writes it back; the toggle is not signed (cannot force a specific value via the message bus). This matches how other toggles work in the codebase (search: `grep -rn "Toggle" crates/fdemon-app/src/message.rs`).
- Do NOT add a `DevToolsInspectorRefreshDetails` variant in this phase — refresh while in details mode falls under Phase 2.

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
