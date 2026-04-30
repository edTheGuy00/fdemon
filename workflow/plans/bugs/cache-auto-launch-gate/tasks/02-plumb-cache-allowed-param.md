# Task 02 — Plumb `cache_allowed: bool` through the auto-launch pipeline

**Plan:** [../BUG.md](../BUG.md) · **Index:** [../TASKS.md](../TASKS.md)
**Agent:** implementor
**Depends on:** —
**Wave:** 1 (parallel with Task 01)

## Goal

Thread a `cache_allowed: bool` parameter through `Message::StartAutoLaunch` → `UpdateAction::DiscoverDevicesAndAutoLaunch` → `spawn::spawn_auto_launch` → `find_auto_launch_target`. When `cache_allowed = false`, `find_auto_launch_target` skips Tier 2 (cached `last_device`) entirely and falls through to Tier 3 / Tier 4. **Hard-code `cache_allowed: false` at all current construction sites** — Wave 2 (Tasks 03 + 04) will replace those with real values from `settings.behavior.auto_launch`.

> **Behavioral effect after this task lands alone:** auto-launch via cache stops working everywhere. This is intentional and short-lived — Tasks 03 + 04 land in the same release and re-enable it under the new flag. Treat Task 02 + 03 + 04 as a single conceptual change split for parallelism.

## Files Modified (Write)

| File | Change |
|------|--------|
| `crates/fdemon-app/src/message.rs` | Add `cache_allowed: bool` field to `Message::StartAutoLaunch` |
| `crates/fdemon-app/src/handler/mod.rs` | Add `cache_allowed: bool` field to `UpdateAction::DiscoverDevicesAndAutoLaunch` |
| `crates/fdemon-app/src/handler/update.rs` | Match arm for `Message::StartAutoLaunch` propagates `cache_allowed` into the `UpdateAction` |
| `crates/fdemon-app/src/handler/tests.rs` | Update all `Message::StartAutoLaunch { configs }` constructions to `Message::StartAutoLaunch { configs, cache_allowed: true }` (preserve existing assertions — these tests pre-date the gate) |
| `crates/fdemon-app/src/actions/mod.rs` | Match arm for `UpdateAction::DiscoverDevicesAndAutoLaunch` passes `cache_allowed` into `spawn::spawn_auto_launch` |
| `crates/fdemon-app/src/spawn.rs` | `spawn_auto_launch` accepts `cache_allowed: bool`; passes it to `find_auto_launch_target`. `find_auto_launch_target` accepts the param and skips `try_cached_selection` when `false`. Add unit tests for both `cache_allowed = true` and `cache_allowed = false`. |
| `crates/fdemon-tui/src/runner.rs` | At `dispatch_startup_action`'s `AutoStart` arm, construct `Message::StartAutoLaunch { configs, cache_allowed: false }` (placeholder — Task 03 replaces with `engine.settings.behavior.auto_launch`) |

## Files Read (dependency)

