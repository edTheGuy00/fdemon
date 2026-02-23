# Plan: v1 Refinements

## TL;DR

Four refinement areas before release: (1) log view word wrap to eliminate horizontal scrolling, (2) fix settings launch tab "Add New Configuration" bug and add fuzzy modals for dart defines / extra args, (3) create GitHub Actions release workflow for macOS/Linux/Windows with install script, (4) update the website with Network Monitor documentation and sync all keybindings.

---

## Background

The app is feature-complete but needs polish before public release. Users currently must scroll horizontally to read long log lines, which is cumbersome. The settings launch tab has a navigation bug preventing "Add New Configuration" from being selected. The project has no CI/CD release pipeline or install script. The website is missing documentation for the Network Monitor panel and has stale/phantom keybindings.

---

## Affected Modules

### Phase 1: Log View Word Wrap
- `crates/fdemon-app/src/log_view_state.rs` — Add `wrap_mode: bool` field
- `crates/fdemon-app/src/handler/keys.rs` — Add `w` toggle keybinding
- `crates/fdemon-app/src/handler/scroll.rs` — Guard horizontal scroll when wrap is on
- `crates/fdemon-tui/src/widgets/log_view/mod.rs` — Conditional wrap rendering path
- `crates/fdemon-tui/src/widgets/log_view/tests.rs` — New wrap mode tests

### Phase 2: Settings Launch Tab Fixes
- `crates/fdemon-app/src/handler/settings_handlers.rs` — Fix item count off-by-one, handle "Add New" selection
- `crates/fdemon-app/src/settings_items.rs` — Handle "Add New" index in `get_selected_item()`
- `crates/fdemon-app/src/state.rs` — Add `fuzzy_modal: Option<FuzzyModalState>` to `SettingsViewState`
- `crates/fdemon-app/src/message.rs` — Add settings fuzzy modal messages
- `crates/fdemon-app/src/handler/keys.rs` — Route Enter on dart_defines/extra_args to fuzzy modal
- `crates/fdemon-app/src/config/settings.rs` — Add dart_defines/extra_args to `apply_launch_config_change()`
- `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` — Render fuzzy modal overlay

### Phase 3: GitHub Actions & Install Script
- `.github/workflows/release.yml` — **NEW** Release workflow
- `install.sh` — **NEW** Install script
- `Cross.toml` — **NEW** Cross-compilation config

### Phase 4: Website Updates
- `website/src/pages/docs/devtools.rs` — Add Network Monitor section
- `website/src/data.rs` — Add Network keybindings, fix phantom `l` binding, add Performance extras
- `website/src/pages/docs/keybindings.rs` — May need minor layout adjustments for new sections
- `website/src/pages/docs/installation.rs` — Update with install command

---

## Development Phases

### Phase 1: Log View Word Wrap

**Goal**: Logs wrap at window width by default, eliminating horizontal scrolling. Users can toggle wrap mode on/off.

#### Research Findings

Lines are currently rendered via `Paragraph::new(lines)` **without** `.wrap()` (explicitly commented as "WITHOUT wrapping" at `log_view/mod.rs:1196`). Each line passes through `apply_horizontal_scroll()` which clips to a character-level viewport and adds `←`/`→` indicators. Horizontal state is tracked per-session via `LogViewState.h_offset`.

Ratatui's `Paragraph` natively supports `.wrap(Wrap { trim: false })` which handles word wrapping at the widget boundary. The main complexity is that wrapped lines occupy multiple terminal rows, which affects `calculate_entry_lines()` and the scroll offset calculations.

#### Steps

1. **Add wrap mode state**
   - Add `wrap_mode: bool` to `LogViewState` (default `true` — wrap on by default)
   - Add `toggle_wrap_mode()` method
   - When `wrap_mode` is true, `scroll_left/right` become no-ops

2. **Add wrap toggle keybinding**
   - Map `w` in normal mode to `Message::ToggleWrapMode`
   - Add `ToggleWrapMode` to `Message` enum
   - Handler calls `log_view_state.toggle_wrap_mode()` and resets `h_offset` to 0

