# Phase 3 — Web leaf + `web_browser_executable` — Task Index

## Overview

Graduate the **Web** platform leaf from its Phase-2 inert placeholder (`StepStatus::Pending`, no
components, no guided commands) into a **live detect + guided-only** step, cross-host, that **never
blocks** the toolchain-healthy handback. A new `ComponentKind::WebBrowser` probe (`CHROME_EXECUTABLE`
→ default Chrome paths → Edge on Windows) feeds the leaf; a new `[toolchain] web_browser_executable`
config field lets users point the probe at any Chromium-based browser; per-OS guided commands tell the
user how to install a browser or export `CHROME_EXECUTABLE`. A missing browser surfaces as
**Partial (warning)**, never **Missing**.

> Web is **detection + guided-only** — there is NO auto-installer (browsers are GUI/App-Store/privileged).
> The leaf is never executable; `Enter` shows guided commands, `c` copies the selected command, `r`
> re-checks. No new keybindings.

### Decisions resolved by research (verified against source)

1. **Component name = `ComponentKind::WebBrowser`** (not `ChromeBrowser`). `Display` → `"Web Browser"`.
   Matches the `PlatformWeb` / `web_browser_executable` vocabulary; reads cleanly in the doctor list.
2. **Plumb the configured executable via `RunToolchainPreflight` → `run_preflight` → embedded component,
   NOT via a `build_steps` signature change.** `apply_report(report)` / `build_steps(report, expanded)`
   are called at ~25 sites with **no settings access** (`install_wizard/state.rs:235`). Adding a param
   would churn every call site. Instead, read `settings.toolchain.web_browser_executable` where the
   preflight is dispatched (`navigation.rs` `handle_show`, `actions.rs` `handle_rerun_preflight`), carry
   it on `RunToolchainPreflight`, pass it to `run_preflight`, and let the daemon embed the result as a
   `WebBrowser` `ComponentCheck`. **`build_steps` stays pure-on-report — no signature change.** This is
   the established pattern (`linux_package_manager` / `winget_available` are pre-computed in
   `run_preflight` for exactly this reason — `toolchain/mod.rs:164-177`).
3. **Daemon reports raw `Missing`; the app caps it to `Partial`** at the Web leaf in `build_steps`
   (`rollup_status` returns `Missing` for a missing component — Android still needs true `Missing`, so
   the cap is local to the Web leaf, not in `rollup_status`).
4. **Component count stays a fixed `10` on all hosts** for Phase 3 (Web is cross-host — always pushed,
   returns `Unknown` on `HostPlatform::Unknown` but the slot still exists). The
   `assert_eq!(report.components.len(), 9)` at `toolchain/mod.rs:253` becomes `10`. It only becomes truly
   host-variable when Phases 4–5 add host-gated probes (Xcode macOS-only, VS Windows-only).

### Why these task boundaries

- `ComponentKind::WebBrowser` hard-errors at two exhaustive `match` sites — the daemon `Display` impl
  (`types.rs:104`) and `build_steps`'s component-routing match (`state.rs:938`). The daemon half
  (Task 01) is a self-contained compiling unit; the app routing arm lives in Task 03.
- **All `handler/install_wizard/actions.rs` edits go in Task 02** (the `handle_rerun_preflight` plumbing
  **and** the `handle_run_selected_step` `PlatformWeb` arm split). **All `install_wizard/state.rs` edits
  go in Task 03** (the Web leaf + guided-command builder). This keeps Task 02 and Task 03 **write-disjoint**
  so they parallelize in separate worktrees after Task 01.
- TUI rendering (Task 04) touches only `step_detail.rs` and depends on Task 03 for meaningful tests.

**Total Tasks:** 5
**Estimated Hours:** 9–13 hours

## Task Dependency Graph

