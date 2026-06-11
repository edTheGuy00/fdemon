# Review: Toolchain Platforms Submenu — Phase 6 (Flutter SDK Version Picker)

**Review Date:** 2026-06-10
**Diff Range:** `4eab5863..0c927d8f` (`git diff 4eab58630508fb4f5d997f2e2556f344757dbe69..HEAD`)
**Change Type:** Feature implementation (7 tasks)
**Verdict (round 0):** ⚠️ NEEDS WORK → **Final verdict (round 1 re-review):** ⚠️ **APPROVED WITH CONCERNS** — see [Re-review (Round 1)](#re-review-round-1) below

## Scope

Phase 6 adds an fvm-style Flutter version picker to the install wizard: `FlutterRelease.release_date`
+ `FlutterInstallTarget.version_tag` + `resolve_version_release` + `validate_ref` in the daemon
(Task 01); `VersionPickerState` pure data module (Task 02); TEA wiring — 9 messages, key intercept,
`FetchFlutterReleaseManifest` action, single-source `dispatch_flutter_install` (Task 03); executor
threading (Task 04); `VersionPickerOverlay` TUI widget + step-detail caption/footer hints (Task 05);
docs (Tasks 06–07). 21 files, +3,546/−121. All 7 tasks validated (6 PASS, 1 CONCERN); waves 3 and 5
integration-verified green (7,453–7,456 tests).

## Agent Verdicts

| Agent | Verdict | Headline |
|-------|---------|----------|
| architecture_enforcer | ⚠️ CONCERNS | 0 layer/TEA violations; REVIEW_FOCUS.md Cell registry gap (mandatory per policy); ungated network test |
| code_quality_inspector | ⚠️ NEEDS WORK | Confirmed layout reservation bug in `step_detail.rs`; live-network test; 5 minors |
| logic_reasoning_checker | ✅ PASS | All 8 phase invariants verified by trace; 2 informational notes |
| risks_tradeoffs_analyzer | ⚠️ CONCERNS | Live-network test (non-hermetic default suite); ARCHITECTURE.md mechanism drift |
| security_reviewer | ✅ PASS w/ concerns | 0 critical/high; byte-slice panic risk on manifest strings; 4 lows |

Verdict per matrix: multiple agents ⚠️ → **NEEDS WORK**; severity floor satisfied by Major findings
M1–M3 below.

## Consolidated Findings

### Major

**M1 — `step_detail.rs` layout reservation bug: component rows overlap the FlutterSdk caption row**
[Source: code_quality_inspector]
`crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` (~line 793). `component_height` is
computed from `bottom_section_height` (= `ACTION_HINT_HEIGHT` = 1) but the new
`effective_bottom_height` is 2 when `has_step_caption` is true. The component loop's clamp
(`component_area_bottom = y + height − 1`) reaches one row past the caption row at
`bottom_y = y + height − 2`, so in tight panels the last component row and the caption share a
terminal row. Fix: compute `effective_bottom_height` before `component_height` and use it in the
`saturating_sub`.

**M2 — Live-CDN network test ships ungated in the default test suite**
[Source: code_quality_inspector, architecture_enforcer, risks_tradeoffs_analyzer]
`test_fetch_flutter_release_manifest_emits_fetched_or_failed`
(`crates/fdemon-app/src/actions/mod.rs` ~2845) performs a real HTTPS GET to the Flutter releases
CDN on every `cargo test --workspace`, with a 30 s timeout. Violates the phase's own rule ("no live
CDN calls in tests — wiremock precedent", TASKS.md Notes); non-hermetic, stalls offline CI. Fix:
`#[ignore = "..."]` at minimum, or preferably make the fetch URL injectable
(daemon already exposes `fetch_release_manifest_from(url)`) and back it with wiremock.

**M3 — Byte-index slicing of network-derived manifest strings can panic in the render loop**
[Source: security_reviewer (MEDIUM), upranked: panic in production render path on external data]
`crates/fdemon-tui/src/widgets/install_wizard/version_picker.rs:211-212` (`&row.version[..version_max]`)
and ~221 (`&s[..10]` on `release_date`). Byte slices on CDN-derived strings panic on non-ASCII
char-boundary straddles, crashing the TUI mid-render. Fix: char-boundary-aware truncation
(`chars().take(n)` or boundary-walk).

**M4 — `VersionPickerState::last_known_visible_height` not registered in `docs/REVIEW_FOCUS.md`**
[Source: architecture_enforcer, code_quality_inspector, security_reviewer]
The project policy is explicit: "New `Cell`-based render-hint fields require explicit review and
documentation here." The field follows the approved pattern (default 0, `// EXCEPTION:` annotation at
the write site) but is absent from the Current-usage registry. Fix: add the entry after the
`InstallWizardState::last_known_visible_height` bullet.

### Minor (tracked, non-blocking)

1. **ARCHITECTURE.md mechanism drift** [risks]: line ~651 says `handle_confirm` routes through
   `handle_run_selected_step`; the shipped code routes through `dispatch_flutter_install`. One-line fix.
2. **Double clone in `confirm()`** [code_quality]: `install_wizard/version_picker.rs:280` —
   clone-heavy anti-pattern; single-clone rewrite available.
3. **`#[allow(dead_code)]` probe fn** [code_quality]: `handler/install_wizard/version_picker.rs:560`
   `_assert_message_variant_exists` is a development artifact; remove.
