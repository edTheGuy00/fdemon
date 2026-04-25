# Plan: Consolidate Launch / Startup Configuration

**Status:** Draft — awaiting user decision on Option A/B/C before task breakdown.
**Owner:** ed
**Related:**
- Bug that surfaced this: [../../bugs/launch-toml-device-ignored/BUG.md](../../bugs/launch-toml-device-ignored/BUG.md)
- Prior startup reworks: `workflow/plans/features/startup-flow-rework/`, `workflow/plans/features/startup-flow-consistency/`

---

## 1. Problem Statement

Today the "what auto-launches?" decision is split across **three files** with a priority order that makes `launch.toml` edits silently ineffective:

1. **`.fdemon/config.toml`** — `[behavior] auto_start` (global bool)
2. **`.fdemon/launch.toml`** — per-configuration `auto_start` + `device`
3. **`.fdemon/settings.local.toml`** — `last_device`, `last_config` (gitignored, auto-written)

User-reported symptom (exact quote):
> *"Every time we run fdemon it writes to settings.local.toml then when we update launch.toml it ignores it."*

This is correct. The sequence is:
- Run #1: user edits `launch.toml` → sets `auto_start = true` → fdemon auto-launches → on success, writes `last_device` + `last_config` to `settings.local.toml`.
- Run #2: fdemon reads `settings.local.toml` **first**, validates, returns → `launch.toml` is never re-consulted.
- User's `launch.toml` edits between runs are invisible until `settings.local.toml` is deleted.

Plus two discovered documentation/design defects:
- `[behavior] auto_start` is not the master toggle docs claim — any per-config `auto_start = true` bypasses it (`startup.rs:36`, `OR` not `AND`).
- CONFIGURATION.md's documented 5-step priority is stale; actual code has 3 tiers.

---

## 2. Current-State Inventory

All fields that affect auto-launch / device selection, with source of truth.

| Field | File | Struct / field | Default | Read by | Written by |
|-------|------|----------------|---------|---------|------------|
| `[behavior] auto_start` | `config.toml` | `BehaviorSettings::auto_start: bool` — `crates/fdemon-app/src/config/types.rs:160` | `false` | `startup_flutter()` — `crates/fdemon-tui/src/startup.rs:36` | `save_settings()` — `settings.rs:487` (Settings Panel → Project tab) |
| per-config `auto_start` | `launch.toml` | `LaunchConfig::auto_start: bool` — `types.rs:46` | `false` | `get_first_auto_start()` — `config/priority.rs:93`; called from `startup.rs:35` and `spawn.rs:242-265` | Settings Panel → Launch tab |
| per-config `device` | `launch.toml` | `LaunchConfig::device: String` — `types.rs:22` | `"auto"` (`default_device()` — `types.rs:64`) | `find_auto_launch_target()` — `spawn.rs:246` | Settings Panel → Launch tab |
| `last_device` | `settings.local.toml` | `UserPreferences::last_device: Option<String>` — `types.rs:1273` | `None` | `load_last_selection()` — `settings.rs:780`; called from `spawn.rs:221` | `save_last_selection()` — `settings.rs:797`; **only** call site is `handler/update.rs:926-930` (AutoLaunchResult::Ok) |
| `last_config` | `settings.local.toml` | `UserPreferences::last_config: Option<String>` — `types.rs:1277` | `None` | Same as `last_device` | Same as `last_device` |

`UserPreferences` also holds `editor`, `theme`, `window` — unrelated to startup, out of scope for this plan.

### Actual `find_auto_launch_target` priority (`spawn.rs:215-275`)

```
Priority 1 (221-239): settings.local.toml → last_device/last_config
  Success condition: device_id matches a discovered device's Device.id
  Config name match is optional; device match is mandatory
Priority 2 (242-265): first launch config with auto_start = true
  or_else → first launch config at all
Priority 3 (267-274): devices.first(), no config (bare flutter run)
```

The auto-launch chain is gated by `startup.rs:36`:
```rust
if has_auto_start_config || behavior_auto_start { /* run chain */ }
```
→ **OR, not AND.** `[behavior] auto_start = false` does not disable auto-launch if any per-config has `auto_start = true`.

### Who persists what, when

| Event | Writes `settings.local.toml` (last_device/config)? |
|-------|---------------------------------------------------|
| Auto-launch succeeds | **Yes** — `handler/update.rs:926` |
| User picks from NewSessionDialog | **No** — `handler/new_session/launch_context.rs:404-577` has no save call |
| User quits / exits | No |
| Settings Panel save | No (saves unrelated fields) |

The asymmetry means: auto-launch seeds the file; manual launches never update it. Once seeded, the file silently overrides launch.toml on future runs.

