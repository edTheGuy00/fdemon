# Action Items: Clippy Rust 1.91 Cleanup

**Review Date:** 2026-04-27
**Verdict:** ⚠️ APPROVED WITH CONCERNS
**Blocking Issues:** 0
**Should-Fix Issues:** 1
**Operational Gates:** 1

---

## Should-Fix Before Closing the Bug

### 1. Resolve pre-existing MSRV violations: 5 `is_multiple_of` call sites

- **Source:** bug_fix_reviewer, architecture_enforcer
- **Files:**
  - `crates/fdemon-app/src/state.rs:756`
  - `crates/fdemon-dap/src/adapter/breakpoints.rs:692`
  - `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/chart.rs:111`
  - `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/chart.rs:223`
  - `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/bars.rs:180`
- **Problem:** `i32::is_multiple_of` and `u64::is_multiple_of` were stabilized in Rust 1.87. The workspace declares `rust-version = "1.77.2"` in `Cargo.toml`. There is no `rust-toolchain.toml` pin, so local and CI builds (using stable) succeed — but `cargo +1.77.2 build --workspace` fails. Clippy's `manual_is_multiple_of` lint cannot detect this because the offending code is already in the "fixed" form.
- **Required Action:** Pick one:
  - **(A)** Replace each `.is_multiple_of(N)` with `% N == 0` and add `#[allow(clippy::manual_is_multiple_of)]` at function/item scope with a comment matching `crates/fdemon-tui/src/widgets/devtools/network/tests.rs:11-12` ("`MSRV guard: …`"). ~10 lines total.
  - **(B)** Bump `rust-version` in the workspace `Cargo.toml` to `1.87` (separate decision with ecosystem implications).
  - **(C)** Document the discrepancy as known debt and open a follow-up task; do not block this PR.
- **Acceptance:** Decision recorded; if (A), all 5 sites use `% N == 0`; if (B), `Cargo.toml` updated and CI uses 1.87; if (C), follow-up task file exists.

---

## Operational Gates (Not Code Changes)

### 2. Confirm CI green on three runners

- **Source:** bug_fix_reviewer, architecture_enforcer, plan success criteria
- **Problem:** Task 07's success criterion requires `cargo clippy --workspace --all-targets -- -D warnings` to pass on `ubuntu-latest`, `macos-latest`, `windows-latest`. PR has not been opened. Local verification was macOS-only, so platform-conditional code paths (`#[cfg(target_os = "windows")]` in `fdemon-app/src/actions/network.rs`, `#[cfg(target_os = "macos")]` in `fdemon-daemon/src/native_logs/`) were not exercised.
- **Required Action:** Push the branch and open a PR. Monitor the CI matrix.
- **Acceptance:** All three runners pass; if any fail with platform-specific warnings, fix forward in this branch (do not revert `-D warnings`).

---

## Optional Improvements

### 3. Add rationale comment for `HangingGetVmBackend` `#[allow(dead_code)]`

- **Source:** code_quality_inspector
- **File:** `crates/fdemon-dap/src/adapter/tests/request_timeouts_events.rs:59`
- **Problem:** The struct is preserved per the plan's "preserve test scaffolding intent" rationale, but the `#[allow(dead_code)]` attribute alone does not document *why* the struct is intentional. A future maintainer may delete it.
- **Suggested Action:** Prepend a `//` comment such as:
  ```rust
  // Preserved as test scaffolding for future timeout/pause-time tests.
  #[allow(dead_code)]
  struct HangingGetVmBackend;
  ```
- **Priority:** Low.

---

## Re-review Checklist

After the actions above are addressed:

- [ ] PR opened against `main`
- [ ] CI green on `ubuntu-latest` ✅
- [ ] CI green on `macos-latest` ✅
- [ ] CI green on `windows-latest` ✅
- [ ] MSRV decision documented (option A, B, or C from item 1)
- [ ] (Optional) `HangingGetVmBackend` rationale comment added
- [ ] Plan directory archived to `workflow/reviews/bugs/clippy-rust-191-cleanup/` (already done — review docs land here)
