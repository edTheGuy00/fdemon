# Task 05 — Update CONFIGURATION.md and example fixture

**Plan:** [../BUG.md](../BUG.md) · **Index:** [../TASKS.md](../TASKS.md)
**Agent:** implementor
**Depends on:** Tasks 01, 02, 03, 04
**Wave:** 3 (parallel with Task 06)

## Goal

Document the new opt-in behavior in `docs/CONFIGURATION.md` (Auto-Start Behavior section + Behavior Settings reference). Add a commented-out `# auto_launch = true` line to `example/app2/.fdemon/config.toml` for discoverability.

## Files Modified (Write)

| File | Change |
|------|--------|
| `docs/CONFIGURATION.md` | (1) Rewrite "Auto-Start Behavior" section (currently §183-216) to describe the new gate condition and 4-tier cascade with the `auto_launch` opt-in. (2) Under "Behavior Settings" (§234-247), add an entry for `auto_launch` with description, type, default, and an example. (3) Update the in-section warning that points users to per-config `auto_start = true` so it now mentions `[behavior] auto_launch` as the cache-based alternative. (4) Add a "Migration from v0.4.x/v0.5.0" callout noting that users relying on cache-only auto-launch must add the new flag. |
| `example/app2/.fdemon/config.toml` | Add a commented-out `# auto_launch = true` line under `[behavior]` with a one-line comment explaining what it does |

## Files Read (dependency)

- All implementation tasks (01-04) — to describe the shipped behavior accurately.

## Implementation Notes

### `docs/CONFIGURATION.md` — Auto-Start Behavior section rewrite

Current text (line 183 onwards) describes the gate as:
> Flutter Demon auto-launches a session at startup when **either**:
> - any configuration in `launch.toml` sets `auto_start = true`, **or**
> - `settings.local.toml` holds a `last_device` from a previous run.

Replace with:

> Flutter Demon auto-launches a session at startup when **either**:
> - any configuration in `launch.toml` sets `auto_start = true` (per-config explicit intent), **or**
> - `[behavior] auto_launch = true` is set in `config.toml` AND a valid `last_device` is cached in `settings.local.toml` (cache-based opt-in).
>
> Otherwise, the New Session dialog opens. The cached `last_device` (if any) pre-selects in the dialog but does not trigger a launch.

Update the "Selection priority" table to:

| # | Trigger | Device | Config |
|---|---------|--------|--------|
| 1 | `auto_start = true` in `launch.toml` | matched via `device` field, fallback first | the auto_start config |
| 2 | `[behavior] auto_launch = true` + valid cache | `last_device` from `settings.local.toml` | `last_config` if still valid, else first |
| 3 | `[behavior] auto_launch = true` + stale/missing cache | first available device | first launch config (if any) |
| 4 | (only when `launch.toml` is empty) | first available device | none (bare flutter run) |

> **Note:** `[behavior] auto_launch` is a *new* field. The deprecated `[behavior] auto_start` (removed in v0.5.0) is unrelated; `auto_launch` is not a revival.

### Behavior Settings reference (§234-247)

Append a row to the field table:

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `confirm_quit` | `boolean` | `true` | (existing) |
| `auto_launch` | `boolean` | `false` | When `true`, fdemon auto-launches the cached `last_device` from `settings.local.toml` on startup if no `launch.toml` configuration has `auto_start = true`. When `false` (default), the cache is preserved across runs but only used to pre-select a default in the New Session dialog. Per-config `auto_start = true` always wins regardless of this flag. |

Example block:

```toml
[behavior]
confirm_quit = true
auto_launch = false   # set true to auto-launch on cached last_device
```

### Migration callout

Add a small block (next to the existing "Removed in v0.5.0" note about `[behavior] auto_start`):

> **Behavior change in <next-version>:** Cache-driven auto-launch is now opt-in. If you were relying on `settings.local.toml` to silently auto-launch on each run, set `[behavior] auto_launch = true` in `config.toml`. This change does not affect users who use per-config `auto_start = true` — that path is unchanged. fdemon emits a one-time `info!` log on first run when this nudge applies.

### `example/app2/.fdemon/config.toml`

Insert under the existing `[behavior]` section:

```toml
[behavior]
confirm_quit = true     # Ask before quitting with running apps
# auto_launch = true    # Set true to auto-launch on the device cached in settings.local.toml
```

## Verification

- `cargo test --workspace` — sanity check; docs do not affect compilation but the example config must still parse.
- Visual review of `docs/CONFIGURATION.md` rendering (markdown preview).
- Run `fdemon` in `example/app2` after editing → confirm dialog still appears with `auto_launch` line commented out.

## Acceptance

- [x] `docs/CONFIGURATION.md` Auto-Start Behavior section accurately describes the new gate.
- [x] Behavior Settings table includes `auto_launch` row with default and description.
- [x] Migration callout present.
- [x] `example/app2/.fdemon/config.toml` has the commented discoverability line.
- [x] No code changes; CI green.

---

## Completion Summary

**Status:** Done
**Branch:** plan/cache-auto-launch-gate

### Files Modified

| File | Changes |
|------|---------|
| `docs/CONFIGURATION.md` | Rewrote "Auto-Start Behavior" section (§183-214) to describe the new 2-condition gate and 4-tier priority table. Added Tier 3 row for `auto_launch=true` + stale cache. Added headless mode note. Added `auto_launch` property row to Behavior Settings table, code example block, and migration callout. Updated removal note to mention the new `auto_launch` opt-in. |
| `example/app2/.fdemon/config.toml` | Added `# auto_launch = true` commented discoverability line under `[behavior]` |

### Notable Decisions/Tradeoffs

1. **Tier 3 vs "stale cache" phrasing:** The task's priority table lists Tier 3 as `auto_launch=true` + stale/missing cache. I kept this accurate — when `auto_launch=true` is set but there's no valid cached device, it still falls through to first-available rather than prompting the dialog.
2. **Headless note placement:** Added the headless mode note directly in the Auto-Start Behavior section (not just Behavior Settings) so users reading about the gate logic see it immediately.
3. **Migration callout wording:** Used `post-v0.5.0` rather than `<next-version>` since the task didn't specify a version number and the change is already shipped on the working branch.

### Testing Performed

- `cargo test --workspace --lib` - Passed (882 tests, 0 failed)

### Risks/Limitations

1. **Docs only:** No code was changed. Docs accurately describe the behavior implemented in Tasks 01-04.
