# Bugfix Plan: Cache-only auto-launch should be opt-in (re-gate `last_device`)

**Status:** Draft — awaiting user approval before writing TASKS.md
**Owner:** ed
**Related, in priority order:**
- Behavior we want to revert/repair: [`c5879fa`](../../../..) "fix(startup): broaden auto-launch gate to use cached last_device (#35 followup) (#36)"
- Originator of the cache-fires-auto-launch path: [`workflow/plans/features/consolidate-launch-config/PLAN.md`](../../features/consolidate-launch-config/PLAN.md) §6 (Option B, recommended path)
- Sibling bug fix in flight: [`workflow/plans/bugs/launch-toml-device-ignored/BUG.md`](../launch-toml-device-ignored/BUG.md) (matcher + headless wiring — out of scope here, complementary)

---

## TL;DR

Right now, fdemon auto-launches whenever `settings.local.toml` has a non-empty `last_device` — even if **no** `auto_start = true` exists in `launch.toml` and the user never explicitly opted into auto-launch. The user's `example/app2` repro demonstrates this: with no `auto_start` anywhere, fdemon still picks up the cached iOS device id and launches it. We want to **re-gate** the cache path behind a new explicit opt-in (`[behavior] auto_launch = true` in `config.toml`). When that flag is off (the default), the cache becomes "remember last device for the dialog" again — not a launch trigger. `launch.toml`'s per-config `auto_start = true` continues to win unconditionally.

---

## Reported Symptom

Running `fdemon` from `example/app2`:

