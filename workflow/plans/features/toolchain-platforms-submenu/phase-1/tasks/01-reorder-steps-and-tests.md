## Task: Reorder PathConfig after FlutterSdk + renumber index-based tests

**Objective**: Swap the `PathConfig` and `FlutterSdk` entries in `build_steps()` so the wizard order
becomes `Prerequisites[0] → AndroidTools[1] → FlutterSdk[2] → PathConfig[3] → Doctor[4]`, and update
every test/doc-comment/soft-tip that encodes the old `PathConfig=2 / FlutterSdk=3` positions. Pure
display reorder — no new types, no behavior change, **no rename of `AndroidTools`**.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/state.rs` — the `build_steps()` `vec![]` order, its doc-comment,
  and 3 index-based tests.
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` — 13 index-based tests + 1 soft-tip reword.
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` — 1 fixture + 2 tests + 4 comment updates.

**Files Read (Dependencies):**
- None (self-contained reorder).

### Details

> Line numbers below are from a research snapshot and **will drift**. Locate each site by the test name,
> the `WizardStepKind::…` variant, or the quoted comment/string — not by absolute line number.

#### 1. The source of truth — `build_steps()` in `state.rs`

**1a. The `vec![]` literal (≈ lines 936–972).** Swap the two `WizardStep { .. }` blocks so the return
order is:

```
[0] WizardStepKind::Prerequisites   (unchanged)
[1] WizardStepKind::AndroidTools    (unchanged — do NOT rename)
[2] WizardStepKind::FlutterSdk      (was [3])
[3] WizardStepKind::PathConfig      (was [2])
[4] WizardStepKind::Doctor          (unchanged)
```

Move the entire `WizardStep { kind: WizardStepKind::FlutterSdk, … }` block to sit **before** the
`WizardStep { kind: WizardStepKind::PathConfig, … }` block. Do not edit the field contents of either
block.

**1b. The `build_steps()` doc-comment (≈ line 855).**
- `/// Step order: Prerequisites → AndroidTools → PathConfig → FlutterSdk → Doctor`
  → `/// Step order: Prerequisites → AndroidTools → FlutterSdk → PathConfig → Doctor`
- In the "Component grouping" bullets (≈ lines 860–861), swap the `PathConfig` and `FlutterSdk` bullet
  lines so they read in the new step order (FlutterSdk bullet first, then PathConfig).

**1c. `path_config_status` derivation — DO NOT TOUCH.** It reads the `flutter_sdk` component bucket
(built by the `ComponentKind` match loop above), not a vec position. It is order-independent.

**1d. `installed_sdk_path` field doc-comment (≈ lines 110–113) — DO NOT CHANGE.** The phrase
"subsequent `PathConfig` step" is still accurate (PathConfig remains the step after FlutterSdk).

#### 2. Renumber index-based tests

The rule everywhere: **`selected_index` for FlutterSdk: 3 → 2; for PathConfig: 2 → 3.** Update the
inline `// index N` / `// PathConfig (index N)` comments to match. Prerequisites (0), AndroidTools (1),
Doctor (4) are unchanged.

**`state.rs` (3 sites):**
- `test_build_steps_produces_five_ordered_steps` — change the two assertions to
  `assert_eq!(steps[2].kind, WizardStepKind::FlutterSdk);` and
  `assert_eq!(steps[3].kind, WizardStepKind::PathConfig);`. (Index 0/1/4 assertions stay.)
