## Task: App config field + preflight plumbing + `PlatformWeb` handler arm

**Objective**: Add the `[toolchain] web_browser_executable` config field and thread it through to the
daemon's `run_preflight`, then split `PlatformWeb` out of the placeholder handler arm into a dedicated
guided-only arm. This task owns **all** `handler/install_wizard/actions.rs` edits and the
`RunToolchainPreflight` plumbing, keeping it write-disjoint from Task 03 (`state.rs`).

**Depends on**: Task 01 (the new `run_preflight(..., web_browser_executable)` signature must exist).

**Agent:** implementor

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/config/types.rs` — `ToolchainSettings.web_browser_executable` + `Default`.
- `crates/fdemon-app/src/handler/mod.rs` — `RunToolchainPreflight` variant field.
- `crates/fdemon-app/src/actions/mod.rs` — executor passes the override to `run_preflight`.
- `crates/fdemon-app/src/handler/install_wizard/navigation.rs` — `handle_show` reads the setting.
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` — `handle_rerun_preflight` reads the setting;
  `handle_run_selected_step` `PlatformWeb` arm split.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/mod.rs` — the new `run_preflight` signature (Task 01).
- `crates/fdemon-app/src/install_wizard/types.rs` — `WizardStepKind`, `GuidedCommand`.

### Details

> Line numbers are a current snapshot and will drift — locate by symbol/test-name/variant.

#### 1. `config/types.rs` — the config field

- `ToolchainSettings` (`:173-218`) — add `pub web_browser_executable: Option<String>,` with a `///`
  doc-comment (e.g. *"Custom Chromium-based browser for `flutter run -d chrome` / the web doctor check.
  Sets `CHROME_EXECUTABLE`. Any Chromium browser (Chrome, Edge, Brave, Chromium)."*). The struct already
  carries `#[serde(default)]` (`:172`), so a missing TOML key is fine.
- `Default` impl (`:220-233`) — add `web_browser_executable: None,`.
- **No collision** with `DevToolsSettings.browser` (`:490`): that key lives under `[devtools]`, this lives
  under `[toolchain]` — separate TOML paths. (Confirm by reading both struct serde paths.)

#### 2. `handler/mod.rs` — carry the override on the action

- `UpdateAction::RunToolchainPreflight` (`:667-677`) — add field `web_browser_executable: Option<String>`.
  Update its doc-comment to note the field feeds the daemon `WebBrowser` probe.

#### 3. `actions/mod.rs` — executor

- The `RunToolchainPreflight` executor arm (`:800-812`) — destructure the new field and pass
  `web_browser_executable.as_deref()` as the new trailing arg to `run_preflight(...)`.
- The `RunWizardStep` non-executable catch-all (`:1184-1198`) — **NO CHANGE**. `PlatformWeb` stays in the
  catch-all as a safety net; it is never reached because `handle_run_selected_step` returns `none()`
  without dispatching `RunWizardStep` for Web.

#### 4. `navigation.rs` — `handle_show`

- `handle_show` (`:20-31`) constructs `RunToolchainPreflight`. Read
  `state.settings.toolchain.web_browser_executable.clone()` and set it on the action.

#### 5. `actions.rs` — `handle_rerun_preflight` + `PlatformWeb` arm

- `handle_rerun_preflight` (`:135`) — same as `handle_show`: thread
  `state.settings.toolchain.web_browser_executable.clone()` into the `RunToolchainPreflight` it builds.
- `handle_run_selected_step` (`:379-387`) — **split `PlatformWeb`** out of the combined
  `PlatformIos | PlatformMacos | PlatformWeb | PlatformWindows` arm:
  - New `WizardStepKind::PlatformWeb` arm, **guided-commands-aware** (mirrors `Prerequisites` at
    `:471-478`): if the selected step has guided commands, set
    `status_message = Some("Run the listed command(s), then press r to re-check.".into())`; if it has
    none (browser already detected), return `UpdateResult::none()` silently (or a brief positive
    confirmation). Return `UpdateResult::none()` either way — Web is **not executable** (no
    `begin_step` / `RunWizardStep`).
  - The remaining `PlatformIos | PlatformMacos | PlatformWindows` arm keeps the
    "Available in a later phase" message.

> Writing the arm guided-commands-aware (reading `selected_step().guided_commands`) makes it correct
> whether Task 02 or Task 03 merges first: before Task 03 populates Web guided commands, the branch is
> simply silent; after, it shows the run-then-recheck hint. No compile dependency on Task 03.

### Acceptance Criteria

1. `cargo build -p fdemon-app` compiles against Task 01's merged daemon (the `run_preflight` arity + the
   `RunToolchainPreflight` field both line up).
2. `web_browser_executable` round-trips through TOML (`#[serde(default)]`, `None` default) and is carried
   on `RunToolchainPreflight` from **both** `handle_show` and `handle_rerun_preflight`.
3. The executor passes the override to `run_preflight`.
4. `handle_run_selected_step` for `PlatformWeb` returns `none()` (guided-only): sets the run/re-check
   status message when guided commands exist, is silent when none; iOS/macOS/Windows keep the placeholder
   message.
5. `cargo test -p fdemon-app --lib` green; `cargo fmt --all` + `cargo clippy -p fdemon-app -- -D warnings` clean.

### Testing

```bash
cargo build -p fdemon-app
cargo test -p fdemon-app --lib config
cargo test -p fdemon-app --lib handler::install_wizard
cargo test -p fdemon-app --lib
cargo fmt --all && cargo clippy -p fdemon-app -- -D warnings
```

New tests to add:
- `config/types.rs`: `web_browser_executable` defaults to `None`; round-trips a set value from TOML.
- `actions.rs`: `test_run_selected_step_web_with_guided_commands_sets_status_message` — build a state
  where the selected `PlatformWeb` step has ≥1 guided command, assert the run/re-check `status_message`
  and `UpdateResult::none()`. Add a sibling asserting silence when guided commands are empty.
- A test asserting `RunToolchainPreflight` carries the configured `web_browser_executable` from
  `handle_show` / `handle_rerun_preflight` when the setting is `Some(...)`.

### Notes

- **Existing `RunToolchainPreflight { .. }` test matchers** that use `..` survive the new field; only
  explicit constructions need the field. Grep for `RunToolchainPreflight` to confirm.
- **Do not** edit `install_wizard/state.rs` here — `build_steps` and the Web-leaf guided commands are
  Task 03. This boundary is what keeps Task 02 and Task 03 parallel.
- **Do not** add Web to `handle_step_completed` / `handle_auto_configure_path` — those fire only for
  `FlutterSdk` / `PlatformAndroid` (verified); Web is guided-only and never dispatches a step.
- `handle_copy_command` / `selected_guided_command()` already work for any step with populated
  `guided_commands` — no change needed; the `c`-copy key works for Web once Task 03 populates them.
