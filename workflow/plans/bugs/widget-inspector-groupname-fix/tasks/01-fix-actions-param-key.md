## Task: Fix parameter key in `try_fetch_widget_tree` (actions.rs)

**Objective**: Fix the `getRootWidgetTree` call in `try_fetch_widget_tree` to use `groupName` instead of `objectGroup`, and set `withPreviews` to `"true"` to match browser DevTools behavior.

**Depends on**: None

**Estimated Time**: 15 minutes

### Scope

- `crates/fdemon-app/src/actions.rs`: Fix `try_fetch_widget_tree` function (line ~1219)

### Details

The `try_fetch_widget_tree` function (line 1210) builds params for two APIs:

1. **`getRootWidgetTree` (newer, Flutter 3.22+)** — currently uses wrong key
2. **`getRootWidgetSummaryTree` (older, fallback)** — correctly uses `objectGroup`

**Change 1 — Parameter key (line 1219):**

```rust
// Before:
newer_args.insert("objectGroup".to_string(), object_group.to_string());

// After:
newer_args.insert("groupName".to_string(), object_group.to_string());
```

**Change 2 — `withPreviews` value (line 1221):**

```rust
// Before:
newer_args.insert("withPreviews".to_string(), "false".to_string());

// After:
newer_args.insert("withPreviews".to_string(), "true".to_string());
```

**Do NOT change** the `older_args` block (line 1251) — `getRootWidgetSummaryTree` correctly uses `objectGroup` because that API's registration helper (`_registerObjectGroupServiceExtension`) extracts it with that exact key.

### Acceptance Criteria

1. `getRootWidgetTree` params contain `groupName` (not `objectGroup`)
2. `getRootWidgetTree` params contain `withPreviews: "true"`
3. `getRootWidgetSummaryTree` fallback still uses `objectGroup` (unchanged)
4. `cargo check -p fdemon-app` compiles
5. `cargo test -p fdemon-app` passes
6. `cargo clippy -p fdemon-app` no warnings

### Testing

No new tests needed — the change is a string literal fix. Existing tests cover the fallback logic and parsing. Manual verification against a live Flutter app is the primary validation.

### Notes

- This is the **primary fix** that resolves the user-visible crash
- The `withPreviews: "true"` change matches what browser DevTools sends and returns richer data (widget previews)
- The `subtreeDepth` parameter (line 1223) is unaffected — it's only added when `tree_max_depth > 0`

---

## Completion Summary

**Status:** Not started
