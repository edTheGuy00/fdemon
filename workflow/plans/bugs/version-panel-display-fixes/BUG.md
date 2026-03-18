# Bugfix Plan: Version Panel Display Fixes

## TL;DR

Three display issues in the Flutter Version Panel: (1) SDK info fields clip at small terminal sizes because the vertical layout height is too short and there's no scrolling, (2) SDK info shows "unknown" version and is missing framework revision, engine hash, and DevTools version because detection is file-only and never runs `flutter --version --machine`, (3) the "SDK Info" tab label disappears when unfocused instead of switching to a dimmed style.

## Bug Reports

### Bug 1: SDK Info Section Clipped at Small Terminal Sizes

**Symptom:** When the terminal is at or near minimum size, the SDK Info pane content is cut off. Fields like DART SDK may be invisible. In vertical (stacked) layout mode, only 6 rows are allocated for SDK info, which is insufficient for the full field grid.

**Expected:** All SDK info fields should be visible at any supported terminal size. If space is truly insufficient, a compact layout variant should show the most important fields.

**Root Cause Analysis:**
1. In vertical layout mode, `VERTICAL_SDK_INFO_HEIGHT` is hardcoded to `6` rows (`mod.rs:70`). The SDK info grid requires: 1 label row + 2 (VERSION/CHANNEL) + 1 spacer + 2 (SOURCE/PATH) + 1 spacer + 2 (DART SDK) = 9 rows minimum when focused. At 6 rows, the last field group and spacer are clipped.
2. The SDK info pane has no scrolling or compact mode — content below the allocated height is silently clipped by Ratatui's buffer bounds.
3. Even in horizontal mode, the content pane area is `Min(5)` which can be as low as 5 rows when the terminal is small.

**Affected Files:**
- `crates/fdemon-tui/src/widgets/flutter_version_panel/mod.rs` — `VERTICAL_SDK_INFO_HEIGHT` constant
- `crates/fdemon-tui/src/widgets/flutter_version_panel/sdk_info.rs` — field layout, no compact mode

---

### Bug 2: SDK Info Incomplete — Missing Version and Extended Metadata

**Symptom:** The VERSION field shows "unknown" instead of the actual Flutter version (e.g., "3.38.6"). The SOURCE field shows "system PAT" (truncated "system PATH"). The panel is missing framework revision, engine hash, and DevTools version that `flutter --version` displays.

**Expected:** The panel should display the complete SDK info matching `flutter --version` output:
- Flutter version: 3.38.6
- Channel: stable
- Framework revision: 8b87286849
- Engine hash: 6f3039bf7c (short) or full hash
- Dart SDK version: 3.10.7
- DevTools version: 2.51.1

**Root Cause Analysis:**
1. **"unknown" version**: The SDK was found via the lenient PATH fallback (strategy 11 in `locator.rs:199-222`), where `read_version_file()` fails because the `VERSION` file is missing at the canonicalized SDK root (common with Homebrew/snap installs). The fallback literal `"unknown"` is stored in `FlutterSdk.version`.
2. **Missing fields**: `FlutterSdk` (`types.rs:88-99`) has only 5 fields: `root`, `executable`, `source`, `version`, `channel`. Framework revision, engine hash, and DevTools version don't exist in the data model. No code ever runs `flutter --version` or parses the machine-readable JSON output.
3. **SOURCE truncation**: "system PATH" is rendered in a `Constraint::Percentage(40)` column (`sdk_info.rs:102`), which on narrow panes can truncate text. This is a display width issue, not a data issue.

**Affected Files:**
- `crates/fdemon-daemon/src/flutter_sdk/types.rs` — `FlutterSdk` struct, needs new fields
- `crates/fdemon-daemon/src/flutter_sdk/locator.rs` — Strategy 11 "unknown" fallback
- `crates/fdemon-daemon/src/flutter_sdk/mod.rs` — module re-exports
- `crates/fdemon-app/src/flutter_version/state.rs` — `SdkInfoState`, needs extended metadata
- `crates/fdemon-app/src/message.rs` — new message for async version probe result
- `crates/fdemon-app/src/handler/flutter_version/` — handler for version probe result
- `crates/fdemon-app/src/actions/mod.rs` — new action to spawn `flutter --version --machine`
- `crates/fdemon-tui/src/widgets/flutter_version_panel/sdk_info.rs` — render extended fields

---

### Bug 3: SDK Info Tab Label Disappears When Unfocused

**Symptom:** When pressing Tab to switch focus from "SDK Info" to "Installed Versions", the "SDK Info" label in the left pane disappears entirely instead of remaining visible in a dimmed/unselected style.

**Expected:** The "SDK Info" label should always be visible — styled with `ACCENT + BOLD` when focused and `TEXT_SECONDARY` when unfocused, matching how the "Installed Versions" header behaves.