3. **Modify log view rendering**
   - In `log_view/mod.rs`, when `wrap_mode == true`:
     - Skip `apply_horizontal_scroll()` — pass raw lines directly
     - Use `Paragraph::new(lines).wrap(Wrap { trim: false })` instead of plain `Paragraph`
     - Import `ratatui::widgets::Wrap`
   - When `wrap_mode == false`: keep existing horizontal scroll behavior

4. **Fix line height calculation for scroll**
   - `calculate_entry_lines()` needs to account for wrapped line heights when `wrap_mode == true`
   - Each logical line's height becomes `ceil(line_char_width / visible_width)`
   - Pass `visible_width` and `wrap_mode` to the calculation
   - Update `total_lines` calculation at line 1036 accordingly

5. **Add status bar indicator**
   - Show a `[wrap]` or `[nowrap]` indicator in the status bar so users know the current mode

6. **Add tests**
   - Test wrap mode toggle
   - Test that horizontal scroll is disabled during wrap mode
   - Test line height calculation with wrapped lines
   - Test rendering output with wrap enabled

**Milestone**: Users see fully-visible log lines without horizontal scrolling. Press `w` to toggle between wrap and horizontal scroll modes.

---

### Phase 2: Settings Launch Tab Fixes

**Goal**: "Add New Configuration" is selectable, and dart defines / extra args use fuzzy modals consistent with the new session dialog.

#### Research Findings

**Bug: "Add New Configuration" unreachable** — `get_item_count_for_tab()` in `settings_handlers.rs:362-368` counts only the setting items for the LaunchConfig tab but does NOT add `+1` for the "Add New Configuration" button. Navigation wraps at `N` items, so `selected_index` can never reach `N` (which is the index where the button is rendered).

**Bug: No action when selected** — Even if selection reached it, `get_selected_item()` returns `None` for index `N` (out of bounds), and `handle_settings_toggle_edit` silently drops the action.

**Bug: Dart defines / extra args not persisted** — `apply_launch_config_change()` in `settings.rs:154-198` only handles name, device, mode, flavor, auto_start. The `dart_defines` and `extra_args` fields fall through to `_ => warn!()`.

**Fuzzy modal reuse** — The `FuzzyModalState` and `fuzzy_filter()` from `new_session_dialog/` are pure data types with no UI dependencies. The `FuzzyModal` widget in `fdemon-tui` is a standalone ratatui `Widget` that takes a `&FuzzyModalState` reference. Both are directly reusable in the settings panel.

#### Steps

1. **Fix item count off-by-one**
   - In `settings_handlers.rs:362-368`, add `+ 1` to the LaunchConfig item count sum
   - This allows `selected_index` to reach the "Add New Configuration" row

2. **Handle "Add New Configuration" selection**
   - In `settings_items.rs:get_selected_item()`, check if `selected_index == all_items.len()` and return a sentinel `SettingItem` with a special kind (e.g., `SettingKind::Action("add_config")`)
   - In `handle_settings_toggle_edit()`, when the selected item is the "Add New" action, dispatch `Message::LaunchConfigCreate`
   - Ensure `LaunchConfigCreate` handler creates a new config with defaults and saves it

3. **Add fuzzy modal state to Settings**
   - Add `fuzzy_modal: Option<FuzzyModalState>` to `SettingsViewState`
   - Add `SettingsFuzzyModalType` enum or reuse/extend `FuzzyModalType` with new variants for `DartDefines` and `ExtraArgs`
   - Add messages: `SettingsFuzzyOpen`, `SettingsFuzzyInput { c: char }`, `SettingsFuzzyBackspace`, `SettingsFuzzyConfirm`, `SettingsFuzzyCancel`, `SettingsFuzzyNavigateUp`, `SettingsFuzzyNavigateDown`

4. **Route dart defines / extra args to fuzzy modal**
   - When Enter/Space is pressed on a `dart_defines` or `extra_args` setting item, open the fuzzy modal instead of the inline edit mode
   - For `dart_defines`: open the `DartDefinesModal` widget (already exists in `new_session_dialog/dart_defines_modal.rs`) or a simplified version
   - For `extra_args`: open a `FuzzyModal` with `allows_custom: true` so users can type arbitrary args

