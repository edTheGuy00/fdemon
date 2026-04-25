# Task 06 — Refactor `try_cached_selection` to log on real validation failure

**Agent:** implementor
**Plan:** [../TASKS.md](../TASKS.md) (Option α)
**Parent PR:** #35 (Copilot review comment #2; same observation as Task 01's own validator note)

## Problem (one-liner)

In `crates/fdemon-app/src/spawn.rs::try_cached_selection`, the `tracing::warn!` branch (around lines 286-291) is **unreachable dead code**: `validate_last_selection` only returns `Some(validated)` when `validated.device_idx.is_some()`, and that index comes from `devices.iter().position(...)`, so `devices.get(i)` cannot be `None`. Worse, the **real** failure case — `validate_last_selection` returning `None` because the cached device has been disconnected — currently logs nothing at all. After Task 05 lands and Tier 2 becomes reachable, this missing warning becomes user-visible (it's the only signal a user gets that their remembered iPhone is no longer connected and fdemon is silently picking another device).

## Desired behavior

Move the `tracing::warn!` from the unreachable inner branch to the real failure point: when `validate_last_selection` returns `None`. Simplify `try_cached_selection` so the dead `else` branch goes away.

## Acceptance criteria

1. `try_cached_selection` returns `None` cleanly when `validate_last_selection` returns `None`, and emits exactly one `tracing::warn!` describing that the saved selection in `settings.local.toml` is no longer valid (device disconnected or referenced config removed).
2. `try_cached_selection` returns `Some(AutoLaunchSuccess)` when validation succeeds, with no spurious warnings.
3. The dead `else` branch (`devices.get(i)` returning `None` and the warning that referenced `validated.device_idx.unwrap_or(0)`) is gone.
4. `find_auto_launch_target`'s priority cascade is unchanged: Priority 2 returning `None` still falls through to Priority 3.
5. Existing tests T1–T4 in `spawn.rs` still pass without modification.
6. New unit test **T5** — valid cache file points to a device that's not in the discovered list (e.g. `last_device = "disconnected"` and `devices = [make_device("ios-1", "ios")]`) plus one launch config without `auto_start`. Direct call to `find_auto_launch_target` returns the Tier 3 result (first config + first device); separately, a direct call to `try_cached_selection` (or testing observable effects via `find_auto_launch_target`) demonstrates the function returned `None`. Asserting on the warning is optional — `tracing::warn!` is hard to capture in unit tests; a comment in the test noting "warns to log file via tracing" is sufficient.

## Files modified (write)

- `crates/fdemon-app/src/spawn.rs` — refactor `try_cached_selection`, add T5.

## Files read (context only)

- `crates/fdemon-app/src/config/settings.rs` — `validate_last_selection` signature and `ValidatedSelection` shape. Do NOT change these.
- The Copilot suggestion text:
  ```rust
  let validated = match validate_last_selection(&selection, configs, devices) {
      Some(validated) => validated,
      None => {
          tracing::warn!(
              "Saved device/config selection is no longer valid, falling back to Priority 3"
          );
          return None;
      }
  };
  let config = validated.config_idx.and_then(|i| configs.configs.get(i));
  let device = validated.device_idx.and_then(|i| devices.get(i))?;
  Some(AutoLaunchSuccess {
      device: device.clone(),
      config: config.map(|c| c.config.clone()),
  })
  ```
  Use this as a starting point. Adapt the wording of the warning to be more user-friendly (the user reads this in the fdemon log file): something like
  > "Cached selection in settings.local.toml is no longer valid (saved device disconnected or config removed); falling back to first available config + device"

## Implementation notes

- Prefer a `let-else` over the `match ... return None` form if it reads more cleanly. Both are idiomatic; match the surrounding style in `spawn.rs`.
- The `?` on `devices.get(i)?` is a defense-in-depth against future contract drift; keep it. The Copilot snippet's `let device = validated.device_idx.and_then(|i| devices.get(i))?;` is fine.
- Do NOT alter the public visibility of `find_auto_launch_target` (still `pub` for testability), `try_auto_start_config`, `try_first_config`, or `bare_flutter_run`.
- Do NOT change the `AutoLaunchSuccess` shape.

## Out of scope

- Changing `validate_last_selection`'s contract (e.g. making it return a richer error type). Wave 1 coordination rule: do not modify `crates/fdemon-app/src/config/settings.rs`.
- Adding a structured "stale cache" return variant (that's option β, deferred).
- Touching `try_auto_start_config`'s "configured device not found" warning — it's reachable today and stays as-is.

## Verification

```bash
cargo test -p fdemon-app spawn::
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

Confirm clippy doesn't flag the refactored `try_cached_selection` (e.g. no `clippy::needless_match`, `clippy::redundant_else`, etc.).

---

## Completion Summary

**Status:** Done
**Branch:** fix/launch-toml-device

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/spawn.rs` | Refactored `try_cached_selection` to move `tracing::warn!` from the dead inner `else` branch to the real failure point (`validate_last_selection` returning `None`). Used `let-else` for idiomatic early return. Kept `?` on `devices.get(i)` for defense-in-depth. Added T5 test. |

### Notable Decisions/Tradeoffs

1. **`let-else` over `match`**: The `let Some(validated) = ... else { ... return None; }` form matches the surrounding style in `spawn.rs` (which uses `let ... = ... ?` patterns) and is more concise than the explicit `match` form suggested in the Copilot snippet. Both are valid; `let-else` reads more cleanly here.
2. **`?` on `devices.get(i)`**: Kept as defense-in-depth against future contract drift, per task spec. The current `validate_last_selection` contract guarantees `device_idx.is_some()` implies a valid index, but the `?` prevents a panic if that contract ever changes.
3. **Warning message wording**: Used "Cached selection in settings.local.toml is no longer valid (saved device disconnected or config removed); falling back to first available config + device" — user-facing description that appears in the fdemon log file.

### Testing Performed

- `cargo test -p fdemon-app spawn::` — Passed (7 tests: T1–T4 unchanged + T5 new + `test_tool_check_timeout_is_reasonable`)
- `cargo fmt --all` — Passed (no changes needed)
- `cargo clippy --workspace -- -D warnings` — Passed (no warnings)
- `cargo check --workspace` — Passed

### Risks/Limitations

1. **Warning not captured in tests**: `tracing::warn!` output is not captured in unit tests; T5 comments that the warning is emitted to the log file. This is acceptable per task spec ("asserting on the warning is optional").
2. **No changes to `validate_last_selection`**: The contract of `validate_last_selection` in `settings.rs` was not modified, per the Wave 1 coordination rule.
