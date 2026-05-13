## Task: Cache the Fallback Isolate ID in `resolve_flutter_ui_isolate`

**Objective**: Eliminate the N+1 RPC pattern on every widget tree fetch during the Flutter app warm-up window by writing the fallback isolate id to the cache before returning.

**Depends on**: None

**Estimated Time**: 0.5-1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/vm_service/client.rs` — add cache write to the fallback branch of `resolve_flutter_ui_isolate`

**Files Read (Dependencies):**
- None

### Details

`crates/fdemon-daemon/src/vm_service/client.rs:311-317` currently returns the fallback isolate id without writing the cache. The method's own doc comment (lines 229-230) claims all paths cache. Match the doc by writing to `isolate_id_cache` before returning.

**Current code (lines 311-317):**
```rust
if let Some(first) = candidates.first() {
    warn!(
        isolate_id = %first.id,
        "VM Service: no Flutter extensions found on any isolate; \
         falling back to first non-system isolate"
    );
    Ok(first.id.clone())   // <-- missing cache write
} else {
    Err(Error::vm_service("no non-system isolates available"))
}
```

**Target code:**
```rust
if let Some(first) = candidates.first() {
    warn!(
        isolate_id = %first.id,
        "VM Service: no Flutter extensions found on any isolate; \
         falling back to first non-system isolate"
    );
    let id = first.id.clone();
    {
        let mut guard = self.isolate_id_cache.lock().await;
        *guard = Some(id.clone());
    }
    Ok(id)
} else {
    Err(Error::vm_service("no non-system isolates available"))
}
```

Mirror the lock pattern used at the success path (lines 304-306).

### Acceptance Criteria

1. Both the success branch and the fallback branch of `resolve_flutter_ui_isolate` write to `isolate_id_cache` before returning.
2. The method's existing doc comment (lines 229-230) remains accurate — no doc edits needed.
3. A new unit test exercises a multi-isolate VM with no `ext.flutter.*` extensions and asserts `cached_isolate_id()` returns `Some(_)` after one call (and that a second call does NOT trigger fresh `getVM`/`getIsolate` RPCs).
4. Existing unit tests (`test_resolve_flutter_ui_isolate_logic_no_flutter_ext_falls_back_to_first`, `test_resolve_flutter_ui_isolate_returns_cached_value_immediately`, etc.) continue to pass.

### Testing

Add a test alongside existing `resolve_flutter_ui_isolate` tests in `crates/fdemon-daemon/src/vm_service/client.rs`:

```rust
#[tokio::test]
async fn test_resolve_flutter_ui_isolate_caches_fallback_value() {
    // Build a mock that returns getVM with 1 non-system isolate (no ext.flutter.*).
    // First call → resolves via fallback path, returns isolates/1.
    // Assert cached_isolate_id() == Some("isolates/1".into()).
    // Second call (with disconnected RPC channel to prove cache hit) → returns same id.
}
```

### Notes

- The cache is shared with `main_isolate_id` ("first caller wins"). Both methods write the same field; no protocol change.
- Hot restart still invalidates via the existing `invalidate_isolate_cache()` path — no change there.
- The fallback was originally documented as "retry on eventual ext.flutter.* registration". User direction is to cache the fallback now; if a future need emerges for retry semantics, add an explicit `invalidate_isolate_cache_if_no_flutter_ext()` mechanism instead.
