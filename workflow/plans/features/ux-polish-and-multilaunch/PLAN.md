# Plan: UX Polish & Multi-Device Launch

## TL;DR

Four user-facing improvements to fdemon's TUI: (1) a **multi-device launch picker** so one confirm spawns N sessions across selected devices; (2) a **shimmer animation** on transient status text to signal in-progress work; (3) **animated spinners in more in-progress states** (device discovery, connecting/reloading rows) beyond the startup screen; and (4) a **"new logs below" jump-to-latest affordance** for the log view (issue #31). A small shared animation-frame counter on `AppState` underpins (2) and (3).

---

## Background

fdemon is feature-rich but has a few rough edges in everyday flows:

- **Single-device launch only.** The new-session dialog's `TargetSelectorState` tracks a single `selected_index`; `Enter` launches exactly one device (`handler/new_session/launch_context.rs::handle_launch`). Yet `SessionManager` already supports up to 9 concurrent sessions, so the backend can run many devices at once — the dialog is the bottleneck. Developers testing on a phone + tablet + simulator must open the dialog and confirm three separate times.

- **Static "work in progress" feedback.** The only animated element in the entire UI is the braille spinner on the **startup loading screen** (`render/mod.rs` `render_loading_screen`, frame driven by `LoadingState::animation_frame`). The new-session dialog shows a static "Discovering devices…" line; connecting/reloading device states show static glyphs. There is no sweeping/shimmer effect anywhere. The 50 ms tick loop (`event.rs`, `Message::Tick`) can drive animation but nothing outside the loading screen consumes it.

- **No affordance for "you're not at the latest log."** `LogViewState` already supports `scroll_to_bottom()` and auto-scroll-follow, bound to `G` / `End` (`handler/scroll.rs`, `handler/keys.rs:312,316`). But when the user scrolls up to read history while logs keep streaming, nothing tells them they've fallen behind or how far. Issue #31 ("Jump to end log line") asks for a quick way back to the live tail; the keybinding exists, but the **discoverability and feedback** do not.

The theme is true-color RGB (`theme/palette.rs`), which makes per-character color interpolation (shimmer) straightforward.

---

## Scope & Delivery Decisions

- **Multi-select scope:** Connected tab only for Phase 1. Bootable (un-booted) simulators/AVDs remain single-launch; boot-then-launch is deferred to Future Enhancements.
- **Delivery:** Split into **independent units** rather than one bundle. Dependency map:
  - **Animations unit** — Phase 0 (shared `animation_frame`) → Phases 2 (shimmer) + 3 (spinner). Phase 6 (reload flash) also belongs here because it reuses Phase 2's `lerp` helper.
  - **Multi-launch unit** — Phase 1. Pairs with Phase 5 (runnable filtering): Phase 5 is independent and valuable on its own, but Phase 1's checkbox selection should respect the supportability flag Phase 5 adds, so sequence Phase 5 before (or with) Phase 1 if both are taken.
  - **Log-indicator unit** — Phase 4. Fully independent.
  - When task breakdowns are generated, expect separate `TASKS.md` sets per unit so they can run in parallel worktrees with minimal file overlap.

---

## Affected Modules

**Shared foundation**
- `crates/fdemon-app/src/state.rs` — add a global, always-incrementing `animation_frame: u64` (or `anim_tick`) to `AppState`, ticked on every `Message::Tick` regardless of `UiMode`.
- `crates/fdemon-app/src/handler/update.rs` — increment the global frame in the `Message::Tick` arm (currently only ticks loading animation).