— (no upstream tasks; this task does not read Task 01's field)

## Implementation Notes

### `find_auto_launch_target` signature change

Current (`spawn.rs:221-243`):
```rust
pub fn find_auto_launch_target(
    configs: &LoadedConfigs,
    devices: &[Device],
    project_path: &Path,
) -> AutoLaunchSuccess {
    if let Some(result) = try_auto_start_config(configs, devices) { return result; }
    if let Some(result) = try_cached_selection(configs, devices, project_path) { return result; }
    if let Some(result) = try_first_config(configs, devices) { return result; }
    bare_flutter_run(devices)
}
```

After:
```rust
pub fn find_auto_launch_target(
    configs: &LoadedConfigs,
    devices: &[Device],
    project_path: &Path,
    cache_allowed: bool,
) -> AutoLaunchSuccess {
    // Tier 1: launch.toml auto_start config — always wins
    if let Some(result) = try_auto_start_config(configs, devices) { return result; }

    // Tier 2: cached selection — gated by caller's cache_allowed flag
    if cache_allowed {
        if let Some(result) = try_cached_selection(configs, devices, project_path) {
            return result;
        }
    }

    // Tier 3: first launch config + first device
    if let Some(result) = try_first_config(configs, devices) { return result; }

    // Tier 4: bare flutter run
    bare_flutter_run(devices)
}
```

### `spawn_auto_launch` signature change

Add `cache_allowed: bool` as the last parameter; pass through to `find_auto_launch_target` at line 203.

### Test updates in `spawn.rs`

The existing tests `test_auto_start_config_beats_cached_selection`, `test_no_auto_start_uses_cached_selection`, etc. all currently call `find_auto_launch_target(&configs, &devices, project_path)`. Update to pass `cache_allowed: true` to preserve the assertions, then add a parallel set of tests with `cache_allowed: false` showing Tier 2 is skipped:

```rust
#[test]
fn cache_allowed_false_skips_tier2_falls_to_tier3() {
    // Setup: no auto_start config, valid cached last_device, multiple devices
    // Expect: returns first device + first config (Tier 3) — cache ignored
}

#[test]
fn cache_allowed_false_still_honors_tier1() {
    // Setup: launch.toml has auto_start = true, also a valid cache
    // Expect: Tier 1 fires regardless of cache_allowed
}
```

### Test updates in `handler/tests.rs`

All test constructors of `Message::StartAutoLaunch { configs }` need the new field. Pass `cache_allowed: true` so existing test semantics (which assert auto-launch fires from cache) keep working.

## Verification

- `cargo check --workspace`
- `cargo test -p fdemon-app spawn::tests`
- `cargo test -p fdemon-app handler::tests`
- `cargo clippy --workspace -- -D warnings`

## Acceptance

- [x] `Message::StartAutoLaunch` has `cache_allowed: bool` field.
- [x] `UpdateAction::DiscoverDevicesAndAutoLaunch` has `cache_allowed: bool` field.
- [x] `spawn_auto_launch` and `find_auto_launch_target` both accept `cache_allowed: bool`.
- [x] `find_auto_launch_target` skips `try_cached_selection` when `cache_allowed == false`.
- [x] All existing tests updated to pass `cache_allowed: true` and still pass.
- [x] New tests cover `cache_allowed: false` (skips Tier 2) and Tier 1's invariance.
- [x] `crates/fdemon-tui/src/runner.rs:181` (`dispatch_startup_action`) constructs `StartAutoLaunch` with `cache_allowed: false` — placeholder for Task 03.

---

## Completion Summary

**Status:** Done
**Branch:** plan/cache-auto-launch-gate

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/message.rs` | Added `cache_allowed: bool` field to `Message::StartAutoLaunch` with doc comment |
| `crates/fdemon-app/src/handler/mod.rs` | Added `cache_allowed: bool` field to `UpdateAction::DiscoverDevicesAndAutoLaunch` with doc comment |
| `crates/fdemon-app/src/handler/update.rs` | Match arm for `Message::StartAutoLaunch` destructures and propagates `cache_allowed` into `UpdateAction` |
| `crates/fdemon-app/src/handler/tests.rs` | Updated all 3 `Message::StartAutoLaunch { configs }` constructions to include `cache_allowed: true` |
| `crates/fdemon-app/src/actions/mod.rs` | Match arm for `UpdateAction::DiscoverDevicesAndAutoLaunch` passes `cache_allowed` to `spawn_auto_launch` |
| `crates/fdemon-app/src/spawn.rs` | `spawn_auto_launch` and `find_auto_launch_target` accept `cache_allowed: bool`; Tier 2 gated on flag; existing tests updated; 2 new tests added |
| `crates/fdemon-tui/src/runner.rs` | `dispatch_startup_action` constructs `StartAutoLaunch` with `cache_allowed: false` (placeholder for Task 03) |

### Notable Decisions/Tradeoffs

1. **Hard-coded `cache_allowed: false` at construction sites**: Per task spec, all current construction sites use `false`. This intentionally disables Tier 2 (cached selection) as a temporary state until Tasks 03 + 04 land and read the real setting.
2. **Existing tests use `cache_allowed: true`**: This preserves pre-existing test semantics (those tests exercise cache-based behavior that remains valid with `true`).
3. **No behavioral change for Tier 1 (auto_start config)**: The `cache_allowed` flag has no effect when a `launch.toml` config with `auto_start = true` is found — Tier 1 always wins.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-app spawn::tests` - Passed (9 tests, including 2 new)
- `cargo test -p fdemon-app handler::tests` - Passed (317 tests)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (no warnings)
- `cargo test --workspace` - Passed (all crates, zero failures)

### Risks/Limitations

1. **Temporary breakage of cache-based auto-launch**: As documented in the task, `cache_allowed: false` at all construction sites means Tier 2 is currently always skipped. This is intentional and short-lived — Tasks 03 + 04 replace the hard-coded value with `settings.behavior.auto_launch`.