5. **Fix persistence for dart defines / extra args**
   - Add `dart_defines` and `extra_args` branches to `apply_launch_config_change()` in `settings.rs`
   - When the fuzzy modal confirms, update the in-memory launch config AND persist to `.fdemon/launch.toml`

6. **Render fuzzy modal over settings panel**
   - In `settings_panel/mod.rs`, after rendering the settings content, check if `state.settings_view_state.fuzzy_modal.is_some()` and render the `FuzzyModal` widget as an overlay
   - The FuzzyModal widget already handles its own layout (bottom 45-50% of screen)

7. **Add tests**
   - Test that item count includes "+1" for add config button
   - Test that selecting the add config index triggers `LaunchConfigCreate`
   - Test fuzzy modal open/close lifecycle in settings
   - Test persistence of dart defines and extra args changes

**Milestone**: Users can navigate to and select "Add New Configuration". Editing dart defines and extra args opens a familiar fuzzy modal matching the new session dialog UX.

---

### Phase 3: GitHub Actions Release Workflow & Install Script

**Goal**: Automated cross-platform binary releases on git tags with a one-line install command.

#### Research Findings

The repo has one existing workflow (`e2e.yml`) for Docker-based E2E tests. No release workflow exists. The binary is named `fdemon` (Cargo.toml `[[bin]]`), version `0.1.0`. The website installation page explicitly says "Pre-built binaries are coming soon."

#### Target Matrix

| Target | Runner | Build Tool | Archive |
|--------|--------|-----------|---------|
| `x86_64-apple-darwin` | `macos-13` (Intel) | Native cargo | `.tar.gz` |
| `aarch64-apple-darwin` | `macos-latest` (M1) | Native cargo | `.tar.gz` |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | Native cargo | `.tar.gz` |
| `aarch64-unknown-linux-gnu` | `ubuntu-latest` | `cross` (Docker) | `.tar.gz` |
| `x86_64-pc-windows-msvc` | `windows-latest` | Native cargo | `.zip` |

**Why `cross` for Linux ARM only**: `cross` uses Docker and cannot target macOS. Windows needs `msvc` (not `gnu`). macOS and x86 Linux use native runners.

#### Steps

1. **Create release workflow** (`.github/workflows/release.yml`)
   - Trigger on tags matching `v[0-9]+.[0-9]+.[0-9]+`
   - 3 build jobs: `build-macos` (matrix: x86_64 + aarch64), `build-linux` (matrix: x86_64 native + aarch64 cross), `build-windows` (x86_64 only)
   - Each job: checkout, install rust, cache cargo, build `--release`, package artifact
   - `release` job: download all artifacts, generate SHA256 checksums, create GitHub Release via `softprops/action-gh-release@v2`
   - Artifact naming: `fdemon-v{VERSION}-{TARGET}.{tar.gz|zip}`

2. **Create Cross.toml** (workspace root)
   - Pin Docker image for `aarch64-unknown-linux-gnu`
   - Passthrough `RUST_BACKTRACE` and `CARGO_TERM_COLOR` env vars

3. **Create install script** (`install.sh`)
   - One-liner: `curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/main/install.sh | bash`
   - Detects OS (`uname -s`) and architecture (`uname -m`)
   - Maps to correct Rust target triple
   - Resolves latest version from GitHub API (or accepts explicit version arg)
   - Downloads from GitHub Releases, extracts, installs to `$HOME/.local/bin` (override via `$FDEMON_INSTALL_DIR`)
   - Shows PATH setup hint if install dir not in PATH
   - Uses `set -euo pipefail`, `mktemp -d` with trap cleanup, `install -m755`

4. **Update website installation page**
   - Replace "coming soon" placeholder with install command and platform download links
   - Show one-liner for macOS/Linux, direct download links for Windows

**Milestone**: Pushing a `v0.1.0` tag triggers automated builds across 5 targets, creates a GitHub Release with binaries + checksums, and users can install with a single curl command.

---

### Phase 4: Website Updates