4. **`clear_manifest` is dead production code** [logic N1]: only referenced by its own test;
   `reset()` covers the documented scenario. Remove, wire, or mark test-only.
5. **`validate_ref` `.expect()` in library code** [code_quality]: `flutter_install.rs:243` —
   justified by preceding guard but avoidable with `let-else`/`starts_with`.
6. **`version_dir_name` not directly validated before path join** [security LOW]:
   `install_flutter` validates `version_tag`/`channel` but trusts `version_dir_name` to equal one of
   them; add `validate_ref(&target.version_dir_name)?` or a validated constructor.
7. **Manifest fetch client lacks the HTTPS-only redirect guard** [security LOW]: `download_to_file`
   rejects redirect downgrades; `fetch_release_manifest_from` uses reqwest defaults. Extract a shared
   https-only client builder.
8. **Tab label spacing inconsistency** [code_quality]: `" Stable "` vs `"Beta "` in the overlay.
9. **`group_releases` unknown-channel routing undocumented** [code_quality nitpick, risks LOW]:
   non-stable/non-dev channels silently land in Beta; document (and optionally lock with a test).
10. **`handle_refetch` guard asymmetry** [logic N2, cosmetic]: no `is_step_running` guard — provably
    unreachable, but worth a one-line comment.
11. **REVIEW_FOCUS.md network-I/O exception note stale** [security LOW, PRE-EXISTING]: the
    "`version_check.rs` is the only fdemon-app module with outbound network I/O" claim predates the
    toolchain phases; not introduced here.

### Documentation Freshness

Tasks 06/07 updated `docs/ARCHITECTURE.md`, `docs/KEYBINDINGS.md`, and the website toolchain page
in-phase. Two registry/accuracy gaps remain: M4 (REVIEW_FOCUS.md Cell registry — mandatory) and
Minor 1 (ARCHITECTURE.md names the wrong dispatch helper). Both are folded into the findings above.

### Strengths (for the record)

- Zero layer-boundary or TEA violations across all 4 crates; the TUI consumes picker types solely via
  the `fdemon-app::install_wizard` re-export gateway.
- All 8 phase invariants verified by independent logic trace — notably: pinned-miss-is-hard-error with
  the legacy stable fallback retained only on the `version_tag: None` path; single-source
  `begin_step`/`run_seq` via `dispatch_flutter_install` (proven by `test_confirm_bumps_run_seq_once`);
  airtight picker key intercept with the offline `Failed`→un-pinned escape hatch.
- `git_clone_args` and `flutter_install_target` extracted as pure, argv-shape-tested helpers;
  `validate_ref` correctly widens to `+` while keeping the leading-`-` argument-injection guard.
- ~100 new unit tests across daemon resolution, picker state, handlers, and widget render states.

## Verdict Rationale

Core functionality is correct and the safety-critical logic is verified, but M1 is a confirmed
user-visible rendering bug, M2 violates an explicit phase rule with CI impact, M3 is a
panic-on-untrusted-input in the render loop, and M4 is a policy-mandated registry update. Per the
matrix (multiple ⚠️ agents + concrete Major findings) the phase needs a fix round before approval.

## Action Items

See [ACTION_ITEMS.md](ACTION_ITEMS.md).

---

## Re-review (Round 1)

**Date:** 2026-06-10
**Fix diff:** `0c927d8f..c51643eb` (4 fix tasks, workflow/plans/.../followups/phase-6-fix-1/)
**Re-reviewers:** bug_fix_reviewer (PASS), code_quality_inspector (PASS), logic_reasoning_checker (PASS)
**Integration verify:** PASS — 7,458 tests, 0 failed; the gated test reported as ignored with its
reason string; fmt/check/clippy clean.

### Prior Findings Resolution

| Finding | Status | Evidence |
|---------|--------|----------|
| M1 — step_detail caption-row overlap | **Resolved** | `has_step_caption`/`effective_bottom_height` hoisted before the component loop; `bottom_section_height` removed from live code (single source of truth); traced no-overlap/no-underflow at heights 1–4, 8, large; 2 regression tests added |
| M2 — ungated live-CDN test | **Resolved** | `#[ignore = "live network: …"]` on the test; body unchanged; doc note points at daemon wiremock coverage; integration run confirms it is skipped |
| M3 — byte-slice panic on manifest strings | **Resolved** | `truncate_chars` via `char_indices().nth(n)` proven boundary-safe for exact/fewer/more/n=0; guard switched to `chars().count()`; ASCII behavior proven identical (`test_date_truncated_to_10_chars` unchanged); 2 multi-byte tests added |
| M4 — REVIEW_FOCUS.md Cell registry gap | **Resolved** | Bullet added after the `InstallWizardState` entry; write site (`render_list`/`VersionPickerOverlay`) and reader (`adjust_scroll`) verified against code |

### New findings in the fix diff (Minor only)

1. `test_no_panic_multibyte_version_string` doesn't actually drive the version-string truncation
   branch (width 50 → `version_max` 46 > 4 chars); panic-freedom is structural and the date test
   covers `truncate_chars` directly — add a narrow-width variant when convenient.
2. The `#[ignore]` doc comment's run command uses a truncated test name (cargo substring matching
   makes it work); cosmetic.

### Verdict rationale

All prior Critical/Major findings resolved; fix diff introduces no Critical/Major issues; Minor
findings remain (the 2 above + the 11 deferred from round 0). Per Re-review Mode rules:
⚠️ **APPROVED WITH CONCERNS** (terminal, passing).
