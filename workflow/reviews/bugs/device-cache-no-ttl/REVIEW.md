# Review: Device Cache Drops After 30s (Issue #33 follow-up)

**Review Date:** 2026-04-25
**Diff Range:** `06202d7..HEAD` (commits `5d52f80`, `9be6b9a`, `b530f7f`, `e96eb4f`, `aef9136`, `87ac4be`)
**Plan:** [`workflow/plans/bugs/device-cache-no-ttl/BUG.md`](../../../plans/bugs/device-cache-no-ttl/BUG.md)
**Tasks:** 6 (all merged)
**Files Changed:** 8 (+391 / -70)

---

## Verdict: ⚠️ NEEDS WORK

The fix is functionally correct for the stated symptom — cached devices now appear instantly regardless of elapsed time, and the `↻` indicator wires through cleanly. However, three independent reviewers (`bug_fix_reviewer`, `architecture_enforcer`, `logic_reasoning_checker`) flagged the same concrete logic gap that contradicts an explicit mitigation in BUG.md, plus a meaningful behavioural gap in the cache-miss path. These should be addressed before this is considered "done done."

| Reviewer | Verdict |
|---|---|
| `bug_fix_reviewer` | ✅ Approved with observations |
| `architecture_enforcer` | ✅ PASS |
| `code_quality_inspector` | ⚠️ NEEDS WORK |
| `logic_reasoning_checker` | ⚠️ CONCERN (one finding labeled Critical) |

Two CONCERN-tier verdicts → overall **NEEDS WORK** per the consolidation rule.

---

## Critical Findings (must fix)

### 🔴 C1. Background discovery failure leaves `refreshing` flag stuck

