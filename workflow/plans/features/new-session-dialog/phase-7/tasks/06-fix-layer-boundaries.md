# Task: Fix Layer Boundary Violations

## Summary

Move dialog state types from TUI layer to App layer to fix architectural violations. Per `docs/ARCHITECTURE.md`, the TUI layer depends on App (View depends on Model), not the reverse.

**Priority:** CRITICAL (Blocking merge)

## Files

| File | Action |
|------|--------|
| `src/app/new_session_dialog/mod.rs` | **NEW** - Module declaration |
| `src/app/new_session_dialog/state.rs` | **NEW** - State types moved from TUI |
| `src/app/new_session_dialog/types.rs` | **NEW** - Type definitions |
| `src/app/mod.rs` | Modify (add new_session_dialog module) |
| `src/app/state.rs` | Modify (update imports) |
| `src/app/message.rs` | Modify (update imports) |
| `src/app/handler/keys.rs` | Modify (update imports) |
| `src/app/handler/new_session/*.rs` | Modify (update imports) |
| `src/tui/widgets/new_session_dialog/mod.rs` | Modify (import from App) |
| `src/tui/widgets/new_session_dialog/state/*.rs` | Remove or re-export from App |

## Problem

Current violations:
- `src/app/state.rs:12` - imports `NewSessionDialogState` from TUI
- `src/app/message.rs:10` - imports `DartDefine`, `FuzzyModalType`, `TargetTab` from TUI
- `src/app/handler/keys.rs:680` - imports `TargetTab` from TUI
- `src/app/handler/new_session/*.rs` - various TUI imports

## Implementation

### Option A (Recommended): Move State Types to App Layer

#### Step 1: Create App module structure

```rust
// src/app/new_session_dialog/mod.rs
mod state;
mod types;

pub use state::*;
pub use types::*;
```

#### Step 2: Move type definitions

Move from `src/tui/widgets/new_session_dialog/state/types.rs` to `src/app/new_session_dialog/types.rs`:
- `DialogPane`
- `TargetTab`
- `LaunchContextField`
- `FuzzyModalType`
- `DartDefine`
- `LaunchParams`

#### Step 3: Move state structs

Move from `src/tui/widgets/new_session_dialog/state/` to `src/app/new_session_dialog/state.rs`:
- `NewSessionDialogState`
- `TargetSelectorState`
- `LaunchContextState`
- `FuzzyModalState`
- `DartDefinesModalState`

#### Step 4: Update TUI widget imports

```rust
// src/tui/widgets/new_session_dialog/mod.rs
use crate::app::new_session_dialog::{
    NewSessionDialogState, DialogPane, TargetTab, LaunchContextField,
    // ... other types
};
```

#### Step 5: Update all App layer imports

Update imports in:
- `src/app/state.rs`
- `src/app/message.rs`
- `src/app/handler/keys.rs`
- `src/app/handler/new_session/*.rs`

To use:
```rust
use crate::app::new_session_dialog::{...};
```

### Option B: Move Types to Core Layer

Alternative approach if types are needed by other layers.

1. Create `src/core/new_session_dialog.rs`
2. Move domain types there
3. Both App and TUI import from Core

## Acceptance Criteria

1. No `use crate::tui::` imports in `src/app/` files
2. TUI layer imports state types from App (correct direction)
3. `cargo check` passes
4. All tests compile and pass

## Verification

```bash
# Check for TUI imports in App layer
grep -r "use crate::tui" src/app/

# Should return empty or only commented-out lines
```

## Testing

```bash
cargo fmt && cargo check && cargo test --lib && cargo clippy -- -D warnings
```

## Notes

- This is a large refactoring task - consider doing it in sub-steps
- Keep `pub use` re-exports in old locations temporarily for backwards compatibility if needed
- Update module documentation to reflect new locations