### Compatibility surface of `settings.local.toml`

- `UserPreferences` struct has **no `#[serde(deny_unknown_fields)]`** — removing fields from the struct won't crash on existing files; extra keys are silently dropped.
- Settings Panel "User Preferences" tab shows `last_device` / `last_config` as **readonly** display items (`settings_items.rs:389-404`). Removing them from the struct auto-removes them from the UI.

---

## 3. Why `settings.local.toml` Exists — Original Intent

Per `workflow/plans/features/startup-flow-consistency/PLAN.md` (the source of the current implementation):
- The goal was a **frictionless re-entry** experience — if you auto-launched on iPhone yesterday, launch on iPhone today without re-prompting.
- The intent was NOT "override launch.toml forever" — that's an emergent bug, not a design choice.
- The feature's value is real when a user's fleet of devices changes across runs (e.g. unplug one phone → it would have been picked last time → remember it → auto-reconnect when plugged back in).

**Implication:** pure deletion of `last_device`/`last_config` loses legitimate UX value. We want to preserve "remember last selection" but fix the override semantics.

---

## 4. Design Principles (proposed)

Before picking an option, align on these:

1. **`launch.toml` is intent; `settings.local.toml` is cache.** An explicit intent should always beat a cached observation.
2. **Manual selection must be rememberable too.** If we remember at all, NewSessionDialog choices must be saved — today's asymmetry is a bug.
3. **One-file-per-purpose stays.** `config.toml` = global app behavior, `launch.toml` = launch configs, `settings.local.toml` = user-local overrides + ephemeral cache. Collapsing into one TOML file would mix gitignored and team-shared content, which is worse.
4. **Documented priority must match the code.** Whichever option we pick, CONFIGURATION.md gets rewritten.

---

## 5. Options

### Option A — Launch.toml-only (minimal, destructive)
**Summary:** Delete `last_device` / `last_config` from `UserPreferences`. Remove Priority 1. Remove `save_last_selection` call site.

| | |
|---|---|
| Pros | Simplest mental model. Every run reads `launch.toml`, full stop. Zero silent overrides. Directly matches the user's stated preference ("only launch.toml"). |
| Cons | **Loses the remember-last-selection UX.** Users who use the manual dialog pay re-selection cost every run. For users who rely on auto_start in launch.toml, no regression — it's only the manual-dialog path that feels worse. |
| Files touched | `types.rs` (remove 2 fields), `spawn.rs` (remove Priority 1 block, lines 221-239), `settings.rs` (remove `LastSelection`, `load_last_selection`, `save_last_selection`, `validate_last_selection`), `handler/update.rs:926-930` (remove save call), `settings_items.rs:389-404` (remove readonly display), CONFIGURATION.md (rewrite priority section). |
| Migration | `settings.local.toml` files that still contain `last_device` / `last_config` load silently (no `deny_unknown_fields`). No explicit migration needed. |

### Option B — Invert priority + symmetric persistence (recommended)
**Summary:** `launch.toml`'s `auto_start = true` strictly beats `settings.local.toml`. Cache only kicks in as a tiebreaker (no `auto_start` set → remember last manual choice). Also fix the persistence asymmetry so manual dialog selections are saved.

New priority order:

```
1. Launch config with auto_start = true (launch.toml)  ← explicit intent wins
2. settings.local.toml last_config + last_device (if both still valid)
3. first launch config + first device (fallback)
4. bare flutter run (no configs at all)
```

Plus: `handle_launch` in the NewSessionDialog path calls `save_last_selection` on success, so manual choices persist too.

| | |
|---|---|
| Pros | Fixes the user's actual bug — launch.toml edits always take effect. Preserves remember-last-selection UX for users who prefer the dialog. Fixes the persistence asymmetry. Smallest behavioral surface change — no file removals, no migration concerns. |
| Cons | Keeps three files. User's stated preference was "fewer files" — this doesn't reduce file count, only fixes semantics. Still two places to look when debugging "why did fdemon pick this device?". |
| Files touched | `spawn.rs:215-275` (swap Priority 1 ↔ Priority 2 blocks), `handler/new_session/launch_context.rs:~540` (add `save_last_selection` call on successful manual launch), CONFIGURATION.md (rewrite priority section + correct the `[behavior] auto_start` gate docs). |
| Also fixes | The separate `[behavior] auto_start` master-toggle doc bug. Since the gate is already an OR and any per-config wins, we can either (i) keep OR and rewrite the docs to match, or (ii) remove `[behavior] auto_start` entirely since per-config `auto_start` is sufficient. I lean toward (ii) — it's a redundant knob. |

