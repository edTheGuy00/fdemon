# Replace migration banner with GitHub version-check banner

## Goal

Remove the stale `"⚠ Cache-driven auto-launch is now opt-in. Set [behavior] auto_launch …"` banner shown above the New Session Dialog at startup, and replace it with a one-line banner that surfaces only when a newer fdemon release is available on GitHub.

Banner copy (when newer release found):
`⬆ New version available: v0.6.0 (current v0.5.4)`

When no newer version is available, or the check fails for any reason, **no banner is rendered** (silent failure — this is a developer tool, not a security update channel).

Scope: TUI mode only. Headless mode does not currently surface the migration banner and will not surface the version banner either (a `tracing::debug!` line is enough there).

## Non-goals

- We are **not** removing or changing the `[behavior] auto_launch` config field itself. That opt-in behavior stays as-is. We are only removing the *one-time-per-process nudge* and the corresponding banner that announced the v0.5.0 behavior change.
- No persistent on-disk cache of the version check (per-launch only — keeps it simple and current).
- No auto-update / self-update mechanism. Just a notice.
- No new key binding to dismiss the banner; it disappears when the dialog dismisses (same lifecycle as today).

## What I learned from the codebase

| Concern | Finding |
|---|---|
| Banner string | `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs:640` |
| Banner state field | `AppState.show_migration_banner: bool` at `crates/fdemon-app/src/state.rs:1422` (default `false` at `:1564`, cleared at `:1666`) |
| Banner is set | `crates/fdemon-tui/src/startup.rs:70` from `migration_applied` boolean returned by `emit_migration_nudge(NudgeMode::Tui, ...)` at `:58` |
| `migration_applied` origin | `crates/fdemon-app/src/config/mod.rs:74-107` — evaluates conditions on `settings.local.toml` and `[behavior] auto_launch`, emits a one-time `tracing::warn!` via `OnceLock` |
| Render wiring | `crates/fdemon-tui/src/render/mod.rs:254` (`.migration_banner(state.show_migration_banner)`) |
| Widget builder field | `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs:165, 187, 194, 635-647, 722-732, 1032` |
| Headless behavior | `src/headless/runner.rs:280` calls `emit_migration_nudge(NudgeMode::Headless, ...)` and discards the result — no banner surface |
| Current crate version | `env!("CARGO_PKG_VERSION")` resolves to `0.5.4` from `Cargo.toml:7` (workspace package, inherited via `version.workspace = true`) |
| GitHub repo | `https://github.com/edTheGuy00/fdemon` (per `README.md:10`, `CONTRIBUTING.md:47`) |
| HTTP client situation | No `reqwest`/`hyper`/`ureq` anywhere. `ready_check.rs:77` rolls a raw `TcpStream + HTTP/1.1` plaintext client. **No TLS crate** (`rustls`/`native-tls`/`webpki`/`ring`) in `Cargo.lock`. |
| Background task pattern | `crates/fdemon-app/src/spawn.rs:356 spawn_tool_availability_check` — `tokio::spawn` an async block that uses `mpsc::Sender<Message>` to deliver a result Message. Spawned from `crates/fdemon-tui/src/runner.rs:199`. |
| Message-handler pattern | E.g. `update.rs:359-365 Message::SuspendFileWatcher` — handler just sets `state.field = ...; UpdateResult::none()`. Same shape works for the new variant. |
| Existing tests | `state.rs:3027-3050` (2 banner tests), `startup.rs:445-525` (3 startup tests), `config/mod.rs:123-209` (4 nudge tests) |

## Design decisions

### Decision 1 — Reuse the banner state field, but rename it

`AppState.show_migration_banner: bool` is too narrow. Replace it with:

```rust
pub startup_notice: Option<StartupNotice>,
```

Where `StartupNotice` lives in `fdemon-core` (or `fdemon-app::state`) as:

```rust
pub enum StartupNotice {
    NewVersionAvailable { latest: String }, // e.g. "0.6.0"
}
```

`enum` (not `String`) so the renderer can format consistently and future notice types can be added without rewriting plumbing.

The widget builder `.migration_banner(bool)` becomes `.startup_notice(Option<&StartupNotice>)`. The render function renders nothing when `None`, and the banner row when `Some`.

### Decision 2 — HTTPS via `reqwest` with `rustls-tls`, not curl

The repo today has zero TLS dependencies. To hit `https://api.github.com`:

- **Chosen:** Add `reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }` as a dependency of `fdemon-app` (where the version-check module will live). Rationale: tiny API surface, async-friendly, well-maintained, the `rustls-tls` feature avoids pulling system OpenSSL. Adds ~300 KB to the binary — acceptable for a feature that runs on every launch.
- **Rejected:** shelling out to `curl` — fragile on minimal Windows installs and we can't enforce its presence.
- **Rejected:** hand-rolling TLS — not worth the maintenance cost for one HTTPS call.

### Decision 3 — Version check is per-launch only, no persisted cache

