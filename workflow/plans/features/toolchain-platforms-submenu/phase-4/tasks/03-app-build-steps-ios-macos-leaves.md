## Task: App `build_steps` — graduate iOS + macOS leaves (shared Xcode/CocoaPods, capped, guided)

**Objective**: Replace the `PlatformIos` / `PlatformMacos` placeholder leaves in `build_steps` with live
detect + guided-only leaves. Route the daemon's `XcodeTools` + `CocoaPods` components into **two** buckets
(cloned, one per leaf), derive each leaf's status with a `Missing → Partial` non-blocking cap, populate
per-leaf guided commands via a new Xcode/CocoaPods guided-command builder (iOS gets the extra
`xcodebuild -downloadPlatform iOS` command; macOS omits it), and fill the leaf bodies inside the existing
`if report.platform == HostPlatform::MacOs` host-gate block. Replaces the Task 01 no-op stub arm.

**Depends on**: Task 01 (merged) — needs `ComponentKind::XcodeTools` + `CocoaPods` and the stub arm.

**Agent:** implementor

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/state.rs` — component bucketing, status cap, guided-command
  builder(s), leaf-body construction; replace the Task 01 stub arm; add unit tests.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs` — `ComponentKind::XcodeTools` / `CocoaPods`,
  `ComponentStatus`.
- `crates/fdemon-app/src/install_wizard/types.rs` — `GuidedCommand`, `StepStatus`, `WizardStepKind`.
- The existing `web_browser_guided_commands`, `platform_web_components` routing, and `web_status` cap in
  the same file (the exact analog to mirror).

### Details

> Line numbers are a current snapshot and will drift — locate by symbol/variant.

#### 1. Component bucketing — replace the Task 01 stub arm

In the `for check in &report.components { match check.kind { ... } }` loop, replace the stub
`ComponentKind::XcodeTools | ComponentKind::CocoaPods => {}` with routing that **clones** each check into
**both** an iOS bucket and a macOS bucket (shared probe, displayed under two leaves):

```rust
// declared alongside platform_web_components, etc.
let mut platform_ios_components: Vec<ComponentCheck> = Vec::new();
let mut platform_macos_components: Vec<ComponentCheck> = Vec::new();

// in the routing match:
ComponentKind::XcodeTools | ComponentKind::CocoaPods => {
    platform_ios_components.push(check.clone());
    platform_macos_components.push(check.clone());
}
```

#### 2. Status derivation — `Missing → Partial` cap (mirror `web_status`)

```rust
let ios_status = if platform_ios_components.is_empty() {
    StepStatus::Pending
} else {
    let raw = rollup_status(&platform_ios_components);
    if raw == StepStatus::Missing { StepStatus::Partial } else { raw }
};
let macos_status = if platform_macos_components.is_empty() {
    StepStatus::Pending
} else {
    let raw = rollup_status(&platform_macos_components);
    if raw == StepStatus::Missing { StepStatus::Partial } else { raw }
};
```

(Both derive from identical data, so `ios_status == macos_status` always — kept as two bindings for
symmetry and so the leaf bodies read cleanly.)

#### 3. Guided-command builder (mirror `web_browser_guided_commands`)

Add a shared builder, parameterized by whether the iOS-only simulator command is included:

```rust
/// Guided commands for the Apple-platform leaves. `include_ios_platform` adds the iOS-only
/// `xcodebuild -downloadPlatform iOS`. Empty unless the leaf status is Partial (a tool is absent).
fn xcode_guided_commands(
    report: &ToolchainReport,
    status: StepStatus,
    include_ios_platform: bool,
) -> Vec<GuidedCommand> {
    if status != StepStatus::Partial {
        return Vec::new();
    }
    let mut cmds = Vec::new();
    let xcode_missing = report.components.iter()
        .any(|c| c.kind == ComponentKind::XcodeTools && c.status != ComponentStatus::Ok);
    let cocoapods_missing = report.components.iter()
        .any(|c| c.kind == ComponentKind::CocoaPods && c.status != ComponentStatus::Ok);

    if xcode_missing {
        // Install Xcode (App Store or xcodes)
        cmds.push(GuidedCommand {
            label: "Install Xcode".to_string(),
            command: "open \"https://apps.apple.com/us/app/xcode/id497799835\"".to_string(),
            note: Some("Or: brew install --cask xcodes && xcodes install --latest".to_string()),
        });
        // Point the active developer dir at the full Xcode (not CLT) + first-launch + license
        cmds.push(GuidedCommand {
            label: "Select Xcode & accept license".to_string(),
            command: "sudo xcode-select -s /Applications/Xcode.app/Contents/Developer \
                      && sudo xcodebuild -runFirstLaunch \
                      && sudo xcodebuild -license accept".to_string(),
            note: None,
        });
        if include_ios_platform {
            cmds.push(GuidedCommand {
                label: "Download the iOS platform".to_string(),
                command: "xcodebuild -downloadPlatform iOS".to_string(),
                note: None,
            });
        }
    }
    if cocoapods_missing {
        cmds.push(GuidedCommand {
            label: "Install CocoaPods".to_string(),
            command: "brew install cocoapods".to_string(),
            note: Some("Or: sudo gem install cocoapods".to_string()),
        });
    }
    cmds
}
```

Thin wrappers (or call sites pass the flag directly):
- iOS leaf → `xcode_guided_commands(report, ios_status, true)`
- macOS leaf → `xcode_guided_commands(report, macos_status, false)`