**Goal**: Add Network Monitor documentation to DevTools page, fix phantom keybindings, add all missing keybinding entries.

#### Research Findings

**DevTools page (`devtools.rs`)**: Comprehensive coverage of Inspector, Layout Explorer, Performance Monitor, Debug Overlays, Browser DevTools, Connection States, Configuration. **Missing entirely: Network Monitor panel.** The Network panel is fully implemented in the codebase with its own panel, request table, request details, sub-tab switching, recording toggle, filter input mode, and history clearing.

**Keybindings data (`data.rs`)**: 16 sections covering all modes. **Issues found:**
- **Phantom binding**: `l` → "Layout Panel" in DevTools Panel Navigation — this does NOT exist in the codebase. The `DevToolsPanel` enum has only `Inspector`, `Performance`, `Network`. No Layout variant.
- **Missing `n` key**: Network panel switch (`n`) is not listed in Panel Navigation
- **Missing section**: No "DevTools — Network Monitor" keybinding section (14+ bindings)
- **Missing Performance bindings**: `s` (toggle allocation sort), `Left`/`Right` (frame navigation)
- **Missing filter input bindings** for Network panel (`/`, type, Backspace, Enter, Esc)

#### Steps

1. **Add Network Monitor section to DevTools page** (`devtools.rs`)
   - Add a new `<Section title="Network Monitor (n)">` between Performance Monitor and Debug Overlays
   - Document: request table layout, navigation, detail view with sub-tabs (General, Headers, Request Body, Response Body, Timing)
   - Document: recording toggle (Space), clear history (Ctrl+X), filter mode (/)
   - Add keybinding quick reference table for Network panel at the bottom section

