# Phase 4 — iOS + macOS leaves (shared Xcode/CocoaPods) — Task Index

## Overview

Graduate the **iOS** and **macOS** platform leaves from their Phase-2 inert placeholders
(`StepStatus::Pending`, no components, no guided commands) into **live detect + guided-only** steps,
**macOS-host-only**, that **never block** the toolchain-healthy handback. Two new
`ComponentKind` variants — `XcodeTools` and `CocoaPods` — are produced by one new macOS-gated probe
(`checks/ios.rs`: `xcode-select -p`, `xcodebuild -version`, license/EULA, `simctl`, `pod --version`).
The **same two component checks are displayed under both leaves** (shared probe, no double-counting in
the rollup). Per-leaf guided commands tell the user how to install/configure Xcode (App Store / `xcodes`,
`xcode-select -s`, `xcodebuild -runFirstLaunch`, `xcodebuild -license accept`, `xcodebuild
-downloadPlatform iOS` for iOS only) and CocoaPods (`brew install cocoapods`). Missing Xcode/CocoaPods
surfaces as **Partial (warning)**, never **Missing** — non-blocking, exactly like Web.

> iOS/macOS are **detection + guided-only** — there is NO auto-installer (Xcode is GUI/App-Store,
> privileged). The leaves are never executable; `Enter` shows guided commands, `c` copies the selected
> command, `[`/`]` cycle commands, `r` re-checks. **No new keybindings, no new config field.**

### Decisions resolved by research (verified against source)

1. **Two new `ComponentKind` variants: `XcodeTools` and `CocoaPods`.** `Display` → `"Xcode"` and
   `"CocoaPods"`. These are **distinct** from the existing macOS `Prerequisites`-embedded Xcode-CLT /
   CocoaPods detection (`PREREQ_KEY_XCODE_CLT` / `PREREQ_KEY_COCOAPODS`): Phase 4 probes **full Xcode**
   (`xcode-select -p` pointing at `Xcode.app`, `xcodebuild -version`, license accepted, `simctl`
   reachable), not just the command-line tools. The Prerequisites Xcode-CLT signal stays as-is.
2. **One shared probe, two components, displayed under two leaves.** The daemon runs the Xcode/CocoaPods
   probe **once** and emits one `XcodeTools` + one `CocoaPods` `ComponentCheck`. `build_steps` **clones**
   both checks into a `platform_ios_components` bucket **and** a `platform_macos_components` bucket
   (mirrors `platform_web_components.push(check.clone())`). Because both leaves derive from identical
   data, their statuses are always equal, so the Platforms-parent rollup (`rollup_step_statuses` over
   leaf statuses) sees two equal values and **does not double-penalize** — no special-casing needed.
3. **macOS-host-gated at the component-push level.** `run_preflight` adds the probe via a new
   `check_ios(&platform) -> Vec<ComponentCheck>` that returns **empty off-macOS** and **two checks on
   macOS**; the components vec is `extend`ed with it. Count is **10 on Linux/Windows, 12 on macOS**.
   The `components.len()` assertion is **already** `>= 10` (forward-compat, landed in Phase 3) — no
   change. `build_steps` already host-gates the iOS/macOS *leaves* at runtime via
   `report.platform == HostPlatform::MacOs`, so off-macOS the leaves are absent and the components never
   exist — consistent.
4. **Daemon reports raw `Missing`; the app caps it to `Partial`** at each leaf in `build_steps` (same
   `Missing → Partial` cap the Web leaf uses). Android still needs true `Missing`, so the cap is local
   to the iOS/macOS leaves, not in `rollup_status`.
5. **No new config field.** Unlike Web's `web_browser_executable`, Xcode has no configurable path the
   wizard needs to drive — the guided commands are hardcoded. `RunToolchainPreflight` gains **no** new
   field; detection happens entirely inside the daemon's `run_preflight`.

### Why these task boundaries

- `ComponentKind::XcodeTools` + `CocoaPods` hard-error at exactly **two** exhaustive `match` sites — the
  daemon `Display` impl (`types.rs`) and `build_steps`'s component-routing match (`state.rs`). The
  daemon half (Task 01) is a self-contained compiling unit; it lands a **minimal no-op stub arm** in
  `state.rs` (route the two kinds nowhere — leaves stay `Pending` placeholders) so the workspace
  compiles. Task 03 replaces that stub with real bucketing + leaf bodies. (This mirrors Phase 3 Task 01's
  cross-crate-stub approach exactly.)
