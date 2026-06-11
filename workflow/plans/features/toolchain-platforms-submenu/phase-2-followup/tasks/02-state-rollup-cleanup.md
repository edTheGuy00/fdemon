## Task: state.rs rollup cleanup — single host-gating source + single-pass rollup (S2 + S4)

**Objective**: Remove the duplicated host-gating in the Platforms parent-status computation by rolling up
over the actually-emitted leaf steps (S2), and rewrite `rollup_step_statuses` as an allocation-free
single-pass scan with the missing single-`Ok` test (S4). Behavior-preserving — the observable parent status
must be identical for every report.

**Depends on**: None.

**Agent:** implementor

**Estimated Time**: ~1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/state.rs` — `build_steps` parent-status block + `rollup_step_statuses`
  + its unit tests.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs` — `HostPlatform`, `StepStatus` (read only).

### Details

> Locate by symbol; line numbers will drift.

#### 1. S2 — roll up the parent status over the emitted leaves, not a parallel `vec![…]`

Today `build_steps` computes the parent status from a **hand-built parallel list** (state.rs ≈1009–1031):

```rust
let platforms_parent_status = {
    let leaf_statuses = if expanded {
        let mut statuses = vec![android_status];          // Android always
        match report.platform {                            // <-- host-gating match #1
            HostPlatform::MacOs => { statuses.push(Pending); statuses.push(Pending); } // iOS, macOS
            HostPlatform::Windows => { statuses.push(Pending); }                        // Windows
            _ => {}
        }
        statuses.push(StepStatus::Pending);                // Web always
        statuses
    } else {
        vec![android_status]                               // collapsed: only Android
    };
    rollup_step_statuses(&leaf_statuses)
};
```

The host-gating match is duplicated by the leaf-**emission** block (state.rs ≈1056–1101, host-gating match
#2), and the leaf order even diverges (parent block pushes iOS/macOS before Web; emission pushes Web before
iOS/macOS). Adding a platform later means editing both in sync — a Phase-3 trap.

**Fix:** build the leaf `WizardStep`s **first** (the emission block becomes the single host-gating site),
then derive the parent status by rolling up over those built leaves' `.status` fields. Sketch:

```rust
// Build the leaf rows once (single host-gating match).
let leaves: Vec<WizardStep> = build_platform_leaves(report, android_leaf /* + placeholders */);
// Parent status rolls up the real leaves. In the collapsed projection the leaves are not
// emitted into `steps`, but the parent status must still reflect Android — see below.
let platforms_parent_status = rollup_step_statuses(
    &leaves.iter().map(|s| s.status).collect::<Vec<_>>()
);
```

Implementation choices (pick whichever keeps the diff smallest and the projection correct):
- Compute the full `leaves` vec unconditionally, derive `platforms_parent_status` from it, then **push the
  leaves into `steps` only when `expanded`**. Collapsed and expanded then share one leaf-set + one parent
  status (Android real + placeholders `Pending` → rolls up to Android's status either way, matching today's
  behavior exactly).
- Keep a tiny local helper (e.g. `fn platform_leaves(report, android_leaf) -> Vec<WizardStep>`) so the
  host-gating match exists in exactly one place.

**Invariant to preserve:** for any report, the new `platforms_parent_status` equals the old value
(placeholders are `Pending`/neutral, so the parent reflects the Android leaf status in both collapsed and
expanded). The collapsed projection still emits exactly 5 rows `[Prerequisites, Platforms, FlutterSdk,
PathConfig, Doctor]`; expanded still inserts the same host-gated leaves in the same order as before
(Android → Web → iOS/macOS on MacOs → Windows on Windows). Do not change the emitted leaf order or count.

#### 2. S4 — single-pass `rollup_step_statuses` + missing test

Replace the collect-then-check body (state.rs ≈875–891) with an allocation-free single pass:

```rust
fn rollup_step_statuses(statuses: &[StepStatus]) -> StepStatus {
    let mut any_real = false;
    let mut any_missing = false;
    let mut any_partial = false;
    for &s in statuses {
        if s == StepStatus::Pending {
            continue;
        }
        any_real = true;
        match s {
            StepStatus::Missing => any_missing = true,
            StepStatus::Partial => any_partial = true,
            _ => {}
        }
    }
    if !any_real {
        StepStatus::Pending
    } else if any_missing {
        StepStatus::Missing
    } else if any_partial {
        StepStatus::Partial
    } else {
        StepStatus::Ok
    }
}
```

