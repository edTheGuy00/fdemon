## Task: Restore MSRV compliance in `fdemon-app` (`state.rs`)

**Objective**: Replace the single `is_multiple_of` call site in `fdemon-app::state::tick()` with `% N == 0` and suppress the resulting clippy lint at function scope, restoring compatibility with the workspace's declared MSRV (`1.77.2`).

**Depends on**: None

**Estimated Time**: 0.25 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/state.rs`: Edit the `tick()` method around line 756. Replace `self.animation_frame.is_multiple_of(15)` with `self.animation_frame % 15 == 0`. Add `#[allow(clippy::manual_is_multiple_of)]` plus the MSRV justification comment on the `tick()` function.

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/devtools/network/tests.rs:11-12`: Use as the reference for the exact comment wording and attribute placement.

### Details

The current code at `crates/fdemon-app/src/state.rs:751-761`:

```rust
/// Tick animation frame and optionally cycle message
///
/// `cycle_messages`: If true, cycle through messages every ~15 ticks (1.5 sec at 100ms)
pub fn tick(&mut self, cycle_messages: bool) {
    self.animation_frame = self.animation_frame.wrapping_add(1);

    if cycle_messages {
        // Cycle message every 15 frames (~1.5 seconds at 100ms tick rate)
        if self.animation_frame.is_multiple_of(15) {
            self.message_index = (self.message_index + 1) % LOADING_MESSAGES.len();
            self.message = LOADING_MESSAGES[self.message_index].to_string();
        }
    }
}
```

After the fix:

```rust
/// Tick animation frame and optionally cycle message
///
/// `cycle_messages`: If true, cycle through messages every ~15 ticks (1.5 sec at 100ms)
// MSRV guard: `is_multiple_of` requires Rust 1.87; MSRV is 1.77.2 — suppress the lint.
#[allow(clippy::manual_is_multiple_of)]
pub fn tick(&mut self, cycle_messages: bool) {
    self.animation_frame = self.animation_frame.wrapping_add(1);

    if cycle_messages {
        // Cycle message every 15 frames (~1.5 seconds at 100ms tick rate)
        if self.animation_frame % 15 == 0 {
            self.message_index = (self.message_index + 1) % LOADING_MESSAGES.len();
            self.message = LOADING_MESSAGES[self.message_index].to_string();
        }
    }
}
```

Note: The MSRV-guard comment goes between the existing rustdoc and the `#[allow]` attribute. Do not delete or modify the existing rustdoc.

### Acceptance Criteria

1. `crates/fdemon-app/src/state.rs` no longer contains `is_multiple_of` (verify with `grep -n 'is_multiple_of' crates/fdemon-app/src/state.rs` → no matches).
2. The `tick()` method has `#[allow(clippy::manual_is_multiple_of)]` preceded by the MSRV justification comment.
3. `cargo clippy -p fdemon-app --all-targets -- -D warnings` exits 0.
4. `cargo test -p fdemon-app` passes — animation-loop tests (any test that exercises `tick()` with `cycle_messages = true`) still validate correct message cycling.
5. `cargo fmt --all` is clean.
6. No other lines in `state.rs` are modified.

### Testing

The existing test suite already exercises `tick()`. No new tests are required — this is a behavior-preserving rewrite. Spot-check that any test asserting the message rotates every 15 ticks still passes. Search for them with:

```bash
grep -rn "tick" crates/fdemon-app/src/state.rs | head
grep -rn "animation_frame\|LOADING_MESSAGES" crates/fdemon-app/src/
```

### Notes

- The literal `15` divisor is non-zero by inspection, so `% 15` is panic-safe and observably identical to `is_multiple_of(15)`.
- Do **not** suppress the lint at module/file scope. Function-level scope matches the precedent and keeps the suppression narrow.
- Do **not** modify any unrelated code in `state.rs`. Keep the diff minimal.

---

## Completion Summary

**Status:** Not Started
**Branch:** _to be filled by implementor_

### Files Modified

| File | Changes |
|------|---------|
| _tbd_ | _tbd_ |

### Notable Decisions/Tradeoffs

_tbd_

### Testing Performed

- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — _tbd_
- `cargo test -p fdemon-app` — _tbd_
- `cargo fmt --all -- --check` — _tbd_

### Risks/Limitations

_tbd_
