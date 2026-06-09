## Task: Daemon preflight test — presence-based component assertions (Phase-4 forward-compat)

**Objective**: Replace the brittle fixed-count + per-index `ComponentKind` assertions in the
`run_preflight` test with presence-based assertions. Phase 3 fixed the count at `10` cross-host by always
pushing a `WebBrowser` slot (even on `HostPlatform::Unknown`); Phases 4–5 add host-gated probes (Xcode
macOS-only, VS Windows-only) that will make the count host-variable and break both `assert_eq!(len, 10)`
and every `components[N].kind == …` index assertion. Make the test robust to host-gating and reordering now.

**Depends on**: Phase 3 (merged). Review finding I (LOW, forward-compat).

**Agent:** implementor

**Estimated Time**: 0.5–1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/mod.rs` — the `test_run_preflight_returns_report_without_panicking` assertions.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs` — `ComponentKind`.

### Details

> Line numbers are a current snapshot and will drift — locate by test name.

`test_run_preflight_returns_report_without_panicking` (`toolchain/mod.rs:~245`) currently asserts an exact
count (`assert_eq!(report.components.len(), 10)`) followed by positional `report.components[N].kind == …`
assertions (`:260–276`). Convert to **presence-based** assertions that survive host-gating and reordering:

```rust
use std::collections::HashSet;
let kinds: HashSet<_> = report.components.iter().map(|c| c.kind).collect();
for expected in [
    ComponentKind::FlutterSdk, ComponentKind::Git, ComponentKind::Jdk,
    ComponentKind::AndroidCmdlineTools, ComponentKind::AndroidPlatformTools,
    ComponentKind::AndroidPlatform, ComponentKind::AndroidBuildTools,
    ComponentKind::AndroidLicenses, ComponentKind::Prerequisites,
    ComponentKind::WebBrowser,
] {
    assert!(kinds.contains(&expected), "missing component: {expected:?}");
}
```

- Keep a **lower-bound** sanity check rather than an exact count (e.g. `assert!(report.components.len() >= 10)`),
  OR drop the count assertion entirely in favour of the presence loop. Do not keep `== 10` — it is the
  Phase-4 tripwire.
- Remove the per-index `components[N]` assertions (replaced by the presence loop).
- Add a short comment noting that the set is expected to grow host-variably in Phases 4–5 (Xcode/VS), so
  presence (not count/index) is the stable invariant.

> `ComponentKind` must be `Hash + Eq` to go in a `HashSet`. If it is not already (it derives
> `PartialEq, Eq`), either add `Hash` to the derive **in `types.rs`** — but that would make this task touch
> `types.rs` and overlap Task 02's crate file set is fine (different file) though it widens scope — OR avoid
> the `HashSet` and use `iter().any(|c| c.kind == expected)` in the loop (no new derive needed). **Prefer the
> `any(...)` form** to keep this task confined to `mod.rs` and avoid a derive change.

### Acceptance Criteria

1. The test no longer asserts an exact component count of `10` nor any `components[N]`-by-index kind.
2. The test asserts every expected `ComponentKind` (the current 10, incl. `WebBrowser`) is **present**, via
   `iter().any(...)` (no new trait derive, no `types.rs` edit).
3. A comment documents that the component set grows host-variably in Phases 4–5, so presence is the stable
   invariant.
4. `cargo test -p fdemon-daemon --lib toolchain` green; `cargo fmt --all` + `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` clean.

### Testing

```bash
cargo test -p fdemon-daemon --lib toolchain
cargo test -p fdemon-daemon --lib
cargo fmt --all && cargo clippy -p fdemon-daemon --all-targets -- -D warnings
```

### Notes

- Confined to `toolchain/mod.rs` (test code only) — parallelizes with Task 02 (`checks/web.rs`) in the
  daemon crate. **Do not** edit `types.rs` (use `any(...)`, not a `HashSet`, to avoid a `Hash` derive).
- This is purely a test-robustness change; `run_preflight`'s behaviour is unchanged.
