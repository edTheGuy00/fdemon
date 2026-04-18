# Task 02 — Surface "configured device not found" warning to the user

**Agent:** implementor
**Worktree:** isolated (no write-file overlap with sibling tasks)
**Depends on:** none
**Plan:** [../BUG.md](../BUG.md)

## Problem

When `find_auto_launch_target` (`crates/fdemon-app/src/spawn.rs:215-275`) cannot find a device that matches the configured `device` field (e.g. typo in a device id, or — pre-Task-01 — the macOS specifier mismatch), it logs a warning via `tracing::warn!` and silently falls back to `devices.first()`:

```rust
let found = devices::find_device(devices, &sourced.config.device);
if found.is_none() {
    tracing::warn!(
        "Configured device '{}' not found, falling back to first available device",
        sourced.config.device
    );
}
found.or_else(|| devices.first())
```

Because `tracing` writes to a file in the system temp dir (per `docs/DEVELOPMENT.md` "Logging" section, stdout is owned by the TUI), the user never sees this in normal operation. The session silently runs on the wrong device.

The user has confirmed they want to **keep the silent `devices.first()` fallback semantically** (for resilience), but that fallback must be **visible** in the in-app log buffer so the user notices their `launch.toml` was effectively overridden.

## Goal

When `find_auto_launch_target` falls back from a non-`"auto"` configured device to `devices.first()` because the matcher missed, push a synthetic `WARN`-level entry into the in-app log buffer (the same surface that user-facing log lines render on) so the user sees it next to their normal output. Continue to log via `tracing` as well.

## Constraint — Public Interface Stability

**Do not change the signature of `find_auto_launch_target` or the shape of `AutoLaunchSuccess`.** Task 03 calls this function from `src/headless/runner.rs` in parallel; a signature change would create a merge conflict. Detect the miss at the call site inside `spawn_auto_launch` (where the engine / log handle is already in scope) by re-running `devices::find_device` once after the call, OR by passing a small `&mut Vec<String>` warnings sink into the function. Whichever you pick, keep all changes inside `crates/fdemon-app/src/spawn.rs` and (if needed for the log push itself) the immediately adjacent log-injection helper.

If a tiny new private helper is needed for "push a synthetic LogEntry into the engine's log buffer", define it inside `spawn.rs` and have it call into the existing log service. Do **not** modify `services/log.rs`'s public API.

### Required visibility change (also belongs to this task)

`find_auto_launch_target` and `AutoLaunchSuccess` are currently module-private (`fn`, `struct`). Task 03 needs to call `find_auto_launch_target` from the binary crate. To avoid two tasks writing to `spawn.rs` in parallel, **this task owns all writes to `spawn.rs`** — including the visibility change. Add `pub` to both `fn find_auto_launch_target` and `struct AutoLaunchSuccess` (and any of its fields the binary crate needs to read — at minimum `device: Device` and `config: Option<LaunchConfig>`). Confirm `spawn` is already re-exported via `pub mod spawn;` in `crates/fdemon-app/src/lib.rs:81` (it is) — no `lib.rs` edit needed.

## Implementation Notes

1. Find the call site of `find_auto_launch_target` inside `spawn_auto_launch` (around `crates/fdemon-app/src/spawn.rs:203`). The chosen `device` and the candidate `LaunchConfig` are both in scope at that point.
2. After the function returns, if `result.config` is `Some(cfg)` AND `cfg.device != "auto"` AND `cfg.device` does not match `result.device` (re-check with `devices::find_device(&[result.device.clone()], &cfg.device).is_some()` or compare ids directly), construct a `LogEntry` (use whatever `LogEntry::warn(...)` / `LogEntry::system(...)` constructor is closest to existing uses — search for current usages in `spawn.rs` and `engine.rs`).
3. Push that entry into the engine's log buffer via the existing log service (look for how `spawn_auto_launch` already emits status messages — there is likely a precedent already in this file).
4. The `tracing::warn!` inside `find_auto_launch_target` should remain (it's useful for logfile diagnostics).

## Acceptance Criteria

- [ ] When a non-`"auto"` configured `device` does not match any discovered device, a warning entry visible in the in-app log buffer informs the user that the configured device was not found and the fallback was used. The text should include the configured device specifier and the actual chosen device's display name.
- [ ] The session still spawns on `devices.first()` (silent fallback semantics preserved).
- [ ] Public signatures of `find_auto_launch_target`, `AutoLaunchSuccess`, and `spawn_auto_launch` are unchanged.
- [ ] When `cfg.device == "auto"` or when `find_device` matches successfully, no extra warning is emitted.
- [ ] When no `launch.toml` config exists at all (Priority 3 path), no extra warning is emitted (the user did not specify a device).

## Tests to Add (in `crates/fdemon-app/src/spawn.rs` test module)

Reuse the existing `tests` module structure. If asserting on log-buffer content requires a test harness that does not yet exist, prefer a unit test that asserts the conditional logic returns the expected "should-emit-warning" boolean by extracting the predicate into a small private helper (`fn should_warn_user(cfg: &LaunchConfig, chosen: &Device) -> bool`).

| Test name | Scenario |
|-----------|----------|
| `test_should_warn_user_when_configured_device_does_not_match` | `cfg.device = "iphone-foo"`, chosen device id `"some-android"` → `true` |
| `test_should_not_warn_user_when_device_is_auto` | `cfg.device = "auto"` → `false` |
| `test_should_not_warn_user_when_configured_device_matches` | `cfg.device = "android"`, chosen device with `platform = "android-arm64"` → `false` |

## Verification

```bash
cargo fmt -p fdemon-app
cargo test -p fdemon-app spawn
cargo clippy -p fdemon-app -- -D warnings
```

## Files

| File | Change |
|------|--------|
| `crates/fdemon-app/src/spawn.rs` | Add post-call detection of matcher-miss in `spawn_auto_launch`; push a `WARN` `LogEntry` into the engine's in-app log buffer; add `pub` to `find_auto_launch_target` and `AutoLaunchSuccess` (consumed by Task 03); add tests for the predicate helper |