**Phase 1 — Multi-device launch picker**
- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` — add multi-select model (checked set), toggle/select-all/clear operations, and a "selected devices" accessor.
- `crates/fdemon-app/src/message.rs` — new messages: toggle selection, select-all, (and reuse existing launch message).
- `crates/fdemon-app/src/handler/new_session/navigation.rs` — key bindings (`Space` toggle, `a` select-all) within the dialog.
- `crates/fdemon-app/src/handler/new_session/launch_context.rs` — `handle_launch` extended to spawn N sessions via `UpdateResult::actions_vec(...)`.
- `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs` + `target_selector.rs` — render checkboxes / multi-highlight and an updated footer hint.

**Phase 2 — Shimmer status text**
- `crates/fdemon-tui/src/widgets/` — **NEW** small `shimmer.rs` helper (phase + per-character RGB lerp), or co-locate in `render/`.
- `crates/fdemon-tui/src/render/mod.rs` and/or `widgets/header.rs` — apply shimmer to transient status text.

**Phase 3 — Spinner in more states**
- `crates/fdemon-tui/src/widgets/` — **NEW** shared `spinner.rs` (extract the existing braille frames into one reusable function keyed by the global frame).
- `crates/fdemon-tui/src/render/mod.rs` — reuse spinner in `render_loading_screen` (replace inline constant).
- `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` — animate the "Discovering devices…" line; show spinner on `refreshing` / `loading`.

**Phase 4 — Jump-to-latest log affordance (#31)**
- `crates/fdemon-app/src/session/session.rs` — track unseen-line count while not following (in `add_log` / `add_logs_batch`).
- `crates/fdemon-app/src/log_view_state.rs` — expose follow state; reset unseen count on `scroll_to_bottom`.
- `crates/fdemon-tui/src/widgets/log_view/mod.rs` — render a floating "↓ N new — G to jump" indicator when scrolled up with pending lines.

**Phase 5 — Runnable-device filtering**
- `crates/fdemon-daemon/src/devices.rs` — add `is_supported` (serde default `true`) and optionally `capabilities` to the `Device` struct/parser.
- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` / `device_groups.rs` — exclude (or disable) unsupported devices in the Connected flat list; ensure multi-select skips them.
- `crates/fdemon-tui/src/widgets/new_session_dialog/` — empty-state messaging when all devices are filtered out.

**Phase 6 — Reload success flash**
- `crates/fdemon-app/src/session/session.rs` — optional `reload_flash_alpha()` helper decaying from `last_reload_time`.
- `crates/fdemon-tui/src/render/mod.rs` / `widgets/header.rs` — tint header background toward success green using the Phase 2 `lerp` while the flash is active.

---

## Development Phases

### Phase 0: Shared animation frame (foundation)

**Goal**: Provide a single time source for all animations, decoupled from the loading screen.

**Duration**: ~0.5–1h

#### Steps

1. **Add global frame counter**
   - Add `animation_frame: u64` to `AppState` (default 0).
   - In `Message::Tick`, `animation_frame = animation_frame.wrapping_add(1)` unconditionally (in addition to the existing loading-screen tick).
   - Tick cadence is already 50 ms (20 fps). Spinner frames can advance every ~4 ticks (~80 ms) by dividing; shimmer phase derives from `frame % period`.

**Milestone**: A monotonically advancing frame value is available to every widget via `&AppState`, with no behavioral change yet.

---

### Phase 1: Multi-device launch picker

**Goal**: Let the user check multiple devices in the new-session dialog and launch them all with one confirm.

**Duration**: ~4–6h

#### Steps

1. **Multi-select state**
   - Add a selection set to `TargetSelectorState` (e.g. `checked_device_ids: HashSet<String>`), keyed by device id/udid/avd-name so it survives list refreshes and tab switches.
   - Keep `selected_index` as the *cursor*; selection is now independent of the cursor.
   - Add methods: `toggle_checked(cursor)`, `select_all_in_tab()`, `clear_checked()`, `checked_devices() -> Vec<…>`, `checked_count()`.
   - Decide cross-tab behavior: selections are tracked per identity across both Connected/Bootable tabs (booting a simulator is a separate concern — see Risks).

2. **Key handling**
   - In the dialog navigation handler: `Space` → toggle current cursor row; `a` → select-all (toggle all on / all off, matching current all-checked state); existing `Enter` → launch.
   - Guard against toggling header rows (`DeviceListItem::Header`).