Each fdemon launch fires the check once. Rationale:
- The cost is one HTTPS GET on startup, fired-and-forgotten in a background task that does not block UI rendering.
- Persisting a cache file adds complexity (where to store, when to invalidate, race with concurrent fdemon instances) for marginal benefit.
- If the check fails (offline, GitHub down, rate-limited), we simply show no banner this launch — silent and forgettable.

A short timeout (3 s) ensures even a slow/hung GitHub doesn't keep a tokio task alive for the whole session.

### Decision 4 — Opt-out config key

Add `version_check: bool` (default `true`) to the existing `BehaviorSettings` at `crates/fdemon-app/src/config/types.rs:156`. Users who want air-gapped operation or distrust outbound traffic can set:

```toml
[behavior]
version_check = false
```

This mirrors the existing pattern of `confirm_quit` / `auto_launch`. When `false`, the background task is never spawned.

### Decision 5 — Endpoint and parsing

- URL: `https://api.github.com/repos/edTheGuy00/fdemon/releases/latest`
- Headers: `User-Agent: fdemon/<CARGO_PKG_VERSION>` (GitHub requires UA), `Accept: application/vnd.github+json`
- Response: parse the `tag_name` field (e.g. `"v0.6.0"`), strip a leading `v`, parse with a minimal homegrown comparator on the `MAJOR.MINOR.PATCH` numeric triple (no `semver` crate needed — fdemon does not use pre-release tags today; if `tag_name` doesn't match `vX.Y.Z`, treat as "no newer version" rather than failing loudly).
- Comparison: `(latest_major, latest_minor, latest_patch) > (current_major, current_minor, current_patch)`
- Timeout: 3 s total (`reqwest::Client::builder().timeout(Duration::from_secs(3))`)
- Error handling: any failure (network, status != 200, JSON parse, version parse, version not newer) → return `None`, no message sent, no banner shown. A `tracing::debug!` line logs the reason for the curious.

### Decision 6 — Delete the entire migration nudge machinery

The `emit_migration_nudge`, `NudgeMode`, and `has_cached_last_device` symbols, plus their tests, become dead code once the banner is gone. Delete them in the same commit rather than leaving stragglers. The `[behavior] auto_launch` config field itself stays — it still gates cache-driven auto-launch in `startup.rs`.

The `tracing::warn!` lines at `config/mod.rs:93-103` go away too. Users who want to know about the v0.5.0 behavior change can read `docs/CONFIGURATION.md`.

### Decision 7 — Headless mode

Headless mode has no banner today and will get none. The version-check task is **not spawned** in headless mode (skipped in `src/headless/runner.rs`). Rationale: headless mode is typically scripted/CI'd, and a chatty stderr line on every CI run is noise. If a future user asks for headless awareness, we'd add a JSON event — out of scope here.

## File-level change list

### Delete

| File | Lines | What |
|---|---|---|
| `crates/fdemon-app/src/config/mod.rs` | 44-107 | `has_cached_last_device`, `NudgeMode`, `emit_migration_nudge` |
| `crates/fdemon-app/src/config/mod.rs` | 123-209 | The 4 nudge tests |
| `crates/fdemon-tui/src/startup.rs` | 9 (import line), 58 (call), 70 (assignment), 62-63 (comment) | `emit_migration_nudge` call + result wiring |
| `crates/fdemon-tui/src/startup.rs` | 445-525 | The 3 banner-state startup tests |
| `crates/fdemon-app/src/state.rs` | 1422 (field), 1564 (default), 1666 (clear) | `show_migration_banner` field |
| `crates/fdemon-app/src/state.rs` | 3027-3050 | The 2 banner field tests |
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | 165, 187, 194 (builder field + setter) | Old `show_migration_banner` plumbing |
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | 635-647 | `render_migration_banner` function with old copy |
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | 722-732, 1032 | Conditional render of the old banner |
| `src/headless/runner.rs` | 12 (import), 280 (call) | `emit_migration_nudge` call from headless path |
| `docs/CONFIGURATION.md` | 272 | The "Behavior change (post-v0.5.0)" callout — keep the `auto_launch` documentation but drop the migration banner reference |

(Exact line numbers are based on inspection done 2026-05-21; implementor should re-verify before editing.)

### Add

| File | Purpose |
|---|---|
| `crates/fdemon-app/src/version_check.rs` (NEW) | `async fn check_for_newer_release() -> Option<String>`; pure async function. Hits the GitHub releases endpoint with a 3 s timeout, parses `tag_name`, compares against `env!("CARGO_PKG_VERSION")`, returns `Some(latest_tag_string)` when newer, `None` otherwise. Includes a small `parse_semver(&str) -> Option<(u32,u32,u32)>` helper. |
| `crates/fdemon-app/src/lib.rs` | Add `pub mod version_check;` |
| `crates/fdemon-app/src/spawn.rs` | New `pub fn spawn_version_check(msg_tx: mpsc::Sender<Message>)` following the exact shape of `spawn_tool_availability_check` at `:356-374`. Calls `version_check::check_for_newer_release()`; on `Some(tag)`, sends `Message::NewVersionAvailable { latest: tag }`. |
| `crates/fdemon-app/src/message.rs` | New variant: `NewVersionAvailable { latest: String }` near the other "result-from-background-task" variants like `ToolAvailabilityChecked`. |
| `crates/fdemon-app/src/handler/update.rs` (or wherever the `Message::ToolAvailabilityChecked` arm lives — same file) | New match arm: `Message::NewVersionAvailable { latest } => { state.startup_notice = Some(StartupNotice::NewVersionAvailable { latest }); UpdateResult::none() }`. |
| `crates/fdemon-app/src/state.rs` | New field `pub startup_notice: Option<StartupNotice>` on `AppState`, default `None`. Cleared in `hide_new_session_dialog` (same place the old field was cleared at `:1666`). Define `StartupNotice` enum in the same file or in `fdemon-core` if cross-crate use is preferable (decided in implementation — leaning `fdemon-app::state`). |
| `crates/fdemon-tui/src/runner.rs` | Add `spawn::spawn_version_check(engine.msg_sender());` next to existing `spawn_tool_availability_check` calls at `:77` and `:199`. Gate on `settings.behavior.version_check`. |
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | Replace `show_migration_banner` field with `startup_notice: Option<&'a StartupNotice>`. Replace `render_migration_banner` with `render_startup_notice` that formats per variant. Same single-row layout. |
| `crates/fdemon-tui/src/render/mod.rs` | `.migration_banner(state.show_migration_banner)` becomes `.startup_notice(state.startup_notice.as_ref())` at `:254`. |
| `crates/fdemon-app/src/config/types.rs` | Add `version_check: bool` field to `BehaviorSettings` at `:156-167` with `#[serde(default = "default_true")]`. Update the `Default` impl at `:169-174` to include it. |
| `crates/fdemon-app/Cargo.toml` | Add `reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }`. |
| `Cargo.toml` (workspace) | Optionally add `reqwest` to `[workspace.dependencies]` to keep the version pinned centrally (matches the existing convention for other shared deps). |
| `docs/CONFIGURATION.md` | Document `[behavior] version_check = true/false` alongside the other behavior keys. |
| `docs/ARCHITECTURE.md` | Add `version_check.rs` to the `fdemon-app` module map. Note the new background task. |
| `crates/fdemon-app/src/version_check.rs` (tests in same file) | Unit tests: `parse_semver` happy/sad paths; comparator tests with hardcoded inputs (no network); a smoke test that constructs the request URL string. **No live network test** — flaky in CI. |
| `crates/fdemon-app/src/spawn.rs` (tests) | Test that `spawn_version_check` sends a `Message::NewVersionAvailable` when the inner function returns `Some` — mock by extracting the inner function and testing it directly (don't actually hit the network). |
| `crates/fdemon-app/src/state.rs` | New test: `startup_notice_defaults_to_none`. New test: `hide_new_session_dialog_clears_startup_notice`. |

### Modify (no new files)

| File | Change |
|---|---|
| `crates/fdemon-app/src/handler/update.rs` | Wire `Message::NewVersionAvailable` arm. |
| `crates/fdemon-tui/src/startup.rs` | Stop importing `emit_migration_nudge`; drop the call; drop the `state.show_migration_banner = ...` line. Other startup logic unchanged. |

## Open questions for the user

1. **Banner copy** — proposed: `"⬆ New version available: v0.6.0 (current v0.5.4)"` (no URL). OK, or prefer shorter / different glyph?
2. **Banner color** — old banner used `STATUS_YELLOW`. Suggest reusing it (consistent "needs your attention" semantics) or switching to a friendlier color (e.g. cyan for "info, not warning"). Recommend: keep `STATUS_YELLOW` for consistency.
3. **Banner placement** — today's banner only appears above the New Session Dialog (i.e. only on the startup screen, not on every screen). The user's prompt says "at the top". Do you want the version banner to keep that same scope (startup screen only, disappears once you've launched a session), or render globally as a persistent top row across all screens? I recommend **keep the current scope** — once you've launched something, the banner is just visual noise. Confirm or override.
4. **HTTP client** — confirm `reqwest` with `rustls-tls` is acceptable (adds ~300 KB to the binary and one transitive crate tree). Alternatives: shell out to `curl`, or skip HTTPS entirely and use a redirect-following proxy on `http://` (not viable for GitHub API).
5. **Opt-out key name** — `[behavior] version_check = true|false`. Acceptable, or prefer `check_updates`, `update_notifications`, etc.?
6. **Headless** — confirm version check is skipped entirely in headless mode (recommended). Alternative: emit a JSON `HeadlessEvent::Notice { ... }` once per process if newer is found.

I'll wait for answers before drafting the task breakdown (TASKS.md + individual task files with File Overlap Analysis). The implementation will fan out cleanly into ~5-6 tasks: (a) dep add + version_check module + tests, (b) message/handler/state plumbing, (c) widget refactor, (d) spawn wiring + config key, (e) delete migration nudge + old tests, (f) docs.