2. **Fix DevTools Panel Navigation keybindings** (`data.rs`)
   - **Remove** phantom `l` → "Layout Panel" entry (doesn't exist in code)
   - **Add** `n` → "Network Panel" → "Switch to Network monitoring panel"
   - Update the section to match actual `DevToolsPanel` enum: Inspector (`i`), Performance (`p`), Network (`n`)

3. **Add "DevTools — Network Monitor" keybinding section** (`data.rs`)
   - New `KeybindingSection` with cyan color
   - Include all 14+ bindings: navigation (j/k/Up/Down, PgUp/PgDn, Enter, Esc), detail tabs (g/h/q/s/t), recording (Space), clear (Ctrl+X), filter (/)
   - Add separate "Network Filter Input" sub-section with Type/Backspace/Enter/Esc bindings

4. **Add missing Performance bindings** (`data.rs`)
   - Add `s` → "Sort Allocations" → "Toggle allocation table sort between BySize and ByInstances"
   - Add `Left` → "Previous Frame" → "Select previous frame in bar chart"
   - Add `Right` → "Next Frame" → "Select next frame in bar chart"

5. **Update installation page** (`installation.rs`)
   - Replace "Pre-built binaries are coming soon" with actual install instructions
   - Show the `curl | bash` one-liner for macOS/Linux
   - Show download links to GitHub Releases for Windows
   - Include `FDEMON_INSTALL_DIR` override documentation

**Milestone**: Website accurately documents all DevTools panels (including Network), all keybindings match the actual codebase, and installation instructions are live.

---

## Edge Cases & Risks

### Log View Wrap Mode
- **Risk:** Wrapped lines change total visible line count, breaking scroll position calculations
- **Mitigation:** Recalculate total wrapped height on each render frame (already done for `max_line_width`). Use Paragraph's built-in wrap which handles the rendering correctly.

- **Risk:** Very long single lines (e.g., JSON dumps) could dominate the viewport when wrapped
- **Mitigation:** This is acceptable behavior — wrapped mode shows all content, horizontal scroll mode is available for users who prefer truncated views. The `w` toggle provides user choice.

### Settings Fuzzy Modal
- **Risk:** Reusing `FuzzyModalState` from `new_session_dialog` creates coupling between unrelated UI features
- **Mitigation:** `FuzzyModalState` is a generic data structure with no new-session-dialog-specific logic. It's already in a separate module. The coupling is on the type, not the feature.

### GitHub Actions
- **Risk:** `cross` Docker builds for aarch64 may fail if C dependencies (crossterm, tokio) have complex native build requirements
- **Mitigation:** `cross` uses a complete sysroot and handles libc + common C dependencies well. The project's dependencies are all well-tested with cross. Pin the Docker image version in `Cross.toml` for reproducibility.

- **Risk:** macOS runner availability — `macos-13` (Intel) may be deprecated
- **Mitigation:** GitHub has committed to maintaining Intel macOS runners. Can fall back to cross-compiling from aarch64 macOS if needed (add `x86_64-apple-darwin` target and build both on `macos-latest`).

### Website
- **Risk:** Removing phantom `l` → "Layout Panel" keybinding may confuse users who read old docs
- **Mitigation:** The binding never worked in code. Removing it corrects the documentation.

---

## Keyboard Shortcuts Summary

### New Bindings

| Key | Mode | Action |
|-----|------|--------|
| `w` | Normal | Toggle wrap mode for log view |

### Documented (Already Existing, Missing from Website)

| Key | Mode | Action |
|-----|------|--------|
| `n` | DevTools | Switch to Network panel |
| `j/k/Up/Down` | Network | Navigate request list |
| `PgUp/PgDn` | Network | Page through request list |
| `Enter` | Network | Select/open request |
| `Esc` | Network (detail) | Deselect current request |
| `g/h/q/s/t` | Network (detail) | Switch detail sub-tabs |
| `Space` | Network | Toggle recording |
| `Ctrl+X` | Network | Clear network history |
| `/` | Network | Enter filter mode |
| `s` | Performance | Toggle allocation sort |
| `Left/Right` | Performance | Navigate frames |

---

## Success Criteria

### Phase 1 Complete When:
- [ ] Logs wrap at window width by default
- [ ] `w` key toggles between wrap and horizontal scroll modes
- [ ] Scroll position remains correct with wrapped lines
- [ ] Horizontal scroll keys (`h/l/0/$`) are no-ops when wrap is on
- [ ] All existing log view tests pass + new wrap mode tests added
- [ ] `cargo test --workspace` passes

### Phase 2 Complete When:
- [ ] "Add New Configuration" is selectable and creates a new config
- [ ] Dart defines editing opens a modal consistent with new session dialog
- [ ] Extra args editing opens a fuzzy modal with custom input
- [ ] Changes to dart defines and extra args persist to `.fdemon/launch.toml`
- [ ] All existing settings tests pass + new tests added
- [ ] `cargo test --workspace` passes

### Phase 3 Complete When:
- [ ] `release.yml` workflow exists and is syntactically valid
- [ ] Workflow builds for all 5 targets (macOS x86_64/aarch64, Linux x86_64/aarch64, Windows x86_64)
- [ ] Release creates artifacts with correct naming and checksums
- [ ] `install.sh` detects OS/arch and installs the correct binary
- [ ] Install script handles missing PATH gracefully

### Phase 4 Complete When:
- [ ] DevTools page documents the Network Monitor panel
- [ ] Phantom `l` → "Layout Panel" keybinding removed from `data.rs`
- [ ] `n` → "Network Panel" keybinding added
- [ ] All Network panel keybindings documented (14+ bindings)
- [ ] Missing Performance panel keybindings added (3 bindings)
- [ ] Installation page updated with install command
- [ ] Website builds successfully (`trunk build`)

---

## Task Dependency Graph

```
Phase 1 (Log Wrap)           Phase 3 (CI/CD)
├── 01-wrap-state             ├── 07-release-workflow
├── 02-wrap-rendering         ├── 08-install-script
│   └── depends on: 01        │   └── depends on: 07
└── 03-wrap-tests             └── 09-cross-config
    └── depends on: 02

Phase 2 (Settings)           Phase 4 (Website)
├── 04-fix-add-config-bug     ├── 10-devtools-network-page
├── 05-settings-fuzzy-modal   ├── 11-fix-keybindings-data
│   └── depends on: 04        └── 12-update-installation-page
└── 06-persist-dart-defines        └── depends on: 08
    └── depends on: 05
```

Phases 1-4 are independent of each other and can be worked on in parallel.