> Exact command text is guidance — keep it accurate to current Flutter docs, copy-paste-safe, and clearly
> `sudo`-prefixed where privilege is required. Split the long `&&` chain into separate `GuidedCommand`s if
> that renders better in the detail pane (the TUI command block windows multiple commands).

#### 4. Leaf-body construction — fill the macOS host-gate block

Inside the existing `if report.platform == HostPlatform::MacOs { ... }` block, replace the two `Pending`
placeholder `WizardStep` pushes with live bodies (mirror the `PlatformWeb` leaf push):

```rust
if report.platform == HostPlatform::MacOs {
    leaves.push(WizardStep {
        kind: WizardStepKind::PlatformIos,
        title: "iOS".to_string(),
        status: ios_status,
        guided_commands: xcode_guided_commands(report, ios_status, true),
        components: platform_ios_components,
        indent: 1,
    });
    leaves.push(WizardStep {
        kind: WizardStepKind::PlatformMacos,
        title: "macOS".to_string(),
        status: macos_status,
        guided_commands: xcode_guided_commands(report, macos_status, false),
        components: platform_macos_components,
        indent: 1,
    });
}
```

(Note the move order — build the guided commands before moving the `components` Vec into the struct, or
clone, to satisfy the borrow checker.)

#### 5. Parent rollup — no change

`platforms_parent_status = rollup_step_statuses(&leaf_statuses)` already includes the iOS/macOS leaf
statuses (they're in `platform_leaves`). Two equal `Partial` values roll up to `Partial` — same as one.
Do **not** alter `rollup_step_statuses` or `rollup_status`.

### Acceptance Criteria

1. On a macOS report, `build_steps(..., expanded=true)` emits live `PlatformIos` and `PlatformMacos`
   leaves whose `components` each contain the `XcodeTools` + `CocoaPods` checks (cloned), `indent == 1`.
2. When a tool is `Missing` in the report, the corresponding leaf status is **`Partial`** (capped), never
   `Missing`; when both are `Ok`, the leaf status is `Ok`; when the components are absent (non-macOS
   report), the leaf is not emitted at all.
3. Guided commands are non-empty only when the leaf status is `Partial`; the iOS leaf includes
   `xcodebuild -downloadPlatform iOS`, the macOS leaf does not; `brew install cocoapods` appears only when
   `CocoaPods` is not `Ok`; Xcode commands appear only when `XcodeTools` is not `Ok`.
4. The Platforms parent rolls up to at most `Partial` from iOS/macOS; `flutter_now_live()` and
   `close_wizard_and_dispatch_discovery` are unaffected (they read only `FlutterSdk`).
5. On a non-macOS report, no `XcodeTools`/`CocoaPods` components exist and no iOS/macOS leaves appear.
6. `cargo test -p fdemon-app --lib` green; `cargo fmt --all` + `cargo clippy -p fdemon-app -- -D warnings`
   clean.

### Testing

Build `ToolchainReport` fixtures with `platform: HostPlatform::MacOs` and synthetic `XcodeTools`/
`CocoaPods` `ComponentCheck`s (mirror the existing Web-leaf tests):
- `test_ios_macos_leaves_present_on_macos_expanded` — both leaves emitted with `indent == 1` and both
  components in each leaf.
- `test_ios_macos_missing_xcode_caps_to_partial` — `XcodeTools = Missing` → both leaf statuses `Partial`,
  guided commands non-empty.
- `test_ios_leaf_includes_download_platform_macos_does_not` — assert the iOS leaf's commands contain
  `xcodebuild -downloadPlatform iOS` and the macOS leaf's do not.
- `test_xcode_ok_yields_no_guided_commands` — both `Ok` → leaf status `Ok`, guided commands empty.
- `test_cocoapods_only_missing_emits_only_cocoapods_command` — `XcodeTools = Ok`, `CocoaPods = Missing`
  → status `Partial`, commands contain only the CocoaPods command.
- `test_no_ios_macos_leaves_on_linux_report` — `platform: Linux`, no XcodeTools/CocoaPods components →
  no iOS/macOS leaves, no panic.
- `test_platforms_parent_rolls_up_to_partial_from_xcode` — parent status `Partial` when Xcode missing,
  Flutter SDK Ok (handback path unaffected — assert `flutter_now_live()` true).

### Notes

- **`all_components_ok()` intentionally stays strict** (Phase 3 Web precedent). A `Partial`/`Missing`
  `XcodeTools` makes it return `false` on a macOS host lacking Xcode — correct, and **non-blocking**:
  handback gates on `flutter_now_live()`/`flutter_executable()`, not `all_components_ok()`. Do **not**
  special-case `XcodeTools`/`CocoaPods`; add a one-line code comment + this note for the reviewer.
- **Shared probe, no double-count.** Cloning the same checks into both buckets is deliberate: the two
  leaf statuses are always equal, so the parent rollup is unaffected. Do not try to merge into one entry.
- **Guided command text does not echo a configured path** (there is none for Xcode). The commands are
  fixed templates with the canonical `/Applications/Xcode.app` path; users adjust if their Xcode lives
  elsewhere. Acceptable — flag in the completion summary.
- **`is_executable` / Enter routing** is owned by the TUI (Task 04) and handler (Task 02); this task
  only produces the data. Keep `build_steps` pure-on-report.

---

## Completion Summary

**Status:** Not Started
