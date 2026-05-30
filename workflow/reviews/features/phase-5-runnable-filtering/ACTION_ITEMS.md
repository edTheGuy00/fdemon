# Action Items: Phase 5 — Runnable-Device Filtering

**Review Date:** 2026-05-30
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 0 hard blockers — 2 items to resolve before merge (1 trivial fix, 1 decision)

## Major Issues (Should Fix Before Merge)

### 1. Test does not exercise the function it guards
- **Source:** code_quality_inspector, architecture_enforcer, logic_reasoning_checker
- **File:** `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs:872-884`
- **Problem:** `toggle_checked_cursor_skips_unsupported` only asserts on the private helper
  `is_connected_device_supported`; it never calls `toggle_checked_cursor()`. The guard the
  test is named for has no coverage through its production path.
- **Required Action:** Rewrite the test body to call `toggle_checked_cursor()` (change to
  `let mut state`), pre-seeding `checked_device_ids` with the unsupported device's id and
  asserting the checked set is unmodified after the call. Keep or fold in the existing
  helper assertions.
- **Acceptance:** Test invokes `toggle_checked_cursor()`; `cargo test -p fdemon-app` passes.

### 2. Decide & document the scope of the "no unrunnable launch" invariant
- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/spawn.rs:242-267`; `src/headless/runner.rs:~300`
- **Problem:** `find_auto_launch_target` (auto-start, cached `last_device`, first-config,
  bare run) and the headless path resolve devices without checking `is_supported`, so the
  feature's invariant holds only in the interactive dialog — and breaks on the paths where
  no empty-state message can warn the user.
- **Required Action (choose one):**
  - **(a) Dialog-only scope (document):** Add a note to `docs/REVIEW_FOCUS.md` (or the
    plan) stating `is_supported` filtering is intentionally dialog-scoped and
    `find_auto_launch_target` is exempt, so it isn't later "fixed" as a regression. Lowest
    effort; matches the `TASKS.md` "Connected tab only" scope note.
  - **(b) System-wide scope (implement):** Apply a **default-true fallback** filter
    (`.filter(\|d\| d.is_supported)`, but fall back to the unfiltered list if all are
    unsupported) in `find_auto_launch_target`; extract a shared `Device::is_runnable()` /
    free function used by both the dialog grouping and the auto path; add tests for an
    unsupported `devices.first()` and an unsupported cached `last_device`.
- **Acceptance:** Either a committed doc note recording the boundary, or the filter applied
  + tests green on the auto/headless paths.

## Minor Issues (Consider Fixing)

1. **Silent partial-set hiding** (`device_list.rs:212-228`) — add a "(N hidden: not runnable
   for this project)" footer for the mixed supported/unsupported case. UX follow-up.
2. **`is_connected_device_supported`** (`target_selector_state.rs:465-471`) — replace
   `.find().map().unwrap_or(false)` with `.any(|d| d.id == id && d.is_supported)`.
3. **`toggle_select_all`** (`target_selector_state.rs:416-421`) — collect directly into
   `BTreeSet<String>`, drop the intermediate `Vec` + second collect.
4. **`DeviceCapabilities` export** (`fdemon-daemon/src/lib.rs`) — add to `pub use
   devices::{...}` (+ module doc), or explicitly note as internal-only this phase.
5. **`debug!` full stdout** (`devices.rs:238`, pre-existing) — demote payload to `trace!`,
   log a summary at `debug`.
6. **Defensive indexing** (`device_groups.rs:266,286,309`) — use `.last()` / `.get(i)`.
7. **`Device` builder/`Default`** — add a test-only builder to stop future fields forcing a
   workspace-wide literal sweep.

## Nitpicks

- `capabilities` dead-stored field — remove if no consumer within ~2 phases.
- Scoped `use ratatui::...` in `device_list.rs` empty-state — hoist to top-of-file.
- `cached_flat_list...unwrap()` (`target_selector_state.rs:162`, pre-existing) — `get_or_insert_with`.
- Em-dash `\u{2014}` — use literal `—`.

## Re-review Checklist

- [ ] M1 resolved — test calls `toggle_checked_cursor()`
- [ ] M2 resolved — scope documented OR filter applied with tests
- [ ] Minor items addressed or consciously deferred
- [ ] Quality gate green: `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
