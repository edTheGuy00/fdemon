# Bug: launch.toml `device` field ignored on auto-launch

## Reported Symptom

When a `launch.toml` configuration declares `device = "ios"`, `device = "android"`, or `device = "macos"` with auto-launch enabled, and devices of those types are connected (verified via `flutter devices`), fdemon ignores the configured device and starts the session on whatever device appears **first** in the discovered list.

## Reproduction

1. Connect at least one iOS, one Android, and one macOS device/simulator (so all three appear in `flutter devices --machine`).
2. In a Flutter project, create `.fdemon/launch.toml` with:
   ```toml
   [[configurations]]
   name = "macOS"
   device = "macos"      # or "ios", or "android"
   auto_start = true
   ```
3. Run `fdemon` (and/or `fdemon --headless`) from the project directory.
4. Observe: a session spawns on the first device returned by Flutter — not the one declared in `launch.toml`.

## Root Cause Analysis

There are **two independent defects** that both manifest as "the configured device is ignored". A complete fix must address both.

### Defect A — `"macos"` never matches a `darwin` device (TUI + headless)

**Location:** `crates/fdemon-daemon/src/devices.rs:96-122` (`Device::matches`)

`flutter devices --machine` reports macOS desktop devices with `targetPlatform = "darwin"`, deserialized into `Device.platform = "darwin"` (see test at `crates/fdemon-daemon/src/devices.rs:388-441`). `Device::matches` performs only:

- exact id/name/case-insensitive contains match,
- `self.platform.to_lowercase().starts_with(&spec_lower)`,
- `self.platform_type == spec_lower` *(but `platform_type` is `None` for `--machine` output — that field is only populated in the daemon-protocol format).*

So `Device::matches("macos")` returns `false` for the only realistic macOS device. In `find_auto_launch_target` (`crates/fdemon-app/src/spawn.rs:249-256`), `find_device(...)` returns `None`, a warning is logged, and execution silently falls through to `devices.first()`:

```rust
let found = devices::find_device(devices, &sourced.config.device);
if found.is_none() {
    tracing::warn!(
        "Configured device '{}' not found, falling back to first available device",
        sourced.config.device
    );
}
found.or_else(|| devices.first())   // <- wrong device picked here
```

Why "ios" and "android" appear to work intermittently in TUI mode: `platform.starts_with("ios")` and `platform.starts_with("android")` succeed against the real platform strings (`"ios"`, `"android-arm64"`, …). For those two specifiers the matcher does the right thing — but if the user also tests `"macos"`, they see the wrong-device behavior.

### Defect B — Headless auto-start ignores `launch.toml` entirely

**Location:** `src/headless/runner.rs:237-296` (`headless_auto_start`)

The headless path does **not** load `launch.toml` at all. It calls `discover_devices`, then unconditionally takes the first device:

```rust
// Pick first device for auto-start
if let Some(device) = result.devices.first() {
    info!("Auto-starting with device: {} ({})", device.name, device.id);
    match engine.state.session_manager.create_session(device) {
        Ok(session_id) => {
            ...
            engine.dispatch_spawn_session(session_id, device.clone(), None);
```

`find_auto_launch_target` is never called in headless mode, and the `LaunchConfig` is never passed to `dispatch_spawn_session` (`None` is passed). This explains why `device = "ios"` / `device = "android"` are also ignored when the user is running fdemon in headless mode — the device hint never enters the matcher in the first place.

## Affected Code Map

| File | Line(s) | Issue |
|------|---------|-------|
| `crates/fdemon-daemon/src/devices.rs` | 96-122 | `Device::matches` does not handle the `"macos" ↔ "darwin"` alias |
| `crates/fdemon-app/src/spawn.rs` | 249-256 | Silent fallback to `devices.first()` masks the matcher miss |
| `src/headless/runner.rs` | 237-296 | Headless auto-start bypasses `launch.toml` entirely |
| `crates/fdemon-app/src/config/types.rs` | 16-47 | `LaunchConfig.device` is a free-form `String` (context only — schema is fine) |

## Out of Scope (explicit non-goals)

- Changing `LaunchConfig.device` from `String` to an enum — too invasive for this fix and not necessary to repair the matcher.
- Reworking the device-discovery pipeline.
- Adding a UI affordance for "no matching device" (today the warning lands in the log file only — that is acceptable for now; we will surface it in a follow-up).
- Re-architecting headless startup. We will reuse `find_auto_launch_target` rather than duplicating logic.

## Proposed Fix (high level)

Three small, focused changes — designed to be parallelizable in worktrees because they touch disjoint files:

1. **Fix the matcher** (`fdemon-daemon`)
   Extend `Device::matches` so a specifier of `"macos"` also matches a device whose `platform` is `"darwin"`. Mirror the existing `platform_short` aliases (`"macos" | "darwin" => "macOS"`, `"chrome" | "web-javascript" => "Web"`) so the matcher and the display layer share a single source of truth (extract a small `platform_canonical(&str) -> &str` helper used by both). Add unit tests covering `"macos"` → `darwin`, `"web"` → `web-javascript`, and the existing happy paths to prevent regression.

2. **Make the matcher miss visible to users** (`fdemon-app`)
   In `find_auto_launch_target` (`spawn.rs:249-256`), when `find_device` returns `None`, additionally push a user-visible warning into the log buffer (not just `tracing::warn!`) so the user notices their `launch.toml` was effectively ignored. Keep the fallback to `devices.first()` so we don't regress the "device disconnected mid-session" recovery story — but the user must be told.

3. **Wire `launch.toml` into headless auto-start** (`flutter-demon` binary)
   In `headless_auto_start`, load `LoadedConfigs` (the same way the TUI runner does it via `engine.state.launch_configs` or a fresh `config::load_configs`), call `find_auto_launch_target(&configs, &result.devices, project_path)`, and pass the resolved `device` and `Some(Box::new(config))` to `engine.dispatch_spawn_session`. Add a headless-path unit test that asserts the configured device is selected when multiple devices are present.

## Verification

- `cargo test -p fdemon-daemon devices::tests` — confirms new matcher cases (macos↔darwin, web↔web-javascript) pass.
- `cargo test -p fdemon-app spawn::tests` — confirms `find_auto_launch_target` selects the configured device, including the macOS case, with a multi-device input.
- `cargo test --test headless_auto_start` (or equivalent) — confirms headless path honors `launch.toml`.
- `cargo fmt --all && cargo check --workspace && cargo clippy --workspace -- -D warnings` — quality gate.
- Manual smoke test: with three devices connected, run `fdemon` and `fdemon --headless` against a project whose `launch.toml` declares `device = "macos"`; confirm the macOS session is started in both modes.

## Open Questions for the User

1. Do you want a hard failure when `find_device` returns `None` for a non-`"auto"` specifier (i.e. *abort* auto-launch and prompt the user) rather than silently falling back to `devices.first()`? Today's behavior is "warn + fall back"; the safer behavior is "warn + show the device picker". Both are reasonable; the latter is a behavior change.
2. For headless mode, if `launch.toml` is missing entirely, should the existing "first device" behavior remain as a fallback, or should headless require a launch config?
