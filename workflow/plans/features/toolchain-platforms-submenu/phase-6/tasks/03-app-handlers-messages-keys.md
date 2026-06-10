## Task: App — picker messages, handlers, key routing, and FlutterSdk-arm version threading

**Objective**: Wire the picker into the TEA loop: new `Message` variants + `UpdateAction::
FetchFlutterReleaseManifest` (with a no-op executor stub arm), a new
`handler/install_wizard/version_picker.rs` handler module (open/close/nav/tab/confirm/fetch
lifecycle), key interception in `handle_key_install_wizard` while the picker is visible (+ the `v`
key), and the `handle_run_selected_step` FlutterSdk arm changes: Enter opens the picker when no
choice exists, and the install params carry `version_tag`.

**Depends on**: Task 01 (daemon types), Task 02 (`VersionPickerState` API).
Runs in parallel with Task 05 (write-disjoint: app vs tui).

**Agent:** implementor

**Complexity:** high

**Estimated Time**: 4–5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/message.rs`
- `crates/fdemon-app/src/handler/mod.rs` — new `UpdateAction` variant;
  `FlutterStepParams.version_tag: Option<String>`
- `crates/fdemon-app/src/handler/keys.rs`
- `crates/fdemon-app/src/handler/update.rs` — message → handler routing
- `crates/fdemon-app/src/handler/install_wizard/version_picker.rs` — **NEW**
- `crates/fdemon-app/src/handler/install_wizard/mod.rs` — module decl + re-exports
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` — FlutterSdk arm
- `crates/fdemon-app/src/actions/mod.rs` — **no-op stub arm only** for
  `FetchFlutterReleaseManifest` (the executor match is exhaustive; Task 04 fills the body)

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/version_picker.rs` — Task 02's state API
- `handler/keys.rs` tag-filter intercept precedent (`handle_key_normal`, ~line 149)
- `handler/install_wizard/navigation.rs` — wizard hide/escape path (call `reset()`; see Notes)

### Details

> Locate by symbol; line numbers drift.

#### 1. Messages (`message.rs`, beside the existing `InstallWizard*` block)

```rust
InstallWizardOpenVersionPicker,            // `v`, or Enter-on-FlutterSdk with no choice
InstallWizardVersionPickerClose,           // Esc
InstallWizardVersionPickerUp,              // k / Up
InstallWizardVersionPickerDown,            // j / Down
InstallWizardVersionPickerNextTab,         // Tab
InstallWizardVersionPickerRefetch,         // r
InstallWizardVersionPickerConfirm,         // Enter
FlutterManifestFetched { manifest: fdemon_daemon::toolchain::FlutterReleaseManifest },
FlutterManifestFetchFailed { error: String },
```

#### 2. `UpdateAction` + params (`handler/mod.rs`)

- `UpdateAction::FetchFlutterReleaseManifest` (no payload — host detected executor-side, the
  `RunToolchainPreflight` precedent).
- `FlutterStepParams` gains `pub version_tag: Option<String>` (doc: exact manifest version or
  `master`/`main`; overrides `channel` for dir name + git ref, **per-run only**). Fix the struct's
  construction sites and any literal-asserting tests.

#### 3. Handler module (`handler/install_wizard/version_picker.rs`, NEW)

- `handle_open_picker(state) -> UpdateResult` — refuse while `is_step_running()` (status message,
  no-op); only meaningful when the selected step is `FlutterSdk` (guard, no-op otherwise);
  `version_picker.open()`; if it reports fetch-needed → `begin_fetch()` + return
  `UpdateResult::action(UpdateAction::FetchFlutterReleaseManifest)`.
- `handle_close_picker` — `close()`, no action.
- `handle_up/down/next_tab` — delegate to state methods.
- `handle_refetch` — `begin_fetch()` + the fetch action (only when picker visible).
- `handle_manifest_fetched(state, manifest)` — `apply_manifest(manifest, HostArch::detect())`.
  Stale-safety: applying with the picker already closed is harmless (state cached for reopen) — no
  seq guard needed because the fetch is idempotent and read-only.
- `handle_manifest_fetch_failed(state, error)` — `apply_fetch_error`.
- `handle_confirm(state) -> UpdateResult` —
  - `PickerFetch::Failed` → close picker and **fall through to the default-channel install**
    (call the same run path as below with `version_tag: None`) — offline escape hatch.
  - Otherwise `confirm()` → `Some(row)`: dispatch the install **through the existing
    `handle_run_selected_step` FlutterSdk path** (see §5) so `begin_step`/`run_seq`/token minting
    stay single-sourced. Empty tab → no-op.

#### 4. Key routing (`handler/keys.rs`, `handle_key_install_wizard`)

At the **top** of the function, before every existing arm (the tag-filter intercept pattern):

```rust
if state.install_wizard_state.version_picker.visible {
    return match key {
        Esc          => msg(InstallWizardVersionPickerClose),
        Up | 'k'     => msg(InstallWizardVersionPickerUp),
        Down | 'j'   => msg(InstallWizardVersionPickerDown),
        Tab          => msg(InstallWizardVersionPickerNextTab),
        'r'          => msg(InstallWizardVersionPickerRefetch),
        Enter        => msg(InstallWizardVersionPickerConfirm),
        Ctrl+C       => Quit,            // never trap quit
        _            => none,            // swallow everything else
    };
}
```

Below the intercept, add `'v'` → `InstallWizardOpenVersionPicker` (the handler no-ops it off the
FlutterSdk step).

#### 5. FlutterSdk arm (`handler/install_wizard/actions.rs`, `handle_run_selected_step`)

- **New gate at the top of the arm**: if `version_picker.selected_release.is_none()` **and** the
  picker has never errored-and-fallen-back this run → behave exactly like
  `handle_open_picker` (open + maybe fetch action) and return. Implementation hint: route both Enter
  and `v` to one helper; pass an explicit `force_default: bool` for the offline fallback so the
  confirm-with-error path can run un-pinned.
- **Param sourcing** when a `selected_release` exists (or fallback):
  - `channel` = `row.channel` (falls back to `settings.toolchain.channel` when un-pinned),
  - `version_tag` = `Some(row.version)` (or `None` for fallback),
  - `method` = `settings.toolchain.install_method()`, **overridden to `InstallMethod::GitClone` when
    `row.git_only`**,
  - `install_root` unchanged.
- Everything downstream (begin_step, token, `RunWizardStep`) is untouched.
- Update the arm's doc-comment + the tests asserting `FlutterStepParams` literals
  (`version_tag: None` for legacy paths).

#### 6. Routing + lifecycle glue

- `handler/update.rs`: route the 9 new messages to the new handlers.
- Wizard hide/escape (`navigation.rs` owns it — if the reset must be called there, make it a
  one-line `state.install_wizard_state.version_picker.reset()` addition and note it; Esc with the
  picker open never reaches navigation because of the key intercept).
- `actions/mod.rs`: add `UpdateAction::FetchFlutterReleaseManifest => { /* Task 04 */ }` no-op stub
  arm so the exhaustive match compiles.

### Acceptance Criteria

1. Enter on FlutterSdk (no choice, step not running) opens the picker and dispatches the fetch
   action exactly once (`NotFetched` → `Loading`); a second open while `Loaded` fetches nothing.
2. Picker-visible key events map per the table; unmapped keys are swallowed; Ctrl+C still quits; no
   underlying wizard message fires while visible.
3. Confirm with a stable row produces `RunWizardStep` whose `FlutterStepParams` has
   `version_tag: Some(version)`, `channel == row.channel`; confirm on a `git_only` row forces
   `InstallMethod::GitClone`; `run_seq` bumps exactly once per dispatch.
4. Confirm in `Failed` state closes the picker and dispatches an un-pinned default-channel install
   (offline path); `r` from `Failed` re-fetches.
5. Opening while a step runs is refused with a status message; `v` off the FlutterSdk step no-ops.
6. Wizard hide resets the picker (manifest dropped, selection cleared).
7. `cargo test -p fdemon-app --lib` green (existing FlutterSdk Enter-runs-immediately tests are
   **updated** to the new two-step flow, not deleted); fmt + clippy clean.

### Testing

```bash
cargo test -p fdemon-app --lib handler::install_wizard
cargo test -p fdemon-app --lib
cargo fmt --all && cargo clippy --workspace -- -D warnings
```

Test idioms: build `AppState` via the existing wizard test helpers; feed `Message`s through
`handler::update`; assert on returned `UpdateAction` shape + state. Key tests go in `keys.rs`'s
existing `handle_key_install_wizard` test mod.

### Notes

- **Single dispatch path**: the confirm flow must call into `handle_run_selected_step` (or the shared
  helper extracted from it) — do not duplicate `begin_step`/token minting in the picker handler.
- **Stale `WizardInstallTaskReady`/`WizardStepStarted` guards** (`run_seq`) are untouched — the
  picker only changes *when* the run starts, not how.
- The executor stub arm means picker-open in a full-engine integration test would hang in `Loading`;
  unit tests don't run the executor, and Task 04 lands the body in the same wave-train — acceptable.
- Do not edit `install_wizard/state.rs` or `version_picker.rs` (Task 02's files) beyond what compiles
  against their API; if the API is missing something, the fix belongs in a tiny follow-up to 02, not
  an in-place fork (flag it in the completion summary).
