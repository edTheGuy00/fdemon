# Task 02 — Eliminate `bare_flutter_run` panic; convert `find_auto_launch_target` to `Option` return

**Plan:** [../BUG.md](../BUG.md) · **Index:** [../TASKS.md](../TASKS.md)
**Agent:** implementor
**Depends on:** —
**Wave:** 2 (parallel with Task 04, sequenced after Task 01 due to shared write of `src/headless/runner.rs`)

## Goal

Resolve review finding **C3** (`find_auto_launch_target` is `pub` but reaches an undocumented `expect()` panic via `bare_flutter_run`; the panic message references a stale line number).

Per the locked-in decision (BUG.md §Decisions §4 — "option (a)"), refactor `bare_flutter_run` and `find_auto_launch_target` to return `Option<AutoLaunchSuccess>`. The `expect()` is removed entirely. Both call sites (`spawn_auto_launch` and `headless_auto_start`) handle the `None` branch with a sensible early-return.

## Files Modified (Write)

| File | Change |
|------|--------|
| `crates/fdemon-app/src/spawn.rs` | (1) Change `bare_flutter_run` signature: `fn bare_flutter_run(devices: &[Device]) -> Option<AutoLaunchSuccess>`. Replace `.expect("...line 137")` with `.first()?.clone()` so the function returns `None` when devices is empty. (2) Change `find_auto_launch_target` signature: `pub fn find_auto_launch_target(...) -> Option<AutoLaunchSuccess>`. The Tier 4 (bare flutter run) tail call propagates `None`. (3) Update doc comment to remove any panic precondition language and instead describe the `Option` semantics: "Returns `None` only if `devices` is empty AND no Tier 1/Tier 2/Tier 3 result is available — in practice, callers should pre-filter empty device lists." (4) Update `spawn_auto_launch`'s call site to handle `None`: log + emit error message + early-return. The existing `is_empty()` guard at the top of `spawn_auto_launch` already prevents the `None` branch in normal operation; the `match`/`if let` on `Option` is a defense-in-depth structural change. (5) Update unit tests `cache_allowed_false_skips_tier2_falls_to_tier3` and `cache_allowed_false_still_honors_tier1` (and any other affected tests) to unwrap `Option` results. (6) Add a new unit test `find_auto_launch_target_returns_none_on_empty_devices` exercising the `None` branch directly. |
| `src/headless/runner.rs` | Update the `find_auto_launch_target` call site (currently lines ~303-306): replace `let AutoLaunchSuccess { device, config } = find_auto_launch_target(&configs, &result.devices, &project_path, false);` with an `if let Some(...) = ... else { ... }` pattern. The `else` branch logs `tracing::error!("Auto-launch resolution returned no target")` and emits `HeadlessEvent::error(...)` then early-returns. The pre-existing `is_empty()` guard (lines 297-301) still runs first, so this `None` branch is defensive. |

## Files Read (dependency)

— (no upstream task dependency in code; sequential-after Task 01 only because both write `src/headless/runner.rs`)

## Implementation Notes

### `bare_flutter_run` after change

```rust
fn bare_flutter_run(devices: &[Device]) -> Option<AutoLaunchSuccess> {
    Some(AutoLaunchSuccess {
        device: devices.first()?.clone(),
        config: None,
    })
}
```

### `find_auto_launch_target` after change

```rust
/// Find the best device/config combination for auto-launch.
///
/// Priority order:
/// 1. `launch.toml` config with `auto_start = true` — always wins over cached selection
/// 2. `settings.local.toml` cached `last_device` / `last_config` — gated by `cache_allowed`
/// 3. First launch config + first device (fallback when cache is stale, missing, or disabled)
/// 4. Bare flutter run with first device (no configs at all)
///
/// When `cache_allowed = false`, Tier 2 is skipped entirely.
///
/// Returns `None` only when no tier produces a result — in practice, this
/// happens only when `devices` is empty. Callers should typically pre-filter
/// empty device lists before invoking this function.
pub fn find_auto_launch_target(
    configs: &LoadedConfigs,
    devices: &[Device],
    project_path: &Path,
    cache_allowed: bool,
) -> Option<AutoLaunchSuccess> {
    if let Some(result) = try_auto_start_config(configs, devices) {
        return Some(result);
    }
    if cache_allowed {
        if let Some(result) = try_cached_selection(configs, devices, project_path) {
            return Some(result);
        }
    }
    if let Some(result) = try_first_config(configs, devices) {
        return Some(result);
    }
    bare_flutter_run(devices)
}
```

### `spawn_auto_launch` call-site update

The function currently has:

```rust
let target = find_auto_launch_target(&configs, &devices, project_path, cache_allowed);
// ... uses target.device and target.config ...
```

After change:

