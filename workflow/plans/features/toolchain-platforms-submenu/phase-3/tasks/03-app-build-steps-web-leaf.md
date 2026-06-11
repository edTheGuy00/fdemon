## Task: `build_steps` Web leaf — route `WebBrowser`, cap to non-blocking, per-OS guided commands

**Objective**: Graduate the `PlatformWeb` leaf in `build_steps` from a Phase-2 placeholder
(`StepStatus::Pending`, empty components/guided commands) to a live step backed by the
`ComponentKind::WebBrowser` component, with a status **capped so `Missing → Partial`** (non-blocking) and
per-OS guided commands. All edits are confined to `install_wizard/state.rs`, keeping this task
write-disjoint from Task 02.

**Depends on**: Task 01 (`ComponentKind::WebBrowser` must exist — the routing match is compiler-forced).

**Agent:** implementor

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/state.rs` — `build_steps` routing arm + Web-leaf block +
  `web_browser_guided_commands` helper + status cap + tests.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs` — `ComponentKind::WebBrowser`, `ComponentStatus`.
- `crates/fdemon-app/src/install_wizard/types.rs` — `GuidedCommand`, `StepStatus`, `WizardStepKind`.

### Details

> Line numbers are a current snapshot and will drift — locate by symbol/test-name/variant.

#### 1. Route `WebBrowser` into a Web bucket

- `build_steps` (`:932`) — before the component loop, declare
  `let mut web_components: Vec<ComponentCheck> = Vec::new();` alongside the existing bucket vecs.
- The component-routing `match check.kind` (`:938-953`, **exhaustive — compiler-forced**) — add
  `ComponentKind::WebBrowser => web_components.push(check.clone()),`.

#### 2. Non-blocking status cap

The Web leaf must never surface `Missing` (it would roll the Platforms parent to `Missing` and break the
"optional web" contract). `rollup_status` (`:479`) returns `Missing` for a missing component and **must
stay that way** for Android. Cap locally at the Web leaf:

```rust
let raw = rollup_status(&web_components);
let web_status = if raw == StepStatus::Missing { StepStatus::Partial } else { raw };
```

- `Unknown`-only / empty `web_components` → `rollup_status` yields `Ok` (empty → `Ok`). That is fine for
  hosts where the probe returned `Unknown` (no display target): the leaf shows `Ok`/neutral and emits no
  guided commands. (If you prefer the leaf to read `Pending` when the only component is `Unknown`, handle
  that explicitly — but `Ok` with no guided commands is acceptable and simplest.)

#### 3. `web_browser_guided_commands` helper

Add `fn web_browser_guided_commands(report: &ToolchainReport, web_status: StepStatus) -> Vec<GuidedCommand>`
modeled on `jdk_guided_command` (`:508`). Return `Vec::new()` when `web_status` is `Ok` (browser found).
Otherwise, per `report.platform`:
- **Linux**: prefer Chromium (cross-distro). e.g. `GuidedCommand { label: "Install a browser",
  command: "<per-package-manager chromium install, using report.linux_package_manager>", note:
  Some("or set: export CHROME_EXECUTABLE=\"/path/to/your/browser\"") }`. If no package manager is known,
  point at the Chrome download URL with the `CHROME_EXECUTABLE` note.
- **macOS**: `label: "Download Chrome", command: "https://www.google.com/chrome/"`, plus a second
  `export CHROME_EXECUTABLE="..."` command (or as the `note`).
- **Windows**: `label: "Install Chrome", command: "winget install Google.Chrome"`, note with the
  `set CHROME_EXECUTABLE=...` alternative.
- **`HostPlatform::Unknown`**: `Vec::new()`.

> **The guided command is a template, not the configured value.** Decision 2 keeps `build_steps`
> pure-on-report (no settings param), so the `export CHROME_EXECUTABLE="<path>"` command uses a
> placeholder, not `web_browser_executable`. This is intentional — flagged in the Phase-3 TASKS notes.

#### 4. Replace the placeholder leaf block

- `PlatformWeb` leaf construction (`:1025-1033`) — replace the hardcoded
  `status: StepStatus::Pending, components: Vec::new(), guided_commands: Vec::new()` with:
  `status: web_status`, `components: web_components`,
  `guided_commands: web_browser_guided_commands(report, web_status)`. Keep `title: "Web"`, `indent: 1`.
- Update the `build_steps` doc-comment (`:912-931`) — Web is now live (not a placeholder); note the
  `Missing → Partial` cap and that Web is non-blocking.

### Acceptance Criteria

1. `cargo build -p fdemon-app` compiles (the exhaustive `ComponentKind` routing match has the `WebBrowser`
   arm).
2. `build_steps` with a `WebBrowser` `Missing` component → `PlatformWeb` leaf status is **`Partial`**,
   never `Missing`; with `Ok` → `Ok` and **empty** guided commands.
3. A `Partial` Web leaf produces non-empty per-OS guided commands matching `report.platform`.
4. The Platforms parent rolls up to at most `Partial` when Android is `Ok` and Web is `Partial` (verify
   via `rollup_step_statuses`).
