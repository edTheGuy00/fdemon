# Task 06 — `handler/update.rs` arms for the three new messages

**Agent:** implementor
**Wave:** 2
**Depends on:** Task 02 (clipboard trait), Task 03 (message + action variants)
**Files written:** `crates/fdemon-app/src/handler/update.rs`

---

## Goal

Add three arms to the `update(state, message)` pattern match:

1. `Message::CopyLogEntryToClipboard { entry_id }` — resolve the entry's rendered text from `state`, write it via the injected `Clipboard`, push a confirmation toast.
2. `Message::ToggleMouseCapture` — return `(state, Some(UpdateAction::SetMouseCapture(!state.mouse_capture_active)))` so the runner performs the side effect.
3. `Message::MouseCaptureChanged { active }` — set `state.mouse_capture_active = active`, no follow-up action.

## Background

The handler signature already returns `(AppState, Option<UpdateAction>)`. The new `SetMouseCapture(bool)` action variant (Task 03) plugs cleanly into that channel. The `Clipboard` reference is the only friction point — see "Signature change" below.

## Implementation

### Signature change for the clipboard

The handler currently does not have a `Clipboard` parameter. Two options:

- **Option A** — thread `clipboard: &mut dyn Clipboard` as an additional parameter on `update()`. Cleanest, but every call site needs the parameter, including tests.
- **Option B** — accept the impl impurity locally: only `CopyLogEntryToClipboard` needs the clipboard; instead of threading it through `update`, expose a `Vec<UpdateAction>` (or extend the single `UpdateAction` with `WriteClipboard(String)`) and let the runner perform the actual write.

**Choose Option B.** Add a new `UpdateAction` variant `WriteClipboard { text: String, preview: String }` (the runner uses `text` for the actual write, `preview` for the toast — the handler builds both up-front and includes the preview so the toast is shown after the write succeeds via a follow-up `Message::ClipboardWriteResult { success: bool, preview: String }`).

Wait — re-reading: simplest is to push the toast in the **handler** at the time the action is emitted, and let the action be a fire-and-forget write. If the write fails the runner emits a follow-up `ClipboardWriteFailed { preview }` that the handler arms to a warning toast + revoke the success toast (track by id).

To keep this task small, take the **simplest viable approach**:

1. Add `UpdateAction::WriteClipboard { text: String }` (no preview, no follow-up). Failure on the runner side is logged at `warn` and surfaced via a one-shot toast emitted from the runner (already a pattern the runner uses for daemon errors).
2. Handler pushes the success toast immediately, optimistically. If the write later fails, the runner pushes a warning toast atop the success one — the warning will catch the user's eye and the success-then-warning sequence is acceptable for a rare path.

This change adds **one** new `UpdateAction` variant in this task (not Task 03's set, because that variant only matters once the consumer wires up). Document the divergence at the top of Task 03's implementation: leave the `UpdateAction::WriteClipboard` addition to **this** task to keep Task 03 a pure-declaration task.

### Arm implementations

```text
Message::CopyLogEntryToClipboard { entry_id } => {
    let entry_text = state.resolve_entry_text(entry_id);   // helper — see below
    let preview = truncate_with_ellipsis(&entry_text, 60);
    state.push_toast(ToastLevel::Info, format!("Copied: {preview}"));
    (state, Some(UpdateAction::WriteClipboard { text: entry_text }))
}

Message::ToggleMouseCapture => {
    let target = !state.mouse_capture_active;
    (state, Some(UpdateAction::SetMouseCapture(target)))
}

Message::MouseCaptureChanged { active } => {
    state.mouse_capture_active = active;
    let label = if active { "Mouse capture on" } else { "Mouse capture off — native selection ready" };
    state.push_toast(ToastLevel::Info, label);
    (state, None)
}
```

`resolve_entry_text(entry_id)`: helper on `AppState` (or a free function in this module) that looks up the entry by id in the active session's log buffer and returns the rendered text. If the entry no longer exists (session switched / cleared mid-click), return an empty string and skip the action — the optimistic toast still fires but with empty preview; alternatively, gate on `is_empty()` and skip emitting the action + push a different toast (`"Entry no longer available"`).

`truncate_with_ellipsis(s, n)`: char-boundary-safe, appends `…` when truncated. Likely already exists somewhere in the codebase — grep for `truncate` / `ellipsis` first; reuse if so, otherwise add as a private helper.

## Tests

- `test_copy_message_pushes_toast_and_emits_action` — set up `AppState` with one entry, dispatch the message, assert the toast text and the `WriteClipboard` action payload.
- `test_copy_message_truncates_preview_to_60_chars` — entry with 200-char text; toast preview ≤ 60 chars + ellipsis.
- `test_copy_message_with_missing_entry_skips_action` — id not in state; assert no `WriteClipboard` action, alternate toast pushed.
- `test_toggle_emits_set_mouse_capture_with_inverted_target` — `mouse_capture_active = true` → action carries `false`, and vice versa.
- `test_toggle_does_not_mutate_state_directly` — the toggle itself does not flip `state.mouse_capture_active`; that field only changes on `MouseCaptureChanged`.
- `test_mouse_capture_changed_updates_state_and_toasts` — both branches (true/false) push the correct toast.

## Acceptance Criteria

- [ ] Six new unit tests pass.
- [ ] No new dependency on `Clipboard` in `handler/update.rs` (writes are deferred to the runner via `UpdateAction::WriteClipboard`).
- [ ] `resolve_entry_text` helper is either reused from existing code or added with its own focused unit test.
- [ ] `truncate_with_ellipsis` uses char-boundary-safe truncation (no panics on multibyte input).

## Notes for Reviewer

- The decision to keep `Clipboard` out of `update` (Option B above) was a deliberate trade-off: the handler stays pure, and the runner is the single place that owns side effects. The cost is a slight delay between the success toast and the actual write (microseconds in practice). Worth the simplification.
- A follow-up improvement (not in scope here) could add a `ClipboardWriteFailed { preview }` message to revoke the optimistic toast cleanly — left in BUG.md's "Future Enhancements" implicit scope.
