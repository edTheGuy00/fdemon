## Task: Add Global Time Budgets for toString/Getter Evaluation

**Objective**: Prevent the IDE variables panel from hanging by adding a total time budget to the sequential toString enrichment loop and getter evaluation loop, so the entire variables response completes within a bounded time regardless of how many candidates exist.

**Depends on**: 05-variables-correctness (shared file: variables.rs)

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/variables.rs`: Add time budgets to two loops

**Files Read (Dependencies):**
- `crates/fdemon-dap/src/adapter/types.rs`: Existing timeout constants

### Details

#### Problem

`enrich_with_to_string` (line 537) and the getter evaluation loop (line 1442) both iterate sequentially with 1-second per-call timeouts. Worst case: 20 PlainInstance vars = 20s, 50 getters = 50s. The IDE panel appears frozen.

#### Fix 1: toString enrichment budget (enrich_with_to_string)

Add a constant and wrap the loop with a deadline:

```rust
/// Maximum total time for all toString() calls in a single variables response.
const TO_STRING_TOTAL_BUDGET: Duration = Duration::from_secs(3);
```

```rust
async fn enrich_with_to_string(&self, variables: &mut [DapVariable], candidates: Vec<ToStringCandidate>) {
    let deadline = tokio::time::Instant::now() + TO_STRING_TOTAL_BUDGET;

    for candidate in candidates {
        // Check budget before each call
        if tokio::time::Instant::now() >= deadline {
            tracing::debug!(
                "toString enrichment budget exhausted ({:?}), skipping remaining {} candidates",
                TO_STRING_TOTAL_BUDGET,
                // remaining count
            );
            break;
        }

        let result = tokio::time::timeout(
            TO_STRING_EVAL_TIMEOUT,  // existing 1s per-call timeout
            self.backend.evaluate(...)
        ).await;
        // ... existing per-call handling
    }
}
```

#### Fix 2: Getter evaluation budget (expand_object getter loop)

Add a constant:

```rust
/// Maximum total time for all getter evaluations on a single object.
const GETTER_EVAL_TOTAL_BUDGET: Duration = Duration::from_secs(5);
```

Apply the same deadline pattern:

```rust
let getter_deadline = tokio::time::Instant::now() + GETTER_EVAL_TOTAL_BUDGET;

for getter_name in &getter_names {
    if tokio::time::Instant::now() >= getter_deadline {
        tracing::debug!(
            "Getter evaluation budget exhausted ({:?}), showing remaining as lazy",
            GETTER_EVAL_TOTAL_BUDGET,
        );
        // Add remaining getters as lazy (unexpanded) items
        for remaining in &getter_names[current_index..] {
            result.push(make_lazy_getter_variable(remaining, ...));
        }
        break;
    }

    // Existing per-getter evaluation with GETTER_EVAL_TIMEOUT...
}
```

When the budget is exhausted, remaining getters should be added as lazy getter items (using `VariableRef::GetterEval`) rather than silently dropped. This way the user can still manually expand individual getters.

### Acceptance Criteria

1. `enrich_with_to_string` completes within `TO_STRING_TOTAL_BUDGET` (3s) regardless of candidate count
2. Getter evaluation completes within `GETTER_EVAL_TOTAL_BUDGET` (5s) regardless of getter count
3. Budget-exceeded getters appear as lazy items (expandable on demand), not dropped
4. Per-call timeouts (`TO_STRING_EVAL_TIMEOUT`, `GETTER_EVAL_TIMEOUT`) still apply within the budget
5. Debug log emitted when budget is exhausted
6. Existing tests pass: `cargo test -p fdemon-dap`
7. `cargo clippy -p fdemon-dap` clean

### Testing

```rust
#[tokio::test]
async fn test_tostring_enrichment_respects_total_budget() {
    // Mock backend.evaluate that sleeps 2s per call
    // Provide 10 candidates (would take 20s without budget)
    // Assert completes in ~3s
    // Assert some candidates enriched, remaining skipped
}

#[tokio::test]
async fn test_getter_evaluation_respects_total_budget() {
    // Mock backend.evaluate that sleeps 2s per call
    // Object has 20 getters
    // Assert completes in ~5s
    // Assert remaining getters added as lazy items
}
```

### Notes

- The 3s/5s budget values are chosen to feel responsive in an IDE. They can be adjusted based on user feedback.
- Sequential evaluation is intentionally preserved (not switched to concurrent) to avoid overwhelming slow devices. The budget just caps the total time.
- Consider adding the budget constants to `types.rs` alongside the existing timeout constants for consistency.
