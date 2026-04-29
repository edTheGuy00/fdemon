# Bugfix Plan: MSRV Violation — Production `is_multiple_of` Calls

## TL;DR

5 production-code call sites use `<int>::is_multiple_of(n)`, an inherent integer method stabilized in Rust 1.87. The workspace declares `rust-version = "1.77.2"` in `Cargo.toml`. Local and CI builds use the stable toolchain (currently ≥1.87) so they succeed, but `cargo +1.77.2 build --workspace` fails with `no method named is_multiple_of found`. Clippy's `manual_is_multiple_of` lint cannot detect this because the offending code is already in the "fixed" form. Fix by reverting each call to `% N == 0` and suppressing the lint at function scope, mirroring the precedent established in `crates/fdemon-tui/src/widgets/devtools/network/tests.rs:11-12`. Also document the `#[allow(dead_code)]` attribute on `HangingGetVmBackend` (rationale exists in the rustdoc above but not adjacent to the attribute itself).

## Bug Reports

### Bug 1: 5 production `is_multiple_of` call sites violate declared MSRV (1.77.2)

**Symptom:** `cargo +1.77.2 build --workspace` fails to compile with `error[E0599]: no method named 'is_multiple_of' found`. Local and CI builds (stable toolchain) succeed because the method exists from 1.87 onward.

**Expected:** All workspace code compiles on the declared MSRV. Either the declaration matches reality (1.87+) or the code respects the declaration (1.77.2). The original `clippy-rust-191-cleanup` plan committed to MSRV 1.77.2 and established the suppression pattern (see `crates/fdemon-tui/src/widgets/devtools/network/tests.rs:11-12`); these 5 production sites pre-date that cleanup and were left unfixed because they were out of declared scope.

