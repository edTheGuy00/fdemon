# Task 04 — Apply the gate to headless mode (`cache_allowed = false`)

**Plan:** [../BUG.md](../BUG.md) · **Index:** [../TASKS.md](../TASKS.md)
**Agent:** implementor
**Depends on:** Task 02 (`cache_allowed` plumbing) AND sibling bug `launch-toml-device-ignored` Task 03 (headless `find_auto_launch_target` wiring)
**Wave:** 2 (parallel with Task 03)

## Goal

Per decision 2(b), headless mode keeps today's "always auto-launch" semantic. Cache is **never** consulted in headless — the call site hard-wires `cache_allowed = false`. Once the sibling bug's Task 03 has wired `find_auto_launch_target` into `headless_auto_start`, this task simply ensures the call passes `cache_allowed: false` and adds a regression test. Also emit the migration `info!` (same as TUI Task 03) so headless users get the same nudge in their log file.

## Files Modified (Write)

| File | Change |
|------|--------|
| `src/headless/runner.rs` | (1) Pass `cache_allowed: false` to `find_auto_launch_target` (or to `spawn_auto_launch`, depending on how the sibling task wires it). (2) Emit migration `info!` when cache is present but `auto_launch` is unset and no auto_start config — same condition as Task 03. (3) Add headless test asserting that a cached `last_device` does NOT fire under default settings. |

## Files Read (dependency)

- `crates/fdemon-app/src/spawn.rs` — `find_auto_launch_target` signature with `cache_allowed` (Task 02)
- `crates/fdemon-app/src/config/mod.rs` — `load_all_configs` (already public)
- Sibling bug: `workflow/plans/bugs/launch-toml-device-ignored/tasks/03-headless-launch-toml-auto-launch.md` — provides the `find_auto_launch_target` integration point in headless

## Implementation Notes

### Coordination with sibling bug

After the sibling bug's Task 03 lands, `headless_auto_start` will look approximately like:

```rust
let configs = config::load_all_configs(project_path);
match devices::discover_devices(&flutter).await {
    Ok(result) => {
        // [sibling task: integrate find_auto_launch_target]
        let target = find_auto_launch_target(
            &configs,
            &result.devices,
            project_path,
            /* cache_allowed: */ ???,    // <-- Task 04 fills this in
        );
        // ... session creation ...
    }
    ...
}
```

This task's job is to make the `???` evaluate to `false`. **Do not** read `engine.settings.behavior.auto_launch` here — per decision 2(b), headless is intentionally cache-blind regardless of the user's flag.

If the sibling task has not yet merged when work starts, this task may also absorb the wiring (call `find_auto_launch_target` directly and dispatch_spawn_session with the result). In that case the sibling Task 03 becomes a no-op on merge. Prefer waiting; only absorb if the sibling is blocked.

### Migration `info!`

Use the same condition and message as Task 03's TUI version. Headless users still benefit from being told "your cache is no longer driving auto-launch — set `auto_launch = true` if you want it back."

```rust
let configs = config::load_all_configs(project_path);
let has_auto_start_config = get_first_auto_start(&configs).is_some();
let has_cache              = has_cached_last_device(project_path); // shared helper
let cache_opt_in           = engine.settings.behavior.auto_launch;

if !has_auto_start_config && has_cache && !cache_opt_in {
    tracing::info!(/* same message as Task 03 */);
}
```

The `has_cached_last_device` helper is currently private to `crates/fdemon-tui/src/startup.rs`. Either:
- (a) duplicate the 4-line helper inline in headless,
- (b) move it to `crates/fdemon-app/src/config/mod.rs` (new public helper) — a small addition, but Task 02 didn't introduce it, so this would be Task 04's write to `config/mod.rs`. **Prefer (b)** for DRY; declare the additional write in this task's File Modified list if so.

If (b) is chosen, `crates/fdemon-tui/src/startup.rs` (Task 03) should also be updated to use the shared helper. Coordinate via Task 03 — if Task 03 has already merged, this task does the helper move and updates Task 03's call site to point at the shared symbol. If Task 04 lands first, Task 03 picks it up.

### Test

Add `crates/fdemon-app` integration test (or place in `src/headless/runner.rs::tests`) that mocks devices + cache and asserts:
1. Cache + no `auto_launch` + no `auto_start` → first device wins (cache ignored, behavior unchanged from today).
2. `auto_launch = true` + cache + no `auto_start` → first device still wins (headless ignores `auto_launch` per decision 2(b)).
3. `auto_start = true` config → that config's device wins (Tier 1, sibling task's verification).

## Verification

- `cargo check --workspace`
- `cargo test --test headless_auto_start` (or the equivalent test target)
- `cargo clippy --workspace -- -D warnings`
- Manual smoke: in `example/app2`, run `fdemon --headless` → auto-launches with first device (today's behavior preserved).

## Acceptance

- [ ] Headless calls `find_auto_launch_target(.., cache_allowed: false)`.
- [ ] Migration `info!` fires in headless when conditions are met.
- [ ] Headless test asserts cache does NOT drive headless auto-launch (regardless of `auto_launch` flag).
- [ ] Sibling bug's Task 03 successfully merged (or absorbed inline if blocked).
- [ ] No regression in CI/script users of `fdemon --headless`.