```rust
let Some(target) = find_auto_launch_target(&configs, &devices, project_path, cache_allowed) else {
    tracing::error!("Auto-launch resolution returned no target (devices may have been emptied between check and call)");
    let _ = msg_tx.send(Message::AutoLaunchResult(Err(
        "Auto-launch resolution failed".to_string(),
    ))).await;
    return;
};
```

The exact error-channel mechanism depends on `spawn_auto_launch`'s existing pattern — match the existing `Err` propagation style (likely `Message::AutoLaunchResult(Err(_))` or similar; verify by reading the existing code).

### `headless_auto_start` call-site update

Currently (line 305-306):

```rust
let AutoLaunchSuccess { device, config } =
    find_auto_launch_target(&configs, &result.devices, &project_path, false);
```

After change:

```rust
let Some(AutoLaunchSuccess { device, config }) =
    find_auto_launch_target(&configs, &result.devices, &project_path, false)
else {
    tracing::error!("Auto-launch resolution returned no target");
    HeadlessEvent::error("Auto-launch resolution returned no target".to_string(), true).emit();
    return;
};
```

### Tests

- All existing tests in `crates/fdemon-app/src/spawn.rs::tests` that destructure `AutoLaunchSuccess` from `find_auto_launch_target` need an `.expect("...")` or `.unwrap()` to pull the value out. Use `.expect("test setup guarantees a result")` with the test scenario description.
- Add new test `find_auto_launch_target_returns_none_on_empty_devices`:
  ```rust
  #[test]
  fn find_auto_launch_target_returns_none_on_empty_devices() {
      let temp = tempfile::tempdir().unwrap();
      let configs = LoadedConfigs::default();
      let devices: Vec<Device> = vec![];
      let result = find_auto_launch_target(&configs, &devices, temp.path(), true);
      assert!(result.is_none());
  }
  ```
- Add test for headless `None` branch IF the existing test harness allows mocking empty devices through `headless_auto_start`. If not, document that the `is_empty` guard at line 297 makes this branch unreachable in practice.

## Verification

- `cargo check --workspace --all-targets`
- `cargo test -p fdemon-app spawn::tests`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- Manual smoke: run `fdemon` against `example/app2` — auto-launch behavior unchanged from parent plan (no observable regression).

## Acceptance

- [x] `bare_flutter_run` returns `Option<AutoLaunchSuccess>`; no `.expect()` or `.unwrap()` in its body.
- [x] `find_auto_launch_target` returns `Option<AutoLaunchSuccess>`; doc comment describes the `None` semantics; no `# Panics` section needed.
- [x] `spawn_auto_launch` handles the `None` branch with a sensible early-return + error log.
- [x] `headless_auto_start` handles the `None` branch with `tracing::error!` + `HeadlessEvent::error` + early-return.
- [x] No `expect("...line 137")`-style panic remains anywhere in `spawn.rs`.
- [x] All existing `spawn::tests` updated for `Option` return; new test `find_auto_launch_target_returns_none_on_empty_devices` passes.
- [x] `cargo clippy --workspace -- -D warnings` clean.

---

## Completion Summary

**Status:** Done
**Branch:** plan/cache-auto-launch-gate

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/spawn.rs` | Changed `bare_flutter_run` to return `Option<AutoLaunchSuccess>` using `?` instead of `expect`. Changed `find_auto_launch_target` to return `Option<AutoLaunchSuccess>` and updated doc comment. Updated `spawn_auto_launch` call site with `let Some(...) else { ... }` guard. Updated 8 existing test call sites with `.expect("...")`. Added new test `find_auto_launch_target_returns_none_on_empty_devices`. |
| `src/headless/runner.rs` | Updated `headless_auto_start` call site from bare destructure to `let Some(...) = ... else { tracing::error!(...); HeadlessEvent::error(...).emit(); return; }`. Updated 3 headless test call sites with `.expect("...")`. |

### Notable Decisions/Tradeoffs

1. **`.expect()` in tests**: Used descriptive `.expect("test setup guarantees ...")` messages rather than bare `.unwrap()` per CODE_STANDARDS guidance on error context.
2. **`spawn_auto_launch` guard placement**: The `None` branch in `spawn_auto_launch` is defense-in-depth — the `is_empty()` guard at line 185 already prevents this in normal operation. The `let Some(...) else` pattern makes the structural guarantee explicit at the type level.

### Testing Performed

- `cargo check --workspace --all-targets` — Passed
- `cargo test -p fdemon-app spawn::tests` — Passed (10 tests)
- `cargo test --workspace` — Passed (all test suites, 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed (clean)

### Risks/Limitations

1. **Headless `None` branch unreachable in practice**: The `is_empty()` guard at line 296 of `headless_auto_start` runs before `find_auto_launch_target` is called, so the new `else` branch is purely defensive. No test exercises it directly because the test harness cannot inject empty devices after the guard; this is documented in the task spec.
