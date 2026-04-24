# Task 04 — Rewrite launch-config docs + add regression test

**Agent:** implementor
**Plan:** [../PLAN.md](../PLAN.md)
**Depends on:** Tasks 01, 02, 03 (merged first; this task describes the final state)

## Scope

Two files, no code changes:

1. **`docs/CONFIGURATION.md`** — rewrite the "Launch Configuration → Priority Order", "Auto-Start Behavior", and "Global Settings Reference → Behavior Settings" sections to reflect the new priority chain and the removal of `[behavior] auto_start`.
2. **`example/TESTING.md`** — add **Test J**, a regression test for the exact scenario that surfaced this plan.

## CONFIGURATION.md changes

### Priority Order section (currently lines 140-146)

**Current (wrong — code has 3 tiers, not 2):**
> When both files exist, configurations are loaded in this order:
> 1. .fdemon/launch.toml configurations (first)
> 2. .vscode/launch.json configurations (second)

Keep this section — it's about *loading*, not *auto-launch selection*. It's accurate.

### Auto-Start Behavior section (currently lines 183-193)

**Current (wrong — documents a 6-step chain and a master toggle that don't match the code):**
> When `behavior.auto_start = true` in `config.toml`:
> 1. Check `settings.local.toml` for last used config/device
> 2. If found and valid, use that selection
> 3. If not found, look for first config with `auto_start = true`
> 4. If no auto_start config, use first config from launch.toml
> 5. If no launch.toml, use first config from launch.json
> 6. If no configs at all, run bare `flutter run` with first available device

**Replace with the post-Task-01/02/03 reality:**

> Flutter Demon auto-launches a session at startup when either:
>
> - Any configuration in `launch.toml` sets `auto_start = true`, OR
> - `settings.local.toml` contains a valid cached `last_device` from a previous run.
>
> **Selection priority (first matching tier wins):**
>
> 1. **Explicit intent** — first launch config with `auto_start = true`. The `device` field resolves via the matcher (see [Device Selection](#device-selection)). This tier always beats the cache.
> 2. **Remembered last selection** — if `settings.local.toml` holds `last_device` + `last_config` and the device is still connected, that selection is used. Used only when no config has `auto_start = true`.
> 3. **First available** — first config in `launch.toml` (or `launch.json`) + first discovered device.
> 4. **Bare `flutter run`** — if no configs exist at all.
>
> If a tier matches but its target device has disappeared (disconnected phone, closed simulator), Flutter Demon falls through to the next tier and logs a warning visible in the log buffer.
>
> **When is the cache updated?** Whenever a session starts successfully — both auto-launch and manual NewSessionDialog launches update `last_device` and `last_config`. Previously only auto-launches did; this was a bug that made the dialog feel forgetful.

### User Preferences section (currently lines 195-205)

Keep this section — the file still holds `last_device` / `last_config`. Add one line clarifying that manual selections now also update it (previously docs didn't need to say this because the behavior was broken).

### Behavior Settings section (currently lines 223-236)

Remove the `auto_start` row from the table. Leave `confirm_quit`.

Add a note below the table:

> **Removed in v0.5.0:** `[behavior] auto_start` — it was redundant with per-config `auto_start` in `launch.toml`, and its documented semantics never matched the code. Existing configs with the flag load cleanly but the flag has no effect; fdemon logs a one-time deprecation warning. Use per-config `auto_start = true` on the launch configuration you want to auto-launch.

### Best Practices section (currently lines 1152-1162)

Section 5 already recommends per-config `auto_start`. Keep. Optionally add a sentence: "Setting `auto_start = true` on a launch config is now the *only* way to trigger auto-launch at startup."

### Settings Panel section (currently lines 1008-1103)

Find and remove any reference to `behavior.auto_start` in the Project Settings tab description. The field no longer appears in the panel.

## TESTING.md changes

Add a new Test J after Test I. Use the existing Test X format for consistency.

### Test J — launch.toml edits take effect across runs (consolidate-launch-config regression)

**Purpose:** Regression test for the bug that surfaced the consolidate-launch-config plan. Ensures that editing `launch.toml` between runs is honored even when `settings.local.toml` holds a stale cached selection.

**Steps:**

1. In `example/app3`, remove any existing `settings.local.toml`:
   ```
   rm example/app3/.fdemon/settings.local.toml
   ```
2. Edit `example/app3/.fdemon/launch.toml`. On the "Development" config (or a config of your choice), set:
   ```toml
   device = "android"
   mode = "debug"
   auto_start = true
   ```
3. Connect at least one Android device and one other device (e.g. macOS).
4. Run:
   ```
   cargo run -- example/app3
   ```
5. Expected: session starts on Android. Confirm `example/app3/.fdemon/settings.local.toml` now contains `last_device = "<android-id>"`.
6. Quit fdemon.
7. Without touching `settings.local.toml`, edit `launch.toml` and change the "Development" config to:
   ```toml
   device = "macos"
   mode = "debug"
   auto_start = true
   ```
8. Run `cargo run -- example/app3` again.
9. **Expected result:** session starts on macOS, not Android. Before this fix, the stale `last_device = "<android-id>"` in `settings.local.toml` would have overridden the edited `launch.toml` and the session would have spawned on Android.
10. Also verify the manual-persistence leg (Task 02 fix): remove `auto_start = true` from the config, launch the NewSessionDialog, pick iPhone simulator, confirm `settings.local.toml` now reflects the simulator UUID.

## Acceptance criteria

1. `docs/CONFIGURATION.md` accurately describes the 4-tier priority chain post-fix.
2. `docs/CONFIGURATION.md` no longer documents `[behavior] auto_start` as an active setting (only as a deprecation note).
3. `example/TESTING.md` has a Test J that exercises the user's exact bug report.
4. All TOC entries and cross-links inside `CONFIGURATION.md` still resolve (re-run any doc-link check tooling if available).

## Files modified (write)

- `docs/CONFIGURATION.md`
- `example/TESTING.md`

## Files read (context only)

- The merged state of Tasks 01, 02, 03 — especially the final content of `spawn.rs` and `startup.rs` to double-check priority description matches code.

## Verification

- Read the new priority section top-to-bottom and confirm each bullet matches `spawn.rs`'s `find_auto_launch_target`.
- Manually walk through Test J's steps on the live codebase with 4 devices connected — it must pass end-to-end.