```
                ┌────────────────────────────────────────┐
                │ 01-daemon-web-browser-detection          │   Wave 1
                │  ComponentKind::WebBrowser + checks/web.rs │
                │  + run_preflight wiring + count assertion │
                └───────────────┬──────────────────────────┘
                                │  (compiles + daemon tests green)
              ┌─────────────────┴──────────────────┐
              ▼                                     ▼            Wave 2 (parallel worktrees)
 ┌──────────────────────────────┐   ┌────────────────────────────────┐
 │ 02-app-config-preflight-plumb │   │ 03-app-build-steps-web-leaf    │
 │ config field + RunToolchain   │   │ build_steps Web leaf + guided  │
 │ Preflight + handler arms      │   │ commands (state.rs only)       │
 │ (actions.rs, mod.rs, nav.rs)  │   └──────────────┬─────────────────┘
 └───────────────┬──────────────┘                  │
                 │                                  ▼            Wave 3
                 │                  ┌────────────────────────────────┐
                 │                  │ 04-tui-web-caption-and-hint     │
                 │                  │ (step_detail.rs)                │
                 │                  └──────────────┬─────────────────┘
                 └──────────────┬──────────────────┘
                                ▼                                  Wave 4
                ┌────────────────────────────────────────┐
                │ 05-update-docs (doc_maintainer)          │
                │ ARCHITECTURE.md + CONFIGURATION.md       │
                └────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-daemon-web-browser-detection](tasks/01-daemon-web-browser-detection.md) | Not started | - | 3–4h | `fdemon-daemon/src/toolchain/{types,mod}.rs`, `toolchain/checks/{web,mod}.rs` |
| 2 | [02-app-config-preflight-plumbing](tasks/02-app-config-preflight-plumbing.md) | Not started | 1 | 2–3h | `fdemon-app/src/config/types.rs`, `handler/mod.rs`, `actions/mod.rs`, `handler/install_wizard/{navigation,actions}.rs` |
| 3 | [03-app-build-steps-web-leaf](tasks/03-app-build-steps-web-leaf.md) | Not started | 1 | 2–3h | `fdemon-app/src/install_wizard/state.rs` |
| 4 | [04-tui-web-caption-and-hint](tasks/04-tui-web-caption-and-hint.md) | Not started | 3 | 1–2h | `fdemon-tui/src/widgets/install_wizard/step_detail.rs` |
| 5 | [05-update-docs](tasks/05-update-docs.md) | Not started | 1, 2, 3, 4 | 1h | `docs/ARCHITECTURE.md`, `docs/CONFIGURATION.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01 | `crates/fdemon-daemon/src/toolchain/types.rs`, `crates/fdemon-daemon/src/toolchain/checks/web.rs` (new), `crates/fdemon-daemon/src/toolchain/checks/mod.rs`, `crates/fdemon-daemon/src/toolchain/mod.rs` | `toolchain/checks/{prerequisites,android}.rs` (templates) |
| 02 | `crates/fdemon-app/src/config/types.rs`, `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/actions/mod.rs`, `crates/fdemon-app/src/handler/install_wizard/navigation.rs`, `crates/fdemon-app/src/handler/install_wizard/actions.rs` | `toolchain/mod.rs` (new `run_preflight` arg), `install_wizard/types.rs` |
| 03 | `crates/fdemon-app/src/install_wizard/state.rs` | `toolchain/types.rs` (`ComponentKind::WebBrowser`), `install_wizard/types.rs` (`GuidedCommand`, `StepStatus`) |
| 04 | `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | `install_wizard/{types,state}.rs` |
| 05 | `docs/ARCHITECTURE.md`, `docs/CONFIGURATION.md` | task 01–04 files, `~/.claude/skills/doc-standards/schemas.md` |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | none (different crates; 02 depends on 01) | Sequential (01 → 02) |
| 01 + 03 | none (different crates; 03 depends on 01) | Sequential (01 → 03) |
| **02 + 03** | **none** (02 = `handler/**` + `config` + `actions/mod` + `handler/mod`; 03 = `install_wizard/state.rs`) | **Parallel (worktree)** after 01 |
| 04 vs 02/03 | none | Sequential (after 03) |
| 05 vs all | none | Sequential (after 01–04) |

> The single overlap risk the research flagged — both the preflight plumbing and the `PlatformWeb`
> handler-arm split touching `handler/install_wizard/actions.rs` — is **avoided by design**: Task 02 owns
> *every* `actions.rs` edit (plumbing **and** the arm split), Task 03 owns *only* `state.rs`. The
> `PlatformWeb` handler arm is written guided-commands-aware (it reads `selected_step().guided_commands`
> at runtime, not a compile dep on Task 03), so it is correct whether Task 02 or Task 03 merges first.

## Success Criteria

Phase 3 is complete when:

- [ ] `ComponentKind::WebBrowser` exists with a `Display` arm (`"Web Browser"`); the daemon `run_preflight`
      always emits a `WebBrowser` component (10 total cross-host), `Unknown` on `HostPlatform::Unknown`.
- [ ] `checks/web.rs` probes `browser_override` → `CHROME_EXECUTABLE` → per-OS defaults (Linux `which`
      google-chrome/chromium; macOS `/Applications/Google Chrome.app/...` via `PathBuf` not `which`;
      Windows Program Files / LocalAppData chrome.exe + `msedge` fallback); `Ok` with path/version when
      found, `Missing` when not.
- [ ] `[toolchain] web_browser_executable: Option<String>` config field (`#[serde(default)]`, `None`
      default); no collision with `[devtools] browser`. It is threaded into `run_preflight` via
      `RunToolchainPreflight` from both `handle_show` and `handle_rerun_preflight`.
- [ ] `build_steps` routes `WebBrowser` onto the `PlatformWeb` leaf; the leaf's status is
      `rollup_status(&web_components)` **capped so `Missing → Partial`** (never blocks); guided commands
      are populated per-OS when the browser is absent and empty when `Ok`.
- [ ] A missing browser is **non-blocking**: `flutter_now_live()` / `close_wizard_and_dispatch_discovery`
      are unaffected (they read only `FlutterSdk`); the Platforms parent rolls up to at most `Partial`.
- [ ] `handle_run_selected_step` has a dedicated `PlatformWeb` arm (guided-only `none()`), no longer the
      "Available in a later phase" placeholder; iOS/macOS/Windows keep the placeholder.
- [ ] TUI: the Web leaf renders a caption + guided-command block with the `c`-copy hint; the "coming soon"
      hint is suppressed when the Web leaf has guided commands (no dual-CTA).
- [ ] `cargo test --workspace --lib` green; `cargo fmt --all` + `cargo clippy --workspace -- -D warnings` clean.
- [ ] `docs/ARCHITECTURE.md` + `docs/CONFIGURATION.md` document the live Web leaf, the `WebBrowser` check,
      the non-blocking semantics, and `web_browser_executable`.

## Notes

- **Web never blocks handback.** Verified: `flutter_now_live` (`state.rs:325`) checks only
  `ComponentKind::FlutterSdk == Ok`; `close_wizard_and_dispatch_discovery` (`actions.rs:97`) gates on
  `flutter_executable().is_some()`. Neither reads `WebBrowser`. Do **not** add a Web check to either.
- **`all_components_ok` (`state.rs:203`) intentionally stays strict.** It iterates *all* components, so a
  `Partial` WebBrowser makes the TUI "All set" subtitle not fire — correct (a toolchain with no browser
  isn't "all set" for web). Do **not** special-case WebBrowser out of it; document the behaviour instead.
- **`observed_unhealthy` latch (`state.rs:239`)** will latch `true` on a `Partial` WebBrowser. Irrelevant
  for Bootstrap (handback unaffected). Minor `UserInvoked` `show_installed_hint` edge case — note only,
  do **not** fix in Phase 3.
- **Linux Chrome guidance:** `google-chrome-stable` requires the Google apt repo; **Chromium** is the
  safer cross-distro recommendation. Guided commands should either note the repo step or point at the
  download URL, plus the `export CHROME_EXECUTABLE=...` alternative.
- **Guided command does not echo the configured path.** Keeping `build_steps` pure-on-report (Decision 2)
  means the `export CHROME_EXECUTABLE="<path>"` guided command is a template with a placeholder, not the
  user's configured value. Acceptable; flagged in Task 03 notes.
- **Locate by symbol, not line.** All line numbers are a current snapshot and will drift — find by
  symbol / test name / variant.
- **Website docs (`website/src/pages/docs/toolchain.rs`) remain deferred** to the Phase-5 wrap-up docs
  task (per the Phase-2 TASKS.md note), to avoid rewriting the Platforms prose before iOS/macOS/Windows
  leaves carry content.
- **Runtime `CHROME_EXECUTABLE` propagation into session launch env is explicitly out of scope** (PLAN
  Phase 3 step 4 marks it optional / deferred to Phase 7).
