# Phase 6 — Flutter SDK version picker (fvm-style) — Task Index

## Overview

Let the wizard's **Flutter SDK** step install *any* Flutter version, not just latest-stable-by-channel.
A new **version picker overlay** inside the install wizard fetches the Flutter releases manifest
lazily (existing `fetch_release_manifest`), groups releases by channel (**Stable / Beta / Master**
tabs — Master holds synthetic git-only `master`/`main` rows), filters by host arch on macOS, defaults
the cursor to the active stable release, and on confirm threads the chosen **`version_tag`** through
`FlutterStepParams` → `FlutterInstallTarget` so a pinned install lands at `~/fvm/versions/<version>`
(git clone `-b <tag> --depth 1`, archive fallback by exact-version manifest match) instead of
overwriting `~/fvm/versions/<channel>`. The new SDK then appears in the existing Flutter Version
panel via the unchanged FVM cache scan.

Research basis: [research/RESEARCH.md](research/RESEARCH.md) (4-agent verification workflow,
2026-06-10). All structural claims below were verified against the live code; line numbers are a
snapshot — **locate by symbol**.

### Decisions resolved by research (verified against source)

1. **`FlutterRelease` gains `release_date: Option<String>`** (serde-default, raw ISO-8601 string kept
   verbatim). The raw manifest carries it but `RawRelease` (`flutter_install.rs:378`) does not
   deserialize it today; the picker row format is `version · date · arch`. `hash`/`dart_sdk_version`
   stay un-deserialized (nothing needs them — `resolve_stable()` already ignores
   `current_stable_hash`, so **active stable = first stable entry**, which is also the picker's
   default cursor).
