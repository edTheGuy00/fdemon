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

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/message.rs` | Added `use crate::session::performance::PerfSection` import; added 7 new variants (`PerfFocusSection`, `PerfScrollUp`, `PerfScrollDown`, `PerfPageUp`, `PerfPageDown`, `PerfJumpToStart`, `PerfJumpToEnd`, `PerfSelectAllocRow`) under `// --- Performance panel interactivity ---` section comment |
| `crates/fdemon-app/src/handler/update.rs` | Added stub match arms for all 7 new variants returning `UpdateResult::none()`, keeping the match exhaustive until Phase 2 handlers are wired in |

### Notable Decisions/Tradeoffs

1. **Stub arms in update.rs**: The task says not to add handlers (Phase 2's job), but `update.rs` has an exhaustive match so the code would not compile without arms. Added `UpdateResult::none()` stubs with a comment noting they are Phase 2 stubs — minimal and clearly labeled for replacement.

### Testing Performed

- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace --lib` - Passed (1018 tests, 0 failed)

### Risks/Limitations

1. **Stub arms**: The 7 new variants are silently no-ops until Phase 2 wires them up. Any key events dispatching these messages before Phase 2 will be quietly dropped.
