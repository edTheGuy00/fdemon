# Changelog

All notable changes to Flutter Demon are documented here.

## [Unreleased]

### Miscellaneous
- Add release workflow fix plan and task breakdown

### Other Changes
- Native DAP server for IDE debugging (#10)

## [0.2.0] - 2026-03-02

### Bug Fixes
- Capture E2E docker output and fix workspace dep caching (#4)- Remove duplicate PR-closed trigger and fix README badge (#6)

### Features
- Migrate to trunk-based release strategy (Wave 1)

### Miscellaneous
- Add trunk-based release strategy plan and task breakdown- Session resilience docs

### Other Changes
- Feat/session resilience (#3)

* chore: mark release-on-merge task 05 (branch migration) as done

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* fix: restore AI-assisted development section in README

The section was accidentally removed during the logo/README rewrite.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* chore: add session resilience phase-1 task breakdown

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* fix: stop zombie network polling on session exit and AppStop

Two of five session cleanup paths cleaned up performance polling but
skipped network polling, leaving orphaned tasks sending messages for
dead sessions. Add matching network_task_handle/network_shutdown_tx
cleanup to handle_session_exited and the AppStop handler, with tests.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* chore: add session resilience phase-2 task breakdown

Break down Phase 2 (Emit VmServiceReconnecting Message) into 4 tasks:
- 04: VmClientEvent enum + daemon channel refactor
- 05: Emit lifecycle events from reconnection loop
- 06: Update forward_vm_events for VmClientEvent
- 07: Unit tests for reconnection message flow

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* feat: emit VM Service reconnection lifecycle events (phase 2)

Add VmClientEvent enum wrapping stream events with Reconnecting,
Reconnected, and PermanentlyDisconnected lifecycle variants. The
daemon emits these during WebSocket backoff and the app layer
forwards them as Messages so users see "Reconnecting (N/10)..."
in DevTools panels during connection recovery.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* chore: add phase-2 review and follow-up task breakdown

Add code review document for session resilience phase 2 with
6 identified issues. Create 3 review-fix tasks (08-10) in phase 2
for must-fix items, and a new phase 2b with 4 tasks addressing
pre-existing reconnect handler design gaps: VmServiceReconnected
message variant, perf polling cleanup, and connection_status
multi-session guarding.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* feat: separate reconnect from initial connect in VM Service handlers

Add Message::VmServiceReconnected variant to distinguish WebSocket
reconnection from initial connection, preserving accumulated performance
telemetry (memory history, frame timings, GC events) across reconnects
instead of wiping it. Guard connection_status writes in VmServiceConnected,
VmServiceDisconnected, and VmServiceConnectionFailed handlers with an
active-session check so background session VM events don't pollute the
foreground session's DevTools indicator. Clean up stale perf and network
polling tasks before spawning replacements on both connect and reconnect
to prevent duplicate messages and leaked tokio tasks.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* chore: add session resilience phase-3 task breakdown

Break down Phase 3 (Process Health Monitoring) into 5 implementable tasks:
process watchdog, getVersion RPC, VM heartbeat, exit code capture, and
tests. Three tasks can run in parallel (wave 1), with dependencies tracked
in TASKS.md.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* feat: implement process health monitoring (phase 3)

Add two watchdog mechanisms for detecting hung processes and stale VM
connections, plus proper exit code capture via a dedicated wait task:

- Process watchdog: 5s interval poll of has_exited() in spawn_session
- VM heartbeat: 30s getVersion probe with 3-failure threshold disconnect
- wait_for_exit task: Child moved to dedicated async task with
  AtomicBool/Notify/oneshot for clean ownership and exit code capture
- VersionInfo struct for getVersion RPC response deserialization
- Exit code surfaced in session logs (normal/unknown/error)

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* chore: add phase-3 review and follow-up task breakdown

Phase-3 review identified 2 critical bugs and 8 improvement items:
- Heartbeat failure counter not reset on reconnection (premature disconnect)
- Watchdog/wait_for_exit race producing duplicate Exited events

Phase-3b breakdown: 5 follow-up tasks in 3 waves covering bug fixes,
idempotency hardening, dead code cleanup, and test hygiene.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* fix: address phase-3 review findings (heartbeat reset, exit race, cleanup)

Reset heartbeat consecutive_failures on reconnect/reconnecting events,
guard watchdog against duplicate exit synthesis, add exit handler
idempotency, move get_version() to VmRequestHandle, and apply test
naming/platform guard hygiene.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* chore: add phase-4 plan for stopped session device reuse fix

Investigate and plan fix for UX bug where stopped sessions block new
session creation on the same device. Root cause: find_by_device_id is
phase-blind, returning stopped sessions that should no longer occupy
the device. Plan adds is_active(), find_active_by_device_id(), and
updates the launch guard with full test coverage.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* feat: fix stopped sessions blocking device reuse (phase 4)

Add Session::is_active(), SessionManager::find_active_by_device_id(),
and swap the launch guard to skip stopped/quitting sessions. Users can
now start a new session on a device that has a stopped session tab.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* chore: add phase-4 review and phase-4b follow-up tasks

Phase-4 review: approved with concerns (stopped session accumulation
against MAX_SESSIONS, dead find_by_device_id code, missing phase tests).
Phase-4b plan: 5 tasks to address all review findings including
auto-eviction of stopped sessions and dead code removal.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* fix: address phase-4 review findings (phase 4b)

Remove dead find_by_device_id method, auto-evict oldest stopped session
when MAX_SESSIONS is reached, add Quitting/Reloading phase test coverage,
fix doc comments and stale task notes.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* chore: add phase-5 plan for heartbeat bug fix and actions.rs refactor

Fix missed heartbeat counter reset (false-positive Phase 3b completion)
and decompose 2,081-line actions.rs into 6 focused submodules to meet
the 500-line CODE_STANDARDS threshold.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* feat: heartbeat bug fix and actions.rs refactor (phase 5)

Fix heartbeat failure counter not resetting on reconnection events, and
refactor the monolithic actions.rs (2,081 lines) into a 7-file directory
module mirroring the handler/devtools/ decomposition pattern.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* chore: add phase-5 review and phase-5b follow-up tasks

Add consolidated review document for phase 5 (approved with concerns)
and create phase-5b plan with 5 tasks addressing 8 review findings:
2 Major (mutex unwrap, unused parameter) and 6 Minor (constants,
visibility, imports, tests).

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* fix: address phase-5 review findings (phase 5b)

Replace mutex unwrap with defensive match/warn pattern, remove unused
_msg_tx parameter from spawn_clear_http_profile, promote magic literals
to named constants (VM_CONNECT_TIMEOUT, LAYOUT_FETCH_TIMEOUT), tighten
submodule visibility to pub(super), hoist inline use declarations to
module top-level, and add #[cfg(test)] modules with assertions to all
7 files in actions/.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* fix: log try_send failures for lifecycle events and update Quick start example

Replace silent `let _ = event_tx.try_send(...)` with warn-level logging
for Reconnecting, Reconnected, and PermanentlyDisconnected events,
matching the existing stream-event error handling pattern. Update the
module-level Quick start example to match on VmClientEvent variants
instead of the stale `event.params.stream_id` access.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

---------

Co-authored-by: Claude Opus 4.6 <noreply@anthropic.com>- Feat/responsive session dialog (#5)

* chore: add responsive session dialog plan and phase 1 task breakdown

Adds feature plan for making the New Session dialog responsive by
decoupling compact/expanded decisions from layout orientation. Phase 1
breaks down into 5 tasks covering threshold constants, render_panes
refactor, height-based decisions for both layout paths, and unit tests.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* feat: implement space-aware compact/expanded decision for session dialog (Phase 1)

Decouple the compact/expanded rendering decision from layout orientation.
Instead of always forcing compact mode in vertical layout, the dialog now
checks actual available height against threshold constants to choose the
right rendering mode for both LaunchContext and TargetSelector.

Changes:
- Add height threshold constants with hysteresis (MIN_EXPANDED_LAUNCH_HEIGHT,
  MIN_EXPANDED_TARGET_HEIGHT) and future compact thresholds
- Add launch_compact parameter to render_panes() for horizontal layout
- Height-based compact decision in render_horizontal() and render_vertical()
- 10 new unit tests covering both layout paths, boundary conditions,
  threshold coupling, and regression for standard terminal sizes

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* chore: phase 2 docs

* feat: layout-managed launch button with overflow prevention (Phase 2)

Move the launch button into Ratatui's layout system instead of manually
calculating its position, preventing out-of-bounds rendering. Extract
named constants (LAUNCH_BUTTON_SLOT, BUTTON_HORIZONTAL_INSET) and a
shared button_render_area() helper to eliminate duplication and magic
numbers. Add 5 overflow prevention tests covering min, small, and large
heights for both widget variants.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* chore: add phase 3 task breakdown for scroll-to-selected fix

Break down Phase 3 (Fix Scroll-to-Selected in Target Selector) into 4
tasks: add Cell<usize> visible height field, renderer write-back with
scroll correction, handler integration, and unit tests.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* feat: fix scroll-to-selected with render-to-state feedback loop (Phase 3)

Close the render-to-state feedback loop so the target selector always
keeps the selected device visible regardless of terminal height. The
renderer writes the actual device list area height to state via
Cell<usize> each frame, and the handler reads it for accurate scroll
calculations. A render-time scroll correction acts as a safety net for
stale offsets after terminal resize.

Includes 11 new unit tests across state, handler, and renderer layers,
plus review action item fixes: documented TEA Cell exception in
REVIEW_FOCUS.md, clarified borrow ordering, added deduplication TODO,
and strengthened test assertions.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* docs: add responsive layout guidelines to CODE_STANDARDS.md (Phase 0)

Codify the five responsive layout principles from Phases 1-3 into
project-wide standards: space-based layout decisions, content within
bounds, scroll-to-selected visibility via Cell<usize> render-hints,
named threshold constants, and hysteresis at breakpoints.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* fix: correct doc comment and use UTF-8-safe truncation in test assertions

Fix LaunchContextWithDevice::min_height() reference to LaunchContext::min_height()
and replace byte-index slicing with chars().take(n) to prevent panics on
Unicode box-drawing characters in assertion failure messages.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* refactor: remove unused hysteresis constants and simplify layout guideline

The hysteresis constants (COMPACT_LAUNCH_HEIGHT_THRESHOLD,
COMPACT_TARGET_HEIGHT_THRESHOLD) were scaffolded but never wired in.
Stateful hysteresis adds complexity without meaningful benefit —
terminal resize is infrequent during modal dialogs and Ratatui's
fast redraw cycle stabilizes layout within one frame. Replace the
hysteresis principle in CODE_STANDARDS.md with a simpler deterministic
single-threshold guideline that matches the actual implementation.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

---------

Co-authored-by: Claude Opus 4.6 <noreply@anthropic.com>- Feat/auto changelog website (#7)

* fix: remove duplicate PR-closed trigger and fix README badge

The e2e workflow triggered on both `push` to main and `pull_request: closed`,
causing duplicate runs that cancelled each other on every PR merge. The push
trigger already covers merges, so the pull_request trigger was redundant.

Also fix the README status badge pointing to nonexistent `ci.yml` — changed
to `e2e.yml` to match the actual workflow file.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* feat: auto-generate website changelog from git-cliff during releases

Replace hardcoded changelog in website/src/data.rs with a build.rs that
reads changelog.json (produced by git cliff --context) at compile time.
The release workflow now generates this JSON before building the Docker
image, so the website changelog stays in sync with releases automatically.
Local dev without the JSON file falls back to an empty changelog.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

* fix: align group ordering with cliff.toml, stabilize sort, fix escape order

- Reorder group_order() to match cliff.toml commit_parsers sequence
  (Documentation before Performance/Refactoring)
- Add alphabetical tiebreak to sort for reproducible output when
  multiple groups share the same order bucket
- Apply upper_first before escape so capitalization targets the real
  first character, not a backslash from escaping

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

---------

Co-authored-by: Claude Opus 4.6 <noreply@anthropic.com>

## [0.1.0] - 2026-02-24

### Bug Fixes
- Resolve Phase 2 bugs - response routing, shutdown, exit handling, selector UI- Cleanup device selector- Update header layout- *(e2e)* Complete Phase 1 follow-up tasks (Wave 5 & 6)- *(e2e)* Enable all 16 settings page tests- *(e2e)* Update tui_interaction patterns for StartupDialog mode- *(settings)* Implement boolean toggle handler and document E2E PTY issue- *(startup)* Address phase 1 review issues (phase 1.1)- *(startup)* Address phase 2 review concerns (phase 2.2)- *(startup)* Address phase 3 review concerns (phase 4.4)- *(new-session-dialog)* Implement phase 4 review fixes- *(new-session-dialog)* Implement phase 5 review fixes- *(new-session-dialog)* Implement phase 6 review fixes- *(new-session-dialog)* Complete review follow-up tasks 9-14- *(new-session-dialog)* Complete phase 5 review follow-up fixes- Phase 4 followup — resolve all review action items- Phase 2 review fixes — critical bugs, palette migration, dead code removal, and layout gap- Nerfonts- Spacing- Phase 3 review action items — 6 bug fixes and cleanup- Address phase 2 review issues — split extensions, refactor ownership, harden error handling- Address phase 3 review issues — lifecycle leaks, stale cache, stats bugs, session split- Devtools phase 4 bugs — layout tab, refresh, narrow window, browser URL, key nav- Devtools v2 phase 3 review fixes — 7 tasks + allocation table bug- Devtools v2 phase 4 review fixes — 7 tasks across 4 crates- Devtools v2 phase 5 review fixes — 6 tasks across 4 crates- Widget inspector groupName bug + timeout improvements for large projects- Phase 2 review remediation — 6 fixes across settings modals- Use macos-latest for x86_64 build (macos-13 retired)- Update install script URLs from /main/ to /master/

### Documentation
- Add log filtering, search, and error navigation to README- *(testing)* Add phase 2 boolean toggle bug verification tasks- *(e2e)* Document PTY/crossterm limitation for Enter/Space keys- *(testing)* Mark settings-page-testing phase 3-5 as blocked- *(planning)* Add startup flow consistency plan and task breakdowns- *(new-session-dialog)* Add phase 6.1 file splitting plan- *(new-session-dialog)* Add phase 7 review and follow-up tasks- *(new-session-dialog)* Add polish plan for 4 post-implementation issues- *(new-session-dialog)* Add phase 5 review follow-up tasks- Clean up ARCHITECTURE.md and move code samples to CODE_STANDARDS.md- Plan phase 1 and phase 2 task breakdowns for cyber-glass redesign

### Features
- Complete Phase 1 - Foundation (Proof of Concept)- Complete Phase 1.1 - Flutter Project Discovery- Complete Phase 2 - Protocol Integration (Basic Control)- Complete Phase 3 tasks 7-8 and fix session shutdown bugs- Complete Phase 3 tasks 9-10 - Refined Layout and Keyboard Shortcuts- Complete Phase 3 tasks 9-10 and fix multi-session bugs- Complete Phase 3 tasks 11 & 11a - LineGauge progress and device caching- Complete Phase 4 refinement tasks 1-3- Complete Task 05 - file watcher reloads all sessions- Complete Phase 1 - Log filtering, search, and error navigation- Complete Phase 2 - Error highlighting, stack traces, and horizontal scroll- Complete Tasks 04 & 05 - Log batching and virtualized display- Add Link Highlight Mode for opening files from logs (Phase 3.1)- *(settings)* Implement full-screen settings panel with tabbed UI- *(settings)* Complete Phase 4 with persistence, gitignore init, and docs- *(startup)* Complete Phase 5 with startup dialog, config priority, and bugfixes- *(startup)* Complete Phase 5 enhancement wave with config editing and UX improvements- *(e2e)* Implement Phase 1 mock daemon testing infrastructure- *(e2e)* Implement headless mode and complete Phase 2 Wave 2- *(e2e)* Complete Phase 3 Wave 1.5 PTY utilities improvements- *(e2e)* Complete Phase 3 Wave 2 & 3, add review followup tasks- *(keys)* Implement double-'q' quick quit feature- *(e2e)* Complete Phase 3 Wave 3.5 test infrastructure improvements- *(e2e)* Complete Phase 3.5 Wave 1-3 test infrastructure improvements- *(e2e)* Complete Phase 3.5 Wave 4-6 TestBackend test infrastructure- *(e2e)* Complete Phase 3.6 review followup & TEA compliance- *(startup)* Rework startup flow to enter Normal mode directly- *(startup)* Implement auto-launch message infrastructure (phase 1)- *(startup)* Implement message-based auto-start flow (phase 2)- *(startup)* Complete auto-launch implementation (phase 3)- *(new-session-dialog)* Implement phase 1 & 2 with review fixes- *(new-session-dialog)* Implement phase 3 dart defines modal with review fixes- *(new-session-dialog)* Implement phase 4 native device discovery- *(new-session-dialog)* Implement phase 5 target selector widget- *(new-session-dialog)* Implement phase 6 launch context widget- *(new-session-dialog)* Complete phase 7 review follow-up tasks- *(new-session-dialog)* Implement phase 8 integration & cleanup- Minor fixes- *(entry-point)* Implement phase 1 entry point support- *(entry-point)* Implement phase 3 UI for entry point selection- *(entry-point)* Complete phase 3 review follow-up tasks- Create website- Containerize website- Fix dependencies violations- Extract Engine abstraction and wire services (phase 2)- Major refactoring- Post-restructure cleanup (7 tasks)- Phase 4 implementation, review, and followup task planning- Ui redesign and fixes docs- Launcher dialog rewrite- Phase 4 settings panel cyber-glass redesign- Add VM Service client foundation with structured errors and hybrid logging (Phase 1)- Devtools phase 2- Devtools phase 3 — performance & memory monitoring data pipeline- Devtools phase 4 — TUI panels, key handlers, and review fix plans- Devtools phase 5 — config expansion, connection UI, error UX, performance polish, docs & website- Devtools v2 phase 2 — merge Inspector and Layout into unified tab- Devtools v2 phase 3 — performance tab overhaul- Devtools v2 phase 4 — network monitor tab- Devtools v2 phase 5 — polish, config, filter input, review & fix plan- Add log view word wrap mode with correct scroll bounds- Widget tree readiness polling, configurable timeout & groupName bug plan- Settings launch tab modals — dart defines editor & extra args picker- Phase 3 — version flag, title bar version, release workflow & install script- Phase 4 — website docs, changelog, GHCR publish- Add official logo, rewrite README, fix repo URLs

### Miscellaneous
- Update docs- Phase 3 docs- Docs and readme- Refactoring docs- Log and config docs- Phase 2 docs- Phase 3 docs- Logging docs- Hyperlinks docs- Logview refactor docs- Claude refactoring, docs updates- Phase 4 docs- Phase 5 tasks- Update docs- E2e-testing docs- *(e2e)* Add follow-up tasks from code review- E2e phase 2 docs- Documentation:- Phase 3 documents- P3w1 work plus followup docs- *(workflow)* Phase 3.5 docs- *(workflow)* Add Phase 3.6 plan for review followup & TEA compliance- License update- Settings page testing docs- Update testing docs- Test snapshots- Update tests and docs- Create docs- Fixes docs- Entry point support docs- Phase 2 and 3 docs- Update features- Update keybindings- Position fix- Update configurations- Update architecture- Resctructure phase 1 docs- Restruture phase 2- Restructure phase 3 docs- Post restructure cleanup docs- Phase 4 restructure documents- Nerd font docs- Update docs- Phase 3 docs- Phase 3 fixes docs- Phase 4 redesign docs- Phase 4 review and follow-up fix plans- Minor fixes- Widget crash log docs- Update docs, attempt widget crash logs- Update devtools plan with UX design, add phase 1 task breakdown- Add phase 2 task breakdown for Flutter service extensions- Devtool phase 3 docs- Devtols tui docs- Devtools phase 5 docs- Init linux- Devtool v2 docs- Devtool v2 phase 2 docs- Devtools v2 phase 3 docs- Devtools v2 phase 3 review and fix plan- Devtools v2 phase 4 docs — network monitor tab task breakdown- Devtools v2 phase 4 review and fix plan- Devtool phase 5 docs- V1-refinements plan and phase 1 log wrap task breakdown- Phase 2 settings launch tab task breakdown — 6 tasks across 3 waves- Phase 2 review + followup fix tasks for 12 identified issues- Phase 3 task breakdown — version, release workflow & install script- Phase 4 task breakdown — website docs, changelog, GHCR publish

### Other Changes
- Add sample 2- Update sample apps- AI agents docs

### Refactoring
- Split tui/mod.rs into focused modules (Task 12)- Split handler.rs into focused modules (Task 13)- Complete Phase 4 Task 4 - remove all legacy single-session code- *(tui)* Split log_view.rs into module directory- *(agents)* Consolidate implementor skill into agent, update dispatcher- *(new-session-dialog)* Implement phase 6.1 file splitting- *(new-session-dialog)* Complete phase 8 review follow-up tasks- Devtools v2 phase 1 — decompose oversized widget and handler files

### Testing
- Update e2e snapshot baselines- Update e2e snapshot baselines


