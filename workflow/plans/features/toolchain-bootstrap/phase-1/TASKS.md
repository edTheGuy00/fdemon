# Phase 1 — Toolchain Preflight & Doctor View — Task Index

## Overview

Phase 1 replaces the dead-end "Flutter SDK not found" error with a **read-only diagnostics
wizard**. It adds a `toolchain/` preflight subsystem to `fdemon-daemon` (structured component
checks + `flutter doctor -v` text capture/parse), a new `UiMode::InstallWizard` modal in
`fdemon-app` (modeled on `UiMode::FlutterVersion`), a two-pane TUI (step list + step detail +
embedded doctor view) in `fdemon-tui`, and a startup hook that opens the wizard when no Flutter
SDK is resolved. **No installation is performed in Phase 1** — every step is diagnostic only.

**Total Tasks:** 6
**Estimated Hours:** 30–40 hours

## Task Dependency Graph

```
                          ┌─────────────────────────────────┐
                          │ 01-toolchain-preflight-subsystem │  (fdemon-daemon)
                          └────────────────┬─────────────────┘
                                           ▼
                          ┌─────────────────────────────────┐
                          │ 02-install-wizard-state-types    │  (fdemon-app, new files)
                          └────────────────┬─────────────────┘
                          ┌────────────────┴─────────────────┐
                          ▼                                   ▼
        ┌─────────────────────────────────┐  ┌─────────────────────────────────┐
        │ 03-install-wizard-app-wiring     │  │ 04-install-wizard-tui-widget     │
        │ (fdemon-app: UiMode/msg/handlers)│  │ (fdemon-tui: widget)             │
        └────────────────┬─────────────────┘  └────────────────┬─────────────────┘
                         └──────────────┬─────────────────────┘
                                        ▼
                       ┌─────────────────────────────────┐
                       │ 05-render-and-startup-hook       │  (fdemon-tui)
                       └────────────────┬─────────────────┘
                                        ▼
                       ┌─────────────────────────────────┐
                       │ 06-update-docs (doc_maintainer)  │
                       └─────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate |
|---|------|--------|------------|------------|-------|
| 1 | [01-toolchain-preflight-subsystem](tasks/01-toolchain-preflight-subsystem.md) | ✅ Done | - | 8-10h | `fdemon-daemon` |
| 2 | [02-install-wizard-state-types](tasks/02-install-wizard-state-types.md) | ✅ Done | 1 | 4-5h | `fdemon-app` |
| 3 | [03-install-wizard-app-wiring](tasks/03-install-wizard-app-wiring.md) | ✅ Done | 2 | 6-8h | `fdemon-app` |
| 4 | [04-install-wizard-tui-widget](tasks/04-install-wizard-tui-widget.md) | ✅ Done | 2 | 6-8h | `fdemon-tui` |
| 5 | [05-render-and-startup-hook](tasks/05-render-and-startup-hook.md) | ✅ Done | 3, 4 | 3-4h | `fdemon-tui` |
| 6 | [06-update-docs](tasks/06-update-docs.md) | ✅ Done | 1,2,3,4,5 | 2-3h | docs |

## Execution Waves

| Wave | Tasks | Notes |
|------|-------|-------|
| 1 | 01 | Foundation: daemon preflight types + checks |
| 2 | 02 | App-layer state types (new files only, compiles standalone) |
| 3 | 03 ∥ 04 | **Parallel** — app wiring (app crate) and TUI widget (tui crate) touch disjoint files |
| 4 | 05 | Render dispatch + startup hook (needs both 03 and 04) |
| 5 | 06 | Docs (doc_maintainer) |

> This feature is an inherently layered, cross-crate TEA change, so the critical path is mostly
> sequential (daemon → app types → app wiring → tui). Wave 3 is the one genuine parallel
> opportunity: task 03 (fdemon-app) and task 04 (fdemon-tui) write entirely disjoint files.

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01 | `crates/fdemon-daemon/src/toolchain/mod.rs` (NEW), `.../toolchain/types.rs` (NEW), `.../toolchain/checks.rs` (NEW), `.../toolchain/doctor.rs` (NEW), `crates/fdemon-daemon/src/lib.rs` | `flutter_sdk/locator.rs`, `flutter_sdk/version_probe.rs`, `flutter_sdk/types.rs`, `tool_availability.rs`, `fdemon-core/src/error.rs` |
| 02 | `crates/fdemon-app/src/install_wizard/mod.rs` (NEW), `.../install_wizard/state.rs` (NEW), `.../install_wizard/types.rs` (NEW), `crates/fdemon-app/src/lib.rs` | `crates/fdemon-app/src/flutter_version/{state,types,mod}.rs` (template), task 01 `toolchain` types |
| 03 | `crates/fdemon-app/src/state.rs`, `.../message.rs`, `.../handler/mod.rs`, `.../handler/update.rs`, `.../handler/keys.rs`, `.../handler/install_wizard/{mod,navigation,actions}.rs` (NEW), `.../handler/mouse/mod.rs`, `.../handler/mouse/install_wizard.rs` (NEW), `.../actions/mod.rs`, `docs/KEYBINDINGS.md` | task 02 types, `handler/flutter_version/*`, `actions/mod.rs` flutter-version block (template) |
| 04 | `crates/fdemon-tui/src/widgets/install_wizard/{mod,step_list,step_detail,doctor_view}.rs` (NEW), `crates/fdemon-tui/src/widgets/mod.rs` | task 02 state types, `widgets/flutter_version_panel/*` (template) |
| 05 | `crates/fdemon-tui/src/render/mod.rs`, `crates/fdemon-tui/src/runner.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` (optional hint) | task 03 messages/UiMode, task 04 widget |
| 06 | `docs/ARCHITECTURE.md`, `docs/CODE_STANDARDS.md` | all task files, `~/.claude/skills/doc-standards/schemas.md` |

### Overlap Matrix

Only wave-peers matter. The only wave with concurrent tasks is **Wave 3 (03 + 04)**.

| Task Pair | Same Wave? | Shared Write Files | Isolation Strategy |
|-----------|-----------|-------------------|-------------------|
| 03 + 04 | Yes (Wave 3) | None (03 = `fdemon-app`, 04 = `fdemon-tui`) | **Parallel (worktree)** |
| 02 + 03 | No (dep chain) | `crates/fdemon-app/src/handler/mod.rs`? No — 02 does not touch it; both touch `lib.rs`? No — 02 owns `lib.rs`, 03 does not. | Sequential (dependency) |
| all others | No | — | Sequential (dependency) |

> **Standalone-compile guarantee:** Each task is designed to leave the workspace compiling green.
> Task 02 adds only new files + `lib.rs` module declarations (no enum/match changes), so it
> compiles alone. Task 03 introduces the `Message`/`UiMode`/`UpdateAction` variants **together
> with** their exhaustive-match arms in `update.rs`/`keys.rs`/`actions/mod.rs`, so no
> non-exhaustive-match break is ever committed. (This is why state-types and message-wiring are
> split across tasks 02 and 03 rather than 02 alone.)

## Success Criteria

Phase 1 is complete when:

- [ ] `toolchain::run_preflight()` returns a `ToolchainReport` with per-component
      `Ok/Partial/Missing/Error/Unknown` status for: Flutter SDK, git, JDK, Android
      cmdline-tools/`sdkmanager`, adb (platform-tools), Android platforms/build-tools, Android
      licenses, and per-OS prerequisites.
- [ ] When Flutter is present, `flutter doctor -v` text is captured and parsed into
      marker-prefixed (`[✓]/[!]/[✗]/[☠]`) lines for display.
- [ ] On a machine with no resolvable Flutter SDK, fdemon opens `UiMode::InstallWizard` at startup
      instead of only emitting `DeviceDiscoveryFailed`.
- [ ] The wizard renders a two-pane diagnostics screen (ordered step list + per-step detail) plus
      the embedded doctor view, navigable by keyboard (`Tab`, `j/k`, `r` to re-run preflight,
      `Esc` to close). `I` opens it from Normal mode.
- [ ] `UiMode::InstallWizard` is registered in the modal-precedence list (mouse suppression).
- [ ] No installation/network/download code is added in Phase 1 (no new crate dependencies).
- [ ] All new public functions have unit tests (doctor-text parse, status derivation, step
      builder, host-platform/shell detection). Existing tests pass; no regressions.
- [ ] Full quality gate is green: `cargo fmt --all -- --check`,
      `cargo check --workspace --all-targets`, `cargo test --workspace`,
      `cargo clippy --workspace --all-targets -- -D warnings`.

## Keyboard Shortcuts (Phase 1 subset)

| Key | Mode | Action |
|-----|------|--------|
| `I` | Normal | Open the Install Wizard |
| `Esc` | InstallWizard | Close wizard |
| `Tab` | InstallWizard | Switch pane (step list ↔ detail) |
| `j`/`k` `↓`/`↑` | InstallWizard | Navigate steps / scroll detail |
| `r` | InstallWizard | Re-run preflight checks |
| `Ctrl+C` | InstallWizard | Quit fdemon |

> Phase 1 has **no** step-execution key (`Enter` to install) and **no** `c` (copy command) — those
> arrive with Phase 2+. `Enter` on a step is a no-op (or re-runs preflight) in Phase 1.

## Notes

- **Scope discipline:** Phase 1 is read-only. Do **not** add `reqwest`/`zip`/`tar`/`sha2`/`lzma-rs`
  to `fdemon-daemon` (Phase 2), and do **not** add `[toolchain]` config keys (Phase 2/3) or any
  `UpdateAction::RunWizardStep` / download/progress messages.
- **Reuse, don't re-detect:** `find_flutter_sdk()` is synchronous and already returns a rich
  `FlutterSdk` (`root`, `executable`, `source`, `version`, `channel`). Call it directly inside
  `run_preflight`. For Dart/DevTools/commit detail, reuse `probe_flutter_version(&sdk.executable)`.
- **Established process idiom:** all external probes use `tokio::process::Command` with
  `Stdio::null()` + `.status()` for existence checks, and `.output()` wrapped in
  `tokio::time::timeout(...)` for captured output. No new deps required — tokio (full),
  `serde_json`, `which`, `dirs`, `tracing` are already in `fdemon-daemon`.
- **Template:** `UiMode::FlutterVersion` is the end-to-end reference. Mirror its file layout:
  `flutter_version/` (state) → `handler/flutter_version/` (handlers) →
  `widgets/flutter_version_panel/` (TUI) → `render/mod.rs` arm → `is_modal_ui_mode` entry.
- Doctor-text parsing is **display-only**; wizard step status is driven exclusively by the
  structured checks. Parse defensively — never panic on unexpected `flutter doctor` output.