2. **`FlutterInstallTarget` gains `version_tag: Option<String>`**; the install dir name becomes
   `version_tag.unwrap_or(channel)` and the git ref becomes `-b version_tag.or(channel)`.
   `git clone -b` accepts **tags**, so a pinned version is a plain shallow clone — no
   clone+`git reset --hard` (cheaper than the PLAN's fvm description, same outcome). Adding the field
   breaks the struct literal at `fdemon-app/src/actions/mod.rs:924` → **Task 01 carries a one-line
   `version_tag: None` stub there** (Phase-5 Task-01 stub pattern); Task 04 replaces it.
3. **Pinned-version manifest miss is a hard error.** `archive_install` today silently falls back to
   stable when a channel is missing from the manifest. A new private `resolve_version_release`
   (exact `version` + two-pass arch match, beside the private `resolve_channel_release`) is used when
   `version_tag` is set; **no entry → clear error**, never the stable fallback. `master`/`main` have
   no manifest entry by design — the app forces `InstallMethod::GitClone` for them, and the archive
   error message says git is required.
4. **Ref validation widens to allow `+`.** `validate_channel` rejects `+`, but old Flutter tags are
   like `1.12.13+hotfix.5`. Refs go to git via `run_streaming` (argv, no shell), so allowing `+`
   after the first char is safe. Leading-`-` rejection stays (argument-injection guard).
5. **Picker state is a sub-state of the wizard, not a new `UiMode`.** New
   `install_wizard/version_picker.rs` module (app) holds `VersionPickerState` — `state.rs` is already
   huge. Key routing intercepts ALL keys at the top of `handle_key_install_wizard` while the picker is
   visible (the `tag_filter` intercept precedent in `handle_key_normal`, keys.rs:149). List scrolling
   copies the canonical `VersionListState` pattern (`selected_index` + `scroll_offset` +
   `Cell<usize>` render-hint + render-time `corrected_scroll`).
6. **Entry semantics** (per PLAN §Phase-6.5): on the FlutterSdk step, `Enter` **opens the picker**
   when no release has been confirmed yet; confirming in the picker (`Enter`) closes it and
   immediately runs the install with the choice; after a confirmed choice, `Enter` re-runs with it;
   `v` (re)opens the picker at any time; `Esc` closes without installing. **Manifest fetch failure
   must not dead-end offline installs**: in the error state the picker shows the error and `Enter`
   falls back to installing the default `settings.toolchain.channel` un-pinned (the git path needs no
   manifest). The picker cannot open while a step is running.
7. **Manifest lifecycle**: fetched lazily on first open via a new `UpdateAction::
   FetchFlutterReleaseManifest` (copying the `RunToolchainPreflight` spawn→`msg_tx.send` pattern),
   cached in `VersionPickerState` for the wizard session, `r` re-fetches inside the picker, and the
   releases vec is dropped on wizard hide (~300 KB JSON). Results re-enter as
   `Message::FlutterManifestFetched / FlutterManifestFetchFailed`.
8. **No new config field.** The picker choice overrides `settings.toolchain.channel` **for that run
   only** (documented precedence); persisting a version preference was considered and rejected —
   `settings.flutter.sdk_path` already pins the active SDK after install.

### Why these task boundaries

- The daemon half (Task 01) is a self-contained compiling unit (with the actions/mod.rs stub line) and
  everything else depends on its types — it goes first, alone.
- The app picker **state module** (Task 02) is pure data + grouping/filter/navigation logic with no
  handler wiring; it isolates the only `install_wizard/state.rs` edit of the phase.
- All **handler/message/key** wiring lands in Task 03 (message.rs, handler/*, plus a no-op executor
  stub arm for the new `UpdateAction` — the executor match is exhaustive). The **executor** body
  (Task 04) then replaces the stub in `actions/mod.rs` — sequential on 03 by design (shared
  variant), keeping 03 ∥ 05 write-disjoint.
- The TUI overlay (Task 05) depends only on Task 02's state types, so it runs in parallel with
  Task 03 in a separate worktree.
- Docs split per Phase-5 precedent: `docs/ARCHITECTURE.md` → `doc_maintainer` (Task 06);
  `docs/KEYBINDINGS.md` + website toolchain page → implementor (Task 07); write-disjoint, parallel.

**Total Tasks:** 7
**Estimated Hours:** 18–23 hours

## Task Dependency Graph

```
                 ┌─────────────────────────────────────────────────┐
                 │ 01-daemon-version-install-plumbing               │  Wave 1
                 │  release_date + version_tag + resolve_version_   │
                 │  release + ref validation + git/archive threading│
                 │  (+ actions/mod.rs `version_tag: None` stub)     │
                 └────────────────────────┬────────────────────────┘
                                          ▼
                 ┌─────────────────────────────────────────────────┐
                 │ 02-app-version-picker-state                      │  Wave 2
                 │  install_wizard/version_picker.rs (NEW):         │
                 │  VersionPickerState, tabs, grouping/arch filter, │
                 │  nav/clamp; wire field into InstallWizardState   │
                 └──────────────┬──────────────────────┬───────────┘
                                ▼                      ▼               Wave 3 (parallel worktrees)
        ┌──────────────────────────────────┐  ┌────────────────────────────────────┐
        │ 03-app-handlers-messages-keys     │  │ 05-tui-version-picker-overlay       │
        │ messages, FetchFlutterRelease-    │  │ widgets/install_wizard/             │
        │ Manifest action (+executor stub), │  │ version_picker.rs (NEW) + render    │
        │ picker handlers, key intercept,   │  │ hook + footer + FlutterSdk hints    │
        │ FlutterSdk arm version threading  │  │                                     │
        └─────────────────┬────────────────┘  └──────────────────┬─────────────────┘
                          ▼                                       │      Wave 4
        ┌──────────────────────────────────┐                      │
        │ 04-app-executor-threading         │                      │
        │ actions/mod.rs: manifest-fetch    │                      │
        │ executor body + version_tag into  │                      │
        │ FlutterInstallTarget/dir name     │                      │
        └─────────────────┬────────────────┘                      │
                          └───────────────┬───────────────────────┘
                                          ▼                            Wave 5 (parallel worktrees)
              ┌───────────────────────────┴───────────────────────────┐
              ▼                                                        ▼
   ┌──────────────────────────────┐                  ┌────────────────────────────────────┐
   │ 06-update-architecture-docs   │                  │ 07-update-keybindings-website-docs  │
   │ (doc_maintainer)              │                  │ (implementor)                       │
   └──────────────────────────────┘                  └────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Complexity | Modules |
|---|------|--------|------------|------------|------------|---------|
| 1 | [01-daemon-version-install-plumbing](tasks/01-daemon-version-install-plumbing.md) | ✅ Done (validated PASS) | - | 4–5h | high | `fdemon-daemon/src/toolchain/{types,flutter_install}.rs` (+ minimal `fdemon-app/src/actions/mod.rs` stub line) |
| 2 | [02-app-version-picker-state](tasks/02-app-version-picker-state.md) | ✅ Done (validated PASS) | 1 | 3–4h | medium | `fdemon-app/src/install_wizard/{version_picker (new), state, mod}.rs` |
| 3 | [03-app-handlers-messages-keys](tasks/03-app-handlers-messages-keys.md) | ✅ Done (validated PASS, merged) | 1, 2 | 4–5h | high | `fdemon-app/src/{message.rs, handler/mod.rs, handler/keys.rs, handler/update.rs, handler/install_wizard/*}` (+ executor stub arm) |
| 4 | [04-app-executor-threading](tasks/04-app-executor-threading.md) | ✅ Done (validated PASS) | 1, 3 | 2h | medium | `fdemon-app/src/actions/mod.rs` |
| 5 | [05-tui-version-picker-overlay](tasks/05-tui-version-picker-overlay.md) | ✅ Done (validated PASS, merged) | 2 | 3–4h | medium | `fdemon-tui/src/widgets/install_wizard/{version_picker (new), mod, step_detail}.rs` |
| 6 | [06-update-architecture-docs](tasks/06-update-architecture-docs.md) | ✅ Done (validated PASS, merged) | 1–5 | 1h | low | `docs/ARCHITECTURE.md` |
| 7 | [07-update-keybindings-website-docs](tasks/07-update-keybindings-website-docs.md) | ✅ Done (validated CONCERN¹, merged) | 1–5 | 1–2h | low | `docs/KEYBINDINGS.md`, `website/src/pages/docs/toolchain.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01 | `crates/fdemon-daemon/src/toolchain/types.rs`, `crates/fdemon-daemon/src/toolchain/flutter_install.rs`, `crates/fdemon-app/src/actions/mod.rs` (one-line `version_tag: None` stub only) | `toolchain/mod.rs` (re-export list) |
| 02 | `crates/fdemon-app/src/install_wizard/version_picker.rs` (new), `crates/fdemon-app/src/install_wizard/state.rs` (field + reset hooks), `crates/fdemon-app/src/install_wizard/mod.rs` (module decl/re-export) | `fdemon-daemon` toolchain types (`FlutterRelease`, `FlutterReleaseManifest`, `HostArch`), `flutter_version/state.rs` (`VersionListState` pattern) |
| 03 | `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/handler/keys.rs`, `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/handler/install_wizard/version_picker.rs` (new), `crates/fdemon-app/src/handler/install_wizard/mod.rs`, `crates/fdemon-app/src/handler/install_wizard/actions.rs`, `crates/fdemon-app/src/actions/mod.rs` (no-op stub arm for the new UpdateAction only) | `install_wizard/version_picker.rs` (Task 02 state API), `handler/keys.rs` tag-filter intercept precedent |
| 04 | `crates/fdemon-app/src/actions/mod.rs` | `handler/mod.rs` (`FetchFlutterReleaseManifest`, `FlutterStepParams.version_tag`), daemon `fetch_release_manifest` / `FlutterInstallTarget` |
| 05 | `crates/fdemon-tui/src/widgets/install_wizard/version_picker.rs` (new), `crates/fdemon-tui/src/widgets/install_wizard/mod.rs`, `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | `fdemon-app` `VersionPickerState` (Task 02), `widgets/flutter_version_panel/version_list.rs` (scroll pattern), `widgets/modal_overlay.rs` |
| 06 | `docs/ARCHITECTURE.md` | task 01–05 files, `~/.claude/skills/doc-standards/schemas.md` |
| 07 | `docs/KEYBINDINGS.md`, `website/src/pages/docs/toolchain.rs` | task 01–05 files, PLAN.md keyboard table |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | none — 02 depends on 01 | Sequential (01 → 02) |
| 02 + 03 | none on paper, but 03 depends on 02's state API | Sequential (02 → 03) |
| **03 + 05** | **none** (03 = app handler/message/actions; 05 = tui only) | **Parallel (worktree)** after 02 |
| 01 + 03 / 01 + 04 / 03 + 04 | **`fdemon-app/src/actions/mod.rs`** (01 stub line → 03 stub arm → 04 real body) | Sequential by dependency chain (01 → 03 → 04) |
| 04 + 05 | none | Parallel-safe (04 in Wave 4 while 05 may still be in flight) |
| **06 + 07** | **none** (`docs/ARCHITECTURE.md` vs `docs/KEYBINDINGS.md` + website) | **Parallel (worktree)** after 01–05 |
| 06/07 vs 01–05 | none | Sequential (after 01–05) |

> The only multi-task file is `fdemon-app/src/actions/mod.rs`, and every pair touching it is
> sequential by dependency (01 → 03 → 04). Wave-3 peers 03 + 05 are write-disjoint across crates;
> Task 05 compiles against Task 02's state types only and renders fields read-only, so it is correct
> whichever of 03/05 merges first. Wave-5 peers 06 + 07 are write-disjoint docs.

> ¹ Task 07 history: first dispatch (haiku) FAILED validation — `<code>` HTML embedded in Leptos
> `view!` string literals broke website-crate compilation. Re-dispatched at sonnet; re-validation
> returned CONCERN only for an out-of-scope empty `[workspace]` table added to `website/Cargo.toml`
> (proven unnecessary — root workspace already `exclude`s website/). The table was dropped at merge
> time; both in-scope files merged as validated.

## Success Criteria

Phase 6 is complete when:

- [ ] `FlutterRelease` carries `release_date: Option<String>` (serde-default; absent in old manifests
      → `None`); the manifest fixture test covers both presence and absence.
- [ ] `FlutterInstallTarget.version_tag: Option<String>` exists; the final install dir is
      `install_root/<version_tag or channel>`; the git path clones `-b <version_tag or channel>
      --depth 1`; ref validation accepts `3.24.0` and `1.12.13+hotfix.5` and still rejects empty /
      leading-`-` refs.
- [ ] A pinned archive install resolves via `resolve_version_release` (exact version, two-pass arch
      match) and a manifest miss is a **hard error** naming the version — never the silent
      stable fallback (which remains for channel-only installs).
- [ ] On the wizard's FlutterSdk step, `Enter` (no prior choice) opens the picker; the picker fetches
      the manifest lazily, shows loading/error states, groups releases under Stable / Beta / Master
      tabs (Master = synthetic git-only `master` + `main` rows), arch-filters on macOS, and defaults
      the cursor to the newest stable release.
- [ ] `j`/`k` navigate, `Tab` cycles channel tabs, `r` re-fetches, `Esc` closes without installing,
      `v` (re)opens the picker; while the picker is visible no underlying wizard key fires; the picker
      cannot open while a step is running.
- [ ] Confirming a release closes the picker and immediately runs the install with
      `FlutterStepParams.version_tag` set (channel = the release's channel); confirming a `master`/
      `main` row forces `InstallMethod::GitClone`; in the fetch-error state `Enter` installs the
      default `settings.toolchain.channel` un-pinned (offline path preserved).
- [ ] A pinned install lands at `~/fvm/versions/<version>` and appears in the Flutter Version panel
      after the post-install rescan; channel installs still land at `~/fvm/versions/<channel>`
      (no behaviour change when the picker is never opened — `version_tag: None`).
- [ ] The TUI overlay renders tabs, the list (`version · date · arch`, "git-only" badge on master
      rows), loading/error states, and footer hints; the FlutterSdk detail pane advertises the picker
      (`v` / Enter).
- [ ] `cargo test --workspace --lib` green; `cargo fmt --all` + `cargo clippy --workspace -- -D
      warnings` clean; no existing FlutterSdk-step test regresses (Enter-opens-picker changes are
      reflected in updated tests, not deleted ones).
- [ ] `docs/ARCHITECTURE.md`, `docs/KEYBINDINGS.md`, and the website toolchain page document the
      picker, the new keys, and the `version_tag` precedence over `settings.toolchain.channel`.

## Keyboard Shortcuts

| Key | Mode | Action |
|-----|------|--------|
| `Enter` | InstallWizard (FlutterSdk step, no confirmed choice) | Open the version picker |
| `Enter` | InstallWizard (FlutterSdk step, choice confirmed) | Run install with the confirmed version |
| `v` | InstallWizard (FlutterSdk step) | (Re)open the version picker |
| `j`/`k`, `↓`/`↑` | Version picker | Move the version cursor |
| `Tab` | Version picker | Cycle channel tab (Stable → Beta → Master) |
| `r` | Version picker | Re-fetch the releases manifest |
| `Enter` | Version picker | Confirm the selected version and install (error state: install default channel) |
| `Esc` | Version picker | Close the picker without installing |

All existing wizard keys are unchanged; while the picker is visible they are intercepted.

## Phase Review

| Round | Verdict | Review | Reviewed HEAD |
|-------|---------|--------|---------------|
| 0 | NEEDS_WORK | workflow/reviews/features/toolchain-platforms-submenu-phase-6/REVIEW.md | 0c927d8f |
| 1 | APPROVED_WITH_CONCERNS | workflow/reviews/features/toolchain-platforms-submenu-phase-6/REVIEW.md (§Re-review Round 1) | c51643eb |

## Notes

### Phase complete — round-1 fixes merged (2026-06-10)

Round-1 followup (followups/phase-6-fix-1/, 4 tasks) resolved all 4 Major review findings:
caption-row layout reservation, `#[ignore]` gate on the live-CDN test, char-boundary-safe
truncation, REVIEW_FOCUS.md Cell registry entry. Final verdict ⚠️ APPROVED WITH CONCERNS.

### Deferred items (Minor — carried for a future phase)

From the phase review (ACTION_ITEMS.md Minor list + round-1 observations), intentionally not fixed:

1. `docs/ARCHITECTURE.md` (~651) names `handle_run_selected_step` where the code uses
   `dispatch_flutter_install` for picker confirm — one-line doc correction.
2. `confirm()` double clone in `install_wizard/version_picker.rs`.
3. Remove the `#[allow(dead_code)]` `_assert_message_variant_exists` probe
   (`handler/install_wizard/version_picker.rs`).
4. `clear_manifest` is dead production code (only its own test references it).
5. `validate_ref` `.expect()` avoidable via `let-else`/`starts_with` (`flutter_install.rs:243`).
6. Validate `version_dir_name` directly in `install_flutter` (or validated constructor).
7. Apply the HTTPS-only redirect policy to `fetch_release_manifest_from` (shared client builder).
8. Picker tab-label spacing inconsistency (`" Stable "` vs `"Beta "`).
9. Document/test `group_releases` unknown-channel→Beta routing.
10. Comment on `handle_refetch` explaining the intentionally absent `is_step_running` guard.
11. (Pre-existing) REVIEW_FOCUS.md "only `version_check.rs` does network I/O" note is stale.
12. (Round 1) `test_no_panic_multibyte_version_string` doesn't drive the version truncation branch —
    add a narrow-width variant.
13. (Round 1) `#[ignore]` doc comment uses a truncated test name (works via substring match).
14. (Deferred refactor) Injectable manifest-fetch URL + wiremock-backed executor test (requires
    widening daemon `pub(crate) fetch_release_manifest_from`).

- **Per-run precedence, no persistence.** The picker choice never writes `settings.toolchain.channel`;
  after a successful install the SDK is pinned via the existing `settings.flutter.sdk_path` write in
  `handle_step_completed` (unchanged — `WizardStepCompleted.sdk_path` already carries the right
  per-version path because the daemon computes it from `version_dir_name`).
- **The completion chain needs no changes.** `WizardStepCompleted { kind: FlutterSdk, sdk_path }` →
  stash + persist + auto-PATH + preflight rerun + `ScanInstalledSdks` all key off the path, not the
  channel — verified in RESEARCH.md §2.
- **`run_seq` guard still applies**: the picker-confirm path must go through `begin_step` exactly like
  today's Enter path (mint token, bump seq) — do not invent a second dispatch path.
- **Manifest size**: ~300 KB JSON, fetched only when the picker opens; releases vec dropped on wizard
  hide. Never fetch at startup or on wizard open.
- **Dev host is Linux** — manifest grouping/filtering and resolution logic must be pure and tested
  with fixtures (extend `MANIFEST_FIXTURE`); no live CDN calls in tests (wiremock precedent).
- **Locate by symbol, not line.** Line numbers in task files are a snapshot and will drift.
- Phase 7 ideas stay out of scope: no "Available" tab in the standalone Flutter Version panel, no
  `CHROME_EXECUTABLE` runtime propagation, no `platforms_enabled`.