- `test_select_next_noop_for_no_command_step` — `selected_index = 2` → `3`; update the
  `// PathConfig (index 2) has 0 guided commands.` comment to `index 3`. (FlutterSdk at 2 also has 0
  guided commands, so the test's intent still holds either way — but keep it pointed at PathConfig.)
- `test_select_prev_noop_for_no_command_step` — same change (`2` → `3`, comment update).

**`actions.rs` (13 sites — FlutterSdk tests 3→2, PathConfig tests 2→3):**
- FlutterSdk (3 → 2): `test_run_selected_flutter_step_dispatches_install_action`,
  `test_run_selected_noop_while_running`, `test_step_failed_records_reason_and_allows_retry`,
  `test_step_started_preserves_install_task_and_run_seq`, `test_stale_cross_kind_step_started_is_noop`,
  `test_step_started_with_current_seq_same_kind_preserves_task`,
  `test_copy_command_sets_status_when_no_command`.
- PathConfig (2 → 3): `test_pathconfig_without_known_sdk_sets_status_message`,
  `test_pathconfig_with_installed_sdk_path_dispatches_action`,
  `test_pathconfig_dispatch_includes_android_sdk_root`,
  `test_pathconfig_hints_when_android_sdk_root_absent`,
  `test_pathconfig_no_hint_when_android_sdk_root_present`,
  `test_pathconfig_no_hint_when_android_home_env_set_to_existing_dir`.
- **Leave untouched** the tests that use the `select_step()` helper / `position(|s| s.kind == …)` search
  (e.g. `test_android_step_*`, `test_copy_command_pushes_write_clipboard`) — they are order-independent.

**`step_detail.rs` (1 fixture + 2 tests + 4 comment-only):**
- Fixture `make_state_components()` — `state.selected_index = 3;` → `2;`, update its
  `// FlutterSdk step (has components)` comment to note index 2. This fixture change cascades the
  correct index to every test that uses it.
- `test_step_detail_shows_enter_hint_for_path_config_step` — `selected_index = 2` → `3`.
- `test_empty_step_shows_no_components_message` — `selected_index = 2` → `3` (and its comment).
- Comment-only `// (index 3)` → `// (index 2)` in: `test_step_detail_shows_enter_hint_for_flutter_step`,
  `test_step_detail_shows_progress_view_when_running`,
  `test_step_detail_progress_not_shown_for_different_step`, `detail_shows_esc_cancels_hint_while_running`.
- The Doctor-step fixtures/tests (`selected_index = 4`) are unchanged.

#### 3. Reword the soft ordering tip (`actions.rs`, ≈ lines 420–424)

Inside the `WizardStepKind::PathConfig` arm of `handle_run_selected_step`, the
`if android_sdk_root.is_none()` block sets:

```rust
"Tip: run Android Tools first so ANDROID_HOME is also configured."
```

Reword so it stays accurate now that FlutterSdk sits between AndroidTools and PathConfig, e.g.:

```rust
"Tip: run the Android Tools step first so ANDROID_HOME is also configured."
```

(Editorial only — the logic is unchanged. Keep it a soft `status_message`; PathConfig still executes.)

#### 4. KEEP the "Install Flutter first" gate (`actions.rs`, ≈ line 451) — DO NOT REMOVE

The `None => { status_message = Some("Install Flutter first") … }` arm in the `bin_dir` match must stay.
It is still reachable when the user manually navigates to PathConfig before any Flutter SDK is known.
Its test (`test_pathconfig_without_known_sdk_sets_status_message`) only needs the index renumber above.

### Acceptance Criteria

1. `build_steps()` returns kinds in order `[Prerequisites, AndroidTools, FlutterSdk, PathConfig, Doctor]`;
   `test_build_steps_produces_five_ordered_steps` asserts FlutterSdk at index 2 and PathConfig at index 3.
2. `AndroidTools` is **not** renamed; `steps.len()` is still 5.
3. The `"Install Flutter first"` gate still fires for a manual PathConfig run with no resolvable SDK.
4. The soft Android-Tools tip wording is updated and still emitted only when no Android SDK is discoverable.
5. `cargo test -p fdemon-app --lib install_wizard` and `cargo test -p fdemon-tui --lib install_wizard`
   pass; `cargo test --workspace --lib` is green.
6. `cargo fmt --all` and `cargo clippy --workspace -- -D warnings` are clean.

### Testing

Run the affected suites, then the full library tests:

```bash
cargo test -p fdemon-app  --lib install_wizard
cargo test -p fdemon-tui  --lib install_wizard
cargo test --workspace --lib
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

Watch these specific tests (they exercise the renumbered indices):
`test_build_steps_produces_five_ordered_steps`, `test_pathconfig_without_known_sdk_sets_status_message`,
`test_pathconfig_with_installed_sdk_path_dispatches_action`,
`test_run_selected_flutter_step_dispatches_install_action`,
`test_step_detail_shows_enter_hint_for_path_config_step`, `test_empty_step_shows_no_components_message`.

### Notes

- This is a display-only reorder. If you find yourself changing a `match WizardStepKind` arm, the
  completion chain, or `path_config_status` logic, stop — those are order-independent and must not change.
- Prefer locating each edit by test name / variant / comment text; the snapshot line numbers will drift.
- No new tests are required, but you may add one asserting `steps[3].kind == PathConfig` is the last
  step before `Doctor` if it helps lock the invariant.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/install_wizard/state.rs` | Swapped FlutterSdk and PathConfig blocks in `build_steps()` vec; updated doc-comment step order; renumbered 3 index-based tests (build_steps assertion, 2x noop_for_no_command_step) |
| `crates/fdemon-app/src/handler/install_wizard/actions.rs` | Rewrote soft-tip wording in PathConfig arm; renumbered 13 index-based tests (7 FlutterSdk 3→2, 6 PathConfig 2→3) |
| `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | Updated `make_state_components()` fixture (3→2); updated 2 tests using index; updated 4 comment-only `(index 3)→(index 2)` sites |

### Notable Decisions/Tradeoffs

1. **Pure reorder, no logic changes**: Only the `vec![]` order in `build_steps()` and the index-based test references changed. All `match WizardStepKind` arms, the `path_config_status` derivation, and the "Install Flutter first" gate are untouched.
2. **Soft tip wording**: Changed "run Android Tools first" to "run the Android Tools step first" to be specific about the step name. Logic unchanged — still only emitted when no Android SDK is discoverable.

### Testing Performed

- `cargo test -p fdemon-app --lib install_wizard` — Passed (256 tests)
- `cargo test -p fdemon-tui --lib install_wizard` — Passed (124 tests)
- `cargo test --workspace --lib` — Passed (1487 tests)
- `cargo fmt --all` — Clean (no formatting changes)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed (no warnings)

### Risks/Limitations

1. **Display-only change**: PathConfig step index changed from 2 to 3. Any code that references PathConfig by index (rather than by `WizardStepKind::PathConfig` variant or `position()` search) would be affected, but all such sites were in tests which are now updated.
