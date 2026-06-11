## Task: Update Documentation for Phase 4 (iOS + macOS leaves)

**Agent:** doc_maintainer

**Objective**: Update core project documentation to reflect the live iOS/macOS install-wizard leaves, the
new `checks/ios.rs` daemon probe, the `XcodeTools` + `CocoaPods` `ComponentKind` variants, the
shared-probe-two-leaves model, and the non-blocking (Missing→Partial) semantics.

**Depends on**: Tasks 01, 02, 03, 04.

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` — update the toolchain daemon + install_wizard descriptions for Phase 4.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules.
- Phase 4 task files 01–04 (change context).
- The relevant `docs/ARCHITECTURE.md` inline comments that Phase 3 last touched (the `toolchain/`,
  `checks/`, `install_wizard/state.rs`, `install_wizard/types.rs`, and `step_detail.rs` annotations).

### Change Context

1. **Daemon `toolchain/types.rs`**: `ComponentKind` gains `XcodeTools` (Display `"Xcode"`) and `CocoaPods`
   (Display `"CocoaPods"`) — macOS-only full-Xcode + CocoaPods detection, distinct from the existing
   `Prerequisites`-embedded Xcode-CLT signal.
2. **Daemon `toolchain/checks/ios.rs` (NEW)**: `check_ios(platform) -> Vec<ComponentCheck>` — empty
   off-macOS, two checks on macOS; probes `xcode-select -p` (full `Xcode.app`, not CLT),
   `xcodebuild -version`, license/EULA, `simctl`, and `pod --version`.
3. **Daemon `toolchain/mod.rs`**: `run_preflight` runs `check_ios` in the `tokio::join!` and `extend`s the
   components vec on macOS (12 components on macOS, 10 elsewhere); the `>= 10` assertion is unchanged.
4. **App `install_wizard/state.rs`**: `build_steps` routes `XcodeTools`/`CocoaPods` into both a
   `platform_ios_components` and a `platform_macos_components` bucket (cloned), caps each leaf
   `Missing → Partial`, and emits live `PlatformIos`/`PlatformMacos` leaves (replacing the Phase-2
   placeholders) inside the macOS host-gate. New `xcode_guided_commands(report, status,
   include_ios_platform)` builder. `all_components_ok()` stays strict (documented as non-blocking for
   handback).
5. **App `install_wizard/types.rs`**: `PlatformIos`/`PlatformMacos` graduate from host-gated placeholders
   to live detect+guided leaves (update the variant doc-comment that currently says Phases 4–5 add them).
6. **App `handler/install_wizard/actions.rs`**: `handle_run_selected_step` splits the placeholder arm —
   `PlatformIos`/`PlatformMacos` become guided-only (like `PlatformWeb`); `PlatformWindows` stays a
   placeholder.
7. **TUI `step_detail.rs`**: `step_caption()` + `render_action_hint()` gain iOS/macOS arms; the
   "coming soon" hint is suppressed when the leaf has guided commands.

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` accurately reflects the Phase 4 changes (new `ComponentKind` variants, the
   `checks/ios.rs` probe, the macOS-gated component push, the shared-probe-two-leaves model, the
   non-blocking Missing→Partial cap, and the graduated iOS/macOS leaves).
2. The `install_wizard/types.rs` variant annotation no longer describes iOS/macOS as Phase-4-pending —
   it describes them as live (Windows remains the Phase-5 placeholder).
3. No content boundary violations (architecture content only in ARCHITECTURE.md).
4. Targeted edits to the existing inline annotations — do not rewrite whole sections.

### Notes

- Follow content boundaries strictly — see `~/.claude/skills/doc-standards/schemas.md`.
- **`docs/CONFIGURATION.md` needs no change** — Phase 4 adds no config field (iOS/macOS guided commands
  are hardcoded; `web_browser_executable` is the only toolchain browser knob).
- **`docs/KEYBINDINGS.md` needs no change** — no new keybindings (iOS/macOS reuse the Phase-3 guided-leaf
  keys: `Enter` hint, `c` copy, `[`/`]` cycle, `r` re-check).
- **Website docs (`website/src/pages/docs/toolchain.rs`) stay deferred** to the Phase-5 wrap-up docs task
  (per the Phase-2/3 notes) — do not touch them here.
- Mirror the style of the Phase-3 doc updates (commits documenting the live `PlatformWeb` leaf + the
  `WebBrowser` check).

---

## Completion Summary

**Status:** Not Started