**Root Cause Analysis:**
1. In `SdkInfoPane::render()` (`sdk_info.rs:155-184`), the "SDK Info" label is only rendered inside the `if self.focused` branch. The `else` branch renders content directly into the full area with no label at all.
2. Compare with `VersionListPane::render_list_header()` (`version_list.rs:111-139`), which always renders the "Installed Versions" header and only varies the style based on focus.
3. The fix is straightforward: always render the "SDK Info" label, using focus-dependent styling (matching the version list pattern), and always consume a row for it.

**Affected Files:**
- `crates/fdemon-tui/src/widgets/flutter_version_panel/sdk_info.rs` — `SdkInfoPane::render()` focus branching

---

## Affected Modules

- `crates/fdemon-daemon/src/flutter_sdk/types.rs` — Add optional extended metadata fields to `FlutterSdk`
- `crates/fdemon-daemon/src/flutter_sdk/version_probe.rs` — **NEW** Async `flutter --version --machine` runner and JSON parser
- `crates/fdemon-daemon/src/flutter_sdk/mod.rs` — Re-export new module
- `crates/fdemon-app/src/flutter_version/state.rs` — Add extended metadata to `SdkInfoState`
- `crates/fdemon-app/src/message.rs` — Add `FlutterVersionProbeCompleted` message variant
- `crates/fdemon-app/src/handler/flutter_version/actions.rs` — Handle probe result message
- `crates/fdemon-app/src/handler/update.rs` — Route new message to handler
- `crates/fdemon-app/src/actions/mod.rs` — Add `ProbeFlutterVersion` action
- `crates/fdemon-tui/src/widgets/flutter_version_panel/sdk_info.rs` — Fix label, add extended fields, improve layout
- `crates/fdemon-tui/src/widgets/flutter_version_panel/mod.rs` — Fix vertical layout height constant

---

## Phases

### Phase 1: Tab Label Fix (Bug 3) — Quick Fix

The simplest fix: always render the "SDK Info" label with focus-dependent styling.

**Steps:**
1. Modify `SdkInfoPane::render()` to always render the "SDK Info" label (styled `ACCENT + BOLD` when focused, `TEXT_SECONDARY` when unfocused)
2. Always consume 1 row for the label and pass the reduced `content_area` to `render_sdk_details()`/`render_no_sdk()`
3. Add an underline separator below the label (matching the version list header pattern with `BORDER_DIM` style)
4. Update tests to verify label renders in both focused and unfocused states

**Measurable Outcomes:**
- "SDK Info" label visible in both focused and unfocused states
- Focused style matches `palette::ACCENT + BOLD`; unfocused matches `palette::TEXT_SECONDARY`
- Existing tests pass; new tests verify unfocused label rendering

---

### Phase 2: Layout Fix for Small Terminals (Bug 1) — Medium Fix

Fix the vertical layout clipping and ensure all fields are visible at supported terminal sizes.

**Steps:**
1. Increase `VERTICAL_SDK_INFO_HEIGHT` from `6` to accommodate the label + field grid (label header(2) + 2 field rows + 1 spacer + 2 field rows + 1 spacer + 2 field rows = 10)
2. Add a compact rendering mode to `SdkInfoPane` for very constrained heights: combine VERSION + CHANNEL on one line, SOURCE + PATH on another, DART on a third — no spacer rows
3. Use `area.height` to decide between compact and expanded field layout (following ARCHITECTURE.md Principle 1: decide based on available space, not orientation)
4. Ensure the `MIN_RENDER_HEIGHT` constant reflects the true minimum for useful display
5. Update tests for compact rendering

**Measurable Outcomes:**
- SDK info fields visible in both horizontal and vertical layouts
- No field clipping at `MIN_RENDER_HEIGHT` terminal size
- Compact mode activates gracefully when space is tight

---

### Phase 3: Extended SDK Metadata via `flutter --version --machine` (Bug 2) — Feature Fix

Add async version probing to populate missing metadata.

**Steps:**
1. Add optional fields to `FlutterSdk`: `framework_revision`, `engine_revision`, `devtools_version`, `repo_url`
2. Create `version_probe.rs` in `fdemon-daemon/src/flutter_sdk/` that runs `flutter --version --machine` and parses the JSON output:
   ```json
   {
     "frameworkVersion": "3.38.6",
     "channel": "stable",
     "repositoryUrl": "https://github.com/flutter/flutter.git",
     "frameworkRevision": "8b87286849",
     "frameworkCommitDate": "2026-01-08 10:49:17 -0800",
     "engineRevision": "6f3039bf7c...",
     "dartSdkVersion": "3.10.7",
     "devToolsVersion": "2.51.1"
   }
   ```
