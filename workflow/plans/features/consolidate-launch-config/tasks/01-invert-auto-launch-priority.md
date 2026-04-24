# Task 01 — Invert auto-launch priority so `launch.toml auto_start` beats cache

**Agent:** implementor
**Plan:** [../PLAN.md](../PLAN.md) (Option B, §5)
**Parent bug:** The user's bug report summarized as *"every time we run fdemon it writes to settings.local.toml then when we update launch.toml it ignores it."*

## Problem (one-liner)

`find_auto_launch_target` in `crates/fdemon-app/src/spawn.rs:215-275` checks `settings.local.toml`'s cached `last_device` / `last_config` **before** it checks `launch.toml`'s `auto_start = true`. Consequence: editing `launch.toml` between runs has no effect until the user manually deletes `settings.local.toml`.

## Desired behavior

Invert the first two priority tiers. The chain becomes:

```
Priority 1 (NEW): Launch config with auto_start = true
  → resolve its device via find_device
  → if device is "auto" → devices.first()
  → if explicit id → find_device, with fall-through to devices.first() on miss

Priority 2 (NEW — old Priority 1): settings.local.toml last_config + last_device
  → only used when no launch config has auto_start = true
  → validate saved device still exists in discovered devices
  → if validation fails, fall through

Priority 3 (unchanged): first launch config in launch.toml + devices.first()

Priority 4 (unchanged): bare flutter run (no configs)
```

## Acceptance criteria

1. When any launch config has `auto_start = true`, that config's device resolution fires regardless of whether `settings.local.toml` contains a valid `last_device` / `last_config`.
2. When no launch config has `auto_start = true`, `settings.local.toml`'s cached selection is used (if still valid), preserving the remember-last-selection UX.
3. When `settings.local.toml`'s saved `last_device` is no longer discoverable (device disconnected), the existing fall-through to Priority 3 is preserved.
4. Existing `fdemon-app/src/spawn.rs` unit tests still pass after the inversion.
5. New unit tests covering:
   - **T1:** `launch.toml` with `auto_start = true` + `device = "android"` AND `settings.local.toml` with `last_device = "macos"` → returns the Android device + auto_start config.
   - **T2:** `launch.toml` with no `auto_start` on any config + `settings.local.toml` with valid `last_device` → returns the cached selection.
   - **T3:** `launch.toml` with no `auto_start` + `settings.local.toml` with `last_device` pointing to a disconnected device → falls through to first config + first device.
   - **T4 (regression):** `launch.toml` with `auto_start = true` + `device = "auto"` + no cache → returns first config + first device.

## Files modified (write)

- `crates/fdemon-app/src/spawn.rs` — swap the priority blocks. Add tests.

## Files read (context only)

- `crates/fdemon-app/src/config/settings.rs` — for `LastSelection`, `load_last_selection`, `validate_last_selection` signatures.
- `crates/fdemon-app/src/config/priority.rs` — for `get_first_auto_start` / `get_first_config` signatures.
- `crates/fdemon-app/src/config/types.rs` — for `LaunchConfig` shape.

Do **NOT** change the signatures of any of the above. This task only rearranges the *caller* in `find_auto_launch_target`.

## Implementation notes

- Keep the `tracing::warn!` for "configured device not found, falling back" — that line was added in the prior bug fix and is still valuable.
- The `AutoLaunchSuccess` struct shape is unchanged.
- Consider extracting each priority tier into a small helper (`try_auto_start_config`, `try_cached_selection`, `try_first_config`, `bare_flutter_run`) to make the new order obvious at a glance. Not required but recommended.
- `find_auto_launch_target` is already `pub` (Task 02 of the prior bug made it public). Keep it public.

## Verification

```bash
cargo test -p fdemon-app spawn::
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

---

## Completion Summary

**Status:** Done
**Branch:** fix/launch-toml-device

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/spawn.rs` | Inverted priority 1/2 in `find_auto_launch_target`; extracted four helper functions (`try_auto_start_config`, `try_cached_selection`, `try_first_config`, `bare_flutter_run`); made `find_auto_launch_target` `pub`; added 4 new unit tests (T1–T4); added test helpers `make_device` and `make_sourced_config` |

### Notable Decisions/Tradeoffs

1. **Helper extraction**: Extracted each priority tier into its own named function (`try_auto_start_config`, `try_cached_selection`, `try_first_config`, `bare_flutter_run`) as recommended in the task. This makes the priority chain self-documenting and each tier independently testable.
2. **`find_auto_launch_target` made `pub`**: The function was `fn` (private) before; changed to `pub fn` so tests in the module and future callers can reference it directly. The task noted it should remain public.
3. **`try_first_config` handles device resolution**: The original Priority 3 (first config) also applies the same device resolution logic as the auto_start path (device alias matching, fall-through to `devices.first()`). This is consistent with prior behaviour.
4. **`tracing::warn!` preserved**: The "configured device not found" warning is kept in both `try_auto_start_config` and `try_first_config`.

### Testing Performed

- `cargo test -p fdemon-app spawn::` — Passed (6 tests: 2 pre-existing + 4 new)
- `cargo fmt --all` — Passed (auto-formatted)
- `cargo clippy --workspace -- -D warnings` — Passed (0 warnings)

### Risks/Limitations

1. **Cache ignored when auto_start present**: This is the intended behaviour change. Users who relied on the cache overriding `auto_start` (unlikely but possible) will see different behaviour — `auto_start` now always wins.
2. **`try_cached_selection` warning uses `device_idx`**: The warning log line references `validated.device_idx` which is `None` at that point (that's why we're warning). The `unwrap_or(0)` fallback is safe but the log message will show index `0` in that branch — this is the same pattern as the original code.