3. **Launch N sessions**
   - Extend `handle_launch`: if `checked_count() == 0`, fall back to current single-device behavior (cursor row) — zero regression for existing muscle memory. If ≥1 checked, iterate the checked devices.
   - For each device: run the existing per-device pipeline (dedup against `find_active_by_device_id`, build config, `create_session_configured` / `_with_config_configured`, decide `SpawnSession` vs `SpawnPreAppSources`), collecting one `UpdateAction` per device.
   - Return `UpdateResult::actions_vec(actions)`.
   - Select the first newly-created session; close the dialog.
   - Per-device failures (e.g. max sessions hit mid-loop, device vanished): collect into a summary error shown in the dialog or as a toast; successfully-spawned sessions still launch.

4. **Rendering**
   - Device rows render a checkbox (`[x]` / `[ ]`) plus the existing cursor highlight.
   - Footer hint updates: `Space select · a all · Enter launch`.
   - Header/title can show `N selected`.

**Milestone**: User checks a phone + a simulator, presses `Enter` once, and two sessions appear and start.

---

### Phase 2: Shimmer status text

**Goal**: A subtle left-to-right color sweep on transient status labels signals "work happening."

**Duration**: ~2–3h

#### Steps

1. **Shimmer helper**
   - New `shimmer.rs`: `shimmer_phase(frame: u64) -> f32` (cycles 0→1 over ~1.5 s = ~30 frames) and `shimmer_line(text, dim, accent, phase, bg) -> Line` that lerps each character's fg between `dim` and `accent` based on distance from a moving "head."
   - RGB `lerp(a, b, t)` helper; assumes `Color::Rgb` (the theme is all true-color) with a graceful fallback for non-RGB colors.

2. **Apply to status text**
   - Identify transient status surfaces (header status during reload/restart, "Building…"/"Starting…" style labels, reconnecting indicators).
   - Render those with `shimmer_line` driven by the global frame while the condition holds; revert to a static style otherwise.
   - Keep static, keybinding-bearing text unshimmered (animate only the status hint, not the key labels).

**Milestone**: While a hot reload/restart is in flight, its status label gently shimmers; static UI is unaffected.

---

### Phase 2.5: Launch lifecycle phases (Preparing → Launching → Running)

**Goal**: Stop showing `Running` the instant the OS process attaches. Surface the real pre-run lifecycle so the user sees a distinct, shimmering transient state while the app is still building/compiling — which can take seconds to minutes on large apps — and while pre-app native-log sources (`start_before_app` + `ready_check`, e.g. `example/app5`) are coming up. Sequenced immediately after Phase 2 because these long-lived transient states are where the Phase 2 shimmer first becomes visible (reload/restart are too fast to notice).

**Duration**: ~4–6h (multi-task; touches a core domain enum)

**Problem (confirmed by research)**: `AppPhase::Running` is set optimistically on `Message::SessionStarted` (`handler/session_lifecycle.rs:21`) the moment the Flutter process pipe opens, and re-affirmed on the `app.start` daemon event via `Session::mark_started` (`session/session.rs:530`). The daemon's true "app is up" signal — `app.started` (`DaemonMessage::AppStarted`) — is parsed but drives **no** phase change. Flutter's build-progress events (`app.progress`, `finished:false`) are parsed but silently dropped (`daemon/protocol.rs:306`). During pre-app `ready_check` polling the session sits at `Initializing` ("Starting") with no distinct indication.

#### Steps

