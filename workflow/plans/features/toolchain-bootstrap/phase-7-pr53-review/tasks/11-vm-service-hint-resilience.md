## Task: Make the VM-service-unavailable hint resilient to late progress and buffered errors (F-PR53-18)

**Severity:** LOW (correctness — cosmetic)

**Objective**: Keep the "VM service unavailable — see logs" inline hint visible
even if a late `app.progress(finished:true)` arrives, and ensure the guidance log
entries render *after* the triggering error line rather than above a still-buffered
one.

**Depends on**: — (disjoint; safe to parallelize)

**Estimated Time**: 1.5–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/session/session.rs`
- `crates/fdemon-app/src/handler/session.rs`

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/render/mod.rs` (`current_progress` consumed at ~231)

### Details

**(a) Hint cleared by late progress.**
`detect_vm_service_failure` sets `current_progress` to the VM-failure hint
(`session/session.rs:655`) and its comment asserts "no further app.progress
arrives, so this hint persists" — an assumption, not an invariant. The session is
stuck `Launching` (`is_running()` false, 772-774), so the `AppProgress` handler in
`handler/session.rs:304-310` still runs and the `(_, true) => clear_progress()`
arm (307) wipes the hint. `set_progress`/`clear_progress` (673-680) are unguarded.
The four guidance **log** entries (631-650) persist, so impact is cosmetic (only
the compact phase-label hint is at risk).

**(b) Guidance may render above a buffered error line.**
`handle_session_stdout` (`handler/session.rs:83-94`) runs `process_raw_line()`
then `detect_vm_service_failure(line)`. The latter flushes only
`flush_batched_logs()` (session.rs:629), not the exception parser's buffer
(`flush_exception_buffer`, 564). If the marker line was returned
`FeedResult::Buffered`, it is held in the exception parser and the guidance appears
above it — contradicting the stated "error appears immediately before our
guidance" intent. Narrow corner case (the marker does not start an exception block;
only triggers when the parser is mid-block).

### Proposed Fix

1. Guard `clear_progress()`/`set_progress()` in the `AppProgress` handler with
   `!handle.session.vm_service_unavailable`, **or** store the VM-failure hint in a
   dedicated field that the renderer prefers over `current_progress` so a late
   `finished:true` cannot erase it.
2. If strict ordering matters, also call `flush_exception_buffer()` (in addition to
   `flush_batched_logs()`) before appending guidance, or route the guidance through
   the same batched/exception pathway. (Documenting the limitation is acceptable
   given the corner-case nature.)

### Acceptance Criteria

1. After `detect_vm_service_failure` sets the hint, a subsequent
   `AppProgress { finished: true }` while the session is `Launching` does **not**
   clear the VM-failure hint.
2. The four guidance log entries continue to be emitted unconditionally (no
   regression).
3. (If addressed) the triggering marker line is flushed to the log before the
   guidance entries.

### Testing

```rust
// session/session.rs (or handler/session.rs) test module
// - test_vm_failure_hint_survives_late_finished_progress: set hint, feed an
//     AppProgress finished:true while Launching, assert current_progress (or the
//     dedicated field) still shows the VM-failure hint.
// - (optional) ordering test if flush_exception_buffer is added.
```

### Notes

- File-disjoint from all other tasks → Wave 1 parallel worktree candidate.
- Lowest-priority item; purely a UX-polish fix since the actionable guidance log
  survives regardless.