[Sources: `bug_fix_reviewer` Warning 2, `architecture_enforcer` Recommendation 3, `logic_reasoning_checker` Critical #1]

**File:** `crates/fdemon-app/src/handler/update.rs:405-428`

`Message::DeviceDiscoveryFailed { is_background: true }` only logs a warning. It never touches
`state.new_session_dialog_state.target_selector.refreshing`:

```rust
if is_background {
    tracing::warn!("Background device refresh failed: {}", error);  // ← stops here
} else {
    if state.ui_mode == UiMode::Startup || state.ui_mode == UiMode::NewSessionDialog {
        state.new_session_dialog_state.target_selector.set_error(error.clone());
    }
    tracing::error!("Device discovery failed: {}", error);
}
```

Since the new `RefreshDevicesAndBootableBackground` action calls
`spawn_device_discovery_background` (which sends `is_background: true` on failure), every transient
`flutter devices` failure leaves `refreshing = true` indefinitely. The `↻` glyph stays stuck on
the Connected tab until either (a) a subsequent successful discovery arrives, or (b) the dialog is
closed and reopened.

The BUG.md "Cache Becoming Severely Stale" section explicitly claims this case is handled:

> The `refreshing` indicator clears even on failure (handled by `set_error()` clearing `refreshing`).

This claim is false for the dominant failure path. `set_error()` is only reached in the
foreground branch.

**Required fix:** in the `is_background: true` arm, clear the flag (guarded by dialog visibility):

```rust
if is_background {
    tracing::warn!("Background device refresh failed: {}", error);
    if state.ui_mode == UiMode::NewSessionDialog || state.ui_mode == UiMode::Startup {
        state.new_session_dialog_state.target_selector.refreshing = false;
    }
}
```

---

## Major Findings (should fix)

### 🟠 M1. Cache-miss fallback never refreshes the bootable list

[Sources: `code_quality_inspector` MAJOR 2, `logic_reasoning_checker` Warning 4]

**File:** `crates/fdemon-app/src/handler/new_session/navigation.rs:258-261`

When **both** caches are empty, the handler dispatches only `UpdateAction::DiscoverDevices`,
which spawns a connected-device discovery only (`actions/mod.rs:75-77`). Bootable discovery is
not triggered — it relies on the one-shot `ToolAvailabilityChecked` message from engine startup
or on the user manually switching to the Bootable tab.

This contradicts the milestone deliverable in BUG.md:

> Both connected and bootable lists are refreshed on every dialog open, eliminating the latent
> bug where bootable devices were frozen after startup.

On a first dialog open after startup tool-availability has fired, the Bootable tab will sit at
its default `bootable_loading = true` state until the user switches tabs.

**Suggested fix:** in the cache-miss fallback, dispatch the combined refresh in addition to
the foreground `DiscoverDevices`, or extend the action to spawn both.

### 🟠 M2. `get_cached_bootable_devices()` clones the entire bootable cache on every call

[Source: `code_quality_inspector` MAJOR 1]

**File:** `crates/fdemon-app/src/state.rs:1261-1265`

```rust
pub fn get_cached_bootable_devices(&self) -> Option<(Vec<IosSimulator>, Vec<AndroidAvd>)>
```

The owned-return signature forces the function body to clone both Vecs unconditionally on every
dialog open. The connected-device equivalent (`get_cached_devices`) returns
`Option<&Vec<Device>>`, which makes the clone explicit at the call site (currently a single
`.clone()` in `navigation.rs:214`).

The asymmetry hides a non-trivial allocation in a path the BUG.md explicitly markets as
"shows the cached list instantly."

**Suggested fix:** change the return to
`Option<(&Vec<IosSimulator>, &Vec<AndroidAvd>)>` and clone at the single call site, mirroring
the connected-device pattern.

---

## Minor Findings (consider fixing)

### 🟡 m1. Missing test: `BootableDevicesDiscovered` clearing `bootable_refreshing`
[Source: `bug_fix_reviewer` Warning 1] — Task 04 added `test_devices_discovered_clears_refreshing`
for the connected side but no symmetric test was added for the bootable side. The mechanism is
correct by construction, but a regression introduced by adding a guard before
`set_bootable_devices()` would go undetected.

### 🟡 m2. Hardcoded `↻` glyph (no named constant)
[Source: `code_quality_inspector` MINOR 3] — `tab_bar.rs:71` and 6+ test assertions across
`tab_bar.rs` and `target_selector.rs` reference the literal `"↻"`. BUG.md's own
"Indicator Glyph Compatibility" risk recommends a possible swap to `*` or `…`. Today that swap
requires touching every test. A `const REFRESHING_GLYPH: &str = "↻"` makes it a one-liner.

### 🟡 m3. `set_error()` asymmetrically clears `refreshing` but not `bootable_refreshing`
[Sources: `code_quality_inspector` / `logic_reasoning_checker` Warning 1] — `set_error()` clears
`refreshing` and `loading` but leaves `bootable_refreshing` and `bootable_loading` untouched.
Today this is masked because `spawn_bootable_device_discovery` swallows errors via
`unwrap_or_default()`, so `set_error()` is never reached from the bootable path. But the
asymmetry has no comment justifying it; the next maintainer who routes a real bootable error
into `set_error()` will silently leave the spinner stuck. Either add a comment explaining the
asymmetry, or make it symmetric.

### 🟡 m4. Compact-mode rendering omits the `↻` indicator
[Sources: `bug_fix_reviewer` Observation 3, `architecture_enforcer` Recommendation 1] —
`render_tabs_compact` in `target_selector.rs` doesn't take or use the refreshing flags. Users on
short terminals get no visual cue that a refresh is in flight. The plan didn't explicitly spell
this out as in/out of scope, but it's a noticeable gap.

### 🟡 m5. Concurrent close+reopen can prematurely clear the indicator
[Source: `logic_reasoning_checker` Warning 3] — A discovery in flight when the user closes the
dialog will, on completion, call `set_connected_devices()` if the dialog is visible again,
clearing the new dialog's `refreshing` flag before its own discovery has finished. Convergence
is correct; the visual cue lies. Comment near the flag-set acknowledging the race would help
future readers.

### 🟡 m6. Stale `TODO: deduplicate with device_list::calculate_scroll_offset` in
`target_selector_state.rs:455`
[Source: `code_quality_inspector` MINOR 4] — Pre-existing, but it sits adjacent to new code and
flags known duplication.

### 🟡 m7. Dead branch in `handle_close_new_session_dialog`
[Source: `code_quality_inspector` MINOR 5] — Both arms of `if state.session_manager.has_running_sessions()`
in `navigation.rs` set `state.ui_mode = UiMode::Normal`; the comment says "stay in startup mode"
but doesn't. Pre-existing, but in modified-file scope.

### 🟡 m8. Indicator visibility on inactive tabs — confirm intent
[Source: `logic_reasoning_checker` Warning 2] — Implementation (and BUG.md Phase 2 §1) shows
`↻` on **both** tabs simultaneously when both flags are set, regardless of which is active.
The orchestration prompt phrased this as "active tab shows ↻". The implementation matches the
plan, but worth a quick decision: should inactive-tab refreshes be visible, or only active-tab?

---

## Nitpicks

- 🔵 n1. `RefreshDevicesAndBootableBackground { flutter: FlutterExecutable }` is a one-liner;
  sibling variants in the enum use multi-line form with a `///` doc on the inner field.
- 🔵 n2. Test assertion in `test_tab_bar_renders_bootable_refreshing_indicator` lacks the
  diagnostic message its sister test has.
- 🔵 n3. `cached_devices.clone()` in `navigation.rs` would read more clearly with `len`
  captured first (style only).

---

## Documentation Freshness Check

- `docs/ARCHITECTURE.md` — no new modules/crates added → no update needed
- `docs/DEVELOPMENT.md` — no new build steps/deps/commands → no update needed
- `docs/CODE_STANDARDS.md` — no new error types/macros/conventions → no update needed
- `docs/REVIEW_FOCUS.md` — `Cell<usize>` exception remains the only render-hint write-back; new
  `refreshing`/`bootable_refreshing` fields are plain `bool` and write through handlers normally
  → no update needed
- `BUG.md` "Cache Becoming Severely Stale" mitigation note (line 191) is **factually
  incorrect** as currently written — should be updated alongside the fix for finding C1

---

## Architecture & TEA Compliance

✅ All 8 modified files stay within their declared layers (`fdemon-app` and `fdemon-tui`).
✅ `update()` remains pure; new side effects deferred via `UpdateAction`.
✅ No new layer-boundary imports.
✅ The combined-action pattern (`RefreshDevicesAndBootableBackground`) is a reasonable narrow
   workaround for `UpdateResult` carrying a single action — not a problematic precedent given
   its narrow scope and rationale.

---

## Test Coverage

| Layer | Added tests | Adequacy |
|---|---|---|
| `target_selector_state.rs` | 4 unit tests for refreshing flag clearing | ✅ |
| `navigation.rs` | 3 new + 2 updated dialog-open tests | ✅ for cache-hit; ⚠️ no test for both-empty branch (M1) |
| `handler/tests.rs` | 1 integration test for `DevicesDiscovered` clearing flag | ⚠️ asymmetric — no bootable counterpart (m1) |
| `tab_bar.rs` | 3 render tests (Connected, Bootable, no-glyph) | ✅ |
| `target_selector.rs` | 3 render tests (mirror of tab_bar tests) | ✅ |

Total: 4,715 unit tests pass workspace-wide, 0 failures, 7 ignored.
`cargo clippy --workspace --lib -- -D warnings` clean.

---

## Re-review Checklist

- [ ] C1 fixed: `refreshing` is cleared in the `is_background: true` branch (and BUG.md note
  corrected)
- [ ] M1 addressed: cache-miss fallback also triggers bootable discovery (or M1 explicitly
  documented as out of scope)
- [ ] M2 addressed: `get_cached_bootable_devices()` returns references, or owned-return is
  documented as deliberate
- [ ] At least m1, m2, m3 addressed (test symmetry, glyph constant, set_error asymmetry note)
- [ ] `cargo fmt && cargo check --workspace && cargo test --workspace --lib && cargo clippy
  --workspace --lib -- -D warnings` passes

See [ACTION_ITEMS.md](ACTION_ITEMS.md) for the prioritized worklist.