- **All `handler/install_wizard/actions.rs` edits go in Task 02** (split the placeholder iOS/macOS arm
  into guided-only arms). **All `install_wizard/state.rs` edits go in Task 03** (buckets, cap, guided
  builders, leaf bodies). This keeps Task 02 and Task 03 **write-disjoint** so they parallelize in
  separate worktrees after Task 01.
- TUI rendering (Task 04) touches only `step_detail.rs` and depends on Task 03 for meaningful tests.
- Docs (Task 05, `doc_maintainer`) update `docs/ARCHITECTURE.md` after the code lands.

**Total Tasks:** 5
**Estimated Hours:** 9–13 hours

## Task Dependency Graph

```
                ┌──────────────────────────────────────────────┐
                │ 01-daemon-xcode-cocoapods-detection            │   Wave 1
                │  ComponentKind::XcodeTools + CocoaPods         │
                │  + checks/ios.rs + run_preflight macOS-gate    │
                │  + state.rs no-op stub arm (keeps ws compiling)│
                └───────────────────┬──────────────────────────┘
                                    │  (compiles + daemon tests green)
              ┌─────────────────────┴──────────────────┐
              ▼                                         ▼            Wave 2 (parallel worktrees)
 ┌──────────────────────────────────┐   ┌────────────────────────────────────┐
 │ 02-app-handler-ios-macos-arms      │   │ 03-app-build-steps-ios-macos-leaves │
 │ split placeholder arm → guided-only │   │ buckets + Missing→Partial cap +     │
 │ iOS/macOS arms (actions.rs only)    │   │ xcode guided builders + leaf bodies │
 │                                     │   │ (state.rs only)                     │
 └─────────────────┬──────────────────┘   └──────────────────┬─────────────────┘
                   │                                          │
                   │                                          ▼            Wave 3
                   │                          ┌────────────────────────────────────┐
                   │                          │ 04-tui-ios-macos-caption-and-hint   │
                   │                          │ (step_detail.rs)                    │
                   │                          └──────────────────┬─────────────────┘
                   └──────────────────┬──────────────────────────┘
                                      ▼                                  Wave 4
                ┌──────────────────────────────────────────────┐
                │ 05-update-docs (doc_maintainer)                │
                │ ARCHITECTURE.md                                │
                └──────────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-daemon-xcode-cocoapods-detection](tasks/01-daemon-xcode-cocoapods-detection.md) | ✅ Done (validated) | - | 3–4h | `fdemon-daemon/src/toolchain/{types,mod}.rs`, `toolchain/checks/{ios,mod}.rs` (+ minimal `fdemon-app/install_wizard/state.rs` stub arm) |
| 2 | [02-app-handler-ios-macos-arms](tasks/02-app-handler-ios-macos-arms.md) | ✅ Done (validated, merged) | 1 | 1–2h | `fdemon-app/src/handler/install_wizard/actions.rs` |
| 3 | [03-app-build-steps-ios-macos-leaves](tasks/03-app-build-steps-ios-macos-leaves.md) | ✅ Done (validated, merged) | 1 | 3–4h | `fdemon-app/src/install_wizard/state.rs` |
| 4 | [04-tui-ios-macos-caption-and-hint](tasks/04-tui-ios-macos-caption-and-hint.md) | ✅ Done (validated) | 3 | 1–2h | `fdemon-tui/src/widgets/install_wizard/step_detail.rs` |
| 5 | [05-update-docs](tasks/05-update-docs.md) | ✅ Done (validated) | 1, 2, 3, 4 | 1h | `docs/ARCHITECTURE.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01 | `crates/fdemon-daemon/src/toolchain/types.rs`, `crates/fdemon-daemon/src/toolchain/checks/ios.rs` (new), `crates/fdemon-daemon/src/toolchain/checks/mod.rs`, `crates/fdemon-daemon/src/toolchain/mod.rs`, `crates/fdemon-app/src/install_wizard/state.rs` (no-op stub arm only) | `toolchain/checks/{web,prerequisites}.rs` (templates) |
| 02 | `crates/fdemon-app/src/handler/install_wizard/actions.rs` | `install_wizard/state.rs` (reads `selected_step().guided_commands` at runtime), `install_wizard/types.rs` |
| 03 | `crates/fdemon-app/src/install_wizard/state.rs` | `toolchain/types.rs` (`ComponentKind::XcodeTools`/`CocoaPods`), `install_wizard/types.rs` (`GuidedCommand`, `StepStatus`), `checks/web.rs` arm shape (analog) |
| 04 | `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | `install_wizard/{types,state}.rs` |
| 05 | `docs/ARCHITECTURE.md` | task 01–04 files, `~/.claude/skills/doc-standards/schemas.md` |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | none (01 = daemon + state.rs stub; 02 = actions.rs) — 02 depends on 01 | Sequential (01 → 02) |
| 01 + 03 | **`install_wizard/state.rs`** (01 lands a no-op stub arm; 03 rewrites it) — 03 depends on 01 | Sequential (01 → 03) |
| **02 + 03** | **none** (02 = `handler/install_wizard/actions.rs`; 03 = `install_wizard/state.rs`) | **Parallel (worktree)** after 01 |
| 04 vs 02/03 | none | Sequential (after 03) |
| 05 vs all | none | Sequential (after 01–04) |

> The one overlap on `state.rs` is **between Task 01 and Task 03**, which are **sequential by
> dependency** (03 builds on 01's merged result and replaces the stub arm) — so there is no parallel
> conflict. Task 02 and Task 03 are the only wave-peers and they are **write-disjoint** (`actions.rs` vs
> `state.rs`), so they run in parallel worktrees. The Task 02 iOS/macOS arms read
> `selected_step().guided_commands` at **runtime** (not a compile dep on Task 03's builders), so they are
> correct whether Task 02 or Task 03 merges first.

## Success Criteria

Phase 4 is complete when:

- [ ] `ComponentKind::XcodeTools` and `ComponentKind::CocoaPods` exist with `Display` arms (`"Xcode"`,
      `"CocoaPods"`); a new `checks/ios.rs` probe emits both as `ComponentCheck`s.
- [ ] `run_preflight` runs the Xcode/CocoaPods probe **only on macOS** (`HostPlatform::MacOs`); the two
      components are present on macOS (count 12), absent on Linux/Windows (count 10). The `>= 10`
      assertion still holds; macOS presence is asserted under `#[cfg(target_os = "macos")]`.