### Option C — Collapse everything into launch.toml
**Summary:** Move `last_device`/`last_config` into a dedicated section of `launch.toml` (e.g. `[last_used] config = "Dev", device = "..."`), gitignore only that section, delete `settings.local.toml` for launch purposes.

| | |
|---|---|
| Pros | Literally one file for "launch stuff". Matches the user's stated preference most directly. |
| Cons | **Can't gitignore a section of a tracked file.** Either (a) the ephemeral `[last_used]` gets committed — terrible for team workflows, or (b) we split launch.toml into two files anyway — back to square one. Also mixes team-shared intent and user-local cache in one file — anti-pattern. |
| Verdict | Fundamentally incompatible with git. Not recommended. |

### Option D — Hybrid (A for auto_start, B for manual)
**Summary:** If any launch.toml config has `auto_start = true`, launch.toml wins absolutely (cache ignored). If no auto_start is set, use cache as today. User gets both: "I declared intent → honor it" AND "I'm clicking through the dialog → remember my choice."

This is effectively **Option B's priority order**, phrased differently. Listing separately for completeness — prefer Option B's framing.

---

## 6. Recommendation

**Option B with `[behavior] auto_start` removed.**

Reasoning:
- **Fixes the actual user bug** — `launch.toml` `auto_start = true` becomes a hard override that cache can never beat. Editing launch.toml between runs always works.
- **Preserves dialog UX** — users who never set `auto_start` in launch.toml still get "remember my last selection."
- **Fixes the persistence asymmetry bug** — manual dialog selections get saved, so the cache actually reflects the user's last real choice (today it only reflects their last *auto*-choice, which is a rare event).
- **Drops the redundant `[behavior] auto_start`** — it's an OR gate that any per-config `auto_start` already bypasses; removing it simplifies both the code and the docs. One-time migration: warn and ignore (don't fail) if the field is present.
- **No data migration, no config file removal** — behaviorally safe. Users just find things work the way they expected.

User can still choose Option A if they genuinely don't care about remembering dialog selections — say so and I'll re-plan. Option A is only ~30 more LOC of deletions than Option B.

---

## 7. Out of Scope

- Changing `LaunchConfig.device` from `String` to an enum (already declined in launch-toml-device-ignored/BUG.md).
- Redesigning the NewSessionDialog UX.
- Merging `config.toml` and `launch.toml` into one file.
- DAP-related launch config (`.vscode/launch.json`) — its precedence relative to `launch.toml` is unchanged.

---

## 8. Open Questions for the User

1. **Option A, B, or something else?** — see §6 recommendation.
2. **Drop `[behavior] auto_start` entirely?** It's redundant with per-config `auto_start` and its current OR-gate semantics contradict the docs. If yes, accept a minor breaking change for users who relied on the flag (emit a `warn!` on load and ignore).
3. **For Option B, should a mismatched device in `settings.local.toml` (saved Android, Android now disconnected) silently fall through to launch.toml's auto_start?** Today Priority 1 falls through on device-id miss (settings.rs:827-851) — we'd preserve that.
4. **Settings Panel implications:** the "User Preferences" tab currently shows `last_device` / `last_config` as readonly. For Option A we remove them; for Option B, consider adding a "Clear last selection" button.
5. **Testing coverage:** the bug we just hit should be a regression test. Add to `example/TESTING.md` a Test J: "With a stale settings.local.toml, editing launch.toml `auto_start` should be honored on next run." Worth including as part of whichever option ships.

---

## 9. If Option B Is Approved — Preview of Task Shape

(NOT a full task breakdown — that's drafted after approval.)

Likely 4 tasks, all worktree-isolatable:
1. `spawn.rs`: invert priority 1/2; extract `find_auto_launch_target` into tiered helper fns.
2. `handler/new_session/launch_context.rs`: add `save_last_selection` call on manual launch success.
3. `startup.rs` + `types.rs` + `settings.rs`: remove `[behavior] auto_start` (or keep + match docs — user choice per Q2).
4. `docs/CONFIGURATION.md`: rewrite the priority section end-to-end, correct the master-toggle claim, document symmetric persistence.

Add a doc-maintainer-routed ARCHITECTURE.md update if we remove `[behavior] auto_start` (module surface change).

---

## 10. If Option A Is Approved — Preview of Task Shape

Likely 3 tasks:
1. Remove `last_device`/`last_config` from `UserPreferences`, remove `LastSelection` + `load/save/validate_last_selection` from `settings.rs`, remove Priority 1 block from `spawn.rs`.
2. Remove save call site at `handler/update.rs:926`; remove readonly display in `settings_items.rs`.
3. `docs/CONFIGURATION.md`: rewrite priority section; remove the "User Preferences" section's `last_*` mentions.