3. Add `Message::FlutterVersionProbeCompleted { result }` message variant
4. Add `UpdateAction::ProbeFlutterVersion` action that spawns the async probe
5. Trigger the probe when the panel opens (alongside existing `ScanInstalledSdks`)
6. Handle probe result in `handler/flutter_version/actions.rs` — merge extended metadata into `SdkInfoState`
7. Update the probe result to also fix "unknown" version: if `FlutterSdk.version` is "unknown" and the probe returns a real version, update it
8. Update `sdk_info.rs` to render the extended fields (framework revision, engine hash, DevTools version) in the layout grid
9. Make SDK PATH column width dynamic based on `area.width` rather than `MAX_PATH_WIDTH = 28`

**Measurable Outcomes:**
- Flutter version shows "3.38.6" instead of "unknown"
- Framework revision, engine hash, DevTools version are displayed
- Panel shows immediate file-based data on open, then enriches asynchronously when probe completes
- Probe failure is non-fatal — file-based data remains displayed
- Source field no longer truncated due to dynamic column widths

---

## Edge Cases & Risks

### Async probe timing
- **Risk:** `flutter --version --machine` can take 1-5 seconds on cold start; probe result arrives after user has already navigated away from the panel
- **Mitigation:** Probe result updates `SdkInfoState` regardless of current panel visibility; data is ready next time the panel opens. Show a "Loading..." indicator for fields that depend on probe data.

### Probe failure on PATH-inferred SDK
- **Risk:** If the SDK was found via lenient PATH fallback, `flutter --version --machine` may also fail (broken symlink, missing deps)
- **Mitigation:** All probe-sourced fields are `Option<String>`. Failure is logged but non-fatal. Display "—" for unavailable fields.

### Layout regression
- **Risk:** Increasing `VERTICAL_SDK_INFO_HEIGHT` may squeeze the version list in vertical mode
- **Mitigation:** The version list has `Min(5)` and handles scrolling. Extra SDK info rows are important for usability. Compact mode ensures graceful degradation.

### FlutterSdk struct change ripple
- **Risk:** Adding fields to `FlutterSdk` affects all construction sites (locator, cache_scanner, tests)
- **Mitigation:** New fields are all `Option<String>` and default to `None`. All existing construction sites add `..Default::default()` or explicit `None` values. Consider using `#[derive(Default)]` with a builder pattern.

---

## Further Considerations

1. **Probe caching**: Should we cache the `flutter --version --machine` output to avoid re-running on every panel open? Current plan: run once at engine startup (not per-panel-open), store in `AppState.resolved_sdk`.
2. **Startup probe vs panel-open probe**: Running at startup adds latency to the initial load; running on panel open means the first open may show stale/incomplete data. Recommendation: run async at engine startup with no blocking, and also re-probe when panel opens if version is still "unknown".

---

## Task Dependency Graph

```
┌──────────────────────────────┐
│  01-fix-tab-label            │
│  (Bug 3 — SDK Info label)    │
└──────────────────────────────┘

┌──────────────────────────────┐
│  02-fix-vertical-layout      │
│  (Bug 1 — layout/compact)   │
│  depends on: 01              │
└──────────┬───────────────────┘
           │
           ▼
┌──────────────────────────────┐     ┌──────────────────────────────┐
│  03-version-probe-backend    │     │  04-sdk-info-extended-fields │
│  (Bug 2 — probe runner)     │     │  (Bug 2 — TUI fields)       │
│  depends on: none            │     │  depends on: 01, 02          │
└──────────┬───────────────────┘     └──────────┬───────────────────┘
           │                                    │
           └──────────┬─────────────────────────┘
                      ▼
           ┌──────────────────────────────┐
           │  05-probe-wiring-and-display │
           │  (Bug 2 — message/handler)   │
           │  depends on: 03, 04          │
           └──────────────────────────────┘
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] "SDK Info" label is always visible with focus-dependent styling
- [ ] Label has underline separator matching the "Installed Versions" header
- [ ] No visual regression in focused state

### Phase 2 Complete When:
- [ ] SDK info fields are fully visible in vertical (stacked) layout
- [ ] Compact mode activates gracefully for constrained heights
- [ ] No field clipping at minimum supported terminal size

### Phase 3 Complete When:
- [ ] Flutter version shows actual version (not "unknown") for PATH-inferred SDKs
- [ ] Framework revision, engine hash, and DevTools version are displayed
- [ ] Probe runs asynchronously and enriches data without blocking UI
- [ ] Probe failure is non-fatal with graceful fallback
- [ ] SOURCE and SDK PATH fields use dynamic column widths
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

---

## Milestone Deliverable

The Flutter Version Panel displays complete, accurate SDK information at all terminal sizes, with consistent tab label behavior matching the rest of the UI. Users see the same level of detail as `flutter --version` directly in the TUI.