- [ ] `checks/ios.rs` probes `xcode-select -p` (must point at a full `Xcode.app`, not bare CLT),
      `xcodebuild -version`, license/EULA acceptance, `simctl` reachability (→ `XcodeTools`), and
      `pod --version` (→ `CocoaPods`); `Ok` with detail when found, `Missing` when absent, `Unknown`
      for `HostPlatform::Unknown`. Probes respect `PROBE_TIMEOUT` and never panic.
- [ ] `build_steps` routes `XcodeTools` + `CocoaPods` onto **both** the `PlatformIos` and `PlatformMacos`
      leaves (cloned into two buckets); each leaf's status is `rollup_status(...)` **capped so
      `Missing → Partial`** (never blocks); guided commands are populated per-leaf when a tool is absent
      and empty when both `Ok`.
- [ ] iOS leaf guided commands include the iOS-only `xcodebuild -downloadPlatform iOS`; the macOS leaf
      omits it. Both include Xcode install/configure + `brew install cocoapods` (when CocoaPods absent).
- [ ] A missing Xcode/CocoaPods is **non-blocking**: `flutter_now_live()` /
      `close_wizard_and_dispatch_discovery` are unaffected (they read only `FlutterSdk`); the Platforms
      parent rolls up to at most `Partial`.
- [ ] `handle_run_selected_step` has dedicated `PlatformIos` / `PlatformMacos` guided-only arms (mirror
      `PlatformWeb`'s `has_guided` guard, return `none()`, never `begin_step`/`RunWizardStep`), no longer
      the "Available in a later phase" placeholder; `PlatformWindows` keeps the placeholder.
- [ ] TUI: the iOS/macOS leaves render a caption + guided-command block with the `c`-copy hint; the
      "coming soon" hint is suppressed when the leaf has guided commands (no dual-CTA), and shown when
      Xcode/CocoaPods are fully `Ok` (display-only).
- [ ] Host-inapplicable: on Linux/Windows the iOS/macOS leaves are **absent** and their components never
      exist; rollups and navigation account only for visible rows.
- [ ] `cargo test --workspace --lib` green; `cargo fmt --all` + `cargo clippy --workspace -- -D warnings` clean.
- [ ] `docs/ARCHITECTURE.md` documents the live iOS/macOS leaves, the `checks/ios.rs` probe, the new
      `ComponentKind` variants, the shared-probe-two-leaves model, and the non-blocking semantics.

## Keyboard Shortcuts

No new keybindings. The iOS/macOS leaves reuse the existing guided-leaf keys established in Phase 3:

| Key | Mode | Action |
|-----|------|--------|
| `Enter` | InstallWizard (iOS/macOS leaf selected) | Show guided-command hint message (guided-only; no install) |
| `c` | InstallWizard (iOS/macOS leaf selected) | Copy the selected guided command to clipboard |
| `[` / `]` | InstallWizard (iOS/macOS leaf selected) | Cycle the selected guided command |
| `r` | InstallWizard | Re-run preflight (re-check Xcode/CocoaPods) |

## Notes

- **iOS/macOS never block handback.** Verified: `flutter_now_live` checks only
  `ComponentKind::FlutterSdk == Ok`; `close_wizard_and_dispatch_discovery` gates on
  `flutter_executable().is_some()`. Neither reads `XcodeTools`/`CocoaPods`. Do **not** add them to either.
- **`all_components_ok()` intentionally stays strict** (matches the Phase 3 Web precedent). It iterates
  *all* `report.components`, so a `Partial`/`Missing` `XcodeTools` makes the TUI "All set" subtitle not
  fire on a macOS host without Xcode — which is correct (a macOS toolchain with no Xcode isn't "all set"
  for Apple platforms). Do **not** special-case `XcodeTools`/`CocoaPods` out of it; the handback gate is
  separate (`flutter_now_live`) and is unaffected. Document the behaviour in Task 03's notes.
- **Shared probe, no double-count.** The same `XcodeTools`/`CocoaPods` checks appear under both leaves.
  Because the two leaf statuses are always equal (identical underlying data), `rollup_step_statuses`
  over `[ios_status, macos_status, ...]` yields the same parent status as a single value — no
  double-penalization. Do **not** try to merge the two leaves into one rollup entry.
- **Full Xcode, not CLT.** The existing `PREREQ_KEY_XCODE_CLT` detection inside `Prerequisites` checks
  command-line tools. Phase 4's `XcodeTools` checks **full Xcode**: `xcode-select -p` must resolve to a
  `Contents/Developer` under an `Xcode.app` (not `/Library/Developer/CommandLineTools`), `xcodebuild
  -version` must succeed, and the license must be accepted. The two signals are independent and both
  legitimately surface (CLT under Prerequisites, full Xcode under iOS/macOS).
- **Guided commands emit only when the leaf status is `Partial`** (the capped form of a determined-absent
  tool), mirroring `web_browser_guided_commands`'s `if status != Partial { return Vec::new(); }`
  early-out. `Ok` (all present) and `Pending` (no signal) both yield no commands. Within the builder,
  inspect the individual `XcodeTools` / `CocoaPods` component statuses to decide which commands to emit
  (Xcode commands when `XcodeTools` not `Ok`; `brew install cocoapods` when `CocoaPods` not `Ok`).
- **`xcodebuild`/`xcode-select` setup commands require `sudo`.** The guided commands are display-only
  copy-paste text — fdemon never runs them. Present them verbatim (e.g.
  `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`).
- **Locate by symbol, not line.** All line numbers in the task files are a current snapshot and will
  drift — find by symbol / test name / variant.
- **Website docs (`website/src/pages/docs/toolchain.rs`) remain deferred** to the Phase-5 wrap-up docs
  task (per the Phase-2/3 TASKS.md notes), to avoid rewriting the Platforms prose before the Windows leaf
  carries content.
- **Runtime propagation is out of scope** (Phase 7). Phase 4 is detection + guided display only.
