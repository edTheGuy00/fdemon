# Phase 6 Fix Round 1 — Task Index

## Overview

Round-1 followup for the Phase 6 review verdict **NEEDS_WORK**
(workflow/reviews/features/toolchain-platforms-submenu-phase-6/REVIEW.md). Scope: the 4 Major
findings (no Criticals). Root causes verified against the merged code on
`feat/toolchain-platforms-submenu` (HEAD `0c927d8f`):

1. **M1** `step_detail.rs:752` computes `component_height` from `bottom_section_height` (1 row)
   while the caption-aware `effective_bottom_height` (2 rows when `has_step_caption`) is computed
   only later at line 794 — the component loop can render onto the caption row.
2. **M2** `actions/mod.rs:2845` `test_fetch_flutter_release_manifest_emits_fetched_or_failed` is an
   ungated `#[tokio::test]` hitting the live CDN. The daemon's wiremock seam
   (`fetch_release_manifest_from`, flutter_install.rs:478) is `pub(crate)` — not reachable from
   fdemon-app — so the round-1 fix is the `#[ignore]` gate (URL-injection refactor deferred).
3. **M3** `widgets/install_wizard/version_picker.rs:212` (`&row.version[..version_max]`) and `:221`
   (`&s[..10]`) byte-slice CDN-derived strings — panics on non-ASCII char-boundary straddle inside
   the render loop.
4. **M4** `docs/REVIEW_FOCUS.md` Current-usage registry lacks the mandatory entry for the new
   `VersionPickerState::last_known_visible_height` Cell render-hint.

**Total Tasks:** 4 — all write-disjoint, single wave, parallel worktrees.
**Estimated Hours:** 3–5

## Task Dependency Graph

```
Wave 1 (all parallel, no dependencies):
  01-fix-step-detail-caption-layout      (fdemon-tui step_detail.rs)
  02-gate-live-manifest-fetch-test       (fdemon-app actions/mod.rs)
  03-char-boundary-truncation-picker     (fdemon-tui widgets/install_wizard/version_picker.rs)
  04-register-cell-render-hint-doc       (docs/REVIEW_FOCUS.md)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Complexity | Modules |
|---|------|--------|------------|------------|------------|---------|
| 1 | [01-fix-step-detail-caption-layout](tasks/01-fix-step-detail-caption-layout.md) | Not Started | - | 1–2h | medium | `fdemon-tui/src/widgets/install_wizard/step_detail.rs` |
| 2 | [02-gate-live-manifest-fetch-test](tasks/02-gate-live-manifest-fetch-test.md) | Not Started | - | 0.5h | low | `fdemon-app/src/actions/mod.rs` |
| 3 | [03-char-boundary-truncation-picker](tasks/03-char-boundary-truncation-picker.md) | Not Started | - | 1h | medium | `fdemon-tui/src/widgets/install_wizard/version_picker.rs` |
| 4 | [04-register-cell-render-hint-doc](tasks/04-register-cell-render-hint-doc.md) | Not Started | - | 0.5h | low | `docs/REVIEW_FOCUS.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01 | `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | `fdemon-app` install_wizard state (read-only) |
| 02 | `crates/fdemon-app/src/actions/mod.rs` | — |
| 03 | `crates/fdemon-tui/src/widgets/install_wizard/version_picker.rs` | `fdemon-app` `PickerRow` (read-only) |
| 04 | `docs/REVIEW_FOCUS.md` | `install_wizard/version_picker.rs` (app + tui, read-only) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | none | Parallel (worktree) |
| 01 + 03 | none (same directory, different files) | Parallel (worktree) |
| 01 + 04 | none | Parallel (worktree) |
| 02 + 03 | none | Parallel (worktree) |
| 02 + 04 | none | Parallel (worktree) |
| 03 + 04 | none | Parallel (worktree) |

## Success Criteria

- [ ] M1: with a FlutterSdk step caption active and a tight pane, the last visible component row
      never shares a terminal row with the caption; regression test added; existing step_detail
      tests pass.
- [ ] M2: `cargo test --workspace` performs no live CDN call — the manifest-fetch executor test is
      `#[ignore]`-gated with a reason string (still passes under `--ignored` with network).
- [ ] M3: picker overlay truncation is char-boundary-safe for both `version` and `release_date`;
      a test with multi-byte characters at the cut points renders without panic.
- [ ] M4: REVIEW_FOCUS.md Current-usage list registers `VersionPickerState::last_known_visible_height`
      naming the write site and reader.
- [ ] Full quality gate green: `cargo fmt --all -- --check && cargo check --workspace --all-targets
      && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`.

## Notes

### Deferred items (Minor — recorded, intentionally NOT tasked this round)

From ACTION_ITEMS.md Minor list: ARCHITECTURE.md `dispatch_flutter_install` mechanism drift;
`confirm()` double clone; `_assert_message_variant_exists` dead probe; `clear_manifest` dead code;
`validate_ref` `.expect()`; direct `version_dir_name` validation; HTTPS-only redirect policy for the
manifest client; tab-label spacing; `group_releases` unknown-channel doc/test; `handle_refetch`
guard comment; stale REVIEW_FOCUS network-I/O exception note (pre-existing).

The **preferred** M2 refactor (injectable fetch URL + wiremock-backed executor test) is also
deferred: it requires widening the daemon's `pub(crate) fetch_release_manifest_from` API surface,
which exceeds round-1 scope.
