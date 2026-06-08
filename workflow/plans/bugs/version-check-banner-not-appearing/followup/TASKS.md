# Version-Check Banner — Review Follow-up Task Index

## Overview

Addresses the non-blocking findings from the post-merge code review
([REVIEW.md](../../../../reviews/bugs/version-check-banner-not-appearing/REVIEW.md)):
1 architecture-consistency concern (C1), 4 documentation/test nits (C2–C5, C8),
1 layout constant (C7), and 3 security hardening items (S1–S3). All findings were
**non-blocking** on the original merge; this phase cleans them up.

**Total Tasks:** 3
**Estimated Hours:** 3–5 hours

## Finding → Task Map

| Finding | Severity | Summary | Task |
|---------|----------|---------|------|
| C1 | ⚠️ arch | `startup_notice` set path uses direct field write, not a named method | 02 |
| C2 | 🟡 doc | Stale `Message::NewVersionAvailable` rustdoc (dialog-only) | 02 |
| C3 | 🟡 doc | `startup_notice` field doc omits the keypress-dismiss path | 02 |
| C4 | 🟡 test | No `UiMode::Startup` dismiss-no-op test | 02 |
| C5 | 🟡 test | No "terminal too short" banner-guard test | 03 |
| C6 | 🟡 test | No redirect-rejection test for `fetch_latest_tag` | 01 |
| C7 | 🔵 nit | Magic `1` literals in banner layout; no `BANNER_ROW_HEIGHT` | 03 |
| C8 | 🔵 nit | `write_stores_raw_tag_not_result` test name is a misnomer | 01 |
| S1 | 🟠 sec(med) | `read_cache_at` has no size cap before `std::fs::read` | 01 |
| S2 | 🟠 sec(med) | Atomic-write `.tmp` sibling has a predictable fixed name | 01 |
| S3 | 🔵 sec(low) | `latest` not validated at the `Message`/state boundary | 02 |

## Task Dependency Graph

```
Wave 1 (all parallel — disjoint write sets, no dependencies)
┌────────────────────────────┐  ┌────────────────────────────┐  ┌────────────────────────────┐
│ 01-version-check-hardening │  │ 02-startup-notice-         │  │ 03-render-banner-constant  │
│ (fdemon-app)               │  │    encapsulation           │  │ (fdemon-tui)               │
│ version_check.rs           │  │ (fdemon-app)               │  │ render/mod.rs + tests.rs   │
│ S1, S2, C6, C8             │  │ state.rs, update.rs,       │  │ C7, C5                     │
│                            │  │ message.rs · C1,C2,C3,C4,S3│  │                            │
└────────────────────────────┘  └────────────────────────────┘  └────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Agent | Modules |
|---|------|--------|------------|------------|-------|---------|
| 1 | [01-version-check-hardening](tasks/01-version-check-hardening.md) | Not Started | - | 1.5-2h | implementor | `fdemon-app/version_check.rs` |
| 2 | [02-startup-notice-encapsulation](tasks/02-startup-notice-encapsulation.md) | Not Started | - | 1-1.5h | implementor | `fdemon-app/state.rs`, `fdemon-app/handler/update.rs`, `fdemon-app/message.rs` |
| 3 | [03-render-banner-constant](tasks/03-render-banner-constant.md) | Not Started | - | 0.5-1h | implementor | `fdemon-tui/render/mod.rs`, `fdemon-tui/render/tests.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-version-check-hardening | `crates/fdemon-app/src/version_check.rs` | — |
| 02-startup-notice-encapsulation | `crates/fdemon-app/src/state.rs`, `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/message.rs` | — |
| 03-render-banner-constant | `crates/fdemon-tui/src/render/mod.rs`, `crates/fdemon-tui/src/render/tests.rs` | `crates/fdemon-app/src/state.rs` (StartupNotice / UiMode — read only) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|--------------------|--------------------|
| 01 + 02 | None | Parallel (worktree) |
| 01 + 03 | None | Parallel (worktree) |
| 02 + 03 | None (`state.rs` is written by 02, only *read* by 03 via the type system) | Parallel (worktree) |

> Note on 02 + 03: Task 02 writes `state.rs`; Task 03 only *consumes* the `StartupNotice`
> and `UiMode` types from it (no edits). Read-only overlap is safe for parallel worktrees.
> Task 03 must not edit `state.rs`.

## Success Criteria

This follow-up is complete when:

- [ ] `startup_notice` is set through a named `AppState` method (C1); the same method enforces
      the digit-and-dot invariant on `latest` via `debug_assert!` (S3).
- [ ] `Message::NewVersionAvailable` rustdoc and the `startup_notice` field doc describe the
      full post-fix lifecycle (C2, C3).
- [ ] New tests exist for: `UiMode::Startup` dismiss no-op (C4), terminal-too-short banner
      guard (C5), and HTTP redirect rejection (C6).
- [ ] `read_cache_at` rejects oversized cache files before reading (S1); the atomic-write
      temp file uses a non-colliding name (S2).
- [ ] `BANNER_ROW_HEIGHT` constant replaces the inline `1` literals; `BANNER_MIN_HEIGHT` carries
      a derivation comment (C7).
- [ ] The misnamed cache test is renamed to describe what it actually exercises (C8).
- [ ] `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` all pass.

## Notes

- Every finding here was rated **non-blocking** by the reviewers; this is hygiene/hardening, not
  a correctness fix. No behavior change is expected except the two security guards (S1, S2),
  which only affect malformed/adversarial inputs.
- S2 is defense-in-depth: `check_for_newer_release` runs once per process (single writer), so the
  predictable temp name is not exploitable today. Implement the unique name OR record an explicit
  deferral decision in the task's Completion Summary — do not silently drop it.
- No core docs (`ARCHITECTURE.md` / `CODE_STANDARDS.md` / `DEVELOPMENT.md`) need changes: C2/C3 are
  in-code rustdoc in `fdemon-app`, editable by the implementor. No `doc_maintainer` task required.
- Do not touch the version comparison semantics, the cache TTL, the HTTP endpoint, or the banner
  render-site/dismiss behavior — those are correct and were validated in the original fix.
