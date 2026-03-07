## Task: Defer on_resume() until resume RPC succeeds

**Objective**: Move the `on_resume()` call (which clears variable/frame stores) to after the `backend.resume()` RPC succeeds, preventing stale reference errors when resume fails.

**Depends on**: 02-split-adapter-mod

**Severity**: Minor

### Scope

- `crates/fdemon-dap/src/adapter/handlers.rs` (post-split; currently `adapter/mod.rs:1888-1897`)

### Details

**Current (eager clear before RPC):**
```rust
self.on_resume();  // clears var_store + frame_store

match self.backend.resume(&isolate_id, None).await {
    Ok(()) => DapResponse::success(request, Some(body)),
    Err(e) => DapResponse::error(request, format!("Continue failed: {e}")),
}
```

If resume fails, stores are already cleared — subsequent `variables` or `stackTrace` requests return "reference not found" even though the isolate is still paused.

**Fixed (clear only on success):**
```rust
match self.backend.resume(&isolate_id, None).await {
    Ok(()) => {
        self.on_resume();  // clear stores only after confirmed resume
        let body = serde_json::json!({ "allThreadsContinued": true });
        DapResponse::success(request, Some(body))
    }
    Err(e) => DapResponse::error(request, format!("Continue failed: {e}")),
}
```

Apply the same pattern to `handle_next`, `handle_step_in`, `handle_step_out`, and the internal `step()` method — anywhere `on_resume()` is called before a backend RPC.

### Acceptance Criteria

1. `on_resume()` only called after successful `backend.resume()` / `backend.step()`
2. If resume/step fails, variable and frame stores remain intact
3. Existing tests pass
4. `cargo test -p fdemon-dap` — Pass

### Notes

- Check the `step()` internal method — it likely has the same eager-clear pattern
- This is a minor correctness issue; resume failures are rare in practice