- `.fdemon/launch.toml` declares two configurations, **both with `device = "auto"` and no `auto_start = true`**.
- `.fdemon/config.toml` defines no auto-launch flag (today's `BehaviorSettings` has only `confirm_quit`).
- `.fdemon/settings.local.toml` contains a non-empty `last_device = "00008110-…"` (iOS device id from a prior run).

Observed: fdemon **auto-launches** a session on the cached iOS device, skipping the New Session dialog. The user did not ask for this — they expect "no auto_start anywhere → show me the dialog."

The user's intent, verbatim:
> *"launch.toml takes priority. If `auto_launch = true` is defined in a configuration we go by that. If `auto_launch = true` is defined in `config.toml` we then use what is in `settings.local.toml`, otherwise we default to the first device on the list of available devices."*

---

## Reproduction

1. From a clean checkout of `example/app2`, ensure:
   ```
   .fdemon/launch.toml         # no auto_start
   .fdemon/config.toml         # no [behavior] auto_launch (field doesn't exist today)
   .fdemon/settings.local.toml # last_device = <some real connected device id>
   ```
2. Run `fdemon` from that directory with the cached device connected.
3. **Observed:** session auto-launches on the cached device.
4. **Expected (per user):** New Session dialog appears. The cached `last_device` may be pre-selected as the default highlight, but no session is auto-spawned.

---

## Root Cause

Two places, one decision:

### Gate (TUI)

`crates/fdemon-tui/src/startup.rs:49-73` — `startup_flutter()`:

```rust
let has_auto_start_config = get_first_auto_start(&configs).is_some();
let cache_trigger        = !has_auto_start_config && has_cached_last_device(project_path);

if has_auto_start_config || cache_trigger {
    return StartupAction::AutoStart { configs };
}
```

The `cache_trigger` branch was added by `c5879fa` to restore "remember my last device" UX after `[behavior] auto_start` was deleted in v0.5.0. The new gate is **too eager** — any non-empty `last_device` (which is now written by both auto-launches *and* manual dialog launches per Task 02 of the consolidation work) becomes a launch trigger forever, with no way to opt out short of deleting the file.

### Selection (TUI + headless)

`crates/fdemon-app/src/spawn.rs:215-243` — `find_auto_launch_target()`:

```
Tier 1: launch.toml auto_start = true        (resolves device via matcher)
Tier 2: settings.local.toml last_device       ← currently the offender when reached unintentionally
Tier 3: first launch config + first device
Tier 4: bare flutter run
```

Once the gate fires, Tier 2 unconditionally consumes the cached device. The cascade itself is fine — the **gate** is the problem. The cache path needs an explicit opt-in.

### Headless

`src/headless/runner.rs:237-296` — `headless_auto_start()`: bypasses `launch.toml` entirely and unconditionally takes the first device. This is being addressed in the sibling bug (`launch-toml-device-ignored` Task 03). Our plan must coordinate with it: **whatever gate we install must also apply to the headless path**, or the headless mode will still ignore the user's intent.

---

## Affected Code Map

| File | Line(s) | Issue / Change |
|------|---------|---------------|
| `crates/fdemon-app/src/config/types.rs` | 155-167 | `BehaviorSettings` needs a new `auto_launch: bool` (default `false`) |
| `crates/fdemon-tui/src/startup.rs` | 26-72 | `cache_trigger` must require `settings.behavior.auto_launch == true` |
| `crates/fdemon-app/src/spawn.rs` | 215-243 | `find_auto_launch_target` Tier 2 must be skipped when caller did not opt in (cleanest: pass a `cache_allowed: bool` param) |
| `src/headless/runner.rs` | 237-296 | Apply the same gate; reuse `find_auto_launch_target` after sibling fix lands |
| `crates/fdemon-app/src/settings_items.rs` | (Behavior section) | Surface the new `auto_launch` toggle in the Settings Panel "Project" / "Behavior" tab |
| `crates/fdemon-app/src/config/settings.rs` | save/load paths | Persist `auto_launch` through `save_settings` |
| `docs/CONFIGURATION.md` | 183-216 | Rewrite the "Auto-Start Behavior" section: document the new opt-in, the priority cascade, and the "remember selection ≠ auto-launch" invariant |
| `example/app2/.fdemon/config.toml` | new line | Add a commented-out `auto_launch = false` example so users discover the knob |

---

## Proposed Behavior (matching the user's spec)

**Auto-launch fires only when one of these is true:**

1. **Per-config explicit intent** — any `launch.toml` configuration has `auto_start = true`. The configured `device` is resolved via the matcher (`fdemon-daemon` `Device::matches` + the alias fix in the sibling bug). If the device is not connected, fall back to the first available device (today's behavior, kept).
2. **Global cache opt-in** — `[behavior] auto_launch = true` in `config.toml` AND a valid (connected) `last_device` is cached in `settings.local.toml`. If the cached device is no longer connected (or `last_device` is missing), fall back to the first available device.

**Otherwise:** show the NewSessionDialog. The cached `last_device` may still be used to **pre-select** a default in the dialog (UX nicety, not a launch trigger).

### Selection priority (when the gate fires)

| # | Trigger | Device source | Config source |
|---|---------|---------------|---------------|
| 1 | `launch.toml` config has `auto_start = true` | config's `device` field (matcher → first if unmatched) | that auto_start config |
| 2 | `[behavior] auto_launch = true` AND valid cache | `last_device` from `settings.local.toml` | `last_config` (if still valid), else first config |
| 3 | `[behavior] auto_launch = true` AND cache stale/missing | first available device | first launch config (if any) |
| 4 | (Internal — only reached if `launch.toml` is empty) | first available device | none (bare `flutter run`) |

Tiers 3 and 4 only become reachable from the gate when `[behavior] auto_launch = true`. They are unreachable from a "no opt-in" startup, which then shows the dialog.

### "What if both are true?"

`launch.toml`'s `auto_start = true` **always wins** over `[behavior] auto_launch`. They are not contradictory — `auto_launch` only governs the cache fallback path; explicit per-config intent always takes precedence.

### "What if `launch.toml` has `auto_start = false` everywhere AND `[behavior] auto_launch = true`?"

Tier 1 doesn't fire (no `auto_start = true`). Tier 2 fires using cached `last_device`, or Tier 3 falls back to first device. This matches the user's spec.

### "What if neither flag is set?"

No auto-launch. Show dialog. The cache continues to be **written** when the user picks something in the dialog, so the next time `auto_launch = true` is enabled, the cache will be ready. Cache writes remain symmetric (both auto-launch and manual launches write `last_device`/`last_config`).

---

## Naming & Compatibility Notes

- The user's spec used **`auto_launch`** for the new flag. The existing per-config field on `LaunchConfig` is **`auto_start`** (already shipped on disk in real users' `launch.toml` files). We keep the per-config name as `auto_start` to avoid a breaking rename, and use **`auto_launch`** as the new global flag — matching the user's wording and avoiding collision with the deprecated `[behavior] auto_start` warning emitted in v0.5.0.
- **`[behavior] auto_start`** was removed in v0.5.0 (CONFIGURATION.md §247). The deprecation warning still fires when the flag is seen. **`[behavior] auto_launch`** is a *new* field, not a revival — the deprecation warning for the old name stays.
- `settings.local.toml` schema is unchanged. `last_device` / `last_config` keep their meaning. We are tightening the *gate* that consumes them, not the file format.
- Existing users who **want** today's "auto-launch on cache" behavior add one line to `config.toml`:
  ```toml
  [behavior]
  auto_launch = true
  ```
  Existing users who have stale `last_device` files and never wanted auto-launch get back the dialog by default. This is the desired UX.

---

## Out of Scope

- The matcher fix and headless `find_auto_launch_target` wiring — owned by the sibling bug `launch-toml-device-ignored`. Our plan **depends** on Task 03 of that bug landing (or is co-merged) so the headless path can share the same gate. If the sibling lands later, we still gate the headless side ourselves.
- Renaming `LaunchConfig.auto_start` → `LaunchConfig.auto_launch`. Out of scope; would require migrating real users' `launch.toml` files.
- Any change to `find_auto_launch_target`'s Tier 1 matcher behavior. Tier 1 is unchanged.
- Adding a "clear last selection" button to the Settings Panel. Nice-to-have follow-up.
- Deprecation of `settings.local.toml`. We keep it; only the gate changes.

---

## Verification

- **Unit:** new tests in `crates/fdemon-tui/src/startup.rs` covering:
  - G1 (cache present, `auto_launch = false` default) → `Ready` (dialog shown). *This is the user's repro — currently asserts `AutoStart`; the test must be updated.*
  - G2 (cache present, `auto_launch = true`) → `AutoStart`.
  - G3 (cache present, `auto_launch = false`, but `auto_start = true` in launch.toml) → `AutoStart`.
  - G4 (no cache, `auto_launch = true`) → `AutoStart` (will fall through to Tier 3 in spawn).
  - G5 (no cache, no `auto_launch`, no `auto_start`) → `Ready`.
- **Unit:** new test in `crates/fdemon-app/src/spawn.rs` for `find_auto_launch_target` parameterized on `cache_allowed` (or whatever signal we choose) so Tier 2 is skipped when the gate disallows cache.
- **Headless:** new test asserting the same gate applies — `auto_launch = false` + cached device + no `auto_start` config → headless does *not* auto-spawn (or whatever the chosen headless semantic is — see Open Question 2).
- **Manual smoke:**
  1. In `example/app2` (config.toml unchanged from this PR, no `auto_launch` line) → fdemon shows the New Session dialog. Cache is still written when the user picks a device.
  2. Add `[behavior] auto_launch = true` to `example/app2/.fdemon/config.toml` → fdemon auto-launches on cached `last_device`.
  3. Add `auto_start = true` to a launch config and re-run → that config wins regardless of `auto_launch`.
- **Quality gate:**
  ```bash
  cargo fmt --all -- --check && \
    cargo check --workspace --all-targets && \
    cargo test --workspace && \
    cargo clippy --workspace --all-targets -- -D warnings
  ```

---

## Coordination With Sibling Bug (`launch-toml-device-ignored`)

That bug introduces three changes:
1. Matcher alias `"macos" ↔ "darwin"` in `Device::matches` — independent, no conflict.
2. User-visible warning when the matcher misses — independent, no conflict.
3. Wire `launch.toml` into headless via `find_auto_launch_target` — **directly overlaps with our headless gate change**.

Recommended merge order:
- Sibling Task 01 (matcher) — anytime.
- Sibling Task 02 (warning + `pub` visibility) — anytime.
- **Sibling Task 03 (headless wiring) MERGED FIRST**, then this plan's headless gate is layered on top. If our work lands first, we duplicate work and risk a merge conflict in `src/headless/runner.rs`.

If the sibling has not merged when we start, our headless task either (a) waits, or (b) re-implements the wiring inline and the sibling Task 03 becomes a no-op. Prefer (a).

---

## Decisions (resolved with user)

1. **Naming:** `[behavior] auto_launch` — confirmed.
2. **Headless default:** option **(b)** — headless keeps today's "always auto-launch with first device" semantic. After the sibling bug's Task 03 lands, this means: headless honors `launch.toml`'s `auto_start = true` (Tier 1) when present, otherwise falls back to first available device. **Cache is never consulted in headless** — `cache_allowed = false` is hard-wired at the headless call site, regardless of `[behavior] auto_launch`. No CI/script breakage.
3. **Settings Panel UX:** the `auto_launch` toggle gets its own row in the Behavior section (alongside the existing `confirm_quit` row).
4. **`example/app2` fixture:** add a commented-out `# auto_launch = true` line in `example/app2/.fdemon/config.toml` as discoverability bait. Default behavior is unchanged (commented out → false).
5. **Migration messaging:** emit a one-time `info!` log (file-based logger, not stdout) the first time fdemon runs against a project with a non-empty cached `last_device` *and* no `[behavior] auto_launch` set, explaining the new opt-in. Helps users who were quietly relying on the `c5879fa` behavior.

---

## Preview of Task Shape (NOT a full breakdown — drafted after approval)

Likely 5 tasks, 4 of them worktree-isolatable:

1. **Add `[behavior] auto_launch` to `BehaviorSettings`** + serde default + Settings Panel surface. Files: `config/types.rs`, `config/settings.rs`, `settings_items.rs`. Tests: round-trip serde, default value.
2. **Re-gate TUI startup** in `crates/fdemon-tui/src/startup.rs`. `cache_trigger` requires `settings.behavior.auto_launch == true`. Update existing G1/G2/G3 tests; add G4/G5. Reads from Task 1's new field.
3. **Skip Tier 2 when cache disallowed** in `crates/fdemon-app/src/spawn.rs`. Cleanest API: add `cache_allowed: bool` parameter to `find_auto_launch_target`. Update unit tests. Reads from Task 1's new field via the call site.
4. **Apply gate to headless** in `src/headless/runner.rs`. **Depends on** sibling `launch-toml-device-ignored` Task 03 (or implements equivalent inline if sibling not yet merged). Add headless test for the gate.
5. **Docs + example** — `docs/CONFIGURATION.md` rewrite of Auto-Start Behavior; `example/app2/.fdemon/config.toml` discoverability comment. **Routed to `doc_maintainer`** for the CONFIGURATION.md rewrite if it crosses managed-doc territory (CONFIGURATION.md is *not* in the doc_maintainer-only list per `docs/DEVELOPMENT.md`, so the implementor can edit it).

Tasks 1, 5 can run in parallel. Tasks 2, 3 read from Task 1's struct change → must run after Task 1. Task 4 also reads Task 1 and overlaps with the sibling bug. Full overlap matrix and dependency graph in TASKS.md once approved.