Precedence unchanged: Missing > Partial > Ok; Pending neutral; all-Pending/empty → Pending. Add the missing
direct unit tests alongside the existing four (`..._all_pending_returns_pending`, `..._missing_wins_over_ok`,
`..._partial_wins_over_ok`, `..._empty_returns_pending`):
- `test_rollup_step_statuses_single_ok_returns_ok` — `[StepStatus::Ok]` → `Ok`.
- `test_rollup_step_statuses_ok_with_pending_returns_ok` — `[StepStatus::Ok, StepStatus::Pending]` → `Ok`.

### Acceptance Criteria

1. The host-gating platform-leaf set is constructed in exactly one place; the parent status is derived from
   the built leaves' statuses, not a separately hand-maintained `vec![…]`.
2. `build_steps(report, false)` still returns exactly `[Prerequisites, Platforms, FlutterSdk, PathConfig,
   Doctor]`; `build_steps(report, true)` still inserts the same host-gated leaves in the same order
   (Android, Web, then iOS+macOS on MacOs / Windows on Windows). Existing `build_steps` projection and
   host-gating tests pass unchanged.
3. The Platforms parent status is identical to the pre-change value for Linux/macOS/Windows reports and for
   Android `Missing`/`Partial`/`Ok` — verified by the existing parent-rollup tests (which must still pass)
   plus, if helpful, a new equality-style assertion.
4. `rollup_step_statuses` performs no heap allocation; the `[Ok]` and `[Ok, Pending] → Ok` cases are tested.
5. `cargo test --workspace --lib` green; `cargo fmt --all` + `cargo clippy --workspace --all-targets -- -D warnings` clean.

### Testing

```bash
cargo test -p fdemon-app --lib install_wizard::state
cargo test -p fdemon-app --lib build_steps
cargo test -p fdemon-app --lib rollup_step_statuses
cargo test --workspace --lib
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
```

Watch the existing parent-status tests (`test_platforms_parent_status_reflects_android_missing`,
`..._reflects_android_ok_with_pending_placeholders`) and the host-gating projection tests
(`test_build_steps_expanded_inserts_android_leaf` and the macOS/Windows/Linux leaf-set tests) — they are the
behavior-preservation guard for this refactor.

### Notes

- This is a behavior-preserving refactor + a test addition. If any existing test changes its expected
  **value** (not just call shape), stop — the refactor diverged from the current behavior.
- Disjoint file from Tasks 01 and 03 → safe to run in a parallel worktree.
- `rollup_step_statuses` is private to the module; keep it private.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/install_wizard/state.rs` | Replaced `rollup_step_statuses` with allocation-free single-pass version; refactored `build_steps` to build all platform leaves unconditionally (single host-gating site), derive parent status from those leaves, and emit them only when `expanded`; added two new `rollup_step_statuses` unit tests (`single_ok_returns_ok`, `ok_with_pending_returns_ok`). |

### Notable Decisions/Tradeoffs

1. **Leaf vec for parent-status derivation allocates transiently**: The `leaf_statuses: Vec<StepStatus>` collect is a small allocation (≤5 elements), used only within `build_steps` to feed `rollup_step_statuses`. The `rollup_step_statuses` function itself is now allocation-free as required. The `platform_leaves` vec is also only an intermediate; both are dropped before `build_steps` returns. This is the cleanest approach without changing the function signature.

2. **Leaf emission order preserved exactly**: Android → Web → iOS/macOS (macOS only) → Windows (Windows only) — unchanged from the pre-refactor expansion block. The old parent-status block used a slightly different internal order (Android → iOS/macOS → Web), but since all non-Android leaves are `Pending`, the parent status was identical regardless of order. The canonical order is now the emission order.

3. **Collapsed projection unchanged**: `build_steps(report, false)` still returns exactly 5 rows. The leaves vec is built but not pushed into `steps` when `expanded == false`, so the collapsed output is identical to before.

### Testing Performed

- `cargo test -p fdemon-app --lib install_wizard::state` — 135 passed, 0 failed
- `cargo fmt --all -- --check` — clean
- `cargo check --workspace --all-targets` — clean
- `cargo test --workspace --lib` — 1491 passed, 0 failed
- `cargo clippy --workspace --all-targets -- -D warnings` — clean

### Risks/Limitations

1. **Minor transient allocation in `build_steps`**: `platform_leaves` and `leaf_statuses` are small vecs allocated and dropped within the function. No observable impact on performance, but `rollup_step_statuses` itself is now allocation-free as the task requires.
2. **Behavior-preserving refactor only**: No logic changes; all existing tests pass unchanged.
