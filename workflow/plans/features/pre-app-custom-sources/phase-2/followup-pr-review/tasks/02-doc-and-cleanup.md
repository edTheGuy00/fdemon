## Task: Fix Stale Doc Comment and Remove Unused Import

**Objective**: Fix the inaccurate doc comment on `SpawnPreAppSources.running_shared_names` and remove the unused `import sys` from the example server script.

**Depends on**: None

**Severity**: MINOR

**Review Reference**: [PR #23 Copilot comments](https://github.com/edTheGuy00/fdemon/pull/23#discussion_r2936678254) and [unused import](https://github.com/edTheGuy00/fdemon/pull/23#discussion_r2936678256)

### Scope

- `crates/fdemon-app/src/handler/mod.rs`: Fix doc comment (~line 460)
- `example/app5/server/server.py`: Remove unused `import sys` (line 11)

### Details

#### Fix 1: Stale doc comment (`handler/mod.rs:460-463`)

**Current text:**
```rust
/// Populated by the hydration step in `process.rs` from
/// `state.running_shared_source_names()` before `handle_action` is called.
/// Sources in this list are skipped by `spawn_pre_app_sources` so a shared
/// source is never spawned twice.
```

**Problem:** There is no hydration step for this field. It is populated at construction time inside the TEA handler, not in `process.rs`. The field is set directly via `state.running_shared_source_names()` at two construction sites:
- `handler/update.rs:928`
- `handler/new_session/launch_context.rs:513`

**Replace with:**
```rust
/// Snapshot of shared custom source names already running at the time
/// this action was constructed, taken from `state.running_shared_source_names()`.
/// Sources in this list are skipped by `spawn_pre_app_sources` so a shared
/// source is never spawned twice.
```

#### Fix 2: Remove unused `import sys` (`server.py:11`)

**Current:**
```python
import json
import sys
import time
```

**Replace with:**
```python
import json
import time
```

`sys` is imported but never referenced anywhere in the 79-line file.

### Acceptance Criteria

1. `SpawnPreAppSources.running_shared_names` doc comment accurately describes the data flow (no reference to `process.rs` or hydration).
2. `import sys` is removed from `server.py`.
3. All existing tests pass. No compilation errors.

### Testing

No new tests needed. Run the standard quality gate:

```bash
cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings
```

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mod.rs` | Replaced stale doc comment on `running_shared_names` field — removed false reference to "hydration step in `process.rs`"; now accurately describes construction-time snapshot |
| `example/app5/server/server.py` | Removed unused `import sys` |

### Notable Decisions/Tradeoffs

1. **Minimal scope**: Only the two lines identified in the task were changed. No other edits were made to surrounding code or comments.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (all existing tests)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **None**: Both changes are purely cosmetic (doc comment + unused import removal) with no behavioural impact.
