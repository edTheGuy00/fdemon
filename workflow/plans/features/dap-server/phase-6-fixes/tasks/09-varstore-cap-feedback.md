## Task: Variable Store Cap IDE Feedback

**Objective**: Emit a DAP `output` event to the IDE debug console when the variable store reaches its capacity limit, so the user understands why some variables appear non-expandable.

**Depends on**: 04-source-ref-reverse-index, 06-events-error-handling (shared files: stack.rs, events.rs)

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/stack.rs`: Add cap-reached flag to `VariableStore`
- `crates/fdemon-dap/src/adapter/variables.rs`: Emit output event when cap reached (alternative: events.rs)

**Files Read (Dependencies):**
- `crates/fdemon-dap/src/adapter/events.rs`: `send_event` pattern

### Details

#### Current State (stack.rs:266–272)

```rust
pub fn allocate(&mut self, target: VariableRef) -> i64 {
    if self.references.len() >= MAX_VARIABLE_REFS {
        tracing::warn!(
            "VariableStore full ({} entries) — returning 0 (non-expandable)",
            MAX_VARIABLE_REFS,
        );
        return 0;
    }
    // ...
}
```

Returns `0` (non-expandable) with a `tracing::warn` that's invisible to the IDE user. The user sees variables that should be expandable rendered as leaf nodes with no explanation.

#### The Fix

**Step 1:** Add a `cap_warning_emitted` flag to `VariableStore`:

```rust
pub struct VariableStore {
    references: HashMap<i64, VariableRef>,
    next_id: i64,
    cap_warning_emitted: bool,  // NEW
}
```

Reset it in `VariableStore::reset()`.

**Step 2:** When `allocate` hits the cap and `cap_warning_emitted` is false, set the flag and return a signal that the caller should emit a warning. Since `VariableStore` doesn't have access to the event sender, use a return value:

```rust
pub enum AllocResult {
    Ok(i64),
    CapReached(i64),  // returns 0 + signals first-time cap hit
}

pub fn allocate(&mut self, target: VariableRef) -> AllocResult {
    if self.references.len() >= MAX_VARIABLE_REFS {
        tracing::warn!("VariableStore full ...");
        if !self.cap_warning_emitted {
            self.cap_warning_emitted = true;
            return AllocResult::CapReached(0);
        }
        return AllocResult::Ok(0);  // subsequent hits — silent
    }
    // normal allocation...
    AllocResult::Ok(id)
}
```

**Alternative (simpler):** Just add a `pub fn is_cap_reached(&self) -> bool` method and have the caller check after `allocate` returns `0`.

**Step 3:** In `variables.rs`, when `allocate` signals cap-reached, emit a DAP output event:

```rust
if self.var_store.is_cap_reached_first_time() {
    self.send_event("output", Some(serde_json::json!({
        "category": "console",
        "output": "Warning: Variable store capacity limit reached (10,000 entries). Some variables may appear non-expandable. This typically happens with very deep object hierarchies.\n"
    }))).await;
}
```

The `\n` at the end ensures proper line separation in the debug console.

### Acceptance Criteria

1. First time variable store cap is hit, a DAP `output` event is emitted with `category: "console"`
2. Subsequent cap hits in the same stop do NOT emit additional warnings (one-shot)
3. Warning is reset on `VariableStore::reset()` (so it can fire again on the next stop)
4. The warning message is clear and actionable for the user
5. Existing tests pass: `cargo test -p fdemon-dap`
6. `cargo clippy -p fdemon-dap` clean

### Testing

```rust
#[test]
fn test_variable_store_cap_signals_first_hit() {
    let mut store = VariableStore::new();
    // Fill to capacity
    for i in 0..MAX_VARIABLE_REFS {
        store.allocate(VariableRef::Object { ... });
    }
    // Next allocation should signal cap reached
    assert!(store.is_cap_reached_first_time());
    // Subsequent allocations should not re-signal
    store.allocate(VariableRef::Object { ... });
    assert!(!store.is_cap_reached_first_time());
}

#[test]
fn test_variable_store_reset_clears_cap_flag() {
    let mut store = VariableStore::new();
    // Hit cap, then reset
    // ...
    store.reset();
    // Should signal again after next cap hit
}
```

### Notes

- The simpler approach (check-after-allocate) is preferred over the enum return type to minimize changes to the `allocate()` call sites.
- The output event should be emitted only once per stop (per `VariableStore::reset()` cycle), not once per session.
- `evaluate_name_map` also has no capacity cap. Adding a cap there is out of scope for this task but is noted as future work.
