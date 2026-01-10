## Task: Add TODO Comments to Dead Code

**Objective**: Add TODO comments to all dead code functions in `startup.rs` referencing the Phase 4 cleanup plan.

**Depends on**: Phase 2

**Estimated Time**: 15 minutes

### Scope

- `src/tui/startup.rs`: Add TODO comments to 6 functions with `#[allow(dead_code)]`

### Details

The Phase 2 review noted that dead code attributes exist without references to when/why they'll be cleaned up. This creates uncertainty about whether the code is intentionally dead (awaiting future use) or accidentally dead (should be removed).

Add a TODO comment before each `#[allow(dead_code)]` attribute:

```rust
// TODO(phase-4): Remove after cleanup - see workflow/plans/features/startup-flow-consistency/phase-4/
#[allow(dead_code)]
async fn function_name(...) { ... }
```

#### Functions to annotate:

| Line | Function |
|------|----------|
| 43 | `animate_during_async<T, F>` |
| 95 | `auto_start_session` |
| 182 | `try_auto_start_config` |
| 220 | `launch_with_validated_selection` |
| 235 | `launch_session` |
| 284 | `enter_normal_mode_disconnected` |

### Acceptance Criteria

1. All 6 dead code functions have TODO comments referencing Phase 4
2. TODO comments follow the pattern: `// TODO(phase-4): Remove after cleanup - see workflow/plans/features/startup-flow-consistency/phase-4/`
3. `cargo fmt` passes (comments don't break formatting)
4. `cargo check` passes

### Testing

No unit tests required - this is comments-only. Manual verification:

```bash
cargo fmt
cargo check
# Verify with grep:
grep -n "TODO(phase-4)" src/tui/startup.rs | wc -l
# Should output: 6
```

### Notes

- These functions were the sync startup path that Phase 2 replaced with the async message-based flow
- Phase 3 will complete the async implementation
- Phase 4 is specifically for cleaning up this dead code
- The TODO tag `(phase-4)` makes it easy to find all related cleanup work

---

## Completion Summary

**Status:** (Not started)