**Root Cause Analysis:**
1. `<int>::is_multiple_of` was stabilized in Rust 1.87 (see [rust-lang/rust#127436](https://github.com/rust-lang/rust/issues/127436)).
2. The workspace `Cargo.toml` line 12 declares `rust-version = "1.77.2"`.
3. The workspace lacks a `rust-toolchain.toml` pin, and CI's `dtolnay/rust-toolchain@stable` action installs the latest stable, so the MSRV declaration is currently advisory rather than enforced.
4. The `manual_is_multiple_of` clippy lint fires only on the `% N == 0` form. Once code uses the stabilized API, clippy is silent, masking the MSRV violation.
5. The 5 call sites listed below were authored before MSRV was a stated concern and were not surfaced by Rust 1.91's clippy pass.

**Affected Files:**
- `crates/fdemon-app/src/state.rs:756` — `tick()` animation loop, `self.animation_frame.is_multiple_of(15)`
- `crates/fdemon-dap/src/adapter/breakpoints.rs:692` — `evaluate_hit_condition()`, `hit_count.is_multiple_of(n)`
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/chart.rs:111` — chart dashed-line render, `dot_x.is_multiple_of(2)`
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/chart.rs:223` — second chart render path, `dot_x.is_multiple_of(2)`
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/bars.rs:180` — frame budget dashed line, `(x - line_start_x).is_multiple_of(2)`

---

### Bug 2: `HangingGetVmBackend` `#[allow(dead_code)]` lacks adjacent rationale

**Symptom:** Future maintainers may delete the struct as "unused" since clippy is silenced and the rationale lives in a separate rustdoc block that could be missed.

**Expected:** The `#[allow(dead_code)]` attribute itself carries a one-line `//` comment explaining why the struct is intentionally retained, in addition to the existing rustdoc.

**Root Cause Analysis:**
1. The struct at `crates/fdemon-dap/src/adapter/tests/request_timeouts_events.rs:59` already has a useful rustdoc block (lines 55-58: "Backend that sleeps longer than `REQUEST_TIMEOUT` in `get_vm`. Used to test that the timeout fires...").
2. However, the `#[allow(dead_code)]` attribute on line 59 has no inline comment of its own.
3. The `clippy-rust-191-cleanup` review (`code_quality_inspector`) flagged this as a minor maintainability concern.

**Affected Files:**
- `crates/fdemon-dap/src/adapter/tests/request_timeouts_events.rs:59`

---

## Affected Modules

- `crates/fdemon-app/src/state.rs` — Replace `is_multiple_of(15)` with `% 15 == 0`; add `#[allow(clippy::manual_is_multiple_of)]` on `tick()`.
- `crates/fdemon-dap/src/adapter/breakpoints.rs` — Replace `is_multiple_of(n)` with `% n == 0`; add `#[allow(clippy::manual_is_multiple_of)]` on `evaluate_hit_condition()`.
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/chart.rs` — Replace 2 occurrences of `dot_x.is_multiple_of(2)` with `dot_x % 2 == 0`; add `#[allow(...)]` on the enclosing function(s).
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/bars.rs` — Replace `(x - line_start_x).is_multiple_of(2)` with `(x - line_start_x) % 2 == 0`; add `#[allow(...)]` on the enclosing function.
- `crates/fdemon-dap/src/adapter/tests/request_timeouts_events.rs` — Add a one-line `//` comment above the `#[allow(dead_code)]` on `HangingGetVmBackend`.

---

## Phases

### Phase 1: MSRV Compliance (Bugs 1 & 2) — Should Fix

**Approach:** Mechanical revert of `is_multiple_of` to `% N == 0` plus narrowly-scoped `#[allow(clippy::manual_is_multiple_of)]` attributes, mirroring the precedent established in `crates/fdemon-tui/src/widgets/devtools/network/tests.rs:11-12`. Plus a one-line rationale comment on `HangingGetVmBackend`'s `#[allow(dead_code)]`.

**Steps:**
1. For each of the 5 call sites: replace `x.is_multiple_of(N)` with `x % N == 0`.
2. On the enclosing function, add:
   ```rust
   // MSRV guard: `is_multiple_of` requires Rust 1.87; MSRV is 1.77.2 — suppress the lint.
   #[allow(clippy::manual_is_multiple_of)]
   ```
3. For `HangingGetVmBackend`, prepend a `//` comment to the `#[allow(dead_code)]` attribute (e.g., "Preserved as test scaffolding; see rustdoc above.").
4. Per-crate verification: `cargo clippy -p <crate> --all-targets -- -D warnings` exits 0; `cargo test -p <crate>` passes; `cargo fmt --all` is clean.

**Measurable Outcomes:**
- `grep -rn 'is_multiple_of' crates/` returns only the `#[allow(...)]` attribute lines and pre-existing test guards (no production-code call sites).
- Each `#[allow(clippy::manual_is_multiple_of)]` is preceded by an MSRV justification comment.
- Workspace clippy gate exits 0; test suites pass per crate.
- `HangingGetVmBackend`'s `#[allow(dead_code)]` is preceded by a `//` comment.

---

## Edge Cases & Risks

### Behavioral equivalence of `% N == 0` vs `is_multiple_of(N)`
- **Risk:** For `N = 0`, `is_multiple_of(0)` and `% 0 == 0` differ — the former returns `true` (every value is a multiple of 0 in std's definition), the latter panics on division by zero.
- **Mitigation:** Inspect each call site for the value of `N`:
  - `state.rs:756`: literal `15` — safe.
  - `breakpoints.rs:692`: variable `n`, but the surrounding code already guards with `Ok(n) if n > 0 =>` — safe.
  - `chart.rs:111`, `chart.rs:223`, `bars.rs:180`: literal `2` — safe.
- All 5 sites are provably non-zero divisors; behavior is preserved.

### Operator precedence on `(x - line_start_x).is_multiple_of(2)`
- **Risk:** `(x - line_start_x) % 2 == 0` requires explicit parentheses to preserve grouping (the method form has unambiguous precedence).
- **Mitigation:** Keep the parentheses around the subtraction in the rewrite.

### Clippy attribute scope
- **Risk:** Placing `#[allow(clippy::manual_is_multiple_of)]` at the wrong scope can either fail to suppress (too narrow) or silence too much (too broad).
- **Mitigation:** Function-level scope is correct (matches the precedent in `network/tests.rs:12`). The lint fires per-call-site, so the function containing the call site needs the attribute.

### MSRV declaration is currently advisory
- **Risk:** Even after this fix, nothing actually verifies the workspace builds on 1.77.2 — CI uses stable.
- **Mitigation:** Out of scope for this fix. A separate, larger decision could add a `rust-toolchain.toml` or a 1.77.2 CI matrix entry. This bug only restores consistency with the declared MSRV.

---

## Further Considerations

1. **Should the project enforce MSRV in CI?** Currently the `1.77.2` declaration is advisory. Options:
   - Add a `rust-toolchain.toml` pinning to 1.77.2 (forces local builds to use it; can break developer workflows that want stable).
   - Add a 1.77.2 matrix entry to `.github/workflows/ci.yml` (verifies MSRV without forcing it on developers).
   - Bump the declaration to match the toolchain actually used (1.87 currently) — drops the MSRV promise but matches reality.

   **Out of scope for this followup** — separate decision with ecosystem implications. Recommend opening a discussion separately if the team wants the MSRV declaration to be load-bearing.

2. **Why not bump MSRV to 1.87 here?** Considered as Option B during planning. Rejected because (a) it's a project-wide policy decision that should not be a side-effect of a follow-up to a lint cleanup, and (b) it would drop support for any downstream pin to ≤1.86.

---

## Task Dependency Graph

```
Wave 1 (parallel — disjoint write-file sets)
├── 01-fix-fdemon-app-msrv
├── 02-fix-fdemon-dap-msrv
├── 03-fix-fdemon-tui-msrv
└── 04-document-hanging-get-vm-backend
```

No dependencies between tasks — all 4 run in parallel worktrees.

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `grep -rn 'is_multiple_of' crates/` returns only the `#[allow(...)]` attribute lines and pre-existing MSRV-suppressed test code; no production-code call sites remain.
- [ ] Each `#[allow(clippy::manual_is_multiple_of)]` is paired with an MSRV justification comment.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] `cargo test --workspace` passes (no regression in animation loop, breakpoint conditions, or chart rendering).
- [ ] `HangingGetVmBackend`'s `#[allow(dead_code)]` is preceded by a `//` comment explaining the retention rationale.
- [ ] (Optional, manual) `cargo +1.77.2 check --workspace` exits 0 if the MSRV toolchain is installed locally.

---

## Milestone Deliverable

The workspace can compile on its declared MSRV (`1.77.2`) without depending on stabilized-after-MSRV API. The `manual_is_multiple_of` suppression pattern is consistently applied across all 5 production sites. `HangingGetVmBackend`'s retention rationale is durable to future cleanup passes.
