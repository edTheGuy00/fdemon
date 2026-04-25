# Task 05 — Broaden the startup gate to fire on cached `last_device`

**Agent:** implementor
**Plan:** [../TASKS.md](../TASKS.md) (Option α)
**Parent PR:** #35 (Copilot review comments #1 and #3)

## Problem (one-liner)

`crates/fdemon-tui/src/startup.rs::startup_flutter` currently fires `StartupAction::AutoStart` only when at least one launch config has `auto_start = true`. The same condition guarantees Priority 1 of `find_auto_launch_target` always wins, which makes Priority 2 (cached `last_device`/`last_config`) **structurally unreachable**. Consequence: Task 02's symmetric persistence writes to `settings.local.toml` but no code path ever reads it back — the "remember last manual selection" UX promised by parent PLAN.md §3 and §5 is silently absent.

## Desired behavior

Widen the gate so it fires when **either**:
- Any launch config has `auto_start = true`, **or**
- `settings.local.toml` exists and has a non-empty `last_device`.

Tier 2 of `find_auto_launch_target` becomes reachable. Tier 1 still wins when an explicit `auto_start` config is present. When the cache is the only trigger and the cached device has been disconnected, the existing Tier 3 / Tier 4 fall-through inside `find_auto_launch_target` handles it (this PR keeps that cascade as-is — option α).

## Acceptance criteria

1. `startup_flutter` returns `StartupAction::AutoStart { configs }` when `settings.local.toml` exists with a non-empty `last_device`, even if no launch config has `auto_start = true`.
2. The behavior when an `auto_start = true` config exists is unchanged (still fires `AutoStart`, regardless of cache state).
3. When neither condition holds (no `auto_start` config and no valid cached `last_device`), `startup_flutter` keeps returning `StartupAction::Ready` and showing the New Session dialog as before.
4. A cache file with `last_device = ""` (empty string) is treated as "no cache" — gate does NOT fire.
5. A cache file that fails to parse is treated as "no cache" — gate does NOT fire.
6. New unit tests cover the new gate branches:
   - **G1:** Cache with `last_device = "foo"`, no auto_start configs → returns `AutoStart`. UI mode is NOT `Startup`.
   - **G2:** Cache file present but `last_device = ""`, no auto_start configs → returns `Ready`. UI mode is `Startup`.
   - **G3:** Cache present with `last_device = "foo"` AND an auto_start config → returns `AutoStart` (auto_start path still fires; cache doesn't matter).
   - Existing 6 tests continue to pass (their tempdirs have no `settings.local.toml`, so the cache-gate branch doesn't fire spuriously).

## Files modified (write)

- `crates/fdemon-tui/src/startup.rs` — broaden the gate, add a small `has_cached_last_device(project_path: &Path) -> bool` helper near the top of the module, add the 3 new tests, update the doc comment on `startup_flutter`.

## Files read (context only)

- `crates/fdemon-app/src/config/settings.rs` — `load_last_selection(project_path) -> Option<LastSelection>` and the `LastSelection` struct shape (`last_device: Option<String>`, `last_config: Option<String>`). Do NOT change these signatures.
- `crates/fdemon-app/src/spawn.rs` — `find_auto_launch_target` and `try_cached_selection` (read-only; for understanding what the gate is enabling).
- `workflow/plans/features/consolidate-launch-config/PLAN.md` §3, §5 — design intent.

## Implementation notes

- The helper is small enough to live as a private fn in `startup.rs`. No need to extend the `fdemon-app/config` API:
  ```rust
  fn has_cached_last_device(project_path: &Path) -> bool {
      crate::config::load_last_selection(project_path)
          .and_then(|s| s.last_device)
          .is_some_and(|d| !d.is_empty())
  }
  ```
  Adjust the import path / module re-exports to match what `startup.rs` already brings in via `fdemon_app::config::...`.
- Keep `_settings` parameter prefix as-is — the field-removal from Task 03 already silenced it.
- Update the `///` doc comment on `startup_flutter` to describe both gate conditions, mentioning that the cache-triggered path still goes through `find_auto_launch_target`'s 4-tier cascade and may fall through to Tier 3 if the cached device has been disconnected.
- Do NOT change the `StartupAction` enum shape — Task 06 and Task 07 (and the runner that consumes `StartupAction`) all assume the existing variants.

## Out of scope

- Changing `find_auto_launch_target`'s tier behavior — Task 06 handles its only refactor (the dead-warn-branch fix).
- Adding a "show dialog on stale cache" path — that's option β, deferred per parent TASKS.md §"Out of scope".
- Changing `load_last_selection`, `LastSelection`, or any other config API.
- Updating `docs/CONFIGURATION.md`, the website page, or `example/app3/.fdemon/launch.toml` header — Task 07 owns docs.

## Verification

```bash
cargo test -p fdemon-tui startup::
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

Quick manual sanity (optional — full regression smoke is in TASKS.md): write a `settings.local.toml` with `last_device = "iphone"` into a tempdir, no launch.toml, call `startup_flutter`, assert it returns `AutoStart` and `state.ui_mode != UiMode::Startup`.

---

## Completion Summary

**Status:** Done
**Branch:** fix/launch-toml-device

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/startup.rs` | Added `has_cached_last_device` helper, broadened startup gate to fire on cached `last_device`, updated `startup_flutter` doc comment, added 3 new tests (G1/G2/G3) |

### Notable Decisions/Tradeoffs

1. **Field name correction**: The task's implementation note example used `s.last_device` but `LastSelection` actually has `device_id: Option<String>`. Used the correct field name `device_id`.
2. **`cache_trigger` short-circuit**: Evaluated `has_cached_last_device` only when `has_auto_start_config` is false, avoiding an unnecessary file read in the common auto-start config path.
3. **`LoadedConfigs` unused warning not triggered**: The `_settings` parameter prefix was already suppressing warnings; no additional changes needed.

### Testing Performed

- `cargo test -p fdemon-tui startup::` - Passed (9 tests: 6 original + 3 new G1/G2/G3)
- `cargo test --workspace --lib` - Passed (869 tests)
- `cargo fmt --all` - Passed (imports reformatted to multi-line by rustfmt)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Stale cache device**: When the cache gate fires but the cached device is gone, `find_auto_launch_target` falls through to Tier 3/Tier 4 as designed. No user-visible feedback is added here (option β deferred per TASKS.md).