5. `flutter_now_live()` returns `true` when `FlutterSdk == Ok` regardless of Web status.
6. `cargo test -p fdemon-app --lib` green; `cargo fmt --all` + `cargo clippy -p fdemon-app -- -D warnings` clean.

### Testing

```bash
cargo build -p fdemon-app
cargo test -p fdemon-app --lib install_wizard::state
cargo test -p fdemon-app --lib install_wizard
cargo test -p fdemon-app --lib
cargo fmt --all && cargo clippy -p fdemon-app -- -D warnings
```

Existing tests to update:
- `test_build_steps_expanded_inserts_android_leaf` (`:~1228`) — it asserts the `PlatformWeb` leaf
  (`steps[3]`) is `Pending`. Update the expected status to the real rollup; the **count stays 7** (Web
  leaf still emitted). Feed a `WebBrowser` component into the fixture report to drive a deterministic
  status.
- `test_non_android_non_prereq_steps_have_no_guided_commands_when_prereqs_absent` (and any sibling
  asserting non-Android steps have empty guided commands) — `PlatformWeb` may now have guided commands
  when the browser is missing. Either exclude `PlatformWeb` from the assertion or use a browser-found
  (`Ok`) fixture so it has none.
- Any `make_report()`-based fixture policy: decide whether `make_report()` includes a `WebBrowser` `Ok`
  component so "all healthy" fixtures stay all-`Ok` (recommended — otherwise a Web `Partial` latches
  `observed_unhealthy` and trips `all_components_ok`). Document the choice in the test module.

New tests to add:
- `test_web_leaf_status_never_missing` — `WebBrowser` `Missing` → leaf `Partial`.
- `test_web_leaf_has_guided_command_when_browser_missing` — `Partial` leaf → `guided_commands.len() > 0`.
- `test_web_no_guided_command_when_browser_ok` — `Ok` leaf → empty guided commands.
- `test_platforms_parent_not_blocked_by_web_partial` — Android `Ok` + Web `Partial` → parent `Partial`.
- `test_flutter_now_live_unaffected_by_web_partial` — `FlutterSdk Ok` + Web `Partial` → `flutter_now_live() == true`.

### Notes

- **`rollup_status` stays unchanged** — the cap is local to the Web leaf. Other platforms (Android) must
  still surface true `Missing`.
- **`all_components_ok` (`:203`) stays strict and unchanged** — a `Partial` Web making the "All set"
  subtitle not fire is correct behaviour; do not special-case Web out of it.
- **Do not** touch `handler/install_wizard/actions.rs` (the `PlatformWeb` handler arm is Task 02) — this
  boundary keeps Tasks 02 and 03 parallel.
- **`build_steps` takes no settings param** — the configured `web_browser_executable` reaches detection
  via Task 02's `run_preflight` plumbing and arrives here already reflected in the `WebBrowser`
  component's status. `build_steps` reads only the report.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/install_wizard/state.rs` | Missing→Partial cap on `web_status`; added `web_browser_guided_commands` helper; wired guided commands into `PlatformWeb` leaf; updated `build_steps` doc comment; updated `test_build_steps_expanded_inserts_android_leaf`; excluded `PlatformWeb` from no-guided-commands assertion; added 5 new tests |

### Notable Decisions/Tradeoffs

1. **Missing→Partial cap via `if raw == StepStatus::Missing { StepStatus::Partial } else { raw }`**: Exactly as specified — local to the Web leaf, `rollup_status` unchanged. Android still surfaces true `Missing`.
2. **`web_browser_guided_commands` returns empty vec when status is `Ok`**: Matches `jdk_guided_command` pattern. The `web_status == StepStatus::Ok` early-return avoids the platform match entirely.
3. **Template paths in `CHROME_EXECUTABLE` note**: Per task spec — `build_steps` takes no settings param so `<path>` placeholders are used rather than the configured `web_browser_executable` value.
4. **Linux per-PM chromium install commands**: Each known package manager maps to its own `chromium`/`chromium-browser` package name with a `CHROME_EXECUTABLE` export note. Unknown PM falls back to the Chrome download URL with the same note.
5. **`test_build_steps_expanded_inserts_android_leaf` now asserts `Ok` not `Pending`**: The fixture was updated to feed a `WebBrowser Ok` component, making the test verify the live routing instead of the placeholder.

### Testing Performed

- `cargo build -p fdemon-app` — Passed
- `cargo test -p fdemon-app --lib install_wizard::state` — Passed (140 tests)
- `cargo test -p fdemon-app --lib` — Passed (2989 tests)
- `cargo fmt --all && cargo clippy -p fdemon-app --all-targets -- -D warnings` — Passed (clean)

### Risks/Limitations

1. **`CHROME_EXECUTABLE` placeholder paths**: The guided commands show `"/path/to/browser"` rather than the user's actual configured path. This is intentional per the task spec (Decision 2: `build_steps` is pure-on-report). The configured value arrives via Task 02's `run_preflight` plumbing reflected in the component status — if the browser is found via the configured path, status is `Ok` and no commands are shown.
2. **Linux Apt command uses `chromium-browser` not `chromium`**: Debian/Ubuntu packages the binary as `chromium-browser` on older releases; the note includes a `google-chrome-stable` alternative. This follows the cross-distro preference for Chromium over Chrome.