1. **Two new `AppPhase` variants** — `Preparing` (pre-app `ready_check` polling, before Flutter spawns) and `Launching` (process attached / building / first run, before `app.started`). Both render in `STATUS_BLUE`, both shimmer (reuse Phase 2's `shimmer` helper / `is_transient`).
2. **Re-map the lifecycle**: `SpawnPreAppSources` → `Preparing`; process attach (`SessionStarted`) → `Launching`; `app.start` keeps `Launching` (still captures `app_id`); **`app.started` → `Running`** (the fix). Steady `Running`/`Stopped` and the reload/restart path are unchanged.
3. **Surface progress text**: add `Session::current_progress: Option<String>`, fed by `app.progress(finished:false)` build messages ("Running Gradle task…") and pre-app readiness updates ("Waiting for services 1/2"); cleared on `Running`. Rendered as a dim suffix next to the shimmering label.
4. **Gate input**: hot reload/restart stay no-ops until `Running` (they already key off `is_running()`, which excludes the new variants — verify and tighten if needed). `is_busy` stays `Reloading`-only so the busy-label path doesn't mislabel the new phases as "Reloading".

**Milestone**: Launching a session shows `Preparing` (if pre-app sources) → shimmering `Launching` (with live build text) and only flips to `Running` when the Flutter app is actually up.

**Tasks**: see `phase-2.5-launch-lifecycle/TASKS.md`.

---

### Phase 3: Spinner in more states

**Goal**: Use the existing braille throbber consistently wherever the UI is waiting, not just at startup.

**Duration**: ~2–3h

#### Steps

1. **Extract a reusable spinner**
   - New `spinner.rs`: `const FRAMES = ['⠋','⠙','⠹','⠸','⠼','⠴','⠦','⠧','⠇','⠏']`; `spinner_char(frame: u64) -> char` advancing one frame per ~80 ms (derive from the global frame).
   - Replace the inline `SPINNER` constant in `render_loading_screen` with this helper (no visual change).

2. **Animate in-progress states**
   - New-session dialog: replace the static "Discovering devices…" with `{spinner} Discovering devices…`; show the spinner on `loading` and (subtly) on `refreshing` / `bootable_refreshing`.
   - Compute the frame **once per render** so every concurrent spinner pulses in unison.
   - (Optional, if a session/device list with phase glyphs exists in the main view) use the spinner glyph for `Reloading`/connecting states.

**Milestone**: Opening the dialog shows an animated discovery spinner; refreshes show motion instead of a frozen line.

---

### Phase 4: Jump-to-latest log affordance (issue #31)

**Goal**: Make "jump to the live tail" discoverable, and tell the user when they've fallen behind the stream.

**Duration**: ~2–4h

#### Steps

1. **Track unseen lines**
   - When `auto_scroll` is **off** and a log is appended (`session.rs::add_log` / `add_logs_batch`), increment a per-session `unseen_log_count`.
   - Reset to 0 whenever auto-scroll re-engages or `scroll_to_bottom()` runs.
   - (`scroll_to_bottom` already sets `auto_scroll = true`; confirm both paths reset the counter.)

2. **Indicator + jump**
   - In the log view widget, when scrolled up with `unseen_log_count > 0` (or simply when not following), render a small floating indicator near the bottom-right of the log area, e.g. `↓ 12 new — G to jump` (or `↓ End` when count is unknown/zero-but-not-following).
   - Keep it cheap: a single right-aligned line overlaid on the last visible row; no extra layout pass.
   - `G` / `End` already jump and re-enable follow — no new keybinding strictly required, but the indicator advertises it. Optionally accept a mouse click on the indicator (mouse routing already exists) to jump.

**Milestone**: Scrolling up during a busy log stream shows a live "N new" badge; pressing `G` returns to the tail and clears it.

---

### Phase 5: Runnable-device filtering (device-list hardening)

**Goal**: Stop offering devices the Flutter toolchain won't actually run for this project, eliminating the "No supported devices found" failure after a confirmed launch.

**Duration**: ~2–3h

#### Steps

1. **Capture supportability from discovery**
   - `flutter devices --machine` already emits `isSupported` and a `capabilities` object (`hotReload`/`hotRestart`); fdemon currently parses neither (`crates/fdemon-daemon/src/devices.rs` `Device` struct).
   - Add `is_supported: bool` (serde default `true` so older/abbreviated payloads don't vanish) and optionally `capabilities` (hot-reload/restart flags) to `Device`.

2. **Filter or flag in the dialog**
   - In the Connected tab's flat-list build, exclude `is_supported == false` devices, **or** render them dimmed/disabled and non-selectable. Default to excluding (matches the "only runnable targets" goal); revisit if users want to see-but-not-pick.
   - Multi-select (Phase 1) must skip unsupported devices — they can't be checked.

3. **Empty-state messaging**
   - If all discovered devices are filtered out, show an actionable empty state ("Devices found but none runnable for this project — check enabled platforms") rather than a bare "no devices."

**Milestone**: The dialog lists only devices `flutter run` will accept; capability flags are available for future UI (e.g. greying out hot-reload where unsupported).

---

### Phase 6: Reload success flash

**Goal**: Brief positive visual feedback when a hot reload lands.

**Duration**: ~1–2h

#### Steps

1. **Drive from existing timestamp**
   - `Session::complete_reload()` already stamps `last_reload_time` (`crates/fdemon-app/src/session/session.rs:545`). No new state needed beyond, optionally, a helper `reload_flash_alpha()` returning a 0→1 value that decays over ~500 ms after `last_reload_time`.

2. **Tint the header**
   - In the header/status render, when `reload_flash_alpha() > 0`, blend the header background toward the success green using the **same `lerp` helper** introduced in Phase 2 (`lerp(bg, success, alpha * k)`).
   - Decay is wall-clock based (from `last_reload_time`), redrawn by the existing tick loop; no extra timer.

**Milestone**: Each successful hot reload briefly pulses the header green, then fades back.

---

### Phase 6.5: Shimmer timing polish + launch spinner icon

**Goal**: Two follow-up refinements after Phases 2/2.5/3 shipped. (1) The status-label
shimmer (Phase 2) "feels off" — it reads as a constant, mechanical sweep with no rest.
(2) The launch-lifecycle phases (`Initializing`/`Preparing`/`Launching`) still show a
**static** `○` icon next to their shimmering label; swap that icon for the existing
braille spinner (Phase 3) so the in-progress state animates on both glyph and text.

**Duration**: ~1.5–2.5h (two independent, small TUI-only tasks)

**Research finding (shimmer)** — the current sweep and a smoother reference variant use
the same ~1.5 s period, so cycle length is **not** the problem. The difference is the
sweep **range**:

| | fdemon (`widgets/shimmer.rs:58,64`) | smoother variant |
|---|---|---|
| Head position | `head = phase * n` → range `[0, n)` | `head = phase * (n + 6) − 3` → range `[−3, n+3)` |
| Behavior | Bright head **pops in** at char 0 and **snaps back** each cycle; no rest | Head **fades in from off-screen left**, exits **off-screen right** → brief all-dim **rest gap** between sweeps |
| Head falloff | `dist / 4.0` | `dist / 3.5` (slightly tighter) |

The "off" feeling is the hard pop-in/snap-back and the absence of a rest gap. Adopting
the off-screen lead-in/lead-out (and matching the 3.5 falloff) makes the sweep breathe.
This is a **pure-math change in one function** (`shimmer_spans`) — every existing call
site (status label, future reuse) benefits with no call-site change.

**Decisions (confirmed with user):**
- **Spinner phases:** launch-lifecycle only — `Initializing`, `Preparing`, `Launching`.
  `Reloading` keeps its `↻` glyph, `Quitting` keeps `✗`, `Running`/`Stopped` unchanged.
- **Surface:** the **bottom status bar only** (`widgets/log_view/mod.rs`
  `render_bottom_metadata`). The header title row and session tabs keep their static
  phase icons.

#### Steps

1. **Refine the shimmer sweep** (`widgets/shimmer.rs`)
   - Change the head computation in `shimmer_spans` so the sweep travels off-screen at
     both ends: `head = phase * (n + LEAD*2) − LEAD` with `LEAD ≈ 3.0`, and narrow the
     falloff from `SHIMMER_HEAD_WIDTH = 4.0` to `3.5` for a slightly tighter head.
   - Keep the frame-counter phase source (`shimmer_phase(frame)`, period 30) so all
     shimmers stay in unison — only the spatial mapping changes.
   - Update/extend the existing `shimmer_spans` unit tests (the `shimmer_spans_head_is_brightest`
     test asserts "char at dist >4 == base" and "head at index 0 brightest at phase 0",
     both of which change under the new range — re-derive the expectations).

2. **Spinner icon for launch phases** (`widgets/log_view/mod.rs`)
   - In `render_bottom_metadata`, when `status.phase` ∈ {`Initializing`, `Preparing`,
     `Launching`}, render `spinner::spinner_char(status.animation_frame / SPINNER_TICKS_PER_FRAME)`
     (styled with the existing `phase_style`) in place of the static `icon`.
   - All other phases (incl. `Reloading`/`Quitting`/`Running`/`Stopped`, and the
     `is_busy` path) keep their static `phase_indicator` icon — unchanged.
   - Label shimmer (`is_transient`) is untouched; only the leading glyph changes.

**Milestone**: The reload/launch shimmer breathes (sweeps in, rests, repeats) instead of
popping; and `Launching`/`Preparing`/`Starting` show a spinning braille glyph in the
status bar in lockstep with the dialog's discovery spinner.

**Tasks**: see `phase-6.5-shimmer-spinner-polish/TASKS.md`.

---

## Edge Cases & Risks

### Multi-launch
- **Risk:** Selecting more devices than the session cap (9) allows. **Mitigation:** launch up to the remaining capacity in selection order; surface a clear "max sessions reached, launched X of Y" message; don't abort the whole batch.
- **Risk:** A checked device already has an active session. **Mitigation:** skip it (reuse existing `find_active_by_device_id` guard) and note it in the summary rather than erroring out.
- **Risk:** Checked bootable (un-booted) simulators/AVDs can't be `flutter run` targets directly. **Mitigation:** scope multi-select to the **Connected** tab initially; treat bootable-tab multi-select as out of scope (or boot-then-launch as a Future Enhancement).
- **Risk:** Selection set going stale across device-list refreshes. **Mitigation:** key the set by stable device identity and prune ids no longer present on read.
- **Risk:** Spawning N sessions emits N actions in one cycle. **Mitigation:** `UpdateResult::actions_vec` is the supported path; verify the action runner processes the batch sequentially without races on `SessionManager`.

### Animation (shimmer + spinner)
- **Risk:** Redraw cost / CPU from per-character styling each tick. **Mitigation:** animate only short status strings and small spinner glyphs; the 20 fps tick already drives redraws and the work is O(status length). No new timers.
- **Risk:** Non-true-color terminals. **Mitigation:** `lerp` falls back gracefully (ratatui/crossterm already down-convert RGB to 256-color); spinner is plain glyphs and unaffected.
- **Risk:** Frame counter `u64` wrap. **Mitigation:** `wrapping_add`; all consumers use modulo.

### Jump-to-latest
- **Risk:** Indicator overlapping log content or link badges. **Mitigation:** render last, right-aligned, minimal width; hide when the area is too narrow.
- **Risk:** Unseen counter drift vs. ring-buffer eviction. **Mitigation:** the count is advisory ("12 new"); clamp/treat as best-effort and always reset on follow.

### Runnable-device filtering
- **Risk:** Older/abbreviated `flutter devices --machine` payloads (or daemon `device.added` events) omit `isSupported`, causing devices to disappear. **Mitigation:** serde `default = true` — absence means "assume runnable," so filtering only ever *removes* explicitly-unsupported devices.
- **Risk:** Over-filtering hides a device the user expected. **Mitigation:** consider "dim + non-selectable" instead of hard-exclude, and always show the actionable empty state explaining *why* the list is empty.
- **Risk:** Daemon-sourced `device.added` events (separate from one-shot discovery) may not carry the flag. **Mitigation:** default-true keeps them visible; treat discovery-time filtering as the authoritative path.

### Reload flash
- **Risk:** Flash competes visually with the shimmer or an error state. **Mitigation:** short decay (~500 ms), low blend factor; suppress when the session is in an error/failed phase.

---

## Keyboard Shortcuts Summary

| Key | Context | Action |
|-----|---------|--------|
| `Space` | New-session dialog (Connected tab) | Toggle device selection |
| `a` | New-session dialog | Select all / clear all |
| `Enter` | New-session dialog | Launch all checked (or cursor device if none checked) |
| `G` / `End` | Log view | Jump to latest & follow (existing; now advertised by the indicator) |

---

## Success Criteria

### Phase 0 Complete When:
- [ ] `AppState::animation_frame` increments on every tick irrespective of `UiMode`, covered by a unit test.

### Phase 1 Complete When:
- [ ] User can check ≥2 connected devices and launch all with one `Enter`.
- [ ] Zero checked → existing single-device behavior is unchanged.
- [ ] Already-active and over-cap devices are skipped with a clear summary; partial success still launches the rest.
- [ ] State logic (toggle, select-all, checked accessor, stale-id pruning) is unit-tested.

### Phase 2 Complete When:
- [ ] Transient status labels shimmer while their condition holds and are static otherwise.
- [ ] `shimmer_phase` / `lerp` have unit tests (phase wrap, endpoint colors, non-RGB fallback).

### Phase 2.5 Complete When:
- [ ] `AppPhase` gains `Preparing` and `Launching`; all exhaustive matches (`phase_indicator`, `status_icon`) and the phase-coverage test array are updated.
- [ ] A freshly launched session shows `Launching` (not `Running`) until the `app.started` daemon event arrives; `app.started` is the sole trigger for `Running` on initial launch.
- [ ] Sessions with `start_before_app` pre-app sources show `Preparing` while `ready_check` polls, before Flutter spawns.
- [ ] The `Launching`/`Preparing` labels shimmer (reuse Phase 2) and render in `STATUS_BLUE`; `Running`/`Stopped`/`Reloading` are visually unchanged.
- [ ] Live build/readiness progress text is shown next to the label and cleared when `Running` is reached.
- [ ] Hot reload/restart remain no-ops while `Preparing`/`Launching`; `is_busy` still matches `Reloading` only (no "Reloading" mislabel of the new phases).
- [ ] `cargo test --workspace`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings` pass.

### Phase 3 Complete When:
- [ ] Startup screen uses the shared spinner with no visual regression.
- [ ] Dialog discovery/refresh shows an animated spinner; concurrent spinners are in phase.
- [ ] `spinner_char` advances deterministically per frame (unit-tested).

### Phase 4 Complete When:
- [ ] Scrolling up during streaming shows a "N new / jump" indicator; following hides it.
- [ ] `unseen_log_count` increments only while not following and resets on follow/jump (unit-tested).
- [ ] `G` / `End` clears the indicator and returns to the tail.

### Phase 5 Complete When:
- [ ] `Device` captures `is_supported` (default true); explicitly-unsupported connected devices are excluded (or disabled) in the dialog.
- [ ] Multi-select cannot check an unsupported device.
- [ ] All-filtered-out shows an actionable empty state; parsing is unit-tested (present/absent/false flag).

### Phase 6 Complete When:
- [ ] A successful hot reload briefly tints the header green and fades within ~500 ms.
- [ ] The flash is driven by `last_reload_time` via the tick loop (no new timer) and suppressed in error phases.

### Phase 6.5 Complete When:
- [ ] The status-label shimmer sweeps in from off-screen, exits off-screen, and has a visible rest gap between cycles (no pop-in/snap-back); the change is confined to `shimmer_spans` and shared by all call sites.
- [ ] `shimmer_spans` unit tests are updated for the new sweep range and falloff (`3.5`) and pass.
- [ ] The bottom status bar shows the braille spinner in place of the static icon for `Initializing`, `Preparing`, and `Launching` only; `Reloading` (`↻`), `Quitting` (`✗`), `Running` (`●`), and `Stopped` (`○`) keep their static icons.
- [ ] The status-bar spinner advances in unison with the new-session dialog spinner (same `SPINNER_TICKS_PER_FRAME` cadence off the global `animation_frame`).
- [ ] `cargo test --workspace`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings` pass.

---

## Future Enhancements

- Boot-then-launch for multi-selected bootable simulators/AVDs.
- Remember last multi-selection for one-key relaunch of a device set.
- Surface per-device `capabilities` (e.g. grey out hot-reload where unsupported) using the flags captured in Phase 5.
- Configurable shimmer/spinner speed and an "animations off" accessibility setting.

---

## References

- Issue #31 — "[Nice To Have] - Jump to end log line"
- `docs/KEYBINDINGS.md` — current scroll/log bindings
- `docs/CODE_STANDARDS.md` — module split >500 lines, fn >50 lines; responsive-layout guidelines
