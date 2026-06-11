# Action Items: Toolchain Platforms Submenu — Phase 6 (Flutter SDK Version Picker)

**Review Date:** 2026-06-10
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 4 (Major)

## Critical Issues (Must Fix)

None.

## Major Issues (Should Fix — round-1 scope)

### 1. Fix layout reservation for the FlutterSdk caption row in step_detail.rs
- **Source:** code_quality_inspector
- **File:** `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`
- **Line:** ~793 (component_height computation) vs the later `effective_bottom_height` block
- **Problem:** `component_height` reserves only `bottom_section_height` (1 row, `ACTION_HINT_HEIGHT`),
  but when `has_step_caption` is true the bottom section actually occupies 2 rows
  (`ACTION_HINT_HEIGHT + 1`). The component render loop clamps at `y + height − 1` while the caption
  renders at `y + height − 2`, so in tight panels the last component row overwrites / shares the
  caption row.
- **Required Action:** Hoist the `has_step_caption` detection and `effective_bottom_height`
  computation above the component loop and compute
  `component_height = content_area.height.saturating_sub(effective_bottom_height)`.
- **Acceptance:** A rendering test with a FlutterSdk step, a component list long enough to fill the
  pane, and a height tight enough to trigger clamping asserts that no component text appears on the
  caption row (and the caption is intact). Existing step_detail tests still pass.

### 2. Make the manifest-fetch executor test hermetic (no live CDN call in the default suite)
- **Source:** code_quality_inspector, architecture_enforcer, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/actions/mod.rs`
- **Line:** ~2845 (`test_fetch_flutter_release_manifest_emits_fetched_or_failed`)
- **Problem:** Plain `#[tokio::test]` performs a real HTTPS GET to the Flutter releases CDN with a
  30 s timeout on every `cargo test --workspace`. Violates the phase rule "no live CDN calls in
  tests (wiremock precedent)"; non-hermetic and stalls offline/air-gapped runs.
- **Required Action:** Minimum: annotate
  `#[ignore = "requires outbound HTTPS to storage.googleapis.com"]`. Preferred: make the fetch URL
  injectable through the executor (the daemon already exposes `fetch_release_manifest_from(url)`)
  and back the test with the existing wiremock harness.
- **Acceptance:** `cargo test --workspace` makes no outbound network call from this test (verified
  by the test passing instantly offline, or by the `#[ignore]` gate); the executor's
  Fetched/Failed message contract remains covered (by the ignored test or a wiremock-backed one).

### 3. Replace byte-index slicing of manifest-derived strings in the picker overlay
- **Source:** security_reviewer (MEDIUM — panic in render loop on external data)
- **File:** `crates/fdemon-tui/src/widgets/install_wizard/version_picker.rs`
- **Line:** ~211–212 (`&row.version[..version_max]`), ~221 (`&s[..10]` on `release_date`)
- **Problem:** Byte-index slices on CDN-derived strings panic (`byte index N is not a char
  boundary`) if a manifest entry ever contains non-ASCII content straddling the cut point — a crash
  inside the TUI render loop.
- **Required Action:** Use char-boundary-aware truncation (e.g. `chars().take(n)` collection, or an
  `is_char_boundary` walk-back; reuse an existing truncate helper if one fits).
- **Acceptance:** A unit test rendering a `PickerRow` whose version and release_date contain
  multi-byte characters at the truncation points does not panic and renders truncated text.

### 4. Register the new Cell render-hint field in docs/REVIEW_FOCUS.md
- **Source:** architecture_enforcer (mandatory per project policy), code_quality_inspector,
  security_reviewer
- **File:** `docs/REVIEW_FOCUS.md`
- **Line:** "Approved TEA Exception: Render-Hint Feedback" → Current-usage list (after the
  `InstallWizardState::last_known_visible_height` bullet)
- **Problem:** `VersionPickerState::last_known_visible_height` is a new `Cell<usize>` render-hint;
  the policy text requires every such field to be documented in the registry. It is absent.
- **Required Action:** Add:
  `- \`VersionPickerState::last_known_visible_height\` — the renderer writes the visible list-row
  count each frame inside \`VersionPickerOverlay\`'s list render; the \`adjust_scroll\` helper reads
  it to keep the selected row visible. Default 0 (safe fallback when no render has happened yet).
  Write site annotated in \`widgets/install_wizard/version_picker.rs\`.`
- **Acceptance:** The bullet exists in the Current-usage list and names the write site and reader.

## Minor Issues (Consider Fixing — deferred, do NOT open a round for these)

1. `docs/ARCHITECTURE.md` (~651): `handle_confirm` routes through `dispatch_flutter_install`, not
   `handle_run_selected_step` — correct the named mechanism.
2. `install_wizard/version_picker.rs:280`: double clone in `confirm()`; single-clone rewrite.
3. `handler/install_wizard/version_picker.rs:560`: remove the `#[allow(dead_code)]`
   `_assert_message_variant_exists` probe.
4. `install_wizard/version_picker.rs:292`: `clear_manifest` is dead production code — remove, wire,
   or mark test-only.
5. `flutter_install.rs:243`: avoid the `.expect()` via `let-else` / `starts_with` pattern.
6. `flutter_install.rs` (`install_flutter`): validate `version_dir_name` directly
   (`validate_ref(&target.version_dir_name)?`) or provide a validated constructor.
7. `flutter_install.rs` (`fetch_release_manifest_from`): apply the HTTPS-only redirect policy used
   by `download_to_file` (shared client builder).
8. TUI tab label spacing: `" Stable "` vs `"Beta "` — normalize.
9. Document (and optionally test) `group_releases`' unknown-channel→Beta routing.
10. One-line comment on `handle_refetch` explaining why no `is_step_running` guard is needed.
11. (Pre-existing) REVIEW_FOCUS.md network-I/O exception note predates toolchain phases — update
    opportunistically.

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] All 4 Major issues resolved
- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
- [ ] No new live-network tests in the default suite
- [ ] Re-review of the fix diff (same PHASE_BASE `4eab5863`)
