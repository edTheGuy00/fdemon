## Task: Add `Message` Variants for Performance Tab Interactivity

**Objective**: Add 7 new `Message` variants supporting section focus, scroll, jump-to-edges, and allocation-row selection.

**Depends on**: 01-perf-section-enum-and-state-fields (`PerfSection` type)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/message.rs`: Add the following variants to the `Message` enum (place near existing `SelectPerformanceFrame` at ~line 858):
  ```rust
  PerfFocusSection(PerfSection),
  PerfScrollUp,
  PerfScrollDown,
  PerfPageUp,
  PerfPageDown,
  PerfJumpToStart,
  PerfJumpToEnd,
  PerfSelectAllocRow { index: Option<usize> },
  ```
- Update `Debug`, `Clone`, `PartialEq` derives if they aren't already auto-derived.
- Import `PerfSection` from `crate::session::performance::PerfSection`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/session/performance.rs` (from task 01): For `PerfSection`.

### Details

Place the new variants together with `// --- Performance panel interactivity ---` as a section comment for readability. Keep the variant style consistent with neighbors (snake-free PascalCase, no abbreviations beyond `Perf` which is already used).

```rust
// In message.rs near SelectPerformanceFrame:

// --- Performance panel interactivity ---
PerfFocusSection(PerfSection),
PerfScrollUp,
PerfScrollDown,
PerfPageUp,
PerfPageDown,
PerfJumpToStart,
PerfJumpToEnd,
PerfSelectAllocRow { index: Option<usize> },
```

### Acceptance Criteria

1. All 7 (counting `PerfSelectAllocRow`'s single variant, technically 7) message variants exist with the correct types.
2. `PerfFocusSection` carries a `PerfSection`, `PerfSelectAllocRow` carries `index: Option<usize>`.
3. `Message` continues to compile and pattern-matches throughout the codebase have a `_ => {}` default (or a unit test asserts that all variants are routed somewhere in `update.rs` after Phase 2).
4. `cargo check --workspace --all-targets` passes.

### Testing

No new tests at the data-type layer; the message types are exercised by handlers in Phase 2.

### Notes

- If `Message` has a `Debug` impl that prints each variant, no change needed — `#[derive(Debug)]` covers it.
- Don't add handlers here — Phase 2's job. Just the variants.
- Existing `match` blocks on `Message` will get warnings about non-exhaustive patterns if `Message` doesn't have a catch-all. Use a wildcard `_ => {}` only at the routing layer; in `update.rs` (Phase 2) we will route each variant explicitly.
